use crate::types::{
    FunctionInfo, NormalizedFunction, SimilarityScore, SimilaritySignals, StructuralToken,
};
use std::collections::BTreeSet;

/// Similarity weight model (documented, tunable).
///
/// Weights sum to 1.0:
///   ast_structure:  0.30 — overall structural shape is the strongest signal
///   token_sim:      0.25 — token-level similarity captures detailed operations
///   call_sim:       0.20 — which functions are called matters semantically
///   control_flow:   0.15 — control flow shape is a strong structural signal
///   complexity:     0.05 — similar complexity is a weak supporting signal
///   param_shape:    0.05 — parameter count similarity is a weak supporting signal
const W_AST: f64 = 0.30;
const W_TOKENS: f64 = 0.25;
const W_CALLS: f64 = 0.20;
const W_CONTROL: f64 = 0.15;
const W_COMPLEXITY: f64 = 0.05;
const W_PARAMS: f64 = 0.05;

/// Compute detailed multi-signal similarity between two functions.
pub fn compute_similarity(
    norm_a: &NormalizedFunction,
    norm_b: &NormalizedFunction,
    info_a: &FunctionInfo,
    info_b: &FunctionInfo,
) -> SimilarityScore {
    let ast_sim = ast_structure_similarity(&norm_a.tokens, &norm_b.tokens);
    let token_sim = token_jaccard_similarity(&norm_a.tokens, &norm_b.tokens);
    let call_sim = call_set_similarity(&info_a.called_functions, &info_b.called_functions);
    let control_sim = control_flow_similarity(&norm_a.tokens, &norm_b.tokens);
    let complexity_sim = complexity_similarity(info_a.complexity, info_b.complexity);
    let param_sim = parameter_similarity(info_a, info_b);

    let overall = W_AST * ast_sim
        + W_TOKENS * token_sim
        + W_CALLS * call_sim
        + W_CONTROL * control_sim
        + W_COMPLEXITY * complexity_sim
        + W_PARAMS * param_sim;

    SimilarityScore {
        func_a: norm_a.function_id,
        func_b: norm_b.function_id,
        overall,
        signals: SimilaritySignals {
            ast: ast_sim,
            tokens: token_sim,
            calls: call_sim,
            control_flow: control_sim,
            complexity: complexity_sim,
            params: param_sim,
        },
    }
}

/// AST structure similarity using longest common subsequence ratio.
///
/// Compares the full normalized token sequences.
/// Returns LCS_length / max(len_a, len_b).
fn ast_structure_similarity(tokens_a: &[StructuralToken], tokens_b: &[StructuralToken]) -> f64 {
    if tokens_a.is_empty() && tokens_b.is_empty() {
        return 1.0;
    }
    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }

    let lcs_len = lcs_length(tokens_a, tokens_b);
    let max_len = tokens_a.len().max(tokens_b.len());
    lcs_len as f64 / max_len as f64
}

/// Token-level Jaccard similarity (multiset intersection / union).
fn token_jaccard_similarity(tokens_a: &[StructuralToken], tokens_b: &[StructuralToken]) -> f64 {
    if tokens_a.is_empty() && tokens_b.is_empty() {
        return 1.0;
    }
    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }

    let mut freq_a = std::collections::HashMap::new();
    let mut freq_b = std::collections::HashMap::new();

    for t in tokens_a {
        *freq_a.entry(t).or_insert(0usize) += 1;
    }
    for t in tokens_b {
        *freq_b.entry(t).or_insert(0usize) += 1;
    }

    let all_keys: BTreeSet<_> = freq_a.keys().chain(freq_b.keys()).cloned().collect();

    let mut intersection = 0usize;
    let mut union = 0usize;

    for key in &all_keys {
        let a = freq_a.get(key).copied().unwrap_or(0);
        let b = freq_b.get(key).copied().unwrap_or(0);
        intersection += a.min(b);
        union += a.max(b);
    }

    if union == 0 {
        return 1.0;
    }
    intersection as f64 / union as f64
}

/// Call-set Jaccard similarity.
fn call_set_similarity(calls_a: &BTreeSet<String>, calls_b: &BTreeSet<String>) -> f64 {
    if calls_a.is_empty() && calls_b.is_empty() {
        return 1.0;
    }
    if calls_a.is_empty() || calls_b.is_empty() {
        return 0.0;
    }

    let intersection = calls_a.intersection(calls_b).count();
    let union = calls_a.union(calls_b).count();

    if union == 0 {
        return 1.0;
    }
    intersection as f64 / union as f64
}

/// Control flow similarity: compare only control flow tokens.
fn control_flow_similarity(tokens_a: &[StructuralToken], tokens_b: &[StructuralToken]) -> f64 {
    let cf_a: Vec<&StructuralToken> = tokens_a.iter().filter(|t| t.is_control_flow()).collect();
    let cf_b: Vec<&StructuralToken> = tokens_b.iter().filter(|t| t.is_control_flow()).collect();

    if cf_a.is_empty() && cf_b.is_empty() {
        return 1.0;
    }
    if cf_a.is_empty() || cf_b.is_empty() {
        return 0.0;
    }

    // Use sequence similarity for control flow
    let lcs = lcs_length_ref(&cf_a, &cf_b);
    let max_len = cf_a.len().max(cf_b.len());
    lcs as f64 / max_len as f64
}

/// Complexity similarity: ratio of smaller to larger.
fn complexity_similarity(c_a: usize, c_b: usize) -> f64 {
    if c_a == 0 && c_b == 0 {
        return 1.0;
    }
    let min = c_a.min(c_b) as f64;
    let max = c_a.max(c_b) as f64;
    if max == 0.0 {
        return 1.0;
    }
    min / max
}

