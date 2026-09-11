use dowsing_core::types::{RefactoringClassification, ScanConfig};
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under workspace/crates/dowsing-core")
        .to_path_buf()
}

fn fixture_path(name: &str) -> PathBuf {
    workspace_root().join("tests").join("fixtures").join(name)
}

fn scan_fixtures() -> dowsing_core::types::ScanResult {
    dowsing_core::scan(ScanConfig {
        path: fixture_path(""),
        use_cache: false,
        ..ScanConfig::default()
    })
    .expect("fixture scan should complete")
}

fn cluster_function_names<'a>(
    result: &'a dowsing_core::types::ScanResult,
    cluster: &'a dowsing_core::types::Cluster,
) -> Vec<&'a str> {
    cluster
        .function_indices
        .iter()
        .filter_map(|&idx| result.functions.get(idx))
        .map(|function| function.function_name.as_str())
        .collect()
}

#[test]
fn fixture_scan_reports_expected_repository_shape() {
    let result = scan_fixtures();

    assert!(result.statistics.files_scanned >= 12);
    assert!(result.statistics.functions_found >= 30);
    assert!(!result.clusters.is_empty());
    assert!(
        result
            .parse_errors
            .iter()
            .any(|error| error.file.ends_with("syntax_errors.py")),
        "syntax_errors.py should be reported without aborting the scan"
    );
}

#[test]
fn fixture_scan_detects_known_refactoring_candidates() {
    let result = scan_fixtures();

    let exact_duplicate = result.clusters.iter().find(|cluster| {
        let names = cluster_function_names(&result, cluster);
        names.contains(&"process_order") && names.contains(&"handle_order")
    });
    assert!(
        exact_duplicate.is_some(),
        "process_order/handle_order should be clustered"
    );

    let strategy = result.clusters.iter().find(|cluster| {
        let names = cluster_function_names(&result, cluster);
        names.contains(&"charge_stripe")
            && names.contains(&"charge_paypal")
            && names.contains(&"charge_adyen")
    });
    assert!(
        matches!(
            strategy.map(|cluster| &cluster.classification),
            Some(RefactoringClassification::StrategyCandidate)
        ),
        "provider charge functions should be classified as a strategy candidate"
    );
}

#[test]
fn fail_on_error_rejects_syntax_errors() {
    let error = dowsing_core::scan(ScanConfig {
        path: fixture_path(""),
        fail_on_error: true,
        use_cache: false,
        ..ScanConfig::default()
    })
    .expect_err("strict scans should fail when a fixture has a syntax error");

    assert!(error.to_string().contains("syntax_errors.py"));
}

#[test]
fn single_file_scan_uses_parent_as_repository_root() {
    let file = fixture_path("exact_duplicates.py");
    let result = dowsing_core::scan(ScanConfig {
        path: file.clone(),
        use_cache: false,
        ..ScanConfig::default()
    })
    .expect("single-file scan should complete");

    assert_eq!(result.statistics.files_scanned, 1);
    assert_eq!(result.repository.path, file.parent().unwrap());
    assert!(result
        .clusters
        .iter()
        .any(|cluster| cluster_function_names(&result, cluster).contains(&"process_order")));
}
