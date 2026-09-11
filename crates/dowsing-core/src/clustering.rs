use crate::classification::classify_cluster;
use crate::differences::{extract_common_pipeline, extract_differences};
use crate::graph::SimilarityGraph;
use crate::types::{Cluster, FunctionInfo, NormalizedFunction, SimilarityScore, SimilaritySignals};
use std::collections::HashMap;

/// Maximum cluster size — prevents meaningless mega-clusters.
const MAX_CLUSTER_SIZE: usize = 20;

/// Cluster functions from the similarity graph.
///
/// Algorithm:
/// 1. Find connected components (thresholded by min_similarity in graph construction).
/// 2. Split oversized components by removing the weakest edges iteratively.
/// 3. Assign cluster IDs. Results are deterministic (sorted by component members).
pub fn cluster_functions(
    graph: &SimilarityGraph,
    scores: &[SimilarityScore],
    functions: &[FunctionInfo],
    normalized: &[NormalizedFunction],
) -> Vec<Cluster> {
    // Build a lookup from (a, b) -> score for fast retrieval
    let mut score_lookup: HashMap<(usize, usize), &SimilarityScore> = HashMap::new();
    for score in scores {
        let key = (
            score.func_a.min(score.func_b),
            score.func_a.max(score.func_b),
        );
        score_lookup.insert(key, score);
    }

    // Build a lookup from function_id -> NormalizedFunction index
    let mut norm_lookup: HashMap<usize, &NormalizedFunction> = HashMap::new();
    for norm in normalized {
        norm_lookup.insert(norm.function_id, norm);
    }

    let components = graph.connected_components();
    let mut clusters = Vec::new();
    let mut cluster_counter = 0usize;

    for component in components {
        // Split oversized components
        let sub_components = split_if_oversized(component, graph, MAX_CLUSTER_SIZE);

        for sub in sub_components {
            if sub.len() < 2 {
                continue;
            }

            cluster_counter += 1;
            let cluster_id = format!("C{cluster_counter}");

            let cluster = build_cluster(cluster_id, sub, functions, &score_lookup, &norm_lookup);
            clusters.push(cluster);
        }
    }

    clusters
}

/// Build a Cluster from a set of function indices.
fn build_cluster(
    id: String,
    members: Vec<usize>,
    functions: &[FunctionInfo],
    score_lookup: &HashMap<(usize, usize), &SimilarityScore>,
    norm_lookup: &HashMap<usize, &NormalizedFunction>,
) -> Cluster {
    // Compute average pairwise similarity
    let mut total_sim = 0.0f64;
    let mut pair_count = 0usize;
    let mut total_signals = SimilaritySignals {
        ast: 0.0,
        tokens: 0.0,
        calls: 0.0,
        control_flow: 0.0,
        complexity: 0.0,
        params: 0.0,
    };

    for i in 0..members.len() {
        for j in (i + 1)..members.len() {
            let a = members[i].min(members[j]);
            let b = members[i].max(members[j]);
            if let Some(score) = score_lookup.get(&(a, b)) {
                total_sim += score.overall;
                total_signals.ast += score.signals.ast;
                total_signals.tokens += score.signals.tokens;
                total_signals.calls += score.signals.calls;
                total_signals.control_flow += score.signals.control_flow;
                total_signals.complexity += score.signals.complexity;
                total_signals.params += score.signals.params;
                pair_count += 1;
            }
        }
    }

    let average_similarity = if pair_count > 0 {
        total_sim / pair_count as f64
    } else {
        0.0
    };

    let avg_signals = if pair_count > 0 {
        SimilaritySignals {
            ast: total_signals.ast / pair_count as f64,
            tokens: total_signals.tokens / pair_count as f64,
            calls: total_signals.calls / pair_count as f64,
            control_flow: total_signals.control_flow / pair_count as f64,
            complexity: total_signals.complexity / pair_count as f64,
            params: total_signals.params / pair_count as f64,
        }
    } else {
        SimilaritySignals {
            ast: 1.0,
            tokens: 1.0,
            calls: 1.0,
            control_flow: 1.0,
            complexity: 1.0,
            params: 1.0,
        }
    };

    // Gather token sequences for difference analysis
    let token_slices: Vec<&[_]> = members
        .iter()
        .filter_map(|&idx| norm_lookup.get(&idx).map(|n| n.tokens.as_slice()))
        .collect();

    // Extract differences between first two members (representative)
    let (common_structure, differences) = if token_slices.len() >= 2 {
        let first = norm_lookup.get(&members[0]);
        let second = norm_lookup.get(&members[1]);
        if let (Some(a), Some(b)) = (first, second) {
            let diff = extract_differences(a, b);
            let common_pipeline = extract_common_pipeline(&token_slices);
            let common = if common_pipeline.is_empty() {
                diff.common_elements
            } else {
                common_pipeline
            };
            (common, diff.differences)
        } else {
            (Vec::new(), Vec::new())
        }
    } else {
        (Vec::new(), Vec::new())
    };

    // Estimate duplicated tokens
    let member_tokens: usize = members
        .iter()
        .filter_map(|&idx| functions.get(idx))
        .map(|f| f.estimated_tokens())
        .sum();

    // Duplicated tokens = total - (what would remain after ideal refactor)
    // Rough estimate: (cluster_size - 1) * average_function_tokens * average_similarity
    let avg_func_tokens = if members.is_empty() {
        0
    } else {
        member_tokens / members.len()
    };
    let duplicated_tokens = ((members.len().saturating_sub(1)) as f64
        * avg_func_tokens as f64
        * average_similarity) as usize;
    let potential_reduction = (duplicated_tokens as f64 * 0.7) as usize;

    // Classify
    let member_infos: Vec<&FunctionInfo> = members
        .iter()
        .filter_map(|&idx| functions.get(idx))
        .collect();

    let (classification, confidence, reason) =
        classify_cluster(&member_infos, &avg_signals, average_similarity);

    // Compute refactoring value score
    let cluster_size_factor = (members.len() as f64).log2().max(0.1);
    let refactoring_value =
        duplicated_tokens as f64 * average_similarity * confidence * cluster_size_factor;

    Cluster {
        id,
        function_indices: members,
        classification,
        confidence,
        average_similarity,
        common_structure,
        differences,
        duplicated_tokens_estimate: duplicated_tokens,
        potential_reduction_estimate: potential_reduction,
        refactoring_value,
        signals: avg_signals,
        reason,
    }
}

