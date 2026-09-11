use crate::types::{NormalizedFunction, StructuralDiff, StructuralToken};

/// Extract structural differences between two similar functions.
///
/// Identifies common structural elements (shared pipeline steps) and
/// differences (provider-specific implementations).
///
/// This is derived purely from the normalized token sequences — no LLM.
pub fn extract_differences(
    norm_a: &NormalizedFunction,
    norm_b: &NormalizedFunction,
) -> StructuralDiff {
    let common = find_common_elements(&norm_a.tokens, &norm_b.tokens);
    let diff = find_different_elements(&norm_a.tokens, &norm_b.tokens);

    StructuralDiff {
        common_elements: common,
        differences: diff,
    }
}

/// Find the common structural elements between two token sequences.
/// Returns human-readable descriptions of shared pipeline steps.
fn find_common_elements(tokens_a: &[StructuralToken], tokens_b: &[StructuralToken]) -> Vec<String> {
    let mut common = Vec::new();
    let lcs = compute_lcs(tokens_a, tokens_b);

    // Group consecutive LCS tokens into meaningful operations
    let mut current_group: Vec<&StructuralToken> = Vec::new();

    for token in &lcs {
        if is_boundary_token(token) && !current_group.is_empty() {
            if let Some(desc) = describe_group(&current_group) {
                if !common.contains(&desc) {
                    common.push(desc);
                }
            }
            current_group.clear();
        }
        current_group.push(token);
    }

    if !current_group.is_empty() {
        if let Some(desc) = describe_group(&current_group) {
            if !common.contains(&desc) {
                common.push(desc);
            }
        }
    }

    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    common.retain(|item| seen.insert(item.clone()));

    common
}

/// Find elements that differ between two token sequences.
fn find_different_elements(
    tokens_a: &[StructuralToken],
    tokens_b: &[StructuralToken],
) -> Vec<String> {
    let mut diffs = Vec::new();

    // Find calls unique to each
    let calls_a: Vec<&str> = tokens_a
        .iter()
        .filter_map(|t| {
            if let StructuralToken::Call(name) = t {
                Some(name.as_str())
            } else {
                None
            }
        })
        .collect();

    let calls_b: Vec<&str> = tokens_b
        .iter()
        .filter_map(|t| {
            if let StructuralToken::Call(name) = t {
                Some(name.as_str())
            } else {
                None
            }
        })
        .collect();

    let unique_a: Vec<&&str> = calls_a.iter().filter(|c| !calls_b.contains(c)).collect();
    let unique_b: Vec<&&str> = calls_b.iter().filter(|c| !calls_a.contains(c)).collect();

    if !unique_a.is_empty() || !unique_b.is_empty() {
        diffs.push("different function calls".to_string());
    }

    // Find different attributes
    let attrs_a: Vec<&str> = tokens_a
        .iter()
        .filter_map(|t| {
            if let StructuralToken::Attribute(name) = t {
                Some(name.as_str())
            } else {
                None
            }
        })
        .collect();

    let attrs_b: Vec<&str> = tokens_b
        .iter()
        .filter_map(|t| {
            if let StructuralToken::Attribute(name) = t {
                Some(name.as_str())
            } else {
                None
            }
        })
        .collect();

    let unique_attrs_a: Vec<&&str> = attrs_a.iter().filter(|a| !attrs_b.contains(a)).collect();
    let unique_attrs_b: Vec<&&str> = attrs_b.iter().filter(|a| !attrs_a.contains(a)).collect();

    if !unique_attrs_a.is_empty() || !unique_attrs_b.is_empty() {
        diffs.push("different attribute access".to_string());
    }

    // Find different operators
    let ops_a: Vec<String> = tokens_a
        .iter()
        .filter_map(|t| match t {
            StructuralToken::BinOp(op) => Some(op.clone()),
            _ => None,
        })
        .collect();

    let ops_b: Vec<String> = tokens_b
        .iter()
        .filter_map(|t| match t {
            StructuralToken::BinOp(op) => Some(op.clone()),
            _ => None,
        })
        .collect();

    if ops_a != ops_b {
        diffs.push("different operators".to_string());
    }

    // Find different external names
    let ext_a: Vec<&str> = tokens_a
        .iter()
        .filter_map(|t| {
            if let StructuralToken::ExternalName(name) = t {
                Some(name.as_str())
            } else {
                None
            }
        })
        .collect();

    let ext_b: Vec<&str> = tokens_b
        .iter()
        .filter_map(|t| {
            if let StructuralToken::ExternalName(name) = t {
                Some(name.as_str())
            } else {
                None
            }
        })
        .collect();

    let unique_ext: Vec<&&str> = ext_a.iter().filter(|e| !ext_b.contains(e)).collect();
    if !unique_ext.is_empty() {
        diffs.push("different external references".to_string());
    }

    // Deduplicate
    let mut seen = std::collections::HashSet::new();
    diffs.retain(|d| seen.insert(d.clone()));

    diffs
}

