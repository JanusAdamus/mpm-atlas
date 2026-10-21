//! Works layer tiles (ADR-0005 §8.4), computed on request: no pre-generated tile files.
//!
//! Position is deterministic and O(n): x = time (the work's mid year on an axis compressed toward
//! antiquity), y = band of the MSC top class of its area (one band per class, plus "unknown"), with
//! a hash jitter inside the band.  Points are stored sorted by their Morton code at `ZMAX`, so the
//! points of any tile z ≤ ZMAX are one contiguous range found by binary search.  A tile shows at
//! most `budget` points, ordered by in-degree of citations internal to the corpus (never the global
//! `cited_by_count`, which favours statistics); for z ≤ `ZPRE` that top list is precomputed (the
//! low-zoom tiles hold millions of points), deeper tiles select it from their range.  Works with no
//! date are not placed and are counted (`undated`).
//!
//! File `works-tiles-<layer hash 16>.bin` (little-endian, sections 8-byte aligned):
//! magic "AWT1" | version u32 | n u64 | undated u64 | zpre u32 | budget u32 | points: WorkPoint x n |
//! for z in 0..=zpre: off u32 x (4^z + 1) | idx u32 x off[last]

use atlas_core::{Error, Result};
use bytemuck::{Pod, Zeroable};
use memmap2::Mmap;
use rayon::prelude::*;
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MAGIC: &[u8; 4] = b"AWT1";
pub const ZMAX: u32 = 16;
pub const ZPRE: u32 = 7;
/// The per-tile budget of round 4 (docs/STATUS.md §15).
pub const BUDGET: u32 = 400;
pub const BANDS: u32 = 64; // the 63 MSC2020 top classes + unknown
const YEAR_MAX: f64 = 2030.0;
const YEAR_MIN: f64 = -3500.0;
const TIME_K: f64 = 30.0;
const MSC_CLASSES: [u8; 63] = [
    0, 1, 3, 5, 6, 8, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 22, 26, 28, 30, 31, 32, 33, 34, 35, 37, 39, 40, 41, 42, 43,
    44, 45, 46, 47, 49, 51, 52, 53, 54, 55, 57, 58, 60, 62, 65, 68, 70, 74, 76, 78, 80, 81, 82, 83, 85, 86, 90, 91, 92,
    93, 94, 97,
];

/// One placed work.  `band` = index into MSC_CLASSES, or BANDS-1 for unknown.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct WorkPoint {
    pub node: u32,
    pub x: f32,
    pub y: f32,
    pub lo: i32,
    pub hi: i32,
    pub indeg: u32,
    pub band: u32,
    pub morton: u32,
}

/// Input of `build`: a work of the layer with its year span (astronomical) and MSC top class.
pub struct WorkInput {
    pub node: u32,
    pub id_hash: u64,
    pub lo: Option<i32>,
    pub hi: Option<i32>,
    pub msc_class: Option<u8>,
    pub indeg: u32,
}

pub fn band_of(class: Option<u8>) -> u32 {
    class.and_then(|c| MSC_CLASSES.iter().position(|&k| k == c)).map_or(BANDS - 1, |i| i as u32)
}

/// Year (astronomical) -> x in [0, 1): logarithmic in the distance to `YEAR_MAX`, so 1900–2026 gets
/// as much width as the whole of antiquity.
pub fn time_x(year: f64) -> f32 {
    let y = year.clamp(YEAR_MIN, YEAR_MAX);
    let s = |v: f64| (1.0 + (YEAR_MAX - v) / TIME_K).ln();
    ((1.0 - s(y) / s(YEAR_MIN)) * 0.999_999) as f32
}

fn spread(v: u32) -> u32 {
    let mut x = v & 0xFFFF;
    x = (x | (x << 8)) & 0x00FF_00FF;
    x = (x | (x << 4)) & 0x0F0F_0F0F;
    x = (x | (x << 2)) & 0x3333_3333;
    (x | (x << 1)) & 0x5555_5555
}

/// Morton code of a tile (x, y) at zoom z (z bits each).
pub fn morton(tx: u32, ty: u32) -> u32 {
    spread(tx) | (spread(ty) << 1)
}

fn cell(v: f32, z: u32) -> u32 {
    ((v as f64 * f64::from(1u32 << z)) as u32).min((1 << z) - 1)
}

