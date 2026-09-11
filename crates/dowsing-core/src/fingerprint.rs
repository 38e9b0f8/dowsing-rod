use crate::types::{Fingerprints, FunctionInfo, NormalizedFunction, StructuralToken};
use siphasher::sip::SipHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

/// Generate all fingerprints for a normalized function.
pub fn generate_fingerprints(
    normalized: &NormalizedFunction,
    function_info: &FunctionInfo,
) -> Fingerprints {
    let exact_hash = compute_exact_hash(&normalized.tokens);
    let simhash = compute_simhash(&normalized.tokens);
    let metadata_hash = compute_metadata_hash(function_info);
    let token_frequencies = compute_token_frequencies(&normalized.tokens);

    Fingerprints {
        function_id: normalized.function_id,
        exact_hash,
        simhash,
        metadata_hash,
        token_frequencies,
    }
}

/// Compute a deterministic hash of the normalized token sequence.
/// Uses SipHash for stability across runs.
fn compute_exact_hash(tokens: &[StructuralToken]) -> String {
    let mut hasher = SipHasher::new_with_keys(0x_dead_beef_cafe_babe, 0x_1234_5678_9abc_def0);
    let serialized = format!("{:?}", tokens);
    serialized.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Compute a 64-bit SimHash from structural token bigrams.
///
/// SimHash is a locality-sensitive hash: similar inputs produce hashes
/// with small Hamming distance. This is used for cheap candidate generation.
///
/// Algorithm:
/// 1. Extract bigrams (pairs of consecutive tokens)
/// 2. Hash each bigram with SipHash
/// 3. For each bit position: sum +1 if bit is set, -1 if not
/// 4. Final hash: bit i is 1 if sum[i] > 0, else 0
pub fn compute_simhash(tokens: &[StructuralToken]) -> u64 {
    if tokens.is_empty() {
        return 0;
    }

    let mut v = [0i64; 64];

    // Use bigrams (pairs of consecutive tokens) as features
    let features: Vec<String> = if tokens.len() == 1 {
        vec![format!("{:?}", tokens[0])]
    } else {
        tokens
            .windows(2)
            .map(|w| format!("{:?}|{:?}", w[0], w[1]))
            .collect()
    };

    for feature in &features {
        let hash = siphash_str(feature);
        for (i, weight) in v.iter_mut().enumerate() {
            if (hash >> i) & 1 == 1 {
                *weight += 1;
            } else {
                *weight -= 1;
            }
        }
    }

    let mut simhash: u64 = 0;
    for (i, weight) in v.iter().enumerate() {
        if *weight > 0 {
            simhash |= 1 << i;
        }
    }
    simhash
}

/// Compute Hamming distance between two SimHashes.
/// Lower distance = more similar.
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Compute metadata hash (param count, complexity bucket, decorators).
fn compute_metadata_hash(info: &FunctionInfo) -> u64 {
    let mut hasher = SipHasher::new_with_keys(0x6d65_7461_5f68_0001, 0x6d65_7461_5f68_0002);
    info.parameters.len().hash(&mut hasher);
    // Complexity bucket (1-3=low, 4-7=medium, 8+=high)
    let complexity_bucket = match info.complexity {
        0..=3 => "low",
        4..=7 => "medium",
        _ => "high",
    };
    complexity_bucket.hash(&mut hasher);
    info.is_async.hash(&mut hasher);
    // Sort decorators for determinism
    let mut decorators = info.decorators.clone();
    decorators.sort();
    for d in &decorators {
        d.hash(&mut hasher);
    }
    hasher.finish()
}

/// Compute frequency map of token types (for Jaccard similarity).
fn compute_token_frequencies(tokens: &[StructuralToken]) -> BTreeMap<String, usize> {
    let mut freq = BTreeMap::new();
    for token in tokens {
        let key = token_type_key(token);
        *freq.entry(key).or_insert(0) += 1;
    }
    freq
}

/// Map a structural token to its type key (for frequency counting).
fn token_type_key(token: &StructuralToken) -> String {
    match token {
        StructuralToken::Call(name) => format!("call:{name}"),
        StructuralToken::Attribute(name) => format!("attr:{name}"),
        StructuralToken::BinOp(op) => format!("binop:{op}"),
        StructuralToken::UnaryOp(op) => format!("unop:{op}"),
        StructuralToken::BoolOp(op) => format!("boolop:{op}"),
        StructuralToken::Compare(op) => format!("cmp:{op}"),
        StructuralToken::ExternalName(name) => format!("ext:{name}"),
        StructuralToken::Import(name) => format!("import:{name}"),
        StructuralToken::Decorator(name) => format!("dec:{name}"),
        StructuralToken::AugAssign(op) => format!("augassign:{op}"),
        StructuralToken::BoolLiteral(b) => format!("bool:{b}"),
        // For all other tokens, use the variant name
        _ => format!("{:?}", token).to_lowercase(),
    }
}

/// Deterministic SipHash of a string.
fn siphash_str(s: &str) -> u64 {
    let mut hasher = SipHasher::new_with_keys(0x_0123_4567_89ab_cdef, 0x_fedc_ba98_7654_3210);
    s.hash(&mut hasher);
    hasher.finish()
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

    fn fingerprint_source(source: &str) -> Fingerprints {
        let path = PathBuf::from("test.py");
        let result = parse_python_source(source, &path).unwrap();
        let body = result.module.unwrap();
        let funcs = extract_functions(&body, source, &path);
        assert!(!funcs.is_empty());

        if let Stmt::FunctionDef(f) = &body[0] {
            let params: Vec<String> = f.args.args.iter().map(|a| a.def.arg.to_string()).collect();
            let norm = normalize_function(
                &f.body,
                &params,
                &f.name,
                0,
                NormalizationLevel::Balanced,
                None,
            );
            generate_fingerprints(&norm, &funcs[0])
        } else {
            panic!("expected function def");
        }
    }

    #[test]
    fn test_exact_hash_deterministic() {
        let source = "def foo(x):\n    return x + 1\n";
        let fp1 = fingerprint_source(source);
        let fp2 = fingerprint_source(source);
        assert_eq!(fp1.exact_hash, fp2.exact_hash);
    }

    #[test]
    fn test_renamed_vars_same_hash() {
        let fp_a = fingerprint_source("def foo(x):\n    y = x + 1\n    return y\n");
        let fp_b = fingerprint_source("def bar(val):\n    result = val + 1\n    return result\n");
        assert_eq!(fp_a.exact_hash, fp_b.exact_hash);
    }

    #[test]
    fn test_different_ops_different_hash() {
        let fp_a = fingerprint_source("def foo(x):\n    return x + 1\n");
        let fp_b = fingerprint_source("def foo(x):\n    return x - 1\n");
        assert_ne!(fp_a.exact_hash, fp_b.exact_hash);
    }

    #[test]
    fn test_simhash_similar_functions_close() {
        let fp_a = fingerprint_source("def foo(x):\n    y = x + 1\n    return y\n");
        let fp_b = fingerprint_source("def bar(val):\n    result = val + 1\n    return result\n");
        let distance = hamming_distance(fp_a.simhash, fp_b.simhash);
        assert!(
            distance <= 5,
            "similar functions should have small Hamming distance, got {distance}"
        );
    }

    #[test]
    fn test_simhash_different_functions_far() {
        let fp_a = fingerprint_source("def foo(x):\n    return x + 1\n");
        let fp_b = fingerprint_source(
            "def complex(data):\n    for item in data:\n        if item > 0:\n            print(item)\n    return len(data)\n",
        );
        let distance = hamming_distance(fp_a.simhash, fp_b.simhash);
        // Very different functions should have larger distance
        assert!(
            distance > 5,
            "very different functions should have large Hamming distance, got {distance}"
        );
    }
}
