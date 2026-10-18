//! Text encoders of the semantic lane.  Both return L2-normalised vectors.
//!
//! - `Neural` (feature `neural`): a multilingual sentence encoder run by fastembed
//!   (ONNX Runtime).  Default `multilingual-e5-small` (384 dims, ES/EN and ~100 other
//!   languages; queries get `query: `, documents `passage: `).  Documents are encoded
//!   in batches by a pool of sessions; the pool is shared by concurrent queries.
//! - `Lsi`: TF-IDF + truncated SVD (latent semantic indexing), fitted on the corpus
//!   by a randomized range finder (Halko, Martinsson & Tropp 2011) with one power
//!   iteration.  No model download, deterministic; it is the fallback when the
//!   neural encoder is not compiled in or cannot load, and the encoder of the tests.

use crate::corpus::{Doc, Field};
use crate::text;
use ahash::AHashMap;
use atlas_core::{Error, Result};
use rayon::prelude::*;
use std::io::{BufWriter, Write};
use std::path::Path;
use tantivy::tokenizer::{TextAnalyzer, TokenizerManager};

/// Characters of each text kept for embedding (e5 sees at most `max_length` tokens anyway).
const MAX_DESC_CHARS: usize = 400;
const MAX_STMT_CHARS: usize = 240;

/// What a document looks like to the encoders: labels (identifiers split into words),
/// aliases, description and statement, truncated.
pub fn doc_text(d: &Doc) -> String {
    let mut s = String::new();
    let mut push = |t: &str, max: usize| {
        if !s.is_empty() {
            s.push_str(". ");
        }
        s.extend(t.chars().take(max));
    };
    for t in d.of(Field::Label) {
        push(&humanize(&t.text), 200);
    }
    for t in d.of(Field::Alias).take(3) {
        push(&humanize(&t.text), 100);
    }
    if let Some(t) = d.of(Field::Description).min_by_key(|t| lang_rank(&t.lang)) {
        push(&t.text, MAX_DESC_CHARS);
    }
    if let Some(t) = d.of(Field::Statement).min_by_key(|t| lang_rank(&t.lang)) {
        push(&t.text, MAX_STMT_CHARS);
    }
    s
}

fn lang_rank(l: &str) -> u8 {
    match l {
        "es" => 0,
        "en" => 1,
        _ => 2,
    }
}