/// Parameter similarity: considers count and async status.
fn parameter_similarity(info_a: &FunctionInfo, info_b: &FunctionInfo) -> f64 {
    let count_a = info_a.parameters.len();
    let count_b = info_b.parameters.len();

    // Count similarity
    let count_sim = if count_a == 0 && count_b == 0 {
        1.0
    } else {
        let min = count_a.min(count_b) as f64;
        let max = count_a.max(count_b) as f64;
        min / max
    };

    // Async compatibility
    let async_sim = if info_a.is_async == info_b.is_async {
        1.0
    } else {
        0.5
    };

    count_sim * 0.7 + async_sim * 0.3
}

/// Longest common subsequence length (O(n*m) dynamic programming).
/// For very long sequences, we cap at 500 tokens to avoid excessive computation.
fn lcs_length(a: &[StructuralToken], b: &[StructuralToken]) -> usize {
    let max_len = 500;
    let a = if a.len() > max_len { &a[..max_len] } else { a };
    let b = if b.len() > max_len { &b[..max_len] } else { b };

    let m = a.len();
    let n = b.len();
    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                curr[j] = prev[j - 1] + 1;
            } else {
                curr[j] = prev[j].max(curr[j - 1]);
            }
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.fill(0);
    }

    prev[n]
}

/// LCS length for reference slices.
fn lcs_length_ref<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    let m = a.len().min(200);
    let n = b.len().min(200);
    let a = &a[..m];
    let b = &b[..n];

    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                curr[j] = prev[j - 1] + 1;
            } else {
                curr[j] = prev[j].max(curr[j - 1]);
            }
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.fill(0);
    }

    prev[n]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extraction::extract_functions;
    use crate::normalization::normalize_function;
    use crate::parser::parse_python_source;
    use crate::types::NormalizationLevel;
    use rustpython_parser::ast::Stmt;
    use std::path::PathBuf;

    fn sim_from_sources(source_a: &str, source_b: &str) -> SimilarityScore {
        let path = PathBuf::from("test.py");

        let result_a = parse_python_source(source_a, &path).unwrap();
        let body_a = result_a.module.unwrap();
        let funcs_a = extract_functions(&body_a, source_a, &path);

        let result_b = parse_python_source(source_b, &path).unwrap();
        let body_b = result_b.module.unwrap();
        let funcs_b = extract_functions(&body_b, source_b, &path);

        let norm_a = if let Stmt::FunctionDef(f) = &body_a[0] {
            let params: Vec<String> = f.args.args.iter().map(|a| a.def.arg.to_string()).collect();
            normalize_function(
                &f.body,
                &params,
                &f.name,
                0,
                NormalizationLevel::Balanced,
                None,
            )
        } else {
            panic!("expected function");
        };

        let norm_b = if let Stmt::FunctionDef(f) = &body_b[0] {
            let params: Vec<String> = f.args.args.iter().map(|a| a.def.arg.to_string()).collect();
            normalize_function(
                &f.body,
                &params,
                &f.name,
                1,
                NormalizationLevel::Balanced,
                None,
            )
        } else {
            panic!("expected function");
        };

        compute_similarity(&norm_a, &norm_b, &funcs_a[0], &funcs_b[0])
    }

    #[test]
    fn test_identical_functions_high_similarity() {
        let source = "def foo(x):\n    y = x + 1\n    return y\n";
        let score = sim_from_sources(source, source);
        assert!(
            score.overall > 0.95,
            "identical functions should have ~1.0 similarity, got {}",
            score.overall
        );
    }

    #[test]
    fn test_renamed_vars_high_similarity() {
        let a = "def foo(x):\n    y = x + 1\n    return y\n";
        let b = "def bar(val):\n    result = val + 1\n    return result\n";
        let score = sim_from_sources(a, b);
        assert!(
            score.overall > 0.90,
            "renamed vars should be highly similar, got {}",
            score.overall
        );
    }

    #[test]
    fn test_save_vs_delete_different() {
        let a = "def save_user(user):\n    db.save(user)\n";
        let b = "def delete_user(user):\n    db.delete(user)\n";
        let score = sim_from_sources(a, b);
        // The call similarity should be low since save != delete
        assert!(
            score.signals.calls < 0.5,
            "save vs delete should have low call similarity, got {}",
            score.signals.calls
        );
        // Overall should be moderate (structure is similar but semantics differ)
        assert!(
            score.overall < 0.85,
            "save vs delete should NOT be highly similar, got {}",
            score.overall
        );
    }

    #[test]
    fn test_completely_different_functions() {
        let a = "def foo(x):\n    return x + 1\n";
        let b = "def complex(data):\n    results = []\n    for item in data:\n        if item > 0:\n            results.append(process(item))\n    return results\n";
        let score = sim_from_sources(a, b);
        assert!(
            score.overall < 0.5,
            "completely different functions should have low similarity, got {}",
            score.overall
        );
    }

    #[test]
    fn test_strategy_pattern_high_similarity() {
        let a = "def charge_stripe(amount, currency):\n    validated = validate(amount)\n    request = build_request(validated, currency)\n    response = stripe_api.execute(request)\n    result = parse_response(response)\n    return normalize(result)\n";
        let b = "def charge_paypal(amount, currency):\n    validated = validate(amount)\n    request = build_request(validated, currency)\n    response = paypal_api.execute(request)\n    result = parse_response(response)\n    return normalize(result)\n";
        let score = sim_from_sources(a, b);
        assert!(
            score.overall > 0.75,
            "strategy pattern candidates should be similar, got {}",
            score.overall
        );
        assert!(
            score.signals.control_flow > 0.8,
            "control flow should be very similar"
        );
    }
}
