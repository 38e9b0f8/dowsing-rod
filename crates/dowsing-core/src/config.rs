use crate::types::{NormalizationLevel, OutputFormat, ScanConfig};
use anyhow::Result;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Standalone configuration file name, written by `dowsing-rod init`.
pub const CONFIG_FILE: &str = "dowsing-rod.toml";

/// Recommended starting configuration written by `dowsing-rod init`.
pub const DEFAULT_CONFIG_TOML: &str = include_str!("default_config.toml");

/// Configuration loaded from dowsing-rod.toml or pyproject.toml [tool.dowsing-rod].
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileConfig {
    pub exclude: Option<Vec<String>>,
    pub include: Option<Vec<String>>,
    pub min_similarity: Option<f64>,
    pub normalization: Option<NormalizationLevel>,
    pub max_clusters: Option<usize>,
    pub max_tokens: Option<usize>,
    /// Output used by the CLI when neither --format nor --ai is passed.
    pub format: Option<OutputFormat>,
    pub fail_on_error: Option<bool>,
    /// Keys this version does not recognize, such as a misspelled `min_simlarity`.
    #[serde(flatten)]
    pub unknown: BTreeMap<String, toml::Value>,
    /// The file these values were read from.
    #[serde(skip)]
    pub source: PathBuf,
}

impl FileConfig {
    /// Built-in defaults overlaid with this file's values. Callers apply their
    /// own explicit options on top, so an explicit value always wins.
    pub fn to_scan_config(&self) -> ScanConfig {
        let defaults = ScanConfig::default();
        ScanConfig {
            exclude: self.exclude.clone().unwrap_or_default(),
            include: self.include.clone().unwrap_or_default(),
            min_similarity: self.min_similarity.unwrap_or(defaults.min_similarity),
            normalization: self.normalization.unwrap_or(defaults.normalization),
            max_clusters: self.max_clusters,
            max_tokens: self.max_tokens,
            fail_on_error: self.fail_on_error.unwrap_or(defaults.fail_on_error),
            ..defaults
        }
    }

    /// One message per key that this version ignores.
    pub fn warnings(&self) -> Vec<String> {
        self.unknown
            .keys()
            .map(|key| format!("{}: unknown key `{key}` is ignored", self.source.display()))
            .collect()
    }
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

/// Read file configuration from the project root. `dowsing-rod.toml` wins over
/// `pyproject.toml`. Returns `None` when neither file configures Dowsing Rod.
pub fn load_file_config(project_root: &Path) -> Result<Option<FileConfig>> {
    let standalone = project_root.join(CONFIG_FILE);
    let pyproject = project_root.join("pyproject.toml");
    let (file, source) = if standalone.is_file() {
        (Some(parse_toml::<FileConfig>(&standalone)?), standalone)
    } else if pyproject.is_file() {
        let section = parse_toml::<PyProjectToml>(&pyproject)?
            .tool
            .and_then(|t| t.dowsing_rod);
        (section, pyproject)
    } else {
        return Ok(None);
    };
    Ok(file.map(|file| FileConfig { source, ..file }))
}

/// Keep the TOML error in the top-level message: the Python bindings report
/// errors with `to_string()`, which drops anyhow context layers.
fn parse_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    toml::from_str(&std::fs::read_to_string(path)?)
        .map_err(|e| anyhow::anyhow!("invalid {}: {e}", path.display()))
}

/// Cheap text check, so an unrelated or broken pyproject.toml in a parent
/// folder never fails a scan.
fn pyproject_has_section(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("pyproject.toml"))
        .is_ok_and(|text| text.contains("[tool.dowsing-rod]"))
}

fn has_config(dir: &Path) -> bool {
    dir.join(CONFIG_FILE).is_file() || pyproject_has_section(dir)
}

/// Write the default `dowsing-rod.toml` into `project_root` and return its path.
/// Refuses to replace existing configuration unless `force` is set.
pub fn init_project(project_root: &Path, force: bool) -> Result<PathBuf> {
    anyhow::ensure!(
        project_root.is_dir(),
        "{} is not a directory",
        project_root.display()
    );
    let target = project_root.join(CONFIG_FILE);
    if !force {
        anyhow::ensure!(
            !target.exists(),
            "{} already exists; pass --force to overwrite it",
            target.display()
        );
        anyhow::ensure!(
            !pyproject_has_section(project_root),
            "{} already has [tool.dowsing-rod], which {CONFIG_FILE} would override; \
             pass --force to create it anyway",
            project_root.join("pyproject.toml").display()
        );
    }
    std::fs::write(&target, DEFAULT_CONFIG_TOML)?;
    Ok(target)
}

