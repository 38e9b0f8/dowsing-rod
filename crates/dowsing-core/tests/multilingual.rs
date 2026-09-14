use dowsing_core::{
    language::Language,
    types::{FunctionKind, ScanConfig},
};
use std::{fs, path::Path};

#[test]
fn all_languages_extract_and_find_known_duplicates() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/languages");
    let result = dowsing_core::scan(ScanConfig {
        path,
        use_cache: false,
        fail_on_error: true,
        min_similarity: 0.75,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(result.statistics.files_scanned, 10);
    assert_eq!(result.statistics.functions_found, 37);
    for language in [
        Language::TypeScript,
        Language::JavaScript,
        Language::C,
        Language::Cpp,
        Language::CSharp,
        Language::Rust,
    ] {
        assert!(
            result
                .clusters
                .iter()
                .any(|c| result.functions[c.function_indices[0]].language == language),
            "missing {language:?} duplicate"
        );
    }
    for cluster in &result.clusters {
        let language = result.functions[cluster.function_indices[0]].language;
        assert!(cluster
            .function_indices
            .iter()
            .all(|&i| result.functions[i].language == language));
        if language.is_hdl() {
            assert_eq!(
                cluster.classification,
                dowsing_core::types::RefactoringClassification::HdlReviewRequired
            );
            assert_eq!(cluster.refactoring_value, 0.0);
        }
        assert!(cluster
            .function_indices
            .iter()
            .all(|&i| { result.functions[i].kind != FunctionKind::Process }));
    }
    assert_eq!(
        result
            .functions
            .iter()
            .filter(|f| f.kind == FunctionKind::Process)
            .count(),
        7
    );
    assert!(result
        .functions
        .iter()
        .any(|f| f.qualified_name == "counter::twice"));
}

#[test]
fn cross_language_pairs_are_never_generated() {
    let dir = tempfile::tempdir().unwrap();
    for (ext, source) in [
        ("c", "int same(int x) { return x + 1; }"),
        ("cpp", "int same(int x) { return x + 1; }"),
        ("rs", "fn same(x: i32) -> i32 { x + 1 }"),
    ] {
        fs::write(dir.path().join(format!("same.{ext}")), source).unwrap();
    }
    let result = dowsing_core::scan(ScanConfig {
        path: dir.path().to_path_buf(),
        use_cache: false,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(result.statistics.functions_found, 3);
    assert_eq!(result.statistics.candidate_pairs_generated, 0);
    assert!(result.clusters.is_empty());

    fs::write(
        dir.path().join("same.js"),
        "function same(x) { return x + 1; }",
    )
    .unwrap();
    fs::write(
        dir.path().join("same.ts"),
        "function same(x: number) { return x + 1; }",
    )
    .unwrap();
    let result = dowsing_core::scan(ScanConfig {
        path: dir.path().to_path_buf(),
        use_cache: false,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(result.statistics.functions_found, 5);
    assert!(result.clusters.is_empty());
}

#[test]
fn cache_keeps_language_and_never_hides_errors() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("a.ts"),
        "function a(x: number) { return x; }",
    )
    .unwrap();
    fs::write(dir.path().join("bad.c"), "int bad( {").unwrap();
    let config = ScanConfig {
        path: dir.path().to_path_buf(),
        ..Default::default()
    };
    let first = dowsing_core::scan(config.clone()).unwrap();
    let second = dowsing_core::scan(config.clone()).unwrap();
    assert_eq!(second.statistics.cache_hits, 1);
    assert_eq!(second.parse_errors.len(), first.parse_errors.len());
    assert_eq!(second.functions[0].language, Language::TypeScript);
    assert!(dowsing_core::scan(ScanConfig {
        fail_on_error: true,
        ..config
    })
    .is_err());
}

#[test]
fn invalid_encoding_is_reported_and_fails_strict_scans() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.cs");
    fs::write(&path, [0xff, 0xfe]).unwrap();
    let result = dowsing_core::scan(ScanConfig {
        path: path.clone(),
        use_cache: false,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(result.statistics.files_with_errors, 1);
    assert_eq!(result.parse_errors.len(), 1);
    assert!(dowsing_core::scan(ScanConfig {
        path,
        fail_on_error: true,
        ..Default::default()
    })
    .is_err());
}

#[test]
fn discovery_handles_headers_extensions_and_exclusions() {
    let dir = tempfile::tempdir().unwrap();
    for (index, ext) in [
        "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "c", "h", "C", "cpp", "hpp", "cc",
        "cxx", "cs", "rs", "v", "vh", "sv", "svh", "vhd", "vhdl", "py",
    ]
    .into_iter()
    .enumerate()
    {
        fs::write(dir.path().join(format!("a{index}.{ext}")), "").unwrap();
    }
    for excluded in ["node_modules", "target", "obj"] {
        fs::create_dir(dir.path().join(excluded)).unwrap();
        fs::write(dir.path().join(excluded).join("a.ts"), "").unwrap();
    }
    assert_eq!(
        dowsing_core::discovery::discover_source_files(dir.path(), &[], &[])
            .unwrap()
            .len(),
        24
    );
    assert_eq!(Language::from_path(Path::new("a.C")), Some(Language::Cpp));
    assert_eq!(Language::from_path(Path::new("a.rs")), Some(Language::Rust));
    assert_eq!(
        Language::from_path(Path::new("a.js")),
        Some(Language::JavaScript)
    );
    assert_eq!(
        Language::from_path(Path::new("a.jsx")),
        Some(Language::JavaScript)
    );
}

#[test]
fn javascript_and_jsx_extract_and_cluster() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("first.js"),
        "function formatUser(user) { const name = user.name; const active = user.isActive ? 'yes' : 'no'; return `${name}: ${active}`; }",
    )
    .unwrap();
    fs::write(
        dir.path().join("second.jsx"),
        "function renderUser(u) { const name = u.name; const active = u.isActive ? 'yes' : 'no'; return `${name}: ${active}`; }",
    )
    .unwrap();
    let result = dowsing_core::scan(ScanConfig {
        path: dir.path().to_path_buf(),
        use_cache: false,
        fail_on_error: true,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(result.statistics.files_scanned, 2);
    assert_eq!(result.statistics.functions_found, 2);
    assert_eq!(result.clusters.len(), 1);
    assert_eq!(result.clusters[0].function_indices.len(), 2);
    assert!(result
        .functions
        .iter()
        .all(|function| function.language == Language::JavaScript));
}

#[test]
fn standalone_configuration_works_without_python_manifest() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("dowsing-rod.toml"),
        "min_similarity = 0.93\nnormalization = 'strict'\n",
    )
    .unwrap();
    let config = dowsing_core::config::load_config(dir.path(), ScanConfig::default()).unwrap();
    assert_eq!(config.min_similarity, 0.93);
    assert_eq!(
        config.normalization,
        dowsing_core::types::NormalizationLevel::Strict
    );
}
