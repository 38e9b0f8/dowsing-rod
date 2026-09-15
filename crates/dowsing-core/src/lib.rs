//! Dowsing Rod — Core Analysis Engine
//!
//! Structural refactoring intelligence for source codebases.
//! This crate contains the full analysis pipeline: discover → parse → extract →
//! normalize → fingerprint → candidates → similarity → graph → cluster → rank.
//!
//! The main entry point is [`scan()`], which runs the entire pipeline and returns
//! a [`ScanResult`].

pub mod cache;
pub mod candidates;
pub mod classification;
pub mod clustering;
pub mod config;
pub mod differences;
pub mod discovery;
pub mod extraction;
pub mod fingerprint;
pub mod graph;
pub mod language;
mod native;
pub mod normalization;
pub mod parser;
pub mod ranking;
pub mod render;
pub mod similarity;
pub mod tokens;
pub mod types;

use anyhow::Result;
use rayon::prelude::*;
use rustpython_parser::ast::Stmt;
use rustpython_parser::source_code::RandomLocator;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use cache::Cache;
use types::*;

/// Tool version string.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_REPORTED_PARSE_ERRORS: usize = 200;

/// Schema version for output compatibility.
pub const SCHEMA_VERSION: &str = "1.1";

/// Run the full analysis pipeline and return a `ScanResult`.
///
/// This is the main entry point for the library. The pipeline:
/// 1. Discover supported source files
/// 2. Parse, extract, normalize, and fingerprint (with caching, in parallel)
/// 3. Generate candidate pairs (SimHash LSH + exact-hash grouping)
/// 4. Compute detailed multi-signal similarity for each candidate pair
/// 5. Build similarity graph and cluster
/// 6. Classify, rank, and assemble results
pub fn scan(config: ScanConfig) -> Result<ScanResult> {
    let start = Instant::now();

    // ── 1. Resolve paths and configuration ──────────────────────────
    let scan_path = config::resolve_scan_path(&config.path)?;
    let project_root = config::resolve_project_root(&scan_path)?;
    let mut config = config::load_config(&project_root, config)?;
    config.path = scan_path.clone();

    // Configure rayon thread pool if jobs specified
    if let Some(jobs) = config.jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .build_global()
            .ok(); // Ignore error if pool already set
    }

    // ── 2. Discover source files ────────────────────────────────────
    let files = discovery::discover_source_files(&scan_path, &config.exclude, &config.include)?;

    if files.is_empty() {
        return Ok(empty_result(
            &project_root,
            start.elapsed().as_millis() as u64,
        ));
    }

    // ── 3. Open cache ───────────────────────────────────────────────
    let cache = if config.use_cache {
        Cache::open(&project_root).ok()
    } else {
        None
    };

    // ── 4. Parse, extract, normalize, fingerprint (parallel) ────────
    let cache_hits = AtomicUsize::new(0);
    let cache_misses = AtomicUsize::new(0);
    let files_with_errors = AtomicUsize::new(0);
    let total_source_bytes = AtomicUsize::new(0);

    let per_file_results: Vec<FileAnalysis> = files
        .par_iter()
        .map(|file_path| {
            let content = match std::fs::read(file_path) {
                Ok(content) => content,
                Err(error) => {
                    files_with_errors.fetch_add(1, Ordering::Relaxed);
                    return FileAnalysis::error(file_path, error.to_string());
                }
            };
            total_source_bytes.fetch_add(content.len(), Ordering::Relaxed);
            if let Some(cached) = cache
                .as_ref()
                .and_then(|c| c.load(file_path, &content, config.normalization))
            {
                cache_hits.fetch_add(1, Ordering::Relaxed);
                return FileAnalysis {
                    functions: cached.functions,
                    normalized: cached.normalized,
                    fingerprints: cached.fingerprints,
                    parse_errors: Vec::new(),
                };
            }
            cache_misses.fetch_add(1, Ordering::Relaxed);
            let analysis = std::str::from_utf8(&content)
                .map_err(anyhow::Error::from)
                .and_then(|source| match language::Language::from_path(file_path) {
                    Some(language::Language::Python) => {
                        analyze_python(source, file_path, config.normalization)
                    }
                    Some(language) => {
                        native::analyze(source, file_path, language, config.normalization)
                    }
                    None => unreachable!("discovery only returns supported files"),
                });
            let result = analysis.unwrap_or_else(|e| FileAnalysis::error(file_path, e.to_string()));
            if !result.parse_errors.is_empty() {
                files_with_errors.fetch_add(1, Ordering::Relaxed);
            } else if let Some(ref c) = cache {
                let cached = Cache::make_result(
                    &content,
                    config.normalization,
                    result.functions.clone(),
                    result.normalized.clone(),
                    result.fingerprints.clone(),
                );
                let _ = c.store(file_path, &content, config.normalization, &cached);
            }
            result
        })
        .collect();

    // ── 5. Merge per-file results with global indexing ───────────────
    let mut all_functions: Vec<FunctionInfo> = Vec::new();
    let mut all_normalized: Vec<NormalizedFunction> = Vec::new();
    let mut all_fingerprints: Vec<Fingerprints> = Vec::new();
    let mut all_errors: Vec<ParseError> = Vec::new();

    let mut global_idx = 0usize;
    for file_result in per_file_results {
        all_errors.extend(file_result.parse_errors);

        for (i, func) in file_result.functions.into_iter().enumerate() {
            all_functions.push(func);

            if let Some(mut norm) = file_result.normalized.get(i).cloned() {
                norm.function_id = global_idx;
                all_normalized.push(norm);
            }

            if let Some(mut fp) = file_result.fingerprints.get(i).cloned() {
                fp.function_id = global_idx;
                all_fingerprints.push(fp);
            }

            global_idx += 1;
        }
    }

    if config.fail_on_error && !all_errors.is_empty() {
        let summary = all_errors
            .iter()
            .take(20)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let omitted = all_errors.len().saturating_sub(20);
        if omitted == 0 {
            anyhow::bail!("parse errors encountered:\n{summary}");
        }
        anyhow::bail!(
            "parse errors encountered:\n{summary}\n... {omitted} more diagnostics omitted"
        );
    }

    if all_errors.len() > MAX_REPORTED_PARSE_ERRORS {
        let omitted = all_errors.len() - MAX_REPORTED_PARSE_ERRORS;
        all_errors.truncate(MAX_REPORTED_PARSE_ERRORS);
        all_errors.push(ParseError {
            file: project_root.clone(),
            line: None,
            column: None,
            message: format!("{omitted} additional parser diagnostics omitted"),
        });
    }

    let total_source_bytes = total_source_bytes.load(Ordering::Relaxed);
    let estimated_source_tokens = total_source_bytes.div_ceil(4);

    if all_functions.len() < 2 {
        return Ok(ScanResult {
            schema_version: SCHEMA_VERSION.to_string(),
            tool_version: VERSION.to_string(),
            repository: RepositoryInfo {
                path: project_root,
                files: files.len(),
                functions: all_functions.len(),
                estimated_tokens: estimated_source_tokens,
            },
            statistics: RepositoryStats {
                files_scanned: files.len(),
                files_with_errors: files_with_errors.load(Ordering::Relaxed),
                functions_found: all_functions.len(),
                total_source_bytes,
                estimated_source_tokens,
                candidate_pairs_generated: 0,
                clusters_found: 0,
                high_value_clusters: 0,
                scan_duration_ms: start.elapsed().as_millis() as u64,
                cache_hits: cache_hits.load(Ordering::Relaxed),
                cache_misses: cache_misses.load(Ordering::Relaxed),
            },
            clusters: Vec::new(),
            functions: all_functions,
            parse_errors: all_errors,
        });
    }

    // ── 6. Generate candidate pairs ──────────────────────────────────
    // Partition before candidate generation: no wasted cross-language scoring.
    let mut partitions = std::collections::BTreeMap::<language::Language, Vec<Fingerprints>>::new();
    for fp in all_fingerprints {
        partitions
            .entry(all_functions[fp.function_id].language)
            .or_default()
            .push(fp);
    }
    let mut candidate_pairs = Vec::new();
    for fingerprints in partitions.values() {
        candidate_pairs.extend(
            candidates::generate_candidates(fingerprints, 12)
                .into_iter()
                .map(|(a, b)| (fingerprints[a].function_id, fingerprints[b].function_id)),
        );
    }
    candidate_pairs.sort_unstable();

    // ── 7. Compute similarity for each candidate pair ────────────────
    let norm_lookup: std::collections::HashMap<usize, &NormalizedFunction> =
        all_normalized.iter().map(|n| (n.function_id, n)).collect();

    let scores: Vec<SimilarityScore> = candidate_pairs
        .par_iter()
        .filter_map(|&(a, b)| {
            let norm_a = norm_lookup.get(&a)?;
            let norm_b = norm_lookup.get(&b)?;
            let info_a = all_functions.get(a)?;
            let info_b = all_functions.get(b)?;
            Some(similarity::compute_similarity(
                norm_a, norm_b, info_a, info_b,
            ))
        })
        .collect();

    // ── 8. Build similarity graph ────────────────────────────────────
    let sim_graph =
        graph::SimilarityGraph::build(all_functions.len(), &scores, config.min_similarity);

    // ── 9. Cluster functions ─────────────────────────────────────────
    let mut clusters =
        clustering::cluster_functions(&sim_graph, &scores, &all_functions, &all_normalized);

    // ── 10. Rank and limit ───────────────────────────────────────────
    ranking::rank_clusters(&mut clusters);
    ranking::apply_limits(&mut clusters, &config);

    let high_value = ranking::count_high_value(&clusters);

    // ── 11. Assemble result ──────────────────────────────────────────
    let elapsed_ms = start.elapsed().as_millis() as u64;

    Ok(ScanResult {
        schema_version: SCHEMA_VERSION.to_string(),
        tool_version: VERSION.to_string(),
        repository: RepositoryInfo {
            path: project_root,
            files: files.len(),
            functions: all_functions.len(),
            estimated_tokens: estimated_source_tokens,
        },
        statistics: RepositoryStats {
            files_scanned: files.len(),
            files_with_errors: files_with_errors.load(Ordering::Relaxed),
            functions_found: all_functions.len(),
            total_source_bytes,
            estimated_source_tokens,
            candidate_pairs_generated: candidate_pairs.len(),
            clusters_found: clusters.len(),
            high_value_clusters: high_value,
            scan_duration_ms: elapsed_ms,
            cache_hits: cache_hits.load(Ordering::Relaxed),
            cache_misses: cache_misses.load(Ordering::Relaxed),
        },
        clusters,
        functions: all_functions,
        parse_errors: all_errors,
    })
}

