use clap::{Parser, Subcommand, ValueEnum};
use dowsing_core::{
    config::FileConfig,
    render,
    types::{OutputFormat, ScanConfig},
};
use std::path::{Path, PathBuf};

// ─── CLI (clap) ──────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "dowsing-rod",
    about = "Structural refactoring intelligence for source code — Rust-native, AI-ready",
    version = dowsing_core::VERSION,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a codebase for structural refactoring opportunities.
    Scan {
        /// Path to scan (file or directory).
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format optimized for AI consumption.
        #[arg(long)]
        ai: bool,

        /// Output format (default: `format` from config, else human).
        #[arg(long, value_enum)]
        format: Option<CliOutputFormat>,

        /// Minimum similarity threshold, 0.0–1.0 (default: config, else 0.75).
        #[arg(long)]
        min_similarity: Option<f64>,

        /// Maximum number of clusters to report.
        #[arg(long)]
        max_clusters: Option<usize>,

        /// Maximum tokens for AI output.
        #[arg(long)]
        max_tokens: Option<usize>,

        /// Normalization level: strict, balanced, or aggressive (default: config, else balanced).
        #[arg(long)]
        normalization: Option<String>,

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
        #[arg(long, conflicts_with = "no_fail_on_error")]
        fail_on_error: bool,

        /// Keep going when files fail to parse, even if the config sets fail_on_error.
        #[arg(long)]
        no_fail_on_error: bool,
    },

    /// Scan using only the project's configuration file (run `init` first).
    Auto {
        /// Project directory or file (default: current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Create a dowsing-rod.toml with the recommended default configuration.
    Init {
        /// Project directory (default: current directory).
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Overwrite existing configuration.
        #[arg(long)]
        force: bool,
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

pub fn run_cli_from(args: Vec<String>) -> anyhow::Result<()> {
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
            no_fail_on_error,
        } => {
            // Start from the config file, then apply only the options actually
            // given, so an explicit value wins even when it equals the default.
            let (_, file_config) = load_project_config(&path)?;
            let file_config = file_config.unwrap_or_default();
            let mut config = ScanConfig {
                path,
                jobs,
                use_cache: !no_cache,
                ..file_config.to_scan_config()
            };
            if let Some(value) = min_similarity {
                config.min_similarity = value;
            }
            if let Some(value) = normalization {
                config.normalization = value.parse().map_err(|e: String| anyhow::anyhow!(e))?;
            }
            if max_clusters.is_some() {
                config.max_clusters = max_clusters;
            }
            if max_tokens.is_some() {
                config.max_tokens = max_tokens;
            }
            if !exclude.is_empty() {
                config.exclude = exclude;
            }
            if !include.is_empty() {
                config.include = include;
            }
            if fail_on_error || no_fail_on_error {
                config.fail_on_error = fail_on_error;
            }
            let max_tokens = config.max_tokens;

            // Determine output format (--ai overrides --format, which overrides config)
            let output_format = match (ai, format) {
                (true, _) => OutputFormat::Ai,
                (false, Some(CliOutputFormat::Human)) => OutputFormat::Human,
                (false, Some(CliOutputFormat::Json)) => OutputFormat::Json,
                (false, Some(CliOutputFormat::Jsonl)) => OutputFormat::Jsonl,
                (false, None) => file_config.format.unwrap_or(OutputFormat::Human),
            };

            scan_and_render(config, output_format, max_tokens)?;
        }

        Commands::Auto { path } => {
            let (root, file_config) = load_project_config(&path)?;
            let file_config = file_config.ok_or_else(|| {
                anyhow::anyhow!(
                    "no configuration found in {} or any folder above it; \
                     run `dowsing-rod init` to create {}",
                    root.display(),
                    dowsing_core::config::CONFIG_FILE
                )
            })?;
            let config = ScanConfig {
                path,
                ..file_config.to_scan_config()
            };
            scan_and_render(
                config,
                file_config.format.unwrap_or(OutputFormat::Human),
                file_config.max_tokens,
            )?;
        }

        Commands::Init { path, force } => {
            let root = dowsing_core::config::resolve_scan_path(&path)?;
            let written = dowsing_core::config::init_project(&root, force)?;
            println!("Created {}", written.display());
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

/// Find the project's configuration, warn about keys it ignores, and return
/// the project root with the parsed file, if any.
fn load_project_config(path: &Path) -> anyhow::Result<(PathBuf, Option<FileConfig>)> {
    let root = dowsing_core::config::resolve_project_root(path)?;
    let file = dowsing_core::config::load_file_config(&root)?;
    for warning in file.iter().flat_map(FileConfig::warnings) {
        eprintln!("warning: {warning}");
    }
    Ok((root, file))
}

/// Run a scan and write it to stdout. Parse failures under `fail_on_error`
/// surface as an error from `dowsing_core::scan`.
fn scan_and_render(
    config: ScanConfig,
    output_format: OutputFormat,
    max_tokens: Option<usize>,
) -> anyhow::Result<()> {
    let result = dowsing_core::scan(config)?;
    let mut stdout = std::io::stdout().lock();
    render::render(&result, output_format, &mut stdout, max_tokens)?;
    Ok(())
}
