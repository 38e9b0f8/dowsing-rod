use crate::types::{Cluster, ScanConfig};

/// Sort clusters by refactoring value (highest first) and assign final IDs.
///
/// Ranking model (documented):
///
///   refactoring_value ≈ duplication_volume × similarity × abstraction_confidence × cluster_size_factor
///
/// Where:
///   duplication_volume   = estimated duplicated tokens across cluster members
///   similarity           = average pairwise similarity (0.0–1.0)
///   abstraction_confidence = classification confidence (0.0–1.0)
///   cluster_size_factor  = log2(cluster_size) — rewards larger clusters logarithmically
///
/// A 99%-similar 2-function cluster will generally rank below an 87%-similar 10-function
/// cluster if the latter has more duplicated token volume.
pub fn rank_clusters(clusters: &mut [Cluster]) {
    // Sort descending by refactoring_value; break ties by cluster ID (alphabetic) for determinism
    clusters.sort_by(|a, b| {
        b.refactoring_value
            .partial_cmp(&a.refactoring_value)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    // Re-assign IDs in ranked order
    for (i, cluster) in clusters.iter_mut().enumerate() {
        cluster.id = format!("C{}", i + 1);
    }
}

/// Apply max_clusters limit from config.
pub fn apply_limits(clusters: &mut Vec<Cluster>, config: &ScanConfig) {
    if let Some(max) = config.max_clusters {
        clusters.truncate(max);
    }
}

/// Determine how many clusters qualify as "high value" (strong signal).
pub fn count_high_value(clusters: &[Cluster]) -> usize {
    clusters
        .iter()
        .filter(|c| c.average_similarity >= 0.85 && c.duplicated_tokens_estimate >= 500)
        .count()
}

/// Determine how many clusters qualify as "strong signal".
pub fn count_strong_signals(clusters: &[Cluster]) -> usize {
    clusters
        .iter()
        .filter(|c| c.average_similarity >= 0.80 && c.duplicated_tokens_estimate >= 200)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{RefactoringClassification, SimilaritySignals};

    fn make_cluster(id: &str, sim: f64, tokens: usize, size: usize) -> Cluster {
        let size_factor = (size as f64).log2().max(0.1);
        Cluster {
            id: id.to_string(),
            function_indices: (0..size).collect(),
            classification: RefactoringClassification::StrategyCandidate,
            confidence: 0.9,
            average_similarity: sim,
            common_structure: vec![],
            differences: vec![],
            duplicated_tokens_estimate: tokens,
            potential_reduction_estimate: (tokens as f64 * 0.7) as usize,
            refactoring_value: tokens as f64 * sim * 0.9 * size_factor,
            signals: SimilaritySignals {
                ast: sim,
                tokens: sim,
                calls: 0.7,
                control_flow: sim,
                complexity: 1.0,
                params: 1.0,
            },
            reason: String::new(),
        }
    }

    #[test]
    fn test_rank_orders_by_value() {
        let mut clusters = vec![
            make_cluster("X", 0.99, 100, 2),   // high sim, small
            make_cluster("Y", 0.87, 5000, 10), // lower sim, large volume
        ];
        rank_clusters(&mut clusters);
        // The large-volume cluster should rank first
        assert_eq!(clusters[0].id, "C1");
        assert!(clusters[0].duplicated_tokens_estimate >= clusters[1].duplicated_tokens_estimate);
    }

    #[test]
    fn test_ids_reassigned_after_ranking() {
        let mut clusters = vec![
            make_cluster("Z5", 0.80, 100, 2),
            make_cluster("Z1", 0.95, 2000, 5),
        ];
        rank_clusters(&mut clusters);
        assert_eq!(clusters[0].id, "C1");
        assert_eq!(clusters[1].id, "C2");
    }
}
