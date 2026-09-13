use crate::types::{NormalizationLevel, ScanConfig};
use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Configuration loaded from pyproject.toml [tool.dowsing-rod] section.
#[derive(Debug, Clone, Default, Deserialize)]
struct FileConfig {
    exclude: Option<Vec<String>>,
    include: Option<Vec<String>>,
    min_similarity: Option<f64>,
    normalization: Option<NormalizationLevel>,
    max_clusters: Option<usize>,
    max_tokens: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct PyProjectToml {
    tool: Option<ToolSection>,
}

#[derive(Debug, Deserialize)]
struct ToolSection {
    #[serde(rename = "dowsing-rod")]
    dowsing_rod: Option<FileConfig>,
}

/// Load configuration from pyproject.toml and merge with CLI overrides.
/// CLI values take precedence over file config, which takes precedence over defaults.
pub fn load_config(project_root: &Path, cli_overrides: ScanConfig) -> Result<ScanConfig> {
    let mut config = ScanConfig::default();

    let standalone = project_root.join("dowsing-rod.toml");
    let pyproject = project_root.join("pyproject.toml");
    let file_config = if standalone.is_file() {
        Some(toml::from_str::<FileConfig>(&std::fs::read_to_string(
            standalone,
        )?)?)
    } else if pyproject.is_file() {
        toml::from_str::<PyProjectToml>(&std::fs::read_to_string(pyproject)?)?
            .tool
            .and_then(|t| t.dowsing_rod)
    } else {
        None
    };
    if let Some(file) = file_config {
        if let Some(value) = file.exclude {
            config.exclude = value;
        }
        if let Some(value) = file.include {
            config.include = value;
        }
        if let Some(value) = file.min_similarity {
            config.min_similarity = value;
        }
        if let Some(value) = file.normalization {
            config.normalization = value;
        }
        config.max_clusters = file.max_clusters;
        config.max_tokens = file.max_tokens;
    }

    // Apply CLI overrides (non-default values override file config)
    config.path = cli_overrides.path;
    config.fail_on_error = cli_overrides.fail_on_error;
    config.use_cache = cli_overrides.use_cache;

    if !cli_overrides.exclude.is_empty() {
        config.exclude = cli_overrides.exclude;
    }
    if !cli_overrides.include.is_empty() {
        config.include = cli_overrides.include;
    }
    if cli_overrides.min_similarity != ScanConfig::default().min_similarity {
        config.min_similarity = cli_overrides.min_similarity;
    }
    if cli_overrides.normalization != NormalizationLevel::default() {
        config.normalization = cli_overrides.normalization;
    }
    if cli_overrides.max_clusters.is_some() {
        config.max_clusters = cli_overrides.max_clusters;
    }
    if cli_overrides.max_tokens.is_some() {
        config.max_tokens = cli_overrides.max_tokens;
    }
    if cli_overrides.jobs.is_some() {
        config.jobs = cli_overrides.jobs;
    }

    anyhow::ensure!(
        config.min_similarity.is_finite() && (0.0..=1.0).contains(&config.min_similarity),
        "min_similarity must be between 0 and 1"
    );
    anyhow::ensure!(config.jobs != Some(0), "jobs must be at least 1");
    Ok(config)
}

/// Resolve the absolute path to the user-requested scan target.
pub fn resolve_scan_path(path: &Path) -> Result<PathBuf> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(abs)
}

/// Resolve the absolute path of the project root used for config and cache.
pub fn resolve_project_root(path: &Path) -> Result<PathBuf> {
    let abs = resolve_scan_path(path)?;
    if abs.is_file() {
        Ok(abs
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| abs.clone()))
    } else {
        Ok(abs)
    }
}
