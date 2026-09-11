use anyhow::Result;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// Default directories to exclude from scanning.
const DEFAULT_EXCLUDES: &[&str] = &[
    ".git",
    ".venv",
    "venv",
    "env",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".tox",
    ".nox",
    "dist",
    "build",
    "node_modules",
    "site-packages",
    ".eggs",
    "*.egg-info",
    ".dowsing-rod-cache",
];

/// Discover all Python files in the given directory.
///
/// Uses the `ignore` crate which natively respects `.gitignore` files.
/// Does not follow symlinks by default.
pub fn discover_python_files(
    root: &Path,
    extra_excludes: &[String],
    extra_includes: &[String],
) -> Result<Vec<PathBuf>> {
    if root.is_file() {
        if root.extension().is_some_and(|ext| ext == "py") {
            return Ok(vec![root.to_path_buf()]);
        }
        return Ok(Vec::new());
    }

    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false) // don't skip hidden files by default (we have explicit excludes)
        .follow_links(false)
        .git_ignore(true) // respect .gitignore
        .git_global(false)
        .git_exclude(true);

    // Add default exclusion globs
    let mut overrides = ignore::overrides::OverrideBuilder::new(root);
    for excl in DEFAULT_EXCLUDES {
        overrides.add(&format!("!{excl}"))?;
        overrides.add(&format!("!{excl}/**"))?;
    }

    // Add user-specified exclusions
    for excl in extra_excludes {
        let pattern = if excl.starts_with('!') {
            excl.clone()
        } else {
            format!("!{excl}")
        };
        overrides.add(&pattern)?;
        if !pattern.ends_with("/**") && !pattern.contains('*') {
            overrides.add(&format!("{pattern}/**"))?;
        }
    }

    // Add user-specified inclusions (these override exclusions)
    for incl in extra_includes {
        overrides.add(incl)?;
    }

    builder.overrides(overrides.build()?);

    let mut files = Vec::new();
    for entry in builder.build() {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "py") {
            files.push(path.to_path_buf());
        }
    }

    // Sort for deterministic output
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_discover_python_files() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        fs::write(root.join("main.py"), "print('hello')").unwrap();
        fs::write(root.join("utils.py"), "def foo(): pass").unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub/module.py"), "x = 1").unwrap();
        fs::write(root.join("readme.md"), "not python").unwrap();

        let files = discover_python_files(root, &[], &[]).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files.iter().all(|f| f.extension().unwrap() == "py"));
    }

    #[test]
    fn test_excludes_pycache() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        fs::write(root.join("main.py"), "x = 1").unwrap();
        fs::create_dir_all(root.join("__pycache__")).unwrap();
        fs::write(root.join("__pycache__/cached.py"), "x = 2").unwrap();

        let files = discover_python_files(root, &[], &[]).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("main.py"));
    }

    #[test]
    fn test_custom_exclude() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        fs::write(root.join("main.py"), "x = 1").unwrap();
        fs::create_dir_all(root.join("generated")).unwrap();
        fs::write(root.join("generated/gen.py"), "x = 2").unwrap();

        let files = discover_python_files(root, &["generated".to_string()], &[]).unwrap();
        assert_eq!(files.len(), 1);
    }
}