/// `Nat.Prime.two_le` -> `Nat Prime two le`; plain text is unchanged.
pub fn humanize(label: &str) -> String {
    if label.contains(' ') {
        return label.to_string();
    }
    let parts = text::split_ident(label);
    if parts.len() <= 1 {
        return label.to_string();
    }
    parts.iter().map(|&(a, b)| &label[a..b]).collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------- LSI

pub const LSI_DIM: usize = 128;
const LSI_OVERSAMPLE: usize = 16;
const LSI_VOCAB: usize = 1 << 16;
const LSI_FIT_DOCS: usize = 200_000;
const LSI_MAGIC: &[u8; 8] = b"ATLLSI01";

pub struct Lsi {
    vocab: AHashMap<String, u32>,
    idf: Vec<f32>,
    /// `[vocab][dim]` row-major projection (right singular vectors).
    proj: Vec<f32>,
    dim: usize,
    analyzer: TextAnalyzer,
}

fn lsi_analyzer() -> TextAnalyzer {
    let m = TokenizerManager::default();
    text::register(&m);
    m.get(text::LEAN).expect("registered")
}

/// Sparse tf-idf row: sorted (term, weight), L2-normalised.
type Row = Vec<(u32, f32)>;

fn counts(analyzer: &mut TextAnalyzer, s: &str) -> AHashMap<String, u32> {
    let mut m = AHashMap::new();
    for t in text::analyze(analyzer, s) {
        if t.len() > 1 || t.chars().all(char::is_alphabetic) {
            *m.entry(t).or_insert(0) += 1;
        }
    }
    m
}

impl Lsi {
    fn row(&self, analyzer: &mut TextAnalyzer, s: &str) -> Row {
        let mut r: Row = counts(analyzer, s)
            .into_iter()
            .filter_map(|(t, c)| self.vocab.get(&t).map(|&i| (i, (1.0 + (c as f32).ln()) * self.idf[i as usize])))
            .collect();
        normalize_sparse(&mut r);
        r
    }

    fn project(&self, r: &Row) -> Vec<f32> {
        let mut v = vec![0f32; self.dim];
        for &(t, w) in r {
            let p = &self.proj[t as usize * self.dim..(t as usize + 1) * self.dim];
            for (a, b) in v.iter_mut().zip(p) {
                *a += w * b;
            }
        }
        normalize(&mut v);
        v
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    pub fn encode(&self, texts: &[String]) -> Vec<Vec<f32>> {
        texts
            .par_iter()
            .map_init(lsi_analyzer, |a, s| {
                let r = self.row(a, s);
                self.project(&r)
            })
            .collect()
    }

    pub fn encode_query(&self, q: &str) -> Vec<f32> {
        let mut a = self.analyzer.clone();
        let r = self.row(&mut a, q);
        self.project(&r)
    }

    /// Fit on `texts` (a deterministic sample of at most `LSI_FIT_DOCS` is used).
    pub fn fit(texts: &[String], dim: usize, seed: u64) -> Lsi {
        let step = texts.len().div_ceil(LSI_FIT_DOCS).max(1);
        let sample: Vec<&String> = texts.iter().step_by(step).collect();
        let tf: Vec<AHashMap<String, u32>> = sample.par_iter().map_init(lsi_analyzer, |a, s| counts(a, s)).collect();
        let mut df: AHashMap<&str, u32> = AHashMap::new();
        for m in &tf {
            for t in m.keys() {
                *df.entry(t.as_str()).or_insert(0) += 1;
            }
        }
        let mut terms: Vec<(&str, u32)> = df.into_iter().filter(|(_, d)| *d >= 2).collect();
        terms.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        terms.truncate(LSI_VOCAB);
        terms.sort_unstable_by(|a, b| a.0.cmp(b.0));
        let n = sample.len().max(1) as f32;
        let vocab: AHashMap<String, u32> = terms.iter().enumerate().map(|(i, (t, _))| (t.to_string(), i as u32)).collect();
        let idf: Vec<f32> = terms.iter().map(|(_, d)| (1.0 + n / *d as f32).ln()).collect();
        let rows: Vec<Row> = tf
            .into_par_iter()
            .map(|m| {
                let mut r: Row = m
                    .into_iter()
                    .filter_map(|(t, c)| vocab.get(&t).map(|&i| (i, (1.0 + (c as f32).ln()) * idf[i as usize])))
                    .collect();
                r.sort_unstable_by_key(|x| x.0);
                normalize_sparse(&mut r);
                r
            })
            .collect();
        let v = vocab.len();
        let dim = dim.min(v.max(1));
        let proj = randomized_svd(&rows, v, dim, seed);
        Lsi { vocab, idf, proj, dim, analyzer: lsi_analyzer() }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let io = |e| Error::io(path, e);
        let mut terms: Vec<(&String, &u32)> = self.vocab.iter().collect();
        terms.sort_unstable_by_key(|x| *x.1);
        let mut f = BufWriter::new(std::fs::File::create(path).map_err(io)?);
        f.write_all(LSI_MAGIC).map_err(io)?;
        f.write_all(&(terms.len() as u64).to_le_bytes()).map_err(io)?;
        f.write_all(&(self.dim as u64).to_le_bytes()).map_err(io)?;
        for (t, _) in &terms {
            f.write_all(&(t.len() as u16).to_le_bytes()).map_err(io)?;
            f.write_all(t.as_bytes()).map_err(io)?;
        }
        f.write_all(bytemuck::cast_slice(&self.idf)).map_err(io)?;
        f.write_all(bytemuck::cast_slice(&self.proj)).map_err(io)?;
        f.flush().map_err(io)
    }

    pub fn load(path: &Path) -> Result<Lsi> {
        let b = std::fs::read(path).map_err(|e| Error::io(path, e))?;
        let bad = || Error::Format(format!("{}: bad LSI model", path.display()));
        if b.len() < 24 || &b[..8] != LSI_MAGIC {
            return Err(bad());
        }
        let v = u64::from_le_bytes(b[8..16].try_into().expect("8")) as usize;
        let dim = u64::from_le_bytes(b[16..24].try_into().expect("8")) as usize;
        let mut at = 24;
        let mut vocab = AHashMap::with_capacity(v);
        for i in 0..v {
            let len = u16::from_le_bytes(b.get(at..at + 2).ok_or_else(bad)?.try_into().expect("2")) as usize;
            let t = std::str::from_utf8(b.get(at + 2..at + 2 + len).ok_or_else(bad)?).map_err(|_| bad())?;
            vocab.insert(t.to_string(), i as u32);
            at += 2 + len;
        }
        let floats = |at: usize, n: usize| -> Result<Vec<f32>> {
            let s = b.get(at..at + n * 4).ok_or_else(bad)?;
            Ok(s.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().expect("4"))).collect())
        };
        let idf = floats(at, v)?;
        let proj = floats(at + v * 4, v * dim)?;
        if at + v * 4 + v * dim * 4 != b.len() {
            return Err(bad());
        }
        Ok(Lsi { vocab, idf, proj, dim, analyzer: lsi_analyzer() })
    }
}