/// Check the values a scan relies on, wherever they came from.
pub fn validate(config: &ScanConfig) -> Result<()> {
    anyhow::ensure!(
        config.min_similarity.is_finite() && (0.0..=1.0).contains(&config.min_similarity),
        "min_similarity must be between 0 and 1"
    );
    anyhow::ensure!(config.jobs != Some(0), "jobs must be at least 1");
    Ok(())
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

/// Resolve the project root used for config, cache, and display paths: the
/// nearest enclosing folder with Dowsing Rod configuration, as ruff and pytest
/// do, or the scanned folder itself when no folder above it is configured.
pub fn resolve_project_root(path: &Path) -> Result<PathBuf> {
    let abs = resolve_scan_path(path)?;
    let start = if abs.is_file() {
        abs.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| abs.clone())
    } else {
        abs
    };
    let configured = start.ancestors().find(|dir| has_config(dir));
    Ok(configured.unwrap_or(&start).to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_template_matches_built_in_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE), DEFAULT_CONFIG_TOML).unwrap();
        let file = load_file_config(dir.path()).unwrap().unwrap();
        let config = file.to_scan_config();
        let defaults = ScanConfig::default();

        assert_eq!(file.format, Some(OutputFormat::Human));
        assert_eq!(config.min_similarity, defaults.min_similarity);
        assert_eq!(config.normalization, defaults.normalization);
        assert_eq!(config.fail_on_error, defaults.fail_on_error);
        assert_eq!(config.exclude, defaults.exclude);
        assert_eq!(config.max_clusters, defaults.max_clusters);
    }

    #[test]
    fn init_writes_default_config_once() {
        let dir = tempfile::tempdir().unwrap();
        let written = init_project(dir.path(), false).unwrap();
        assert_eq!(
            std::fs::read_to_string(&written).unwrap(),
            DEFAULT_CONFIG_TOML
        );

        std::fs::write(&written, "min_similarity = 0.9\n").unwrap();
        assert!(init_project(dir.path(), false).is_err());
        assert_eq!(
            std::fs::read_to_string(&written).unwrap(),
            "min_similarity = 0.9\n"
        );

        init_project(dir.path(), true).unwrap();
        assert_eq!(
            std::fs::read_to_string(&written).unwrap(),
            DEFAULT_CONFIG_TOML
        );
    }

    #[test]
    fn init_does_not_shadow_pyproject_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[tool.dowsing-rod]\nmin_similarity = 0.9\n",
        )
        .unwrap();
        assert!(init_project(dir.path(), false).is_err());
        assert!(!dir.path().join(CONFIG_FILE).exists());
    }

    #[test]
    fn file_config_can_enable_fail_on_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE), "fail_on_error = true\n").unwrap();
        let file = load_file_config(dir.path()).unwrap().unwrap();
        assert!(file.to_scan_config().fail_on_error);
    }

    #[test]
    fn unknown_keys_are_reported_with_their_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(CONFIG_FILE),
            "min_simlarity = 0.9\nnormalization = 'strict'\n",
        )
        .unwrap();
        let file = load_file_config(dir.path()).unwrap().unwrap();
        let warnings = file.warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("min_simlarity") && warnings[0].contains(CONFIG_FILE));
        assert_eq!(file.normalization, Some(NormalizationLevel::Strict));
    }

    #[test]
    fn template_and_other_pyproject_tools_produce_no_warnings() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE), DEFAULT_CONFIG_TOML).unwrap();
        assert!(load_file_config(dir.path())
            .unwrap()
            .unwrap()
            .warnings()
            .is_empty());

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[tool.ruff]\nline-length = 100\n[tool.dowsing-rod]\nmax_clusters = 5\n",
        )
        .unwrap();
        let file = load_file_config(dir.path()).unwrap().unwrap();
        assert!(file.warnings().is_empty(), "{:?}", file.warnings());
        assert_eq!(file.max_clusters, Some(5));
    }

    #[test]
    fn project_root_is_nearest_configured_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("src").join("deep");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("a.py"), "def f(): pass\n").unwrap();
        assert_eq!(resolve_project_root(&nested).unwrap(), nested);

        std::fs::write(dir.path().join(CONFIG_FILE), "").unwrap();
        assert_eq!(resolve_project_root(&nested).unwrap(), dir.path());
        assert_eq!(
            resolve_project_root(&nested.join("a.py")).unwrap(),
            dir.path()
        );

        std::fs::write(
            dir.path().join("src").join("pyproject.toml"),
            "[tool.dowsing-rod]\n",
        )
        .unwrap();
        assert_eq!(
            resolve_project_root(&nested).unwrap(),
            dir.path().join("src")
        );
    }

    #[test]
    fn pyproject_format_is_read() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[tool.dowsing-rod]\nformat = 'jsonl'\n",
        )
        .unwrap();
        let file = load_file_config(dir.path()).unwrap().unwrap();
        assert_eq!(file.format, Some(OutputFormat::Jsonl));
    }

    #[test]
    fn missing_config_is_reported_as_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_file_config(dir.path()).unwrap().is_none());
        std::fs::write(dir.path().join("pyproject.toml"), "[project]\nname = 'x'\n").unwrap();
        assert!(load_file_config(dir.path()).unwrap().is_none());
    }

    #[test]
    fn invalid_format_names_the_config_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE), "format = 'xml'\n").unwrap();
        // Plain Display, as the Python bindings render it.
        let message = load_file_config(dir.path()).unwrap_err().to_string();
        assert!(
            message.contains(CONFIG_FILE) && message.contains("xml"),
            "{message}"
        );
    }
}
