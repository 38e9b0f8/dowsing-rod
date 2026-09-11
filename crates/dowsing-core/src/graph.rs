use crate::types::{SimilarityEdge, SimilarityScore};

/// Similarity graph: functions as nodes, similarity scores as weighted edges.
///
/// Uses an adjacency list representation. All data stays local and lightweight —
/// no graph database needed.
pub struct SimilarityGraph {
    pub num_nodes: usize,
    pub edges: Vec<SimilarityEdge>,
    adjacency: Vec<Vec<(usize, f64)>>,
}

impl SimilarityGraph {
    /// Build a similarity graph from scored pairs, filtering by minimum similarity.
    pub fn build(num_functions: usize, scores: &[SimilarityScore], min_similarity: f64) -> Self {
        let mut edges = Vec::new();
        let mut adjacency = vec![Vec::new(); num_functions];

        for score in scores {
            if score.overall >= min_similarity {
                let a = score.func_a;
                let b = score.func_b;
                edges.push(SimilarityEdge {
                    func_a: a,
                    func_b: b,
                    weight: score.overall,
                });
                adjacency[a].push((b, score.overall));
                adjacency[b].push((a, score.overall));
            }
        }

        // Sort adjacency lists for determinism
        for adj in &mut adjacency {
            adj.sort_by_key(|a| a.0);
        }

        Self {
            num_nodes: num_functions,
            edges,
            adjacency,
        }
    }

    /// Get neighbors of a node.
    pub fn neighbors(&self, node: usize) -> &[(usize, f64)] {
        &self.adjacency[node]
    }

    /// Get the edge weight between two nodes (0.0 if not connected).
    pub fn edge_weight(&self, a: usize, b: usize) -> f64 {
        self.adjacency[a]
            .iter()
            .find(|(n, _)| *n == b)
            .map(|(_, w)| *w)
            .unwrap_or(0.0)
    }

    /// Find connected components using BFS.
    /// Returns a list of components, each being a sorted list of node indices.
    pub fn connected_components(&self) -> Vec<Vec<usize>> {
        let mut visited = vec![false; self.num_nodes];
        let mut components = Vec::new();

        for start in 0..self.num_nodes {
            if visited[start] || self.adjacency[start].is_empty() {
                visited[start] = true;
                continue;
            }

            let mut component = Vec::new();
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(start);
            visited[start] = true;

            while let Some(node) = queue.pop_front() {
                component.push(node);
                for &(neighbor, _) in &self.adjacency[node] {
                    if !visited[neighbor] {
                        visited[neighbor] = true;
                        queue.push_back(neighbor);
                    }
                }
            }

            component.sort();
            if component.len() >= 2 {
                components.push(component);
            }
        }

        // Sort components by size (largest first), then by first element for determinism
        components.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
        components
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_connected_components() {
        let scores = vec![
            make_score(0, 1, 0.90),
            make_score(1, 2, 0.85),
            make_score(3, 4, 0.92),
        ];
        let graph = SimilarityGraph::build(6, &scores, 0.80);
        let components = graph.connected_components();

        assert_eq!(components.len(), 2);
        assert_eq!(components[0], vec![0, 1, 2]); // larger component first
        assert_eq!(components[1], vec![3, 4]);
    }

    #[test]
    fn test_filtering_by_threshold() {
        let scores = vec![
            make_score(0, 1, 0.90),
            make_score(1, 2, 0.50), // below threshold
        ];
        let graph = SimilarityGraph::build(3, &scores, 0.75);
        let components = graph.connected_components();

        assert_eq!(components.len(), 1);
        assert_eq!(components[0], vec![0, 1]);
    }

    #[test]
    fn test_empty_graph() {
        let graph = SimilarityGraph::build(5, &[], 0.75);
        let components = graph.connected_components();
        assert!(components.is_empty());
    }
}
