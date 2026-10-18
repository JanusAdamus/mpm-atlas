//! Semantic lane storage and nearest-neighbour search.
//!
//! `vectors.bin` (little-endian, memory-mapped, 64-byte aligned sections):
//! `magic | num_nodes: u64 | dim: u64 | scale: [f32; num_nodes] | codes: [i8; num_nodes * dim]`
//! One row per snapshot node; `scale == 0` means "no vector".  Rows are L2-normalised
//! vectors quantised symmetrically per row (`x ≈ code * scale`, max |code| = 127), so
//! `cos(q, d) ≈ scale_q * scale_d * Σ code_q code_d`.
//!
//! Two searchers over the same vectors, picked per index (`DenseMethod`):
//! - `Brute`: exact int8 scan, AVX2 when the CPU has it, split over all cores;
//! - `Hnsw`: usearch HNSW graph (`hnsw.usearch`, int8, cosine), memory-mapped.
//!
//! The embedding cache (`emb-<model>.cache`, next to the per-snapshot directories)
//! keeps quantised vectors keyed by the blake3 of the encoder input, so rebuilding for
//! a new snapshot only encodes texts that changed.

use crate::formula::top_k;
use ahash::AHashMap;
use atlas_core::{Error, NodeIx, Result};
use memmap2::Mmap;
use rayon::prelude::*;
use std::io::{BufWriter, Write};
use std::path::Path;

const MAGIC: &[u8; 8] = b"ATLVEC01";
const HEADER: usize = 64;
/// Rows scanned per parallel task in `Brute` (2048 x 384 B = 768 KiB: fits L2).
const SCAN_ROWS: usize = 2048;

pub const HNSW_CONNECTIVITY: usize = 16;
pub const HNSW_EXPANSION_ADD: usize = 128;
pub const HNSW_EXPANSION_SEARCH: usize = 96;

fn align64(x: usize) -> usize {
    x.div_ceil(64) * 64
}

/// Per-row symmetric int8 quantisation: (codes, scale).  A zero vector gets scale 0.
pub fn quantize(v: &[f32]) -> (Vec<i8>, f32) {
    let m = v.iter().fold(0f32, |a, x| a.max(x.abs()));
    if m == 0.0 || !m.is_finite() {
        return (vec![0; v.len()], 0.0);
    }
    let s = m / 127.0;
    (v.iter().map(|x| (x / s).round().clamp(-127.0, 127.0) as i8).collect(), s)
}

fn dot_scalar(a: &[i8], b: &[i8]) -> i32 {
    a.iter().zip(b).map(|(&x, &y)| x as i32 * y as i32).sum()
}

/// Same loop compiled with AVX2 enabled: LLVM vectorises it to `vpmaddwd`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn scan_avx2(q: &[i8], rows: &[i8], dim: usize, out: &mut [i32]) {
    for (o, r) in out.iter_mut().zip(rows.chunks_exact(dim)) {
        *o = r.iter().zip(q).map(|(&x, &y)| x as i32 * y as i32).sum();
    }
}

fn scan(q: &[i8], rows: &[i8], dim: usize, out: &mut [i32]) {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") {
        // SAFETY: the CPU supports AVX2 (checked just above).
        unsafe { scan_avx2(q, rows, dim, out) };
        return;
    }
    for (o, r) in out.iter_mut().zip(rows.chunks_exact(dim)) {
        *o = dot_scalar(q, r);
    }
}

/// Write `vectors.bin`: `rows[i]` is the (normalised) vector of `nodes[i]`.
pub fn write(nodes: &[NodeIx], codes: &[(Vec<i8>, f32)], num_nodes: usize, dim: usize, path: &Path) -> Result<u64> {
    let io = |e| Error::io(path, e);
    let mut scale = vec![0f32; num_nodes];
    let mut all = vec![0i8; num_nodes * dim];
    for (&n, (c, s)) in nodes.iter().zip(codes) {
        if c.len() != dim {
            return Err(Error::Format(format!("vector of node {n} has {} dims, expected {dim}", c.len())));
        }
        scale[n as usize] = *s;
        all[n as usize * dim..(n as usize + 1) * dim].copy_from_slice(c);
    }
    let codes_at = align64(HEADER + num_nodes * 4);
    let mut f = BufWriter::new(std::fs::File::create(path).map_err(io)?);
    let mut head = [0u8; HEADER];
    head[..8].copy_from_slice(MAGIC);
    head[8..16].copy_from_slice(&(num_nodes as u64).to_le_bytes());
    head[16..24].copy_from_slice(&(dim as u64).to_le_bytes());
    f.write_all(&head).map_err(io)?;
    f.write_all(bytemuck::cast_slice(&scale)).map_err(io)?;
    f.write_all(&vec![0u8; codes_at - HEADER - num_nodes * 4]).map_err(io)?;
    f.write_all(bytemuck::cast_slice(&all)).map_err(io)?;
    f.flush().map_err(io)?;
    Ok((codes_at + all.len()) as u64)
}

