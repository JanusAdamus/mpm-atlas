//! Formula lane: operator trees -> structural features -> inverted index with
//! tf-idf cosine scoring.
//!
//! On disk (`formula.bin`, little-endian, memory-mapped, arrays 64-byte aligned):
//! `header | keys: [u64; K] sorted | offsets: [u64; K+1] | nodes: [u32; P] | tf: [u16; P]
//!  | idf: [f32; K] | norm: [f32; num_nodes]`
//! A posting list is the sorted node ids of the documents with that feature.  Scores:
//! `cos(q, d)` with weights `(1 + ln tf) * idf`, `idf = ln(1 + D / df)`, unified
//! features scaled by `UNIFIED_WEIGHT` on the query side.  Features present in more
//! than `MAX_DF_FRACTION` of the documents are skipped at query time (they carry
//! almost no idf and cost the longest posting lists).

pub mod features;
pub mod mathml;
pub mod parse;

use atlas_core::{Error, NodeIx, Result};
use features::{Feature, UNIFIED_WEIGHT, feature_counts};
use memmap2::Mmap;
use rayon::prelude::*;
use std::cell::RefCell;
use std::io::Write;
use std::path::Path;

const MAGIC: &[u8; 8] = b"ATLFORM1";
const HEADER: usize = 64;
const MAX_DF_FRACTION: f64 = 0.25;

