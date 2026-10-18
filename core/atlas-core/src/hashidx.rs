//! Id -> index hash table stored in the snapshot (sections NodeHash, ClaimHash).
//!
//! Layout: `REGIONS` equal regions of `size` u32 slots (`size` a power of two,
//! `table.len() == REGIONS * size`); `NONE` marks an empty slot.  An id goes to
//! region `key_hash(id) >> 58` and probes linearly from `key_hash(id) & (size - 1)`,
//! wrapping inside its region.  Each region is at most half full.  Regions are
//! built independently (in parallel) and each in index order, so the table is
//! deterministic.  `key_hash` is fixed here: the table is part of the file format.

use rayon::prelude::*;

use crate::format::NONE;

pub const REGIONS: usize = 64;
const REGION_SHIFT: u32 = 58; // 64 - log2(REGIONS)

/// FNV-1a 64 followed by the splitmix64 finalizer (FNV alone mixes low bits poorly,
/// and the slot index uses the low bits).
pub fn key_hash(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}

fn region(h: u64) -> usize {
    (h >> REGION_SHIFT) as usize
}

/// Table over keys `0..n`; `key(i)` must be distinct for distinct `i`.
pub(crate) fn build<'a>(n: usize, key: impl Fn(u32) -> &'a str + Sync) -> Vec<u32> {
    let hashes: Vec<u64> = (0..n as u32).into_par_iter().map(|i| key_hash(key(i).as_bytes())).collect();
    let mut start = vec![0usize; REGIONS + 1];
    for &h in &hashes {
        start[region(h) + 1] += 1;
    }
    let max = start.iter().copied().max().unwrap_or(0);
    let size = (2 * max).max(1).next_power_of_two();
    for r in 0..REGIONS {
        start[r + 1] += start[r];
    }
    let mut fill = start.clone();
    let mut members = vec![0u32; n];
    for (i, &h) in hashes.iter().enumerate() {
        let r = region(h);
        members[fill[r]] = i as u32;
        fill[r] += 1;
    }
    let mut table = vec![NONE; REGIONS * size];
    table.par_chunks_mut(size).enumerate().for_each(|(r, slots)| {
        for &i in &members[start[r]..start[r + 1]] {
            let mut s = hashes[i as usize] as usize & (size - 1);
            while slots[s] != NONE {
                s = (s + 1) & (size - 1);
            }
            slots[s] = i;
        }
    });
    table
}

/// Index whose key equals `id`.  At most one region is probed; a corrupt (full)
/// region ends after `size` probes instead of looping.
pub fn lookup<'a>(table: &[u32], id: &str, key: impl Fn(u32) -> &'a str) -> Option<u32> {
    let size = table.len() / REGIONS;
    if size == 0 {
        return None;
    }
    let h = key_hash(id.as_bytes());
    let slots = &table[region(h) * size..(region(h) + 1) * size];
    let mut s = h as usize & (size - 1);
    for _ in 0..size {
        match slots[s] {
            NONE => return None,
            v if key(v) == id => return Some(v),
            _ => s = (s + 1) & (size - 1),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_every_key_and_rejects_others() {
        let keys: Vec<String> = (0..5000).map(|i| format!("c:{i:x}")).collect();
        let t = build(keys.len(), |i| keys[i as usize].as_str());
        assert_eq!(t.len() % REGIONS, 0);
        assert!((t.len() / REGIONS).is_power_of_two());
        for (i, k) in keys.iter().enumerate() {
            assert_eq!(lookup(&t, k, |j| keys[j as usize].as_str()), Some(i as u32));
        }
        assert_eq!(lookup(&t, "c:missing", |j| keys[j as usize].as_str()), None);
        let empty = build(0, |_| "");
        assert_eq!(lookup(&empty, "x", |_| ""), None);
        assert_eq!(t, build(keys.len(), |i| keys[i as usize].as_str()), "deterministic");
    }
}