pub struct Vectors {
    map: Mmap,
    num_nodes: usize,
    dim: usize,
    codes_at: usize,
}

impl Vectors {
    pub fn open(path: &Path) -> Result<Vectors> {
        let file = std::fs::File::open(path).map_err(|e| Error::io(path, e))?;
        // SAFETY: written once by `write`, never modified in place.
        let map = unsafe { Mmap::map(&file) }.map_err(|e| Error::io(path, e))?;
        if map.len() < HEADER || &map[..8] != MAGIC {
            return Err(Error::Format(format!("{}: not a vector file", path.display())));
        }
        let rd = |i: usize| u64::from_le_bytes(map[8 + i * 8..16 + i * 8].try_into().expect("8 bytes")) as usize;
        let (num_nodes, dim) = (rd(0), rd(1));
        let codes_at = align64(HEADER + num_nodes * 4);
        if dim == 0 || map.len() != codes_at + num_nodes * dim {
            return Err(Error::Format(format!("{}: bad length", path.display())));
        }
        Ok(Vectors { map, num_nodes, dim, codes_at })
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    pub fn num_nodes(&self) -> usize {
        self.num_nodes
    }

    pub fn bytes(&self) -> usize {
        self.map.len()
    }

    pub fn scales(&self) -> &[f32] {
        bytemuck::cast_slice(&self.map[HEADER..HEADER + self.num_nodes * 4])
    }

    fn codes(&self) -> &[i8] {
        bytemuck::cast_slice(&self.map[self.codes_at..])
    }

    pub fn row(&self, n: NodeIx) -> &[i8] {
        &self.codes()[n as usize * self.dim..(n as usize + 1) * self.dim]
    }

    /// Dequantised row (for building the HNSW graph).
    pub fn row_f32(&self, n: NodeIx) -> Vec<f32> {
        let s = self.scales()[n as usize];
        self.row(n).iter().map(|&c| c as f32 * s).collect()
    }

    /// Exact top `k` by cosine over rows that have a vector and pass `keep`.
    /// `parallel` splits the scan over the rayon pool.
    pub fn brute(&self, q: &[f32], k: usize, parallel: bool, keep: &(dyn Fn(NodeIx) -> bool + Sync)) -> Vec<(NodeIx, f32)> {
        if k == 0 || q.len() != self.dim {
            return Vec::new();
        }
        let (qc, qs) = quantize(q);
        let scales = self.scales();
        let codes = self.codes();
        let block = |b: usize| -> Vec<(NodeIx, f32)> {
            let lo = b * SCAN_ROWS;
            let hi = (lo + SCAN_ROWS).min(self.num_nodes);
            let mut dots = vec![0i32; hi - lo];
            scan(&qc, &codes[lo * self.dim..hi * self.dim], self.dim, &mut dots);
            let mut v: Vec<(NodeIx, f32)> = Vec::with_capacity(k.min(hi - lo));
            let mut worst = f32::NEG_INFINITY;
            for (i, &d) in dots.iter().enumerate() {
                let n = (lo + i) as NodeIx;
                let s = scales[lo + i];
                if s == 0.0 {
                    continue;
                }
                let score = d as f32 * s * qs;
                if v.len() >= k && score <= worst {
                    continue;
                }
                if !keep(n) {
                    continue;
                }
                v.push((n, score));
                if v.len() >= 2 * k {
                    v = top_k(v, k);
                    worst = v.last().map_or(f32::NEG_INFINITY, |x| x.1);
                }
            }
            top_k(v, k)
        };
        let blocks = self.num_nodes.div_ceil(SCAN_ROWS);
        let all: Vec<(NodeIx, f32)> = if parallel {
            (0..blocks).into_par_iter().flat_map_iter(block).collect()
        } else {
            (0..blocks).flat_map(block).collect()
        };
        top_k(all, k)
    }
}

// ---------------------------------------------------------------- HNSW

pub struct Hnsw {
    index: usearch::Index,
}

fn us(e: impl std::fmt::Display) -> Error {
    Error::Format(format!("usearch: {e}"))
}

impl Hnsw {
    /// Build over every row of `vectors` that has a vector, with `threads` threads.
    pub fn build(vectors: &Vectors, path: &Path, threads: usize) -> Result<u64> {
        let opts = usearch::IndexOptions {
            dimensions: vectors.dim(),
            metric: usearch::MetricKind::Cos,
            quantization: usearch::ScalarKind::I8,
            connectivity: HNSW_CONNECTIVITY,
            expansion_add: HNSW_EXPANSION_ADD,
            expansion_search: HNSW_EXPANSION_SEARCH,
            multi: false,
        };
        let index = usearch::Index::new(&opts).map_err(us)?;
        let rows: Vec<NodeIx> =
            (0..vectors.num_nodes() as NodeIx).filter(|&n| vectors.scales()[n as usize] != 0.0).collect();
        let threads = threads.max(1);
        index.reserve_capacity_and_threads(rows.len(), threads).map_err(us)?;
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().map_err(|e| Error::Format(e.to_string()))?;
        pool.install(|| rows.par_iter().try_for_each(|&n| index.add(n as u64, &vectors.row_f32(n)).map_err(us)))?;
        let p = path.to_str().ok_or_else(|| Error::Format(format!("{}: path is not UTF-8", path.display())))?;
        index.save(p).map_err(us)?;
        Ok(std::fs::metadata(path).map_err(|e| Error::io(path, e))?.len())
    }