pub fn normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
}

fn normalize_sparse(r: &mut Row) {
    let n = r.iter().map(|x| x.1 * x.1).sum::<f32>().sqrt();
    if n > 0.0 {
        r.iter_mut().for_each(|x| x.1 /= n);
    }
}

/// splitmix64 -> uniform in [-1, 1)
fn rand_unit(state: &mut u64) -> f32 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    ((z >> 40) as f32 / (1u64 << 23) as f32) - 1.0
}

/// `A · Q` for sparse rows `A` (n×v) and dense `Q` (v×l, row-major): n×l.
fn sparse_times(rows: &[Row], q: &[f32], l: usize) -> Vec<f32> {
    let mut out = vec![0f32; rows.len() * l];
    out.par_chunks_mut(l).zip(rows).for_each(|(o, r)| {
        for &(t, w) in r {
            for (a, b) in o.iter_mut().zip(&q[t as usize * l..(t as usize + 1) * l]) {
                *a += w * b;
            }
        }
    });
    out
}

/// `Aᵀ · Z` for sparse rows `A` (n×v) and dense `Z` (n×l): v×l.
fn sparse_t_times(rows: &[Row], z: &[f32], v: usize, l: usize) -> Vec<f32> {
    let chunk = rows.len().div_ceil(rayon::current_num_threads()).max(1024);
    rows.par_chunks(chunk)
        .enumerate()
        .map(|(ci, rs)| {
            let mut acc = vec![0f32; v * l];
            for (i, r) in rs.iter().enumerate() {
                let zr = &z[(ci * chunk + i) * l..(ci * chunk + i + 1) * l];
                for &(t, w) in r {
                    for (a, b) in acc[t as usize * l..(t as usize + 1) * l].iter_mut().zip(zr) {
                        *a += w * b;
                    }
                }
            }
            acc
        })
        .reduce(
            || vec![0f32; v * l],
            |mut a, b| {
                a.iter_mut().zip(&b).for_each(|(x, y)| *x += y);
                a
            },
        )
}

/// Orthonormalise the columns of `y` (v×l, row-major) in place: modified Gram-Schmidt
/// in f64 over columns.
fn orthonormalize(y: &mut [f32], v: usize, l: usize) {
    let mut cols: Vec<Vec<f64>> = (0..l).map(|j| (0..v).map(|i| y[i * l + j] as f64).collect()).collect();
    for j in 0..l {
        let (done, rest) = cols.split_at_mut(j);
        let c = &mut rest[0];
        for _ in 0..2 {
            // re-orthogonalise once: MGS loses orthogonality on near-dependent columns
            for p in done.iter() {
                let d: f64 = p.par_iter().zip(c.par_iter()).map(|(a, b)| a * b).sum();
                c.par_iter_mut().zip(p.par_iter()).for_each(|(x, y)| *x -= d * y);
            }
        }
        let n = c.par_iter().map(|x| x * x).sum::<f64>().sqrt();
        if n > 1e-12 {
            c.par_iter_mut().for_each(|x| *x /= n);
        } else {
            c.iter_mut().for_each(|x| *x = 0.0);
        }
    }
    for (j, c) in cols.iter().enumerate() {
        for i in 0..v {
            y[i * l + j] = c[i] as f32;
        }
    }
}

