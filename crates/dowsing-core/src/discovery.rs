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
    "target",
    "obj",
    "site-packages",
    ".eggs",
    "*.egg-info",
    ".dowsing-rod-cache",
    "vendor",
    "vendors",
    "third_party",
    "3rdparty",
    "external",
    "deps",
    "generated",
    "snapshots",
    "baseline",
    "baselines",
    "*.generated.*",
    "*_generated.*",
    "*.g.cs",
    "*.Designer.cs",
    "*.designer.cs",
    "*.pb.cc",
    "*.pb.h",
    "*.min.js",
];

/// Discover all supported source files in the given directory.
///
/// Uses the `ignore` crate which natively respects `.gitignore` files.
/// Does not follow symlinks by default.
pub fn discover_source_files(
    root: &Path,
    extra_excludes: &[String],
    extra_includes: &[String],
) -> Result<Vec<PathBuf>> {
    if root.is_file() {
        if crate::language::Language::from_path(root).is_some() {
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
        if !excl.contains('*') {
            overrides.add(&format!("!{excl}/**"))?;
        }
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

    // Add user-specified inclusions. A directory pattern must include its
    // descendants or the override engine will not yield its source files.
    for incl in extra_includes {
        overrides.add(incl)?;
        if !incl.ends_with("/**") && !incl.contains('*') {
            overrides.add(&format!("{incl}/**"))?;
        }
    }

    builder.overrides(overrides.build()?);

    let mut files = Vec::new();
    for entry in builder.build() {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && crate::language::Language::from_path(path).is_some() {
            files.push(path.to_path_buf());
        }
    }

    // Sort for deterministic output
    files.sort();
    Ok(files)
}

/// Python-only discovery retained for existing Rust callers.
pub fn discover_python_files(
    root: &Path,
    excludes: &[String],
    includes: &[String],
) -> Result<Vec<PathBuf>> {
    Ok(discover_source_files(root, excludes, includes)?
        .into_iter()
        .filter(|p| {
            crate::language::Language::from_path(p) == Some(crate::language::Language::Python)
        })
        .collect())
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

    #[test]
    fn test_multiple_directory_includes_select_their_descendants() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        for directory in ["rtl", "tb", "other"] {
            fs::create_dir_all(root.join(directory)).unwrap();
            fs::write(root.join(directory).join("unit.ts"), "function unit() {}").unwrap();
        }

        let files = discover_source_files(root, &[], &["rtl".into(), "tb".into()]).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files
            .iter()
            .all(|path| path.starts_with(root.join("rtl")) || path.starts_with(root.join("tb"))));
    }

    #[test]
    fn generated_and_vendored_sources_are_excluded_but_can_be_included() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.cs"), "class Main {}").unwrap();
        fs::write(root.join("model.generated.cs"), "class Generated {}").unwrap();
        fs::write(root.join("schema_generated.h"), "int generated(void);").unwrap();
        fs::write(root.join("bundle.min.js"), "function generated() {}").unwrap();
        fs::create_dir(root.join("snapshots")).unwrap();
        fs::write(
            root.join("snapshots/result.cs"),
            "class GeneratedSnapshot {}",
        )
        .unwrap();
        fs::create_dir(root.join("baselines")).unwrap();
        fs::write(root.join("baselines/output.js"), "function output() {}").unwrap();
        fs::create_dir(root.join("vendor")).unwrap();
        fs::write(
            root.join("vendor/library.c"),
            "int library(void) { return 1; }",
        )
        .unwrap();

        let files = discover_source_files(root, &[], &[]).unwrap();
        assert_eq!(files, vec![root.join("main.cs")]);

        let files = discover_source_files(root, &[], &["vendor".into()]).unwrap();
        assert_eq!(files, vec![root.join("vendor/library.c")]);
    }
}
