//! Layout and tile files.
//!
//! `<dir>/layout-<version>.lay`:
//!   magic "ATLSLAY1" | u64 meta length | meta JSON (`LayoutMeta`) | zero pad to 8 |
//!   positions [f32; 2] × n | id hash u64 × n | per level: community u32 × n |
//!   per level: size u32 × count, centroid [f32; 2] × count, central members [u32; 3] × count
//! The meta records the BLAKE3 hash of everything after the padding; `open` checks it.
//!
//! `<dir>/tiles-<version>.til`:
//!   magic "ATLSTIL1" | u64 meta length | meta JSON (`TileMeta`) | zero pad to 8 |
//!   index [u64; 3] × count (key, offset, length; sorted by key) | tile blobs
//! The tile file is memory-mapped; tiles are read in place.
//!
//! `<dir>/CURRENT` names the current version; it is replaced last (write, then
//! rename), so a reader never sees a half-written pair.

use atlas_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::tiles::{Bytes, TileMeta, TileSet};
use crate::{Layout, LayoutStats, LevelInfo};

const LAYOUT_MAGIC: &[u8; 8] = b"ATLSLAY1";
const TILES_MAGIC: &[u8; 8] = b"ATLSTIL1";

#[derive(Serialize, Deserialize)]
struct LayoutMeta {
    version: String,
    snapshot_hash: String,
    nodes: usize,
    affine: [f64; 3],
    levels: Vec<LevelInfo>,
    counts: Vec<usize>,
    stats: LayoutStats,
    payload_blake3: String,
}

fn io(p: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |e| Error::io(p, e)
}

fn header(magic: &[u8; 8], meta: &[u8]) -> Vec<u8> {
    let mut h = Vec::with_capacity(16 + meta.len() + 8);
    h.extend_from_slice(magic);
    h.extend_from_slice(&(meta.len() as u64).to_le_bytes());
    h.extend_from_slice(meta);
    while h.len() % 8 != 0 {
        h.push(0);
    }
    h
}

/// Split a file into (meta JSON, payload) after checking the magic.
fn split<'a>(bytes: &'a [u8], magic: &[u8; 8], p: &Path) -> Result<(&'a [u8], usize)> {
    let bad = |m: &str| Error::Format(format!("{}: {m}", p.display()));
    if bytes.len() < 16 || &bytes[..8] != magic {
        return Err(bad("not an atlas layout/tile file"));
    }
    let len = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    let end = 16usize
        .checked_add(len)
        .filter(|&e| e <= bytes.len())
        .ok_or_else(|| bad("truncated meta"))?;
    Ok((&bytes[16..end], end.div_ceil(8) * 8))
}

pub(crate) fn layout_path(dir: &Path, version: &str) -> PathBuf {
    dir.join(format!("layout-{version}.lay"))
}

pub(crate) fn tiles_path(dir: &Path, version: &str) -> PathBuf {
    dir.join(format!("tiles-{version}.til"))
}

pub(crate) fn write(layout: &Layout, dir: &Path) -> Result<(PathBuf, PathBuf)> {
    fs::create_dir_all(dir).map_err(io(dir))?;
    let mut payload: Vec<u8> = Vec::new();
    payload.extend_from_slice(bytemuck::cast_slice(&layout.positions));
    payload.extend_from_slice(bytemuck::cast_slice(&layout.id_hash));
    for m in &layout.levels {
        payload.extend_from_slice(bytemuck::cast_slice(m));
    }
    for c in &layout.communities {
        payload.extend_from_slice(bytemuck::cast_slice(&c.sizes));
        payload.extend_from_slice(bytemuck::cast_slice(&c.centroids));
        payload.extend_from_slice(bytemuck::cast_slice(&c.central));
    }
    let meta = LayoutMeta {
        version: layout.version.clone(),
        snapshot_hash: layout.snapshot_hash.clone(),
        nodes: layout.positions.len(),
        affine: layout.affine,
        levels: layout.communities.iter().map(LevelInfo::summary).collect(),
        counts: layout.communities.iter().map(|c| c.count).collect(),
        stats: layout.stats.clone(),
        payload_blake3: blake3::hash(&payload).to_hex().to_string(),
    };
    let lp = layout_path(dir, &layout.version);
    let tp = tiles_path(dir, &layout.version);
    // Same version = same content (the build is deterministic); a server may have it mapped.
    if lp.exists() && tp.exists() {
        return set_current(dir, &layout.version).map(|_| (lp, tp));
    }
    let meta_json = serde_json::to_vec(&meta).expect("meta serializes");
    write_file(&lp, &[&header(LAYOUT_MAGIC, &meta_json), &payload])?;

    let ts = &layout.tiles;
    let tmeta = serde_json::to_vec(&ts.meta).expect("tile meta serializes");
    write_file(
        &tp,
        &[
            &header(TILES_MAGIC, &tmeta),
            bytemuck::cast_slice(&ts.index),
            ts.blob.as_slice(),
        ],
    )?;

    set_current(dir, &layout.version)?;
    Ok((lp, tp))
}