pub struct FormulaIndex {
    map: Mmap,
    num_keys: usize,
    num_postings: usize,
    num_nodes: usize,
    num_docs: usize,
    offs: [usize; 6],
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct FormulaBuildStats {
    pub documents: u64,
    pub features: u64,
    pub postings: u64,
    pub bytes: u64,
}

fn align64(x: usize) -> usize {
    x.div_ceil(64) * 64
}

fn layout(num_keys: usize, num_postings: usize, num_nodes: usize) -> ([usize; 6], usize) {
    let mut o = [0usize; 6];
    let mut at = HEADER;
    let sizes = [num_keys * 8, (num_keys + 1) * 8, num_postings * 4, num_postings * 2, num_keys * 4, num_nodes * 4];
    for (i, s) in sizes.iter().enumerate() {
        at = align64(at);
        o[i] = at;
        at += s;
    }
    (o, at)
}

/// Build from `(node, formula sources)`; one document per node.
pub fn build(docs: &[(NodeIx, Vec<&str>)], num_nodes: usize, path: &Path) -> Result<FormulaBuildStats> {
    // (key, node, tf) triples, parallel over documents
    let per_doc: Vec<Vec<(u64, NodeIx, u16)>> = docs
        .par_iter()
        .map(|(node, srcs)| {
            let trees: Vec<_> = srcs.iter().flat_map(|s| trees_of(s)).collect();
            feature_counts(&trees).into_iter().map(|(f, tf)| (f.key, *node, tf)).collect()
        })
        .collect();
    let num_docs = per_doc.iter().filter(|v| !v.is_empty()).count();
    let mut triples: Vec<(u64, NodeIx, u16)> = per_doc.into_iter().flatten().collect();
    triples.par_sort_unstable();
    triples.dedup_by(|a, b| {
        // same feature in two docs merged into one node (entity merges): add tf
        if a.0 == b.0 && a.1 == b.1 {
            b.2 = b.2.saturating_add(a.2);
            true
        } else {
            false
        }
    });
    let mut keys: Vec<u64> = Vec::new();
    let mut offsets: Vec<u64> = Vec::new();
    for (i, t) in triples.iter().enumerate() {
        if keys.last() != Some(&t.0) {
            keys.push(t.0);
            offsets.push(i as u64);
        }
    }
    offsets.push(triples.len() as u64);
    let d = num_docs.max(1) as f64;
    let idf: Vec<f32> = offsets.windows(2).map(|w| (1.0 + d / (w[1] - w[0]) as f64).ln() as f32).collect();
    let mut norm2 = vec![0f64; num_nodes];
    for (k, w) in offsets.windows(2).enumerate() {
        for t in &triples[w[0] as usize..w[1] as usize] {
            let x = weight(t.2) as f64 * idf[k] as f64;
            norm2[t.1 as usize] += x * x;
        }
    }
    let norm: Vec<f32> = norm2.iter().map(|x| x.sqrt() as f32).collect();
    let (offs, total) = layout(keys.len(), triples.len(), num_nodes);
    let mut buf = vec![0u8; total];
    buf[..8].copy_from_slice(MAGIC);
    for (i, v) in [keys.len() as u64, triples.len() as u64, num_nodes as u64, num_docs as u64].iter().enumerate() {
        buf[8 + i * 8..16 + i * 8].copy_from_slice(&v.to_le_bytes());
    }
    let nodes: Vec<u32> = triples.iter().map(|t| t.1).collect();
    let tfs: Vec<u16> = triples.iter().map(|t| t.2).collect();
    let sections: [&[u8]; 6] = [
        bytemuck::cast_slice(&keys),
        bytemuck::cast_slice(&offsets),
        bytemuck::cast_slice(&nodes),
        bytemuck::cast_slice(&tfs),
        bytemuck::cast_slice(&idf),
        bytemuck::cast_slice(&norm),
    ];
    for (o, s) in offs.iter().zip(sections) {
        buf[*o..*o + s.len()].copy_from_slice(s);
    }
    let mut f = std::fs::File::create(path).map_err(|e| Error::io(path, e))?;
    f.write_all(&buf).map_err(|e| Error::io(path, e))?;
    Ok(FormulaBuildStats {
        documents: num_docs as u64,
        features: keys.len() as u64,
        postings: triples.len() as u64,
        bytes: total as u64,
    })
}

const LEAN_KEYWORDS: [&str; 14] = [
    "theorem", "lemma", "def", "structure", "class", "instance", "abbrev", "inductive", "axiom", "example", "alias",
    "irreducible_def", "noncomputable", "opaque",
];

/// A Lean declaration, LaTeX, or text where words are a minority (`2 ≤ p`); not prose.
pub fn looks_formal(s: &str) -> bool {
    let first = s.split_whitespace().find(|w| !matches!(*w, "protected" | "private" | "@[simp]"));
    if first.is_some_and(|w| LEAN_KEYWORDS.contains(&w)) || s.contains('\\') || s.contains('$') || mathml::is_mathml(s) {
        return true;
    }
    let toks: Vec<&str> = s.split_whitespace().collect();
    let words = toks
        .iter()
        .map(|w| w.trim_matches(|c: char| ",.;:¿?¡!()\"'".contains(c)))
        .filter(|w| w.chars().count() >= 2 && w.chars().all(char::is_alphabetic))
        .count();
    !toks.is_empty() && words * 2 < toks.len()
}

/// Operator trees of one source: MathML is linearised first, LaTeX/Lean parsed directly.
pub fn trees_of(src: &str) -> Vec<parse::Node> {
    if mathml::is_mathml(src) { parse::parse_all(&mathml::to_latex(src)) } else { parse::parse_all(src) }
}

fn weight(tf: u16) -> f32 {
    1.0 + (tf as f32).ln()
}

thread_local! {
    /// Dense score accumulator per thread (num_nodes floats) plus the touched list.
    static ACC: RefCell<(Vec<f32>, Vec<NodeIx>)> = const { RefCell::new((Vec::new(), Vec::new())) };
}

impl FormulaIndex {
    pub fn open(path: &Path) -> Result<FormulaIndex> {
        let file = std::fs::File::open(path).map_err(|e| Error::io(path, e))?;
        // SAFETY: the index file is written once by `build` and never modified in place.
        let map = unsafe { Mmap::map(&file) }.map_err(|e| Error::io(path, e))?;
        if map.len() < HEADER || &map[..8] != MAGIC {
            return Err(Error::Format(format!("{}: not a formula index", path.display())));
        }
        let rd = |i: usize| u64::from_le_bytes(map[8 + i * 8..16 + i * 8].try_into().unwrap()) as usize;
        let (num_keys, num_postings, num_nodes, num_docs) = (rd(0), rd(1), rd(2), rd(3));
        let (offs, total) = layout(num_keys, num_postings, num_nodes);
        if map.len() != total {
            return Err(Error::Format(format!("{}: length {} != {}", path.display(), map.len(), total)));
        }
        let idx = FormulaIndex { map, num_keys, num_postings, num_nodes, num_docs, offs };
        let offsets = idx.offsets();
        if offsets.first() != Some(&0)
            || offsets.last() != Some(&(num_postings as u64))
            || offsets.windows(2).any(|w| w[0] > w[1])
        {
            return Err(Error::Integrity(format!("{}: bad posting offsets", path.display())));
        }
        if idx.nodes().iter().any(|&n| n as usize >= num_nodes) {
            return Err(Error::Integrity(format!("{}: node id out of range", path.display())));
        }
        Ok(idx)
    }

