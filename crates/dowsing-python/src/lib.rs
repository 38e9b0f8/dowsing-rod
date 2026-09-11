use clap::{Parser, Subcommand, ValueEnum};
use dowsing_core::render;
use dowsing_core::types::{NormalizationLevel, OutputFormat, ScanConfig};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::path::PathBuf;

// ─── PyO3 Module ─────────────────────────────────────────────────────────

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(py_scan, m)?)?;
    m.add_function(wrap_pyfunction!(py_version, m)?)?;
    m.add_function(wrap_pyfunction!(py_clear_cache, m)?)?;
    m.add_function(wrap_pyfunction!(py_cli_main, m)?)?;
    Ok(())
}

/// Scan a Python codebase for structural refactoring opportunities.
///
/// Returns a dict with schema_version, tool_version, repository, statistics,
/// clusters, functions, and parse_errors.
#[pyfunction]
#[pyo3(name = "scan", signature = (path=".", **kwargs))]
fn py_scan(py: Python<'_>, path: &str, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<PyObject> {
    let mut config = ScanConfig {
        path: PathBuf::from(path),
        ..ScanConfig::default()
    };

    if let Some(kw) = kwargs {
        if let Some(v) = kw.get_item("min_similarity")? {
            config.min_similarity = v.extract()?;
        }
        if let Some(v) = kw.get_item("normalization")? {
            let s: String = v.extract()?;
            config.normalization = s
                .parse()
                .map_err(|e: String| PyErr::new::<pyo3::exceptions::PyValueError, _>(e))?;
        }
        if let Some(v) = kw.get_item("max_clusters")? {
            config.max_clusters = Some(v.extract()?);
        }
        if let Some(v) = kw.get_item("max_tokens")? {
            config.max_tokens = Some(v.extract()?);
        }
        if let Some(v) = kw.get_item("exclude")? {
            config.exclude = v.extract()?;
        }
        if let Some(v) = kw.get_item("include")? {
            config.include = v.extract()?;
        }
        if let Some(v) = kw.get_item("jobs")? {
            config.jobs = Some(v.extract()?);
        }
        if let Some(v) = kw.get_item("no_cache")? {
            let no_cache: bool = v.extract()?;
            config.use_cache = !no_cache;
        }
        if let Some(v) = kw.get_item("use_cache")? {
            config.use_cache = v.extract()?;
        }
        if let Some(v) = kw.get_item("fail_on_error")? {
            config.fail_on_error = v.extract()?;
        }
    }

    let result = dowsing_core::scan(config)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    // Serialize to JSON then parse into Python dict for clean interop
    let json_str = serde_json::to_string(&result)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let json_mod = py.import("json")?;
    let py_dict = json_mod.call_method1("loads", (json_str,))?;
    Ok(py_dict.unbind())
}

/// Return the tool version string.
#[pyfunction]
#[pyo3(name = "version")]
fn py_version() -> String {
    dowsing_core::VERSION.to_string()
}

/// Clear the analysis cache for a project directory.
///
/// Returns the number of cache entries removed.
#[pyfunction]
#[pyo3(name = "clear_cache", signature = (path="."))]
fn py_clear_cache(path: &str) -> PyResult<usize> {
    let root = PathBuf::from(path);
    let root = dowsing_core::config::resolve_project_root(&root)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
    dowsing_core::clear_cache(&root)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// CLI entry point, called from Python's console_scripts.
#[pyfunction]
#[pyo3(name = "cli_main")]
fn py_cli_main(py: Python<'_>) -> PyResult<()> {
    let sys = py.import("sys")?;
    let mut argv: Vec<String> = sys.getattr("argv")?.extract()?;
    if argv.is_empty() {
        argv.push("dowsing-rod".to_string());
    } else {
        argv[0] = "dowsing-rod".to_string();
    }

    run_cli_from(argv).map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

// ─── CLI (clap) ──────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "dowsing-rod",
    about = "Structural refactoring intelligence for Python — Rust-native, AI-ready",
    version = dowsing_core::VERSION,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a Python codebase for structural refactoring opportunities.
    Scan {
        /// Path to scan (file or directory).
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format optimized for AI consumption.
        #[arg(long)]
        ai: bool,

        /// Output format.
        #[arg(long, value_enum, default_value = "human")]
        format: CliOutputFormat,

        /// Minimum similarity threshold (0.0–1.0).
        #[arg(long, default_value = "0.75")]
        min_similarity: f64,

        /// Maximum number of clusters to report.
        #[arg(long)]
        max_clusters: Option<usize>,

        /// Maximum tokens for AI output.
        #[arg(long)]
        max_tokens: Option<usize>,

        /// Normalization level.
        #[arg(long, default_value = "balanced")]
        normalization: String,

        /// Number of parallel threads.
        #[arg(long, short = 'j')]
        jobs: Option<usize>,

        /// Additional path patterns to exclude.
        #[arg(long)]
        exclude: Vec<String>,

        /// Path patterns to include (overrides exclude).
        #[arg(long)]
        include: Vec<String>,

        /// Disable the file cache.
        #[arg(long)]
        no_cache: bool,

        /// Exit with error if any file fails to parse.
        #[arg(long)]
        fail_on_error: bool,
    },

    /// Manage the analysis cache.
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },

    /// Print version information.
    Version,
}

#[derive(Subcommand)]
enum CacheAction {
    /// Clear all cache entries.
    Clear {
        /// Project directory (default: current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Clone, ValueEnum)]
enum CliOutputFormat {
    Human,
    Json,
    Jsonl,
}

fn run_cli_from(args: Vec<String>) -> anyhow::Result<()> {
    let cli = Cli::parse_from(args);

    match cli.command {
        Commands::Scan {
            path,
            ai,
            format,
            min_similarity,
            max_clusters,
            max_tokens,
            normalization,
            jobs,
            exclude,
            include,
            no_cache,
            fail_on_error,
        } => {
            let norm_level: NormalizationLevel = normalization
                .parse()
                .map_err(|e: String| anyhow::anyhow!(e))?;

            let config = ScanConfig {
                path,
                exclude,
                include,
                min_similarity,
                normalization: norm_level,
                max_clusters,
                max_tokens,
                jobs,
                fail_on_error,
                use_cache: !no_cache,
            };

            // Determine output format (--ai overrides --format)
            let output_format = if ai {
                OutputFormat::Ai
            } else {
                match format {
                    CliOutputFormat::Human => OutputFormat::Human,
                    CliOutputFormat::Json => OutputFormat::Json,
                    CliOutputFormat::Jsonl => OutputFormat::Jsonl,
                }
            };

            let result = dowsing_core::scan(config)?;

            // Check fail-on-error
            if fail_on_error && !result.parse_errors.is_empty() {
                for err in &result.parse_errors {
                    eprintln!("error: {err}");
                }
                std::process::exit(1);
            }

            let mut stdout = std::io::stdout().lock();
            render::render(&result, output_format, &mut stdout, max_tokens)?;
        }

        Commands::Cache { action } => match action {
            CacheAction::Clear { path } => {
                let root = dowsing_core::config::resolve_project_root(&path)?;
                let count = dowsing_core::clear_cache(&root)?;
                println!("Cleared {count} cache entries.");
            }
        },

        Commands::Version => {
            println!("dowsing-rod {}", dowsing_core::VERSION);
        }
    }

    Ok(())
}