pub struct WorksTiles {
    map: Option<Mmap>,
    owned: Option<Vec<u8>>,
    n: usize,
    pub undated: u64,
    zpre: u32,
    pub budget: u32,
    points_at: usize,
    top_at: Vec<(usize, usize)>, // per z: (offsets byte pos, idx byte pos)
    pub path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct WorksTile {
    pub z: u32,
    pub x: u32,
    pub y: u32,
    /// Placed works whose position falls in the tile.
    pub in_tile: u32,
    /// Indexes of the shown points (at most `budget`), by internal in-degree, then node.
    #[serde(skip)]
    pub shown: Vec<WorkPoint>,
}

fn align8(v: &mut Vec<u8>) {
    v.resize(v.len().next_multiple_of(8), 0);
}

/// Points sorted by Morton code plus the precomputed top lists, as the file bytes.
pub fn build(inputs: &[WorkInput], budget: u32) -> Vec<u8> {
    let mut pts: Vec<WorkPoint> = inputs
        .par_iter()
        .filter_map(|w| {
            let (lo, hi) = match (w.lo, w.hi) {
                (Some(a), Some(b)) => (a, b),
                (Some(a), None) => (a, a),
                (None, Some(b)) => (b, b),
                (None, None) => return None,
            };
            let band = band_of(w.msc_class);
            let jitter = 0.1 + 0.8 * ((w.id_hash >> 11) as f64 / (1u64 << 53) as f64);
            let x = time_x((f64::from(lo) + f64::from(hi)) / 2.0);
            let y = ((f64::from(band) + jitter) / f64::from(BANDS)) as f32;
            let m = morton(cell(x, ZMAX), cell(y, ZMAX));
            Some(WorkPoint { node: w.node, x, y, lo, hi, indeg: w.indeg, band, morton: m })
        })
        .collect();
    let undated = (inputs.len() - pts.len()) as u64;
    pts.par_sort_unstable_by_key(|p| (p.morton, p.node));
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(pts.len() as u64).to_le_bytes());
    out.extend_from_slice(&undated.to_le_bytes());
    out.extend_from_slice(&ZPRE.to_le_bytes());
    out.extend_from_slice(&budget.to_le_bytes());
    align8(&mut out);
    out.extend_from_slice(bytemuck::cast_slice(&pts));
    // rank order: in-degree desc, node asc; fill each low-zoom tile up to the budget
    let mut order: Vec<u32> = (0..pts.len() as u32).collect();
    order.par_sort_unstable_by_key(|&i| (std::cmp::Reverse(pts[i as usize].indeg), pts[i as usize].node));
    for z in 0..=ZPRE {
        let tiles = 1usize << (2 * z);
        let mut lists: Vec<Vec<u32>> = vec![Vec::new(); tiles];
        for &i in &order {
            let t = (u64::from(pts[i as usize].morton) >> (2 * (ZMAX - z))) as usize;
            if lists[t].len() < budget as usize {
                lists[t].push(i);
            }
        }
        let mut off = Vec::with_capacity(tiles + 1);
        off.push(0u32);
        for l in &lists {
            off.push(off.last().copied().unwrap_or(0) + l.len() as u32);
        }
        out.extend_from_slice(bytemuck::cast_slice(&off));
        align8(&mut out);
        let idx: Vec<u32> = lists.into_iter().flatten().collect();
        out.extend_from_slice(bytemuck::cast_slice(&idx));
        align8(&mut out);
    }
    out
}

pub fn save(bytes: &[u8], dir: &Path, layer_hash16: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let path = dir.join(format!("works-tiles-{layer_hash16}.bin"));
    let tmp = path.with_extension("tmp");
    let mut f = File::create(&tmp).map_err(|e| Error::io(&tmp, e))?;
    f.write_all(bytes).map_err(|e| Error::io(&tmp, e))?;
    drop(f);
    std::fs::rename(&tmp, &path).map_err(|e| Error::io(&path, e))?;
    Ok(path)
}

impl WorksTiles {
    pub fn open(path: &Path) -> Result<WorksTiles> {
        let file = File::open(path).map_err(|e| Error::io(path, e))?;
        // SAFETY: written once under a fresh name by `save` (temp file + rename), never modified in place.
        let map = unsafe { Mmap::map(&file) }.map_err(|e| Error::io(path, e))?;
        let mut t = Self::parse(&map)?;
        t.map = Some(map);
        t.path = Some(path.to_path_buf());
        Ok(t)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<WorksTiles> {
        let mut t = Self::parse(&bytes)?;
        t.owned = Some(bytes);
        Ok(t)
    }

    fn parse(b: &[u8]) -> Result<WorksTiles> {
        let bad = |m: &str| Error::Format(format!("works tiles: {m}"));
        if b.len() < 32 || &b[..4] != MAGIC {
            return Err(bad("bad magic"));
        }
        let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().expect("4 bytes"));
        let u64_at = |o: usize| u64::from_le_bytes(b[o..o + 8].try_into().expect("8 bytes"));
        let n = u64_at(8) as usize;
        let (undated, zpre, budget) = (u64_at(16), u32_at(24), u32_at(28));
        let points_at = 32;
        let mut pos = points_at + n * std::mem::size_of::<WorkPoint>();
        let mut top_at = Vec::new();
        for z in 0..=zpre {
            let tiles = 1usize << (2 * z);
            let off_at = pos;
            pos = (pos + 4 * (tiles + 1)).next_multiple_of(8);
            if pos > b.len() {
                return Err(bad("truncated"));
            }
            let total = u32_at(off_at + 4 * tiles) as usize;
            top_at.push((off_at, pos));
            pos = (pos + 4 * total).next_multiple_of(8);
        }
        if pos != b.len() {
            return Err(bad("size mismatch"));
        }
        Ok(WorksTiles { map: None, owned: None, n, undated, zpre, budget, points_at, top_at, path: None })
    }