/// Eigen-decomposition of a symmetric matrix (cyclic Jacobi).  Returns (values, vectors
/// as columns of a row-major l×l matrix), sorted by decreasing value.
fn sym_eigen(mut a: Vec<f64>, l: usize) -> (Vec<f64>, Vec<f64>) {
    let mut vecs = vec![0f64; l * l];
    for i in 0..l {
        vecs[i * l + i] = 1.0;
    }
    for _sweep in 0..60 {
        let off: f64 = (0..l).flat_map(|i| (0..l).filter(move |&j| j != i).map(move |j| (i, j))).map(|(i, j)| a[i * l + j].powi(2)).sum();
        if off < 1e-18 {
            break;
        }
        for p in 0..l {
            for q in p + 1..l {
                let apq = a[p * l + q];
                if apq.abs() < 1e-30 {
                    continue;
                }
                let theta = (a[q * l + q] - a[p * l + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..l {
                    let (akp, akq) = (a[k * l + p], a[k * l + q]);
                    a[k * l + p] = c * akp - s * akq;
                    a[k * l + q] = s * akp + c * akq;
                }
                for k in 0..l {
                    let (apk, aqk) = (a[p * l + k], a[q * l + k]);
                    a[p * l + k] = c * apk - s * aqk;
                    a[q * l + k] = s * apk + c * aqk;
                }
                for k in 0..l {
                    let (vkp, vkq) = (vecs[k * l + p], vecs[k * l + q]);
                    vecs[k * l + p] = c * vkp - s * vkq;
                    vecs[k * l + q] = s * vkp + c * vkq;
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..l).collect();
    order.sort_by(|&i, &j| a[j * l + j].total_cmp(&a[i * l + i]));
    let vals = order.iter().map(|&i| a[i * l + i]).collect();
    let mut sorted = vec![0f64; l * l];
    for (new, &old) in order.iter().enumerate() {
        for k in 0..l {
            sorted[k * l + new] = vecs[k * l + old];
        }
    }
    (vals, sorted)
}

/// Top-`k` right singular vectors of the sparse matrix (v×k, row-major).
fn randomized_svd(rows: &[Row], v: usize, k: usize, seed: u64) -> Vec<f32> {
    let l = (k + LSI_OVERSAMPLE).min(v.max(1));
    let mut st = seed ^ 0x5EED;
    let omega: Vec<f32> = (0..v * l).map(|_| rand_unit(&mut st)).collect();
    // range of AᵀA via one power iteration: Q = orth(AᵀA Ω), then again
    let mut q = sparse_t_times(rows, &sparse_times(rows, &omega, l), v, l);
    orthonormalize(&mut q, v, l);
    q = sparse_t_times(rows, &sparse_times(rows, &q, l), v, l);
    orthonormalize(&mut q, v, l);
    // M = Qᵀ AᵀA Q = (AQ)ᵀ(AQ), l×l
    let z = sparse_times(rows, &q, l);
    let m: Vec<f64> = (0..l * l)
        .into_par_iter()
        .map(|ij| {
            let (i, j) = (ij / l, ij % l);
            z.chunks_exact(l).map(|r| r[i] as f64 * r[j] as f64).sum()
        })
        .collect();
    let (_, w) = sym_eigen(m, l);
    // P = Q W[:, :k]
    let mut p = vec![0f32; v * k];
    p.par_chunks_mut(k).enumerate().for_each(|(t, row)| {
        let qt = &q[t * l..(t + 1) * l];
        for (c, out) in row.iter_mut().enumerate() {
            *out = (0..l).map(|i| qt[i] as f64 * w[i * l + c]).sum::<f64>() as f32;
        }
    });
    p
}

// ---------------------------------------------------------------- neural

#[cfg(feature = "neural")]
pub mod neural {
    use atlas_core::{Error, Result};
    use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub const DEFAULT_MODEL: &str = "multilingual-e5-small";
    const MAX_TOKENS: usize = 128;
    const BATCH: usize = 64;

    pub fn model_of(name: &str) -> Option<(EmbeddingModel, &'static str, &'static str)> {
        match name {
            "multilingual-e5-small" => Some((EmbeddingModel::MultilingualE5Small, "query: ", "passage: ")),
            "paraphrase-multilingual-minilm-l12-v2-q" => Some((EmbeddingModel::ParaphraseMLMiniLML12V2Q, "", "")),
            "paraphrase-multilingual-minilm-l12-v2" => Some((EmbeddingModel::ParaphraseMLMiniLML12V2, "", "")),
            _ => None,
        }
    }

    /// Model cache: `ATLAS_MODEL_DIR`, else `~/.cache/atlas-models` (outside OneDrive).
    pub fn cache_dir() -> PathBuf {
        if let Some(d) = std::env::var_os("ATLAS_MODEL_DIR") {
            return PathBuf::from(d);
        }
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default();
        PathBuf::from(home).join(".cache").join("atlas-models")
    }

    pub struct Neural {
        pub name: String,
        sessions: Vec<Mutex<TextEmbedding>>,
        next: AtomicUsize,
        query_prefix: &'static str,
        passage_prefix: &'static str,
    }

    impl Neural {
        /// `sessions` ONNX sessions with `intra` threads each.
        pub fn load(name: &str, sessions: usize, intra: usize) -> Result<Neural> {
            let (model, qp, pp) =
                model_of(name).ok_or_else(|| Error::BadRequest(format!("unknown embedding model {name}")))?;
            let mk = || {
                TextEmbedding::try_new(
                    TextInitOptions::new(model.clone())
                        .with_max_length(MAX_TOKENS)
                        .with_intra_threads(intra.max(1))
                        .with_cache_dir(cache_dir())
                        .with_show_download_progress(false),
                )
                .map(Mutex::new)
                .map_err(|e| Error::Registry(format!("embedding model {name}: {e}")))
            };
            let sessions = (0..sessions.max(1)).map(|_| mk()).collect::<Result<Vec<_>>>()?;
            Ok(Neural { name: name.to_string(), sessions, next: AtomicUsize::new(0), query_prefix: qp, passage_prefix: pp })
        }

        fn run(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            let start = self.next.fetch_add(1, Ordering::Relaxed);
            let n = self.sessions.len();
            // a free session if there is one, else wait for ours
            let mut guard = (0..n).find_map(|i| self.sessions[(start + i) % n].try_lock().ok());
            if guard.is_none() {
                guard = Some(self.sessions[start % n].lock().map_err(|_| Error::Registry("embedding session poisoned".into()))?);
            }
            let mut out = guard
                .expect("set above")
                .embed(texts, Some(BATCH))
                .map_err(|e| Error::Registry(format!("embedding: {e}")))?;
            out.iter_mut().for_each(|v| super::normalize(v));
            Ok(out)
        }

        pub fn encode_query(&self, q: &str) -> Result<Vec<f32>> {
            Ok(self.run(&[format!("{}{q}", self.query_prefix)])?.pop().unwrap_or_default())
        }

        /// Encode documents, batches spread over all sessions in parallel.  Texts are
        /// batched by length (fewer padding tokens), results come back in input order.
        pub fn encode_docs(&self, texts: &[String], progress: &(dyn Fn(usize) + Sync)) -> Result<Vec<Vec<f32>>> {
            let mut order: Vec<usize> = (0..texts.len()).collect();
            order.sort_by_key(|&i| texts[i].len());
            let prefixed: Vec<String> = order.iter().map(|&i| format!("{}{}", self.passage_prefix, texts[i])).collect();
            let chunks: Vec<&[String]> = prefixed.chunks(BATCH * 4).collect();
            let results: Vec<OnceLock<Vec<Vec<f32>>>> = (0..chunks.len()).map(|_| OnceLock::new()).collect();
            let (next, done) = (AtomicUsize::new(0), AtomicUsize::new(0));
            let err: Mutex<Option<Error>> = Mutex::new(None);
            std::thread::scope(|s| {
                for _ in 0..self.sessions.len() {
                    s.spawn(|| {
                        loop {
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            if i >= chunks.len() || err.lock().map(|e| e.is_some()).unwrap_or(true) {
                                break;
                            }
                            match self.run(chunks[i]) {
                                Ok(v) => {
                                    let _ = results[i].set(v);
                                    progress(done.fetch_add(chunks[i].len(), Ordering::Relaxed) + chunks[i].len());
                                }
                                Err(e) => {
                                    *err.lock().expect("err") = Some(e);
                                    break;
                                }
                            }
                        }
                    });
                }
            });
            if let Some(e) = err.into_inner().expect("err") {
                return Err(e);
            }
            let sorted: Vec<Vec<f32>> = results.into_iter().flat_map(|r| r.into_inner().unwrap_or_default()).collect();
            if sorted.len() != texts.len() {
                return Err(Error::Registry(format!("embedding returned {} of {} vectors", sorted.len(), texts.len())));
            }
            let mut out = vec![Vec::new(); texts.len()];
            for (v, &i) in sorted.into_iter().zip(&order) {
                out[i] = v;
            }
            Ok(out)
        }
    }
}

// ---------------------------------------------------------------- choice

/// The encoder of one index.  `name()` is stored in the index metadata and keys the
/// embedding cache, so vectors of different encoders never mix.
pub enum Encoder {
    Lsi(Lsi),
    #[cfg(feature = "neural")]
    Neural(neural::Neural),
}

impl Encoder {
    pub fn name(&self) -> String {
        match self {
            Encoder::Lsi(l) => format!("lsi-tfidf-svd-{}", l.dim()),
            #[cfg(feature = "neural")]
            Encoder::Neural(n) => n.name.clone(),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Encoder::Lsi(_) => "lsi",
            #[cfg(feature = "neural")]
            Encoder::Neural(_) => "neural",
        }
    }

    pub fn encode_query(&self, q: &str) -> Result<Vec<f32>> {
        match self {
            Encoder::Lsi(l) => Ok(l.encode_query(q)),
            #[cfg(feature = "neural")]
            Encoder::Neural(n) => n.encode_query(q),
        }
    }

    pub fn encode_docs(&self, texts: &[String], progress: &(dyn Fn(usize) + Sync)) -> Result<Vec<Vec<f32>>> {
        match self {
            Encoder::Lsi(l) => {
                let v = l.encode(texts);
                progress(v.len());
                Ok(v)
            }
            #[cfg(feature = "neural")]
            Encoder::Neural(n) => n.encode_docs(texts, progress),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanize_splits_identifiers_only() {
        assert_eq!(humanize("Nat.Prime.two_le"), "Nat Prime two le");
        assert_eq!(humanize("Teorema de Pitágoras"), "Teorema de Pitágoras");
        assert_eq!(humanize("group"), "group");
    }

    #[test]
    fn jacobi_recovers_eigenvalues() {
        // [[2,1],[1,2]] -> 3, 1
        let (vals, vecs) = sym_eigen(vec![2.0, 1.0, 1.0, 2.0], 2);
        assert!((vals[0] - 3.0).abs() < 1e-9 && (vals[1] - 1.0).abs() < 1e-9);
        assert!((vecs[0].abs() - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-9);
    }

    #[test]
    fn lsi_maps_cooccurring_words_together_and_roundtrips() {
        let mut texts = Vec::new();
        for i in 0..60 {
            texts.push(format!("triangle hypotenuse right angle squares {i}"));
            texts.push(format!("group homomorphism kernel subgroup quotient {i}"));
            texts.push(format!("integral derivative calculus limit continuous {i}"));
        }
        let lsi = Lsi::fit(&texts, 8, 7);
        let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        let tri = lsi.encode_query("hypotenuse");
        let tri2 = lsi.encode_query("right triangle");
        let grp = lsi.encode_query("kernel quotient");
        assert!(dot(&tri, &tri2) > 0.9, "{}", dot(&tri, &tri2));
        assert!(dot(&tri, &grp) < 0.3, "{}", dot(&tri, &grp));
        let dir = std::env::temp_dir().join(format!("atlas-lsi-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("lsi.bin");
        lsi.save(&p).unwrap();
        let back = Lsi::load(&p).unwrap();
        assert_eq!(back.encode_query("hypotenuse"), tri);
        std::fs::remove_dir_all(&dir).ok();
    }
}