/// Split an oversized component into smaller ones by removing weakest edges.
fn split_if_oversized(
    component: Vec<usize>,
    graph: &SimilarityGraph,
    max_size: usize,
) -> Vec<Vec<usize>> {
    if component.len() <= max_size {
        return vec![component];
    }

    // Iteratively remove the weakest edges until all components are <= max_size
    let mut adj: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
    for &node in &component {
        for &(neighbor, weight) in graph.neighbors(node) {
            if component.contains(&neighbor) {
                adj.entry(node).or_default().push((neighbor, weight));
            }
        }
    }

    // Collect edges sorted by weight (weakest first)
    let mut edges: Vec<(usize, usize, f64)> = Vec::new();
    for &node in &component {
        for &(neighbor, weight) in adj.get(&node).unwrap_or(&Vec::new()) {
            if node < neighbor {
                edges.push((node, neighbor, weight));
            }
        }
    }
    edges.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

    // Remove weakest edges until all components fit within max_size
    let mut removed: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    loop {
        let components = bfs_components(&component, &adj, &removed);
        if components.iter().all(|c| c.len() <= max_size) {
            return components;
        }
        if let Some(edge) = edges.first() {
            removed.insert((edge.0, edge.1));
            edges.remove(0);
        } else {
            break;
        }
    }

    bfs_components(&component, &adj, &removed)
}

/// BFS connected components within a set of nodes, excluding removed edges.
fn bfs_components(
    nodes: &[usize],
    adj: &HashMap<usize, Vec<(usize, f64)>>,
    removed: &std::collections::HashSet<(usize, usize)>,
) -> Vec<Vec<usize>> {
    let node_set: std::collections::HashSet<usize> = nodes.iter().copied().collect();
    let mut visited: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut components = Vec::new();

    for &start in nodes {
        if visited.contains(&start) {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start);
        visited.insert(start);

        while let Some(node) = queue.pop_front() {
            component.push(node);
            for &(neighbor, _) in adj.get(&node).unwrap_or(&Vec::new()) {
                if !visited.contains(&neighbor) && node_set.contains(&neighbor) {
                    let key = (node.min(neighbor), node.max(neighbor));
                    if !removed.contains(&key) {
                        visited.insert(neighbor);
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        component.sort();
        if !component.is_empty() {
            components.push(component);
        }
    }

    components.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
    components
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::SimilarityGraph;
    use crate::types::SimilaritySignals;

    fn make_score(a: usize, b: usize, overall: f64) -> SimilarityScore {
        SimilarityScore {
            func_a: a,
            func_b: b,
            overall,
            signals: SimilaritySignals {
                ast: overall,
                tokens: overall,
                calls: overall,
                control_flow: overall,
                complexity: 1.0,
                params: 1.0,
            },
        }
    }

    #[test]
    fn test_basic_clustering() {
        let scores = vec![make_score(0, 1, 0.90), make_score(1, 2, 0.85)];
        let graph = SimilarityGraph::build(3, &scores, 0.75);
        let functions: Vec<FunctionInfo> = (0..3)
            .map(|i| FunctionInfo {
                file: std::path::PathBuf::from(format!("test{i}.py")),
                module: format!("test{i}"),
                qualified_name: format!("func{i}"),
                class_name: None,
                function_name: format!("func{i}"),
                kind: crate::types::FunctionKind::Function,
                start_line: 1,
                end_line: 5,
                source_bytes: 100,
                ast_node_count: 10,
                complexity: 2,
                decorators: vec![],
                parameters: vec!["x".to_string()],
                return_annotation: None,
                called_functions: std::collections::BTreeSet::new(),
                is_public: true,
                is_dunder: false,
                is_test: false,
                is_property: false,
                is_classmethod: false,
                is_staticmethod: false,
                is_async: false,
            })
            .collect();
        let normalized: Vec<NormalizedFunction> = (0..3)
            .map(|i| NormalizedFunction {
                function_id: i,
                tokens: vec![],
            })
            .collect();
        let clusters = cluster_functions(&graph, &scores, &functions, &normalized);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].function_indices.len(), 3);
    }
}
