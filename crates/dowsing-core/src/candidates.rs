use crate::fingerprint::hamming_distance;
use crate::types::Fingerprints;
use std::collections::BTreeMap;

/// Exact duplicates are emitted in report-sized groups. This keeps a generated
/// file containing thousands of identical accessors from becoming quadratic.
const MAX_EXACT_GROUP_SIZE: usize = 20;
/// A bounded scoring set keeps large repositories on the LSH path.
const MAX_CANDIDATES_PER_LANGUAGE: usize = 25_000;
/// The exhaustive fallback is useful for small scans but is deliberately tiny.
const EXHAUSTIVE_SIMHASH_LIMIT: usize = 256;

/// Generate candidate pairs for detailed comparison using SimHash locality-sensitive bucketing.
///
/// Instead of comparing all N*(N-1)/2 pairs, we bucket functions by bands of their SimHash.
/// Only functions that share at least one band are compared.
///
/// The SimHash is split into `num_bands` bands of `bits_per_band` bits.
/// Two functions become candidates if any band matches exactly.
///
/// Additionally, functions with identical exact hashes are always candidates.
///
/// Returns a set of (func_a, func_b) pairs where func_a < func_b.
pub fn generate_candidates(
    fingerprints: &[Fingerprints],
    max_hamming_distance: u32,
) -> Vec<(usize, usize)> {
    let n = fingerprints.len();
    if n < 2 {
        return Vec::new();
    }

    let mut candidate_set = std::collections::BTreeSet::new();

    // Strategy 1: Exact hash grouping
    // Functions with identical normalized AST hash are definitely candidates
    let mut exact_groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, fp) in fingerprints.iter().enumerate() {
        exact_groups.entry(&fp.exact_hash).or_default().push(i);
    }
    for group in exact_groups.values() {
        for chunk in group.chunks(MAX_EXACT_GROUP_SIZE) {
            // A spanning tree preserves the exact-duplicate component while
            // avoiding O(chunk²) equivalent scores. All its edges are exact.
            for &member in &chunk[1..] {
                candidate_set.insert((chunk[0].min(member), chunk[0].max(member)));
            }
        }
    }

    // Strategy 2: SimHash band partitioning
    // Split the 64-bit SimHash into bands and bucket by each band
    let num_bands = 8;
    let bits_per_band = 8; // 8 bands × 8 bits = 64 bits

    for band_idx in 0..num_bands {
        let mut band_buckets: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
        let shift = band_idx * bits_per_band;

        for (i, fp) in fingerprints.iter().enumerate() {
            let band_value = ((fp.simhash >> shift) & 0xFF) as u8;
            band_buckets.entry(band_value).or_default().push(i);
        }

        for bucket in band_buckets.values() {
            if candidate_set.len() >= MAX_CANDIDATES_PER_LANGUAGE {
                break;
            }
            if bucket.len() >= 2 && bucket.len() <= 100 {
                // Cap bucket size to avoid O(N²) within a single bucket
                for i in 0..bucket.len() {
                    for j in (i + 1)..bucket.len() {
                        if candidate_set.len() >= MAX_CANDIDATES_PER_LANGUAGE {
                            break;
                        }
                        let idx_a = bucket[i].min(bucket[j]);
                        let idx_b = bucket[i].max(bucket[j]);
                        candidate_set.insert((idx_a, idx_b));
                    }
                }
            }
        }
    }

    // Strategy 3: For smaller sets, also check nearby SimHash by Hamming distance
    // This catches pairs that might fall in different bands but are still similar
    if n <= EXHAUSTIVE_SIMHASH_LIMIT && candidate_set.len() < MAX_CANDIDATES_PER_LANGUAGE {
        for i in 0..n {
            for j in (i + 1)..n {
                if candidate_set.len() >= MAX_CANDIDATES_PER_LANGUAGE {
                    break;
                }
                let dist = hamming_distance(fingerprints[i].simhash, fingerprints[j].simhash);
                if dist <= max_hamming_distance {
                    candidate_set.insert((i, j));
                }
            }
        }
    }

    candidate_set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Fingerprints;
    use std::collections::BTreeMap;

    fn make_fp(id: usize, exact_hash: &str, simhash: u64) -> Fingerprints {
        Fingerprints {
            function_id: id,
            exact_hash: exact_hash.to_string(),
            simhash,
            metadata_hash: 0,
            token_frequencies: BTreeMap::new(),
        }
    }

    #[test]
    fn test_exact_hash_candidates() {
        let fps = vec![
            make_fp(0, "aaa", 0),
            make_fp(1, "aaa", 100), // same exact hash, different simhash
            make_fp(2, "bbb", 200),
        ];
        let candidates = generate_candidates(&fps, 10);
        assert!(candidates.contains(&(0, 1)));
        // (0,2) and (1,2) should NOT be candidates (different hash, far simhash)
    }

    #[test]
    fn test_simhash_close_candidates() {
        let fps = vec![
            make_fp(0, "aaa", 0b1111_1111),
            make_fp(1, "bbb", 0b1111_1110), // 1 bit different
            make_fp(2, "ccc", 0xFF00_FF00_FF00_FF00), // very different
        ];
        let candidates = generate_candidates(&fps, 5);
        assert!(candidates.contains(&(0, 1)));
    }

    #[test]
    fn test_empty_input() {
        let candidates = generate_candidates(&[], 10);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_single_function() {
        let fps = vec![make_fp(0, "aaa", 0)];
        let candidates = generate_candidates(&fps, 10);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_large_exact_group_is_partitioned_without_quadratic_pairs() {
        let fingerprints = (0..1_000)
            .map(|id| make_fp(id, "same", 0))
            .collect::<Vec<_>>();
        let candidates = generate_candidates(&fingerprints, 0);
        assert_eq!(candidates.len(), 950);
        assert!(candidates.iter().all(|(a, b)| a / 20 == b / 20));
    }

    #[test]
    fn test_candidate_count_is_bounded() {
        let fingerprints = (0..2_000)
            .map(|id| make_fp(id, &format!("{id}"), 0))
            .collect::<Vec<_>>();
        assert!(generate_candidates(&fingerprints, 12).len() <= MAX_CANDIDATES_PER_LANGUAGE);
    }
}