/// Clear the analysis cache for a project.
pub fn clear_cache(project_root: &std::path::Path) -> Result<usize> {
    let cache = Cache::open(project_root)?;
    cache.clear()
}

// ── Internal helpers ─────────────────────────────────────────────────────

/// Per-file analysis result before global re-indexing.
#[derive(Default)]
struct FileAnalysis {
    functions: Vec<FunctionInfo>,
    normalized: Vec<NormalizedFunction>,
    fingerprints: Vec<Fingerprints>,
    parse_errors: Vec<ParseError>,
}

impl FileAnalysis {
    fn error(path: &std::path::Path, message: String) -> Self {
        Self {
            parse_errors: vec![ParseError {
                file: path.to_path_buf(),
                line: None,
                column: None,
                message,
            }],
            ..Self::default()
        }
    }
}

fn analyze_python(
    source: &str,
    path: &std::path::Path,
    level: NormalizationLevel,
) -> Result<FileAnalysis> {
    let parsed = parser::parse_python_source(source, path)?;
    let mut result = FileAnalysis {
        parse_errors: parsed.errors,
        ..FileAnalysis::default()
    };
    if let Some(body) = parsed.module {
        for info in extraction::extract_functions(&body, source, path) {
            if let Some(function_body) =
                find_function_body(&body, &info.function_name, info.start_line, source)
            {
                let norm = normalization::normalize_function(
                    function_body,
                    &info.parameters,
                    &info.function_name,
                    result.functions.len(),
                    level,
                    info.class_name.as_deref(),
                );
                result
                    .fingerprints
                    .push(fingerprint::generate_fingerprints(&norm, &info));
                result.normalized.push(norm);
                result.functions.push(info);
            }
        }
    }
    Ok(result)
}

