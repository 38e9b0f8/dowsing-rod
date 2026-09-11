/// Token economics for the --ai output feature.
///
/// IMPORTANT: Token estimation in this module is APPROXIMATE.
/// We use the heuristic of ~4 characters per token, which is a reasonable
/// approximation for English-language source code and ASCII output.
/// Actual token counts depend on the specific tokenizer used by the target model.
/// For exact counts, integrate a model-specific tokenizer (e.g., tiktoken for GPT models).
use crate::types::{Cluster, FunctionInfo};

/// Approximate characters-per-token ratio for source code.
const CHARS_PER_TOKEN: usize = 4;

/// Estimate the token count of a string (approximate).
pub fn estimate_tokens(s: &str) -> usize {
    s.len().div_ceil(CHARS_PER_TOKEN)
}

/// Estimate total source tokens in the repository.
pub fn estimate_repository_tokens(functions: &[FunctionInfo]) -> usize {
    functions.iter().map(|f| f.estimated_tokens()).sum()
}

/// Estimate how many tokens a cluster's AI output will use.
pub fn estimate_cluster_ai_tokens(
    cluster: &Cluster,
    functions: &[FunctionInfo],
    base_path: &std::path::Path,
) -> usize {
    let mut s = String::new();

    // Header line
    s.push_str(&format!(
        "{} | {} | signal={:.0}%\n",
        cluster.id,
        cluster.classification,
        cluster.average_similarity * 100.0
    ));
    s.push_str(&format!(
        "{} functions | duplicated≈{} tokens\n",
        cluster.function_indices.len(),
        cluster.duplicated_tokens_estimate
    ));

    // Function locations
    for &idx in &cluster.function_indices {
        if let Some(f) = functions.get(idx) {
            s.push_str(&format!("{}\n", f.display_location(base_path)));
        }
    }

    // Common structure
    if !cluster.common_structure.is_empty() {
        s.push_str("COMMON\n");
        s.push_str(&cluster.common_structure.join(" → "));
        s.push('\n');
    }

    // Differences
    if !cluster.differences.is_empty() {
        s.push_str("DIFF\n");
        for d in &cluster.differences {
            s.push_str(&format!("{d}\n"));
        }
    }

    // Signals
    s.push_str(&format!(
        "SIGNALS\nast={:.2} tokens={:.2} calls={:.2} control={:.2}\n",
        cluster.signals.ast,
        cluster.signals.tokens,
        cluster.signals.calls,
        cluster.signals.control_flow,
    ));

    // Reason
    s.push_str(&format!("WHY\n{}\n", cluster.reason));

    estimate_tokens(&s)
}

/// Select the top clusters that fit within a token budget.
///
/// If max_tokens is None, returns all clusters.
/// Selections are greedy (most valuable first, stop when budget exhausted).
pub fn select_within_budget(
    clusters: &[Cluster],
    functions: &[FunctionInfo],
    base_path: &std::path::Path,
    max_tokens: Option<usize>,
) -> Vec<usize> {
    let max = match max_tokens {
        None => return (0..clusters.len()).collect(),
        Some(m) => m,
    };

    // Reserve ~200 tokens for the header
    let header_tokens = 200;
    let available = max.saturating_sub(header_tokens);

    let mut selected = Vec::new();
    let mut used = 0usize;

    for (i, cluster) in clusters.iter().enumerate() {
        let tokens = estimate_cluster_ai_tokens(cluster, functions, base_path);
        if used + tokens <= available {
            selected.push(i);
            used += tokens;
        } else if selected.is_empty() {
            // Always include at least one cluster even if it exceeds budget
            selected.push(i);
            break;
        } else {
            break;
        }
    }

    selected
}