    /// Memory-map a saved graph.
    pub fn open(path: &Path) -> Result<Hnsw> {
        let p = path.to_str().ok_or_else(|| Error::Format(format!("{}: path is not UTF-8", path.display())))?;
        let index = usearch::Index::restore_view(p).map_err(us)?;
        index.change_expansion_search(HNSW_EXPANSION_SEARCH);
        Ok(Hnsw { index })
    }

    pub fn len(&self) -> usize {
        self.index.size()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn set_expansion(&self, ef: usize) {
        self.index.change_expansion_search(ef);
    }

    /// Approximate top `k` by cosine; `keep` is applied inside the graph search.
    pub fn search(&self, q: &[f32], k: usize, keep: Option<&(dyn Fn(NodeIx) -> bool + Sync)>) -> Result<Vec<(NodeIx, f32)>> {
        if k == 0 || self.is_empty() {
            return Ok(Vec::new());
        }
        let m = match keep {
            None => self.index.search(q, k),
            Some(f) => self.index.filtered_search(q, k, |key: u64| f(key as NodeIx)),
        }
        .map_err(us)?;
        Ok(m.keys.iter().zip(&m.distances).map(|(&key, &d)| (key as NodeIx, 1.0 - d)).collect())
    }
}

// ---------------------------------------------------------------- embedding cache

const CACHE_MAGIC: &[u8; 8] = b"ATLEMB01";

/// Quantised embeddings keyed by blake3(model name, encoder input).
pub struct EmbCache {
    dim: usize,
    index: AHashMap<[u8; 16], usize>,
    rows: Vec<(Vec<i8>, f32)>,
    keys: Vec<[u8; 16]>,
}

pub fn cache_key(model: &str, text: &str) -> [u8; 16] {
    let mut h = blake3::Hasher::new();
    h.update(model.as_bytes());
    h.update(&[0]);
    h.update(text.as_bytes());
    let mut k = [0u8; 16];
    k.copy_from_slice(&h.finalize().as_bytes()[..16]);
    k
}

impl EmbCache {
    pub fn new(dim: usize) -> EmbCache {
        EmbCache { dim, index: AHashMap::new(), rows: Vec::new(), keys: Vec::new() }
    }