fn set_current(dir: &Path, version: &str) -> Result<()> {
    let cur = dir.join("CURRENT");
    let tmp = dir.join("CURRENT.tmp");
    fs::write(&tmp, version).map_err(io(&tmp))?;
    fs::rename(&tmp, &cur).map_err(io(&cur))
}

fn write_file(p: &Path, parts: &[&[u8]]) -> Result<()> {
    let tmp = p.with_extension("tmp");
    let mut f =
        std::io::BufWriter::with_capacity(1 << 20, fs::File::create(&tmp).map_err(io(&tmp))?);
    for part in parts {
        f.write_all(part).map_err(io(&tmp))?;
    }
    f.into_inner()
        .map_err(|e| Error::io(&tmp, e.into_error()))?
        .sync_all()
        .map_err(io(&tmp))?;
    fs::rename(&tmp, p).map_err(io(p))
}

pub(crate) fn current(dir: &Path) -> Result<Option<String>> {
    let cur = dir.join("CURRENT");
    match fs::read_to_string(&cur) {
        Ok(s) => Ok(Some(s.trim().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(cur, e)),
    }
}

struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
    p: &'a Path,
}

impl Cursor<'_> {
    fn take<T: bytemuck::Pod>(&mut self, count: usize) -> Result<Vec<T>> {
        let len = count * std::mem::size_of::<T>();
        let s = self
            .b
            .get(self.at..self.at + len)
            .ok_or_else(|| Error::Format(format!("{}: payload truncated", self.p.display())))?;
        self.at += len;
        Ok(bytemuck::pod_collect_to_vec(s))
    }
}

/// Read a layout written by `write` (without its tiles).
pub(crate) fn read_layout(dir: &Path, version: &str) -> Result<Layout> {
    let p = layout_path(dir, version);
    let bytes = fs::read(&p).map_err(io(&p))?;
    let (meta, start) = split(&bytes, LAYOUT_MAGIC, &p)?;
    let meta: LayoutMeta = serde_json::from_slice(meta)
        .map_err(|e| Error::Format(format!("{}: meta: {e}", p.display())))?;
    let payload = &bytes[start..];
    if blake3::hash(payload).to_hex().as_str() != meta.payload_blake3 {
        return Err(Error::Integrity(format!(
            "{}: payload hash mismatch",
            p.display()
        )));
    }
    let n = meta.nodes;
    let mut c = Cursor {
        b: payload,
        at: 0,
        p: &p,
    };
    let positions = c.take::<[f32; 2]>(n)?;
    let id_hash = c.take::<u64>(n)?;
    let levels = meta
        .counts
        .iter()
        .map(|_| c.take::<u32>(n))
        .collect::<Result<Vec<_>>>()?;
    let mut communities = Vec::new();
    for (info, &count) in meta.levels.into_iter().zip(&meta.counts) {
        let sizes = c.take::<u32>(count)?;
        let centroids = c.take::<[f32; 2]>(count)?;
        let central = c.take::<[u32; 3]>(count)?;
        communities.push(LevelInfo {
            sizes,
            centroids,
            central,
            ..info
        });
    }
    for (l, m) in levels.iter().enumerate() {
        if m.iter().any(|&x| x as usize >= meta.counts[l]) {
            return Err(Error::Integrity(format!(
                "{}: community id out of range",
                p.display()
            )));
        }
    }
    Ok(Layout {
        version: meta.version,
        snapshot_hash: meta.snapshot_hash,
        positions,
        levels,
        communities,
        id_hash,
        affine: meta.affine,
        stats: meta.stats,
        tiles: TileSet {
            meta: TileMeta::default(),
            index: Vec::new(),
            blob: Bytes::Owned(Vec::new()),
        },
    })
}

pub(crate) fn read_tiles(dir: &Path, version: &str) -> Result<TileSet> {
    let p = tiles_path(dir, version);
    let f = fs::File::open(&p).map_err(io(&p))?;
    // SAFETY: the file is opened read-only and replaced only by rename, never written in place.
    let map = unsafe { memmap2::Mmap::map(&f) }.map_err(io(&p))?;
    let (meta, start) = split(&map, TILES_MAGIC, &p)?;
    let meta: TileMeta = serde_json::from_slice(meta)
        .map_err(|e| Error::Format(format!("{}: meta: {e}", p.display())))?;
    let ilen = meta.count * 24;
    let index_bytes = map
        .get(start..start + ilen)
        .ok_or_else(|| Error::Format(format!("{}: index truncated", p.display())))?;
    let index: Vec<[u64; 3]> = bytemuck::pod_collect_to_vec(index_bytes);
    let blob_len = (map.len() - start - ilen) as u64;
    if index.windows(2).any(|w| w[0][0] >= w[1][0]) || index.iter().any(|e| e[1] + e[2] > blob_len)
    {
        return Err(Error::Integrity(format!("{}: bad tile index", p.display())));
    }
    Ok(TileSet {
        meta,
        index,
        blob: Bytes::Mapped(map, start + ilen),
    })
}