    fn bytes(&self) -> &[u8] {
        self.map.as_deref().or(self.owned.as_deref()).expect("loaded")
    }

    pub fn points(&self) -> &[WorkPoint] {
        bytemuck::cast_slice(&self.bytes()[self.points_at..self.points_at + self.n * std::mem::size_of::<WorkPoint>()])
    }

    fn u32s(&self, at: usize, len: usize) -> &[u32] {
        bytemuck::cast_slice(&self.bytes()[at..at + 4 * len])
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// The works of tile z/x/y (at most `budget`, by internal in-degree) and how many fall in it.
    pub fn tile(&self, z: u32, x: u32, y: u32) -> Result<WorksTile> {
        if z > ZMAX || x >= 1 << z || y >= 1 << z {
            return Err(Error::BadRequest(format!("works tile {z}/{x}/{y} outside 0 <= x, y < 2^z, z <= {ZMAX}")));
        }
        let pts = self.points();
        let shift = 2 * (ZMAX - z);
        let key = morton(x, y);
        let (lo, hi) = ((key as u64) << shift, ((key as u64) + 1) << shift);
        let a = pts.partition_point(|p| (p.morton as u64) < lo);
        let b = pts.partition_point(|p| (p.morton as u64) < hi);
        let shown: Vec<WorkPoint> = if z <= self.zpre {
            let (off_at, idx_at) = self.top_at[z as usize];
            let off = self.u32s(off_at, (1 << (2 * z)) + 1);
            let (s, e) = (off[key as usize] as usize, off[key as usize + 1] as usize);
            self.u32s(idx_at + 4 * s, e - s).iter().map(|&i| pts[i as usize]).collect()
        } else {
            let mut v: Vec<WorkPoint> = pts[a..b].to_vec();
            let k = self.budget as usize;
            let rank = |p: &WorkPoint| (std::cmp::Reverse(p.indeg), p.node);
            if v.len() > k {
                v.select_nth_unstable_by_key(k - 1, rank);
                v.truncate(k);
            }
            v.sort_unstable_by_key(rank);
            v
        };
        Ok(WorksTile { z, x, y, in_tile: (b - a) as u32, shown })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(node: u32, year: Option<i32>, class: Option<u8>, indeg: u32) -> WorkInput {
        WorkInput { node, id_hash: u64::from(node).wrapping_mul(0x9E37_79B9_7F4A_7C15), lo: year, hi: year, msc_class: class, indeg }
    }

    #[test]
    fn time_axis_is_monotone_and_compresses_antiquity() {
        assert!(time_x(-1800.0) < time_x(-300.0) && time_x(-300.0) < time_x(1801.0) && time_x(1801.0) < time_x(2020.0));
        assert!(time_x(2026.0) - time_x(1900.0) > time_x(0.0) - time_x(-3500.0), "modern decades get the width");
        assert!((0.0..1.0).contains(&time_x(-5000.0)) && (0.0..1.0).contains(&time_x(3000.0)));
    }

    #[test]
    fn tiles_partition_points_respect_budget_and_order_by_indegree() {
        let inputs: Vec<WorkInput> =
            (0..5000).map(|i| input(i, (i % 7 != 0).then_some(1800 + (i % 220) as i32), Some(30), i % 50)).collect();
        let t = WorksTiles::from_bytes(build(&inputs, 40)).unwrap();
        let undated = inputs.iter().filter(|w| w.lo.is_none()).count();
        assert_eq!((t.len(), t.undated as usize), (5000 - undated, undated));
        let root = t.tile(0, 0, 0).unwrap();
        assert_eq!(root.in_tile as usize, t.len());
        assert_eq!(root.shown.len(), 40);
        assert!(root.shown.windows(2).all(|w| w[0].indeg >= w[1].indeg));
        // the four children of the root hold every point once, at a precomputed and a scanned zoom
        for z in [1u32, ZPRE + 2] {
            let mut sum = 0;
            for x in 0..1u32 << z {
                for y in 0..1u32 << z {
                    let tl = t.tile(z, x, y).unwrap();
                    assert!(tl.shown.len() <= 40 && tl.shown.len() <= tl.in_tile as usize);
                    for p in &tl.shown {
                        assert_eq!((cell(p.x, z), cell(p.y, z)), (x, y));
                    }
                    sum += tl.in_tile;
                }
            }
            assert_eq!(sum as usize, t.len(), "zoom {z}");
        }
        assert!(t.tile(ZMAX + 1, 0, 0).is_err());
    }

    #[test]
    fn unknown_area_has_its_own_band() {
        assert_eq!(band_of(None), BANDS - 1);
        assert_eq!(band_of(Some(30)), MSC_CLASSES.iter().position(|&c| c == 30).unwrap() as u32);
        assert_eq!(band_of(Some(2)), BANDS - 1, "not an MSC class");
    }
}
