use crate::types::Fingerprints;
use rayon::prelude::*;
use std::collections::BTreeMap;

/// Exact duplicates are emitted in report-sized groups. This keeps a generated
/// file containing thousands of identical accessors from becoming quadratic.
const MAX_EXACT_GROUP_SIZE: usize = 20;
/// Up to this many pairs, every pair is scored: small scans stay exhaustive.
const EXHAUSTIVE_PAIR_LIMIT: usize = 50_000;
/// On larger scans, each function is scored against at most this many of its
/// nearest SimHash neighbours. A per-function budget (instead of one global
/// cap) keeps dense groups of near-identical small functions from using up
/// the comparisons that larger duplicated functions need.
const NEIGHBOURS_PER_FUNCTION: usize = 6;
/// Similar functions differ in size by at most ~1.7x in practice, so pairs
/// further apart than this are never similar and are not considered.
const MAX_SIZE_RATIO: usize = 2;

/// Generate candidate pairs for detailed similarity scoring.
///
/// * Functions with fewer than `min_tokens` structural tokens are skipped
///   (see [`crate::language::Language::min_function_tokens`]).
/// * Functions with identical normalized hashes are always paired (as a
///   spanning tree per group of [`MAX_EXACT_GROUP_SIZE`]).
/// * Small scans pair everything else exhaustively.
/// * Larger scans pair each function with its [`NEIGHBOURS_PER_FUNCTION`]
///   nearest neighbours by SimHash Hamming distance (at most
///   `max_hamming_distance`) among functions of comparable size. The work is
///   linear in the number of functions per size band, and the number of pairs
///   is at most `NEIGHBOURS_PER_FUNCTION` per function, so nothing is dropped
///   silently by a global cap.
///
/// Returns sorted, unique (func_a, func_b) pairs where func_a < func_b.
pub fn generate_candidates(
    fingerprints: &[Fingerprints],
    max_hamming_distance: u32,
    min_tokens: usize,
) -> Vec<(usize, usize)> {
    let mut eligible: Vec<usize> = (0..fingerprints.len())
        .filter(|&i| token_count(&fingerprints[i]) >= min_tokens)
        .collect();
    if eligible.len() < 2 {
        return Vec::new();
    }

    let mut candidates = Vec::new();

    // Exact normalized duplicates.
    let mut exact_groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for &i in &eligible {
        exact_groups
            .entry(&fingerprints[i].exact_hash)
            .or_default()
            .push(i);
    }
    for group in exact_groups.values() {
        for chunk in group.chunks(MAX_EXACT_GROUP_SIZE) {
            // A spanning tree preserves the exact-duplicate component while
            // avoiding O(chunk²) equivalent scores. All its edges are exact.
            for &member in &chunk[1..] {
                candidates.push((chunk[0].min(member), chunk[0].max(member)));
            }
        }
    }

    let n = eligible.len();
    let distinct_pair =
        |a: usize, b: usize| fingerprints[a].exact_hash != fingerprints[b].exact_hash;
    if n * (n - 1) / 2 <= EXHAUSTIVE_PAIR_LIMIT {
        for (x, &a) in eligible.iter().enumerate() {
            for &b in &eligible[x + 1..] {
                if distinct_pair(a, b) {
                    candidates.push((a.min(b), a.max(b)));
                }
            }
        }
    } else {
        // Sorted by size, the comparable functions of each one form a window.
        let size = |i: usize| token_count(&fingerprints[i]);
        eligible.sort_by_key(|&i| (size(i), i));
        let sizes: Vec<usize> = eligible.iter().map(|&i| size(i)).collect();
        let hashes: Vec<u64> = eligible.iter().map(|&i| fingerprints[i].simhash).collect();
        let neighbours: Vec<Vec<(usize, usize)>> = eligible
            .par_iter()
            .enumerate()
            .map(|(x, &a)| {
                let lo = sizes.partition_point(|&s| s * MAX_SIZE_RATIO < sizes[x]);
                let hi = sizes.partition_point(|&s| s <= sizes[x] * MAX_SIZE_RATIO);
                // (distance, size gap, function), kept sorted; the last entry is the worst.
                let mut best: Vec<(u32, usize, usize)> =
                    Vec::with_capacity(NEIGHBOURS_PER_FUNCTION + 1);
                let mut limit = max_hamming_distance;
                for y in lo..hi {
                    let distance = (hashes[x] ^ hashes[y]).count_ones();
                    if distance > limit || y == x {
                        continue;
                    }
                    let b = eligible[y];
                    if !distinct_pair(a, b) {
                        continue;
                    }
                    let key = (distance, sizes[x].abs_diff(sizes[y]), b);
                    if best.len() == NEIGHBOURS_PER_FUNCTION {
                        if key >= best[NEIGHBOURS_PER_FUNCTION - 1] {
                            continue;
                        }
                        best.pop();
                    }
                    let at = best.partition_point(|k| *k < key);
                    best.insert(at, key);
                    if best.len() == NEIGHBOURS_PER_FUNCTION {
                        limit = best[NEIGHBOURS_PER_FUNCTION - 1].0;
                    }
                }
                best.into_iter()
                    .map(|(_, _, b)| (a.min(b), a.max(b)))
                    .collect()
            })
            .collect();
        candidates.extend(neighbours.into_iter().flatten());
    }

    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

/// Number of structural tokens of a function.
fn token_count(fp: &Fingerprints) -> usize {
    fp.token_frequencies.values().sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Fingerprints;
    use std::collections::BTreeMap;

    const MIN: usize = 30;

    fn make_sized_fp(id: usize, exact_hash: &str, simhash: u64, tokens: usize) -> Fingerprints {
        Fingerprints {
            function_id: id,
            exact_hash: exact_hash.to_string(),
            simhash,
            metadata_hash: 0,
            token_frequencies: BTreeMap::from([("call:f".to_string(), tokens)]),
        }
    }

    fn make_fp(id: usize, exact_hash: &str, simhash: u64) -> Fingerprints {
        make_sized_fp(id, exact_hash, simhash, 50)
    }

    #[test]
    fn test_exact_hash_candidates() {
        let fps = vec![
            make_fp(0, "aaa", 0),
            make_fp(1, "aaa", 100), // same exact hash, different simhash
            make_fp(2, "bbb", 200),
        ];
        let candidates = generate_candidates(&fps, 10, MIN);
        assert!(candidates.contains(&(0, 1)));
    }

    #[test]
    fn test_simhash_close_candidates() {
        let fps = vec![
            make_fp(0, "aaa", 0b1111_1111),
            make_fp(1, "bbb", 0b1111_1110), // 1 bit different
            make_fp(2, "ccc", 0xFF00_FF00_FF00_FF00), // very different
        ];
        let candidates = generate_candidates(&fps, 5, MIN);
        assert!(candidates.contains(&(0, 1)));
    }

    #[test]
    fn test_empty_input() {
        let candidates = generate_candidates(&[], 10, MIN);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_single_function() {
        let fps = vec![make_fp(0, "aaa", 0)];
        let candidates = generate_candidates(&fps, 10, MIN);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_tiny_functions_are_not_compared() {
        let fps = vec![
            make_sized_fp(0, "aaa", 0, MIN - 1),
            make_sized_fp(1, "aaa", 0, MIN - 1),
            make_sized_fp(2, "bbb", 0, MIN),
            make_sized_fp(3, "ccc", 0, MIN),
        ];
        assert_eq!(generate_candidates(&fps, 10, MIN), vec![(2, 3)]);
    }

    #[test]
    fn test_large_exact_group_is_partitioned_without_quadratic_pairs() {
        let fingerprints = (0..1_000)
            .map(|id| make_fp(id, "same", 0))
            .collect::<Vec<_>>();
        let candidates = generate_candidates(&fingerprints, 0, MIN);
        assert_eq!(candidates.len(), 950);
        assert!(candidates.iter().all(|(a, b)| a / 20 == b / 20));
    }

    #[test]
    fn test_candidates_per_function_are_bounded() {
        let fingerprints = (0..2_000)
            .map(|id| make_fp(id, &format!("{id}"), 0))
            .collect::<Vec<_>>();
        let candidates = generate_candidates(&fingerprints, 12, MIN);
        assert!(candidates.len() <= 2_000 * NEIGHBOURS_PER_FUNCTION);
        let mut degree = vec![0usize; 2_000];
        for (a, b) in &candidates {
            degree[*a] += 1;
            degree[*b] += 1;
        }
        assert!(degree.iter().all(|&d| d >= 1));
    }

    /// The regression behind this design: a large set of small, mutually
    /// similar functions used to exhaust a global candidate cap, so larger
    /// duplicated functions were never compared at all.
    #[test]
    fn test_dense_small_functions_do_not_starve_larger_duplicates() {
        let mut fingerprints: Vec<Fingerprints> = (0..5_000)
            .map(|id| make_sized_fp(id, &format!("small{id}"), (id % 8) as u64, 40))
            .collect();
        fingerprints.push(make_sized_fp(5_000, "big-a", 0xF0F0_F0F0_0000_0000, 400));
        fingerprints.push(make_sized_fp(5_001, "big-b", 0xF0F0_F0F0_0000_0003, 420));
        let candidates = generate_candidates(&fingerprints, 12, MIN);
        assert!(candidates.contains(&(5_000, 5_001)));
    }

    #[test]
    fn test_functions_of_very_different_size_are_not_paired() {
        let mut fingerprints: Vec<Fingerprints> = (0..400)
            .map(|id| make_sized_fp(id, &format!("f{id}"), 0xFFFF_0000_0000_0000, 1_000))
            .collect();
        fingerprints.push(make_sized_fp(400, "small", 0, 40));
        fingerprints.push(make_sized_fp(401, "large", 0, 400));
        let candidates = generate_candidates(&fingerprints, 12, MIN);
        assert!(!candidates.contains(&(400, 401)));
    }
}