    fn slice<T: bytemuck::Pod>(&self, i: usize, len: usize) -> &[T] {
        bytemuck::cast_slice(&self.map[self.offs[i]..self.offs[i] + len * std::mem::size_of::<T>()])
    }
    fn keys(&self) -> &[u64] {
        self.slice(0, self.num_keys)
    }
    fn offsets(&self) -> &[u64] {
        self.slice(1, self.num_keys + 1)
    }
    fn nodes(&self) -> &[u32] {
        self.slice(2, self.num_postings)
    }
    fn tfs(&self) -> &[u16] {
        self.slice(3, self.num_postings)
    }
    fn idf(&self) -> &[f32] {
        self.slice(4, self.num_keys)
    }
    fn norms(&self) -> &[f32] {
        self.slice(5, self.num_nodes)
    }

    pub fn num_docs(&self) -> usize {
        self.num_docs
    }

    pub fn num_nodes(&self) -> usize {
        self.num_nodes
    }

    /// Top `k` nodes by cosine similarity to the query trees, best first.
    /// `keep(node)` filters candidates (types, merges) before ranking.
    pub fn search(&self, trees: &[parse::Node], k: usize, keep: impl Fn(NodeIx) -> bool) -> Vec<(NodeIx, f32)> {
        let q = feature_counts(trees);
        if q.is_empty() || k == 0 {
            return Vec::new();
        }
        let (keys, offsets, nodes, tfs, idf, norms) =
            (self.keys(), self.offsets(), self.nodes(), self.tfs(), self.idf(), self.norms());
        let max_df = ((self.num_docs as f64 * MAX_DF_FRACTION) as u64).max(1000);
        let mut qnorm2 = 0f32;
        ACC.with(|cell| {
            let (acc, touched) = &mut *cell.borrow_mut();
            if acc.len() != self.num_nodes {
                *acc = vec![0.0; self.num_nodes];
            }
            for (Feature { key, unified }, qtf) in &q {
                let Ok(ki) = keys.binary_search(key) else { continue };
                let qw = weight(*qtf) * idf[ki] * if *unified { UNIFIED_WEIGHT } else { 1.0 };
                qnorm2 += qw * qw;
                let (a, b) = (offsets[ki] as usize, offsets[ki + 1] as usize);
                if (b - a) as u64 > max_df {
                    continue;
                }
                let w = qw * idf[ki];
                for (&n, &tf) in nodes[a..b].iter().zip(&tfs[a..b]) {
                    let slot = &mut acc[n as usize];
                    if *slot == 0.0 {
                        touched.push(n);
                    }
                    *slot += w * weight(tf);
                }
            }
            let qn = qnorm2.sqrt().max(f32::MIN_POSITIVE);
            let mut scored: Vec<(NodeIx, f32)> = Vec::with_capacity(touched.len());
            for &n in touched.iter() {
                let s = acc[n as usize];
                acc[n as usize] = 0.0;
                if keep(n) {
                    scored.push((n, s / (qn * norms[n as usize].max(f32::MIN_POSITIVE))));
                }
            }
            touched.clear();
            top_k(scored, k)
        })
    }
}

/// The `k` best `(id, score)` pairs, best first; ties by id for determinism.
pub fn top_k(mut v: Vec<(NodeIx, f32)>, k: usize) -> Vec<(NodeIx, f32)> {
    let cmp = |a: &(NodeIx, f32), b: &(NodeIx, f32)| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0));
    if v.len() > k {
        v.select_nth_unstable_by(k - 1, cmp);
        v.truncate(k);
    }
    v.sort_unstable_by(cmp);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_ranks_same_structure_first_and_survives_reopen() {
        let docs: Vec<(NodeIx, Vec<&str>)> = vec![
            (0, vec!["a^2 + b^2 = c^2"]),
            (1, vec![r"c^2 = a^2 + b^2 - 2ab\cos\gamma"]),
            (2, vec![r"\int_0^1 f(x)\,dx"]),
            (3, vec![r"e^{i\pi} + 1 = 0"]),
            (5, vec!["theorem Nat.add_comm (n m : ℕ) : n + m = m + n"]),
        ];
        let dir = std::env::temp_dir().join(format!("atlas-formula-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("formula.bin");
        let stats = build(&docs, 6, &path).unwrap();
        assert_eq!(stats.documents, 5);
        let idx = FormulaIndex::open(&path).unwrap();
        let q = parse::parse_all("x^2 + y^2 = z^2");
        let hits = idx.search(&q, 3, |_| true);
        assert_eq!(hits[0].0, 0, "{hits:?}");
        assert_eq!(hits[1].0, 1, "{hits:?}");
        let q = parse::parse_all("a + b = b + a");
        assert_eq!(idx.search(&q, 1, |_| true)[0].0, 5);
        assert!(idx.search(&q, 5, |n| n != 5).iter().all(|h| h.0 != 5));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn norm_and_inner_product_structure_beats_a_sum_of_squares() {
        let parseval = r"\|x\|^2 = \sum_k |\langle x, e_k\rangle|^2";
        let docs: Vec<(NodeIx, Vec<&str>)> = vec![(0, vec!["a^2 + b^2 = c^2"]), (1, vec![parseval])];
        let dir = std::env::temp_dir().join(format!("atlas-formula-parseval-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        build(&docs, 2, &dir.join("f.bin")).unwrap();
        let idx = FormulaIndex::open(&dir.join("f.bin")).unwrap();
        let q = parse::parse_all(r"\|v\|^2 = \sum_j |\langle v, u_j\rangle|^2");
        let shown: Vec<String> = q.iter().chain(&parse::parse_all(parseval)).map(parse::Node::sexpr).collect();
        let hits = idx.search(&q, 2, |_| true);
        assert_eq!(hits.first().map(|h| h.0), Some(1), "{hits:?} {shown:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prose_is_not_formal() {
        assert!(looks_formal("theorem Nat.add_comm (n m : ℕ) : n + m = m + n"));
        assert!(looks_formal(r"c^2 = a^2 + b^2 - 2ab\cos\gamma"));
        assert!(looks_formal("2 ≤ p"));
        assert!(!looks_formal("En un triángulo rectángulo, el cuadrado de la hipotenusa es igual a la suma"));
    }

    #[test]
    fn truncated_file_is_rejected() {
        let dir = std::env::temp_dir().join(format!("atlas-formula-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("formula.bin");
        build(&[(0, vec!["a=b"])], 1, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() - 4]).unwrap();
        assert!(FormulaIndex::open(&path).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
