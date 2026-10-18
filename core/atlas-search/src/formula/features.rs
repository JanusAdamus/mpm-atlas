//! Structural features of an operator tree, in the spirit of Tangent-CFT (R24):
//!
//! - `P`: leaf-to-ancestor paths up to `MAX_UP` operators (`V<pow#0<add`);
//! - `T`: leaf-pair tuples `(s1, s2, path)` for leaves at most `WINDOW` edges apart;
//! - `S`: node shapes, an operator with the labels of its operands (`add(pow,pow)`);
//! - `R`: the root operator.
//!
//! Every feature exists twice: exact (variable names and numbers kept) and unified
//! (variables -> `V`, numbers -> `N`), so `x^2+y^2=z^2` matches `a^2+b^2=c^2`
//! through the unified copy and the exact copy rewards identical notation.  Child
//! positions are kept for non-commutative operators (`pow#0` base, `pow#1` exponent).
//! Features are hashed with 64-bit FNV-1a (stable across builds and platforms).

use super::parse::{Node, is_commutative};

pub const MAX_UP: usize = 4;
pub const WINDOW: usize = 4;
const MAX_LEAVES: usize = 48;

/// Feature namespace weight applied at query time: exact features count fully.
pub const UNIFIED_WEIGHT: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Feature {
    pub key: u64,
    pub unified: bool,
}

pub fn fnv1a(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            h ^= 0x1f;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        for b in p.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

fn label(n: &Node, unified: bool) -> String {
    match n {
        Node::Op(name, _) => name.clone(),
        Node::Var(v) if unified => {
            let _ = v;
            "V".into()
        }
        Node::Var(v) => format!("v:{v}"),
        Node::Num(_) if unified => "N".into(),
        Node::Num(x) => format!("n:{x}"),
        Node::Sym(s) => format!("s:{s}"),
    }
}

/// Edge label from a parent to its `i`-th child.
fn edge(parent: &Node, i: usize) -> String {
    match parent {
        Node::Op(name, _) if is_commutative(name) => format!("{name}#*"),
        Node::Op(name, _) => format!("{name}#{i}"),
        _ => String::new(),
    }
}

/// All features of `tree`, with multiplicity (the caller counts term frequencies).
pub fn features(tree: &Node, out: &mut Vec<Feature>) {
    for unified in [false, true] {
        let ns = if unified { "u" } else { "e" };
        let mut push = |parts: &[&str]| {
            let mut all = Vec::with_capacity(parts.len() + 1);
            all.push(ns);
            all.extend_from_slice(parts);
            out.push(Feature { key: fnv1a(&all), unified });
        };
        push(&["R", &label(tree, unified)]);
        // leaves with their root paths: (leaf, [(ancestor, child index)])
        let mut leaves: Vec<(&Node, Vec<(&Node, usize)>)> = Vec::new();
        let mut stack: Vec<(&Node, Vec<(&Node, usize)>)> = vec![(tree, Vec::new())];
        while let Some((n, path)) = stack.pop() {
            match n {
                Node::Op(_, kids) if !kids.is_empty() => {
                    let shape: Vec<String> = kids.iter().map(|k| label(k, unified)).collect();
                    push(&["S", &label(n, unified), &shape.join(",")]);
                    for (i, k) in kids.iter().enumerate().rev() {
                        let mut p = path.clone();
                        p.push((n, i));
                        stack.push((k, p));
                    }
                }
                _ => {
                    if leaves.len() < MAX_LEAVES {
                        leaves.push((n, path));
                    }
                }
            }
        }
        for (leaf, path) in &leaves {
            let mut s = label(leaf, unified);
            for (anc, i) in path.iter().rev().take(MAX_UP) {
                s.push('<');
                s.push_str(&edge(anc, *i));
                push(&["P", &s]);
            }
        }
        for a in 0..leaves.len() {
            for b in a + 1..leaves.len() {
                let (la, pa) = &leaves[a];
                let (lb, pb) = &leaves[b];
                let common = pa.iter().zip(pb).take_while(|(x, y)| std::ptr::eq(x.0, y.0) && x.1 == y.1).count();
                // the LCA is the deepest common ancestor: pa[common] and pb[common] share it
                if common >= pa.len() || common >= pb.len() || !std::ptr::eq(pa[common].0, pb[common].0) {
                    continue;
                }
                let up = pa.len() - common;
                let down = pb.len() - common;
                if up + down > WINDOW {
                    continue;
                }
                let lca = pa[common].0;
                let mut path = String::new();
                for (anc, i) in pa[common + 1..].iter().rev() {
                    path.push_str(&edge(anc, *i));
                    path.push('^');
                }
                path.push_str(&format!("{}|{}|{}", label(lca, unified), edge(lca, pa[common].1), edge(lca, pb[common].1)));
                for (anc, i) in &pb[common + 1..] {
                    path.push('v');
                    path.push_str(&edge(anc, *i));
                }
                push(&["T", &label(la, unified), &label(lb, unified), &path]);
            }
        }
    }
}

/// Features of several trees with term frequencies, sorted by key.
pub fn feature_counts(trees: &[Node]) -> Vec<(Feature, u16)> {
    let mut all = Vec::new();
    for t in trees {
        features(t, &mut all);
    }
    all.sort_unstable();
    let mut out: Vec<(Feature, u16)> = Vec::with_capacity(all.len());
    for f in all {
        match out.last_mut() {
            Some((g, c)) if *g == f => *c = c.saturating_add(1),
            _ => out.push((f, 1)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::parse::parse;
    use super::*;
    use std::collections::HashSet;

    fn keys(src: &str, unified: bool) -> HashSet<u64> {
        let mut v = Vec::new();
        features(&parse(src).unwrap(), &mut v);
        v.into_iter().filter(|f| f.unified == unified).map(|f| f.key).collect()
    }

    #[test]
    fn renamed_variables_share_all_unified_features_and_no_exact_leaf_features() {
        let a = keys("a^2 + b^2 = c^2", true);
        let b = keys("x^2 + y^2 = z^2", true);
        assert_eq!(a, b);
        let ea = keys("a^2 + b^2 = c^2", false);
        let eb = keys("x^2 + y^2 = z^2", false);
        assert!(ea.intersection(&eb).count() < ea.len());
    }

    #[test]
    fn different_structure_shares_few_features() {
        let a = keys("a^2 + b^2 = c^2", true);
        let c = keys(r"\int_0^1 f(x) dx", true);
        let shared = a.intersection(&c).count();
        assert!(shared * 4 < a.len(), "shared {shared} of {}", a.len());
    }

    #[test]
    fn fnv_is_stable() {
        assert_eq!(fnv1a(&["e", "R", "eq"]), fnv1a(&["e", "R", "eq"]));
        assert_ne!(fnv1a(&["e", "Req"]), fnv1a(&["e", "R", "eq"]));
    }
}
