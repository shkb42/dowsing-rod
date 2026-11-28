use crate::types::{
    FunctionInfo, NormalizedFunction, SimilarityScore, SimilaritySignals, StructuralToken,
};
use std::collections::{BTreeSet, HashMap};

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

/// Normalized tokens of one function as interned IDs: equal IDs mean equal
/// tokens, so integer comparisons give exactly the same results as comparing
/// the tokens themselves, at a fraction of the cost.
#[derive(Debug, Clone, Default)]
pub struct TokenIds {
    all: Vec<u32>,
    control_flow: Vec<u32>,
}

/// Assigns one ID per distinct token. Share one interner across all functions
/// whose `TokenIds` will be compared with each other.
#[derive(Debug, Default)]
pub struct TokenInterner<'a> {
    ids: HashMap<&'a StructuralToken, u32>,
}

impl<'a> TokenInterner<'a> {
    pub fn intern(&mut self, tokens: &'a [StructuralToken]) -> TokenIds {
        let mut ids = TokenIds::default();
        for token in tokens {
            let next = self.ids.len() as u32;
            let id = *self.ids.entry(token).or_insert(next);
            ids.all.push(id);
            if token.is_control_flow() {
                ids.control_flow.push(id);
            }
        }
        ids
    }
}

/// Compute detailed multi-signal similarity between two functions.
pub fn compute_similarity(
    norm_a: &NormalizedFunction,
    norm_b: &NormalizedFunction,
    info_a: &FunctionInfo,
    info_b: &FunctionInfo,
) -> SimilarityScore {
    let mut interner = TokenInterner::default();
    let ids_a = interner.intern(&norm_a.tokens);
    let ids_b = interner.intern(&norm_b.tokens);
    compute_similarity_with_ids(
        norm_a.function_id,
        norm_b.function_id,
        &ids_a,
        &ids_b,
        info_a,
        info_b,
    )
}

/// [`compute_similarity`] for functions whose tokens were interned by one shared
/// [`TokenInterner`].
pub fn compute_similarity_with_ids(
    func_a: usize,
    func_b: usize,
    ids_a: &TokenIds,
    ids_b: &TokenIds,
    info_a: &FunctionInfo,
    info_b: &FunctionInfo,
) -> SimilarityScore {
    let ast_sim = ast_structure_similarity(&ids_a.all, &ids_b.all);
    let token_sim = token_jaccard_similarity(&ids_a.all, &ids_b.all);
    let call_sim = call_set_similarity(&info_a.called_functions, &info_b.called_functions);
    let control_sim = control_flow_similarity(&ids_a.control_flow, &ids_b.control_flow);
    let complexity_sim = complexity_similarity(info_a.complexity, info_b.complexity);
    let param_sim = parameter_similarity(info_a, info_b);

    let overall = W_AST * ast_sim
        + W_TOKENS * token_sim
        + W_CALLS * call_sim
        + W_CONTROL * control_sim
        + W_COMPLEXITY * complexity_sim
        + W_PARAMS * param_sim;

    SimilarityScore {
        func_a,
        func_b,
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
fn ast_structure_similarity(tokens_a: &[u32], tokens_b: &[u32]) -> f64 {
    if tokens_a.is_empty() && tokens_b.is_empty() {
        return 1.0;
    }
    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }

    // Very long sequences are capped at 500 tokens to bound the O(n*m) cost.
    let lcs_len = lcs_length(tokens_a, tokens_b, 500);
    let max_len = tokens_a.len().max(tokens_b.len());
    lcs_len as f64 / max_len as f64
}

/// Token-level Jaccard similarity (multiset intersection / union).
fn token_jaccard_similarity(tokens_a: &[u32], tokens_b: &[u32]) -> f64 {
    if tokens_a.is_empty() && tokens_b.is_empty() {
        return 1.0;
    }
    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }

    let mut counts: HashMap<u32, (usize, usize)> = HashMap::new();
    for &t in tokens_a {
        counts.entry(t).or_default().0 += 1;
    }
    for &t in tokens_b {
        counts.entry(t).or_default().1 += 1;
    }

    let mut intersection = 0usize;
    let mut union = 0usize;
    for &(a, b) in counts.values() {
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
fn control_flow_similarity(cf_a: &[u32], cf_b: &[u32]) -> f64 {
    if cf_a.is_empty() && cf_b.is_empty() {
        return 1.0;
    }
    if cf_a.is_empty() || cf_b.is_empty() {
        return 0.0;
    }

    // Use sequence similarity for control flow
    let lcs = lcs_length(cf_a, cf_b, 200);
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

/// Longest common subsequence length (O(n*m) dynamic programming) of the
/// first `cap` elements of each sequence.
fn lcs_length(a: &[u32], b: &[u32], cap: usize) -> usize {
    let a = &a[..a.len().min(cap)];
    let b = &b[..b.len().min(cap)];

    let n = b.len();
    let mut prev = vec![0u32; n + 1];
    let mut curr = vec![0u32; n + 1];

    for &x in a {
        for j in 1..=n {
            curr[j] = if x == b[j - 1] {
                prev[j - 1] + 1
            } else {
                prev[j].max(curr[j - 1])
            };
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n] as usize
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