/// Compute the LCS (longest common subsequence) of two token sequences.
fn compute_lcs(a: &[StructuralToken], b: &[StructuralToken]) -> Vec<StructuralToken> {
    let max_len = 300;
    let a = if a.len() > max_len { &a[..max_len] } else { a };
    let b = if b.len() > max_len { &b[..max_len] } else { b };

    let m = a.len();
    let n = b.len();

    // Build DP table
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to find the actual LCS
    let mut lcs = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            lcs.push(a[i - 1].clone());
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] > dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    lcs.reverse();
    lcs
}

/// Whether a token represents a boundary between logical operations.
fn is_boundary_token(token: &StructuralToken) -> bool {
    matches!(
        token,
        StructuralToken::Assign
            | StructuralToken::Return
            | StructuralToken::If
            | StructuralToken::For
            | StructuralToken::While
            | StructuralToken::Try
            | StructuralToken::Raise
            | StructuralToken::BlockStart
            | StructuralToken::BlockEnd
    )
}

/// Describe a group of tokens as a human-readable operation.
fn describe_group(tokens: &[&StructuralToken]) -> Option<String> {
    if tokens.is_empty() {
        return None;
    }

    // Look for the most significant token in the group
    for token in tokens {
        match token {
            StructuralToken::Call(name) => return Some(format!("call {name}")),
            StructuralToken::Return => return Some("return".to_string()),
            StructuralToken::If => return Some("conditional check".to_string()),
            StructuralToken::For | StructuralToken::While => return Some("iteration".to_string()),
            StructuralToken::Try => return Some("error handling".to_string()),
            StructuralToken::Raise => return Some("raise exception".to_string()),
            StructuralToken::Assign => return Some("assignment".to_string()),
            StructuralToken::With => return Some("context manager".to_string()),
            StructuralToken::Assert => return Some("assertion".to_string()),
            StructuralToken::Await => return Some("await".to_string()),
            _ => continue,
        }
    }

    None
}

/// Extract common call pipeline from a cluster's functions.
/// Returns a list like: ["validate", "construct", "execute", "parse", "normalize"]
pub fn extract_common_pipeline(all_tokens: &[&[StructuralToken]]) -> Vec<String> {
    if all_tokens.is_empty() {
        return Vec::new();
    }

    // Extract call sequences from each function
    let call_sequences: Vec<Vec<&str>> = all_tokens
        .iter()
        .map(|tokens| {
            tokens
                .iter()
                .filter_map(|t| {
                    if let StructuralToken::Call(name) = t {
                        Some(name.as_str())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();

    if call_sequences.is_empty() || call_sequences[0].is_empty() {
        return Vec::new();
    }

    // Find calls common to all functions (by frequency: present in all)
    let first = &call_sequences[0];
    let common: Vec<String> = first
        .iter()
        .filter(|call| call_sequences.iter().all(|seq| seq.contains(call)))
        .map(|s| s.to_string())
        .collect();

    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    common
        .into_iter()
        .filter(|c| seen.insert(c.clone()))
        .collect()
}
