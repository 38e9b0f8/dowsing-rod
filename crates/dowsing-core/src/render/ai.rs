use crate::tokens::select_within_budget;
use crate::types::ScanResult;
use std::io::Write;

/// Render scan results in the ultra-compact AI-oriented format.
///
/// Token-budget-aware: selects the top clusters that fit within `max_tokens`.
/// Format is designed for LLM consumption — minimal chrome, maximum signal.
pub fn render<W: Write>(
    result: &ScanResult,
    writer: &mut W,
    max_tokens: Option<usize>,
) -> std::io::Result<()> {
    let base_path = &result.repository.path;

    // Header
    writeln!(writer, "DOWSING-ROD v{}", result.tool_version)?;
    writeln!(writer, "SCHEMA {}", result.schema_version)?;
    writeln!(
        writer,
        "REPO {} files={} functions={} tokens≈{}",
        base_path.display(),
        result.statistics.files_scanned,
        result.statistics.functions_found,
        result.statistics.estimated_source_tokens,
    )?;
    writeln!(
        writer,
        "SCAN clusters={} high_value={} duration={}ms",
        result.statistics.clusters_found,
        result.statistics.high_value_clusters,
        result.statistics.scan_duration_ms,
    )?;

    if !result.parse_errors.is_empty() {
        writeln!(
            writer,
            "PARSE_ERRORS {} (affected units omitted; use JSON for diagnostics)",
            result.parse_errors.len()
        )?;
    }

    if result.clusters.is_empty() {
        writeln!(writer, "NO_CLUSTERS")?;
        return Ok(());
    }

    // Select clusters that fit within token budget
    let selected = select_within_budget(&result.clusters, &result.functions, base_path, max_tokens);

    if selected.len() < result.clusters.len() {
        writeln!(
            writer,
            "SHOWING {}/{} clusters (token budget)",
            selected.len(),
            result.clusters.len()
        )?;
    }

    writeln!(writer, "---")?;

    for &idx in &selected {
        let cluster = &result.clusters[idx];

        // Cluster header
        writeln!(
            writer,
            "{} | {} | signal={:.0}% | confidence={:.0}%",
            cluster.id,
            cluster.classification,
            cluster.average_similarity * 100.0,
            cluster.confidence * 100.0,
        )?;

        writeln!(
            writer,
            "{} functions | duplicated≈{} tokens | reduction≈{} tokens | value={:.0}",
            cluster.function_indices.len(),
            cluster.duplicated_tokens_estimate,
            cluster.potential_reduction_estimate,
            cluster.refactoring_value,
        )?;

        if let Some(member) = cluster
            .function_indices
            .first()
            .and_then(|&i| result.functions.get(i))
        {
            writeln!(writer, "LANGUAGE {}", member.language)?;
        }

        // Function locations
        writeln!(writer, "MEMBERS")?;
        for &func_idx in &cluster.function_indices {
            if let Some(f) = result.functions.get(func_idx) {
                writeln!(writer, "  {}", f.display_location(base_path))?;
            }
        }

        // Common structure
        if !cluster.common_structure.is_empty() {
            writeln!(writer, "COMMON")?;
            writeln!(writer, "  {}", cluster.common_structure.join(" → "))?;
        }

        // Differences
        if !cluster.differences.is_empty() {
            writeln!(writer, "DIFF")?;
            for d in &cluster.differences {
                writeln!(writer, "  {d}")?;
            }
        }

        // Signals
        writeln!(
            writer,
            "SIGNALS ast={:.2} tokens={:.2} calls={:.2} control={:.2} complexity={:.2} params={:.2}",
            cluster.signals.ast,
            cluster.signals.tokens,
            cluster.signals.calls,
            cluster.signals.control_flow,
            cluster.signals.complexity,
            cluster.signals.params,
        )?;

        // Reason
        writeln!(writer, "WHY")?;
        writeln!(writer, "  {}", cluster.reason)?;

        writeln!(writer, "---")?;
    }

    Ok(())
}
