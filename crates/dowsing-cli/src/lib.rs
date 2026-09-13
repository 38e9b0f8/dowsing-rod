use clap::{Parser, Subcommand, ValueEnum};
use dowsing_core::{
    render,
    types::{NormalizationLevel, OutputFormat, ScanConfig},
};
use std::path::PathBuf;

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
