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

    // Try to load from pyproject.toml
    let pyproject_path = project_root.join("pyproject.toml");
    if pyproject_path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&pyproject_path) {
            if let Ok(pyproject) = toml::from_str::<PyProjectToml>(&contents) {
                if let Some(tool) = pyproject.tool {
                    if let Some(file_config) = tool.dowsing_rod {
                        if let Some(exclude) = file_config.exclude {
                            config.exclude = exclude;
                        }
                        if let Some(include) = file_config.include {
                            config.include = include;
                        }
                        if let Some(min_sim) = file_config.min_similarity {
                            config.min_similarity = min_sim;
                        }
                        if let Some(norm) = file_config.normalization {
                            config.normalization = norm;
                        }
                        if let Some(max_c) = file_config.max_clusters {
                            config.max_clusters = Some(max_c);
                        }
                        if let Some(max_t) = file_config.max_tokens {
                            config.max_tokens = Some(max_t);
                        }
                    }
                }
            }
        }
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