/// Find a function's AST body by name and start line.
fn find_function_body<'a>(
    stmts: &'a [Stmt],
    name: &str,
    start_line: usize,
    source: &str,
) -> Option<&'a [Stmt]> {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(f) => {
                if f.name.as_str() == name && line_for_offset(source, f.range.start()) == start_line
                {
                    return Some(&f.body);
                }
                // Search nested
                if let Some(body) = find_function_body(&f.body, name, start_line, source) {
                    return Some(body);
                }
            }
            Stmt::AsyncFunctionDef(f) => {
                if f.name.as_str() == name && line_for_offset(source, f.range.start()) == start_line
                {
                    return Some(&f.body);
                }
                // Search nested
                if let Some(body) = find_function_body(&f.body, name, start_line, source) {
                    return Some(body);
                }
            }
            Stmt::ClassDef(c) => {
                if let Some(body) = find_function_body(&c.body, name, start_line, source) {
                    return Some(body);
                }
            }
            Stmt::If(i) => {
                if let Some(body) = find_function_body(&i.body, name, start_line, source) {
                    return Some(body);
                }
                if let Some(body) = find_function_body(&i.orelse, name, start_line, source) {
                    return Some(body);
                }
            }
            Stmt::Try(t) => {
                if let Some(body) = find_function_body(&t.body, name, start_line, source) {
                    return Some(body);
                }
                if let Some(body) = find_function_body(&t.finalbody, name, start_line, source) {
                    return Some(body);
                }
            }
            _ => {}
        }
    }
    None
}

fn line_for_offset(source: &str, offset: rustpython_parser::text_size::TextSize) -> usize {
    let mut locator = RandomLocator::new(source);
    locator.locate(offset).row.to_usize()
}

/// Build an empty result when no files are found.
fn empty_result(project_root: &std::path::Path, elapsed_ms: u64) -> ScanResult {
    ScanResult {
        schema_version: SCHEMA_VERSION.to_string(),
        tool_version: VERSION.to_string(),
        repository: RepositoryInfo {
            path: project_root.to_path_buf(),
            files: 0,
            functions: 0,
            estimated_tokens: 0,
        },
        statistics: RepositoryStats {
            files_scanned: 0,
            files_with_errors: 0,
            functions_found: 0,
            total_source_bytes: 0,
            estimated_source_tokens: 0,
            candidate_pairs_generated: 0,
            clusters_found: 0,
            high_value_clusters: 0,
            scan_duration_ms: elapsed_ms,
            cache_hits: 0,
            cache_misses: 0,
        },
        clusters: Vec::new(),
        functions: Vec::new(),
        parse_errors: Vec::new(),
    }
}