    /// Load `path`; a missing, corrupt or other-dimension file gives an empty cache.
    pub fn load(path: &Path, dim: usize) -> EmbCache {
        let mut c = EmbCache::new(dim);
        let Ok(b) = std::fs::read(path) else { return c };
        if b.len() < 24 || &b[..8] != CACHE_MAGIC || u64::from_le_bytes(b[8..16].try_into().expect("8")) as usize != dim {
            return c;
        }
        let n = u64::from_le_bytes(b[16..24].try_into().expect("8")) as usize;
        let rec = 16 + 4 + dim;
        if b.len() != 24 + n * rec {
            return c;
        }
        for r in b[24..].chunks_exact(rec) {
            let key: [u8; 16] = r[..16].try_into().expect("16");
            let s = f32::from_le_bytes(r[16..20].try_into().expect("4"));
            c.insert(key, (bytemuck::cast_slice(&r[20..]).to_vec(), s));
        }
        c
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn get(&self, key: &[u8; 16]) -> Option<&(Vec<i8>, f32)> {
        self.index.get(key).map(|&i| &self.rows[i])
    }

    pub fn insert(&mut self, key: [u8; 16], row: (Vec<i8>, f32)) {
        if row.0.len() != self.dim || self.index.contains_key(&key) {
            return;
        }
        self.index.insert(key, self.rows.len());
        self.rows.push(row);
        self.keys.push(key);
    }

    /// Write atomically (temp file + rename).
    pub fn save(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("cache.tmp");
        let io = |e| Error::io(&tmp, e);
        let mut f = BufWriter::new(std::fs::File::create(&tmp).map_err(io)?);
        f.write_all(CACHE_MAGIC).map_err(io)?;
        f.write_all(&(self.dim as u64).to_le_bytes()).map_err(io)?;
        f.write_all(&(self.rows.len() as u64).to_le_bytes()).map_err(io)?;
        for (k, (c, s)) in self.keys.iter().zip(&self.rows) {
            f.write_all(k).map_err(io)?;
            f.write_all(&s.to_le_bytes()).map_err(io)?;
            f.write_all(bytemuck::cast_slice(c)).map_err(io)?;
        }
        f.flush().map_err(io)?;
        drop(f);
        std::fs::rename(&tmp, path).map_err(|e| Error::io(path, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(seed: u64, dim: usize) -> Vec<f32> {
        let mut s = seed;
        let mut v: Vec<f32> = (0..dim)
            .map(|_| {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((s >> 33) as f32 / (1u64 << 31) as f32) - 1.0
            })
            .collect();
        crate::encoder::normalize(&mut v);
        v
    }

    #[test]
    fn brute_and_hnsw_find_the_query_vector_itself() {
        let dir = std::env::temp_dir().join(format!("atlas-dense-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (n, dim) = (3000usize, 48usize);
        let vecs: Vec<Vec<f32>> = (0..n).map(|i| unit(i as u64 + 1, dim)).collect();
        // node 7 has no vector
        let nodes: Vec<NodeIx> = (0..n as NodeIx).filter(|&i| i != 7).collect();
        let codes: Vec<(Vec<i8>, f32)> = nodes.iter().map(|&i| quantize(&vecs[i as usize])).collect();
        write(&nodes, &codes, n, dim, &dir.join("v.bin")).unwrap();
        let v = Vectors::open(&dir.join("v.bin")).unwrap();
        for target in [0usize, 1234, 2999] {
            let hits = v.brute(&vecs[target], 5, true, &|_| true);
            assert_eq!(hits[0].0 as usize, target);
            assert!((hits[0].1 - 1.0).abs() < 0.02, "{}", hits[0].1);
            assert_eq!(v.brute(&vecs[target], 5, false, &|_| true), hits, "serial == parallel");
        }
        assert!(v.brute(&vecs[7], 3000, true, &|_| true).iter().all(|h| h.0 != 7));
        assert!(v.brute(&vecs[5], 10, true, &|n| n % 2 == 0).iter().all(|h| h.0 % 2 == 0));
        Hnsw::build(&v, &dir.join("h.usearch"), 4).unwrap();
        let h = Hnsw::open(&dir.join("h.usearch")).unwrap();
        assert_eq!(h.len(), n - 1);
        let hits = h.search(&vecs[1234], 5, None).unwrap();
        assert_eq!(hits[0].0, 1234);
        let odd = h.search(&vecs[1234], 5, Some(&|n| n % 2 == 1)).unwrap();
        assert!(!odd.is_empty() && odd.iter().all(|x| x.0 % 2 == 1));
        drop(h);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn embedding_cache_roundtrips_and_rejects_other_dims() {
        let dir = std::env::temp_dir().join(format!("atlas-embcache-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("emb.cache");
        let mut c = EmbCache::new(4);
        let k = cache_key("m", "hola");
        c.insert(k, (vec![1, -2, 3, 127], 0.5));
        c.insert(cache_key("m", "x"), (vec![1, 2], 0.5)); // wrong dim: ignored
        c.save(&p).unwrap();
        let back = EmbCache::load(&p, 4);
        assert_eq!(back.len(), 1);
        assert_eq!(back.get(&k), Some(&(vec![1, -2, 3, 127], 0.5)));
        assert!(EmbCache::load(&p, 8).is_empty());
        assert_ne!(cache_key("m", "hola"), cache_key("n", "hola"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
