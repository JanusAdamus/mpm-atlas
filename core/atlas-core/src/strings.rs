//! String buffers used while compiling: chunk-local `StrVec`s are filled in
//! parallel, then appended (memcpy + offset shift) into one global `StrTable`.

use ahash::RandomState;
use std::collections::HashMap;

use crate::error::{Error, Result};

/// Append-only list of strings in one byte buffer.
#[derive(Default, Debug)]
pub(crate) struct StrVec {
    bytes: Vec<u8>,
    ends: Vec<u32>,
}

impl StrVec {
    pub fn push(&mut self, s: &str) -> u32 {
        self.bytes.extend_from_slice(s.as_bytes());
        self.ends.push(self.bytes.len() as u32);
        (self.ends.len() - 1) as u32
    }

    pub fn len(&self) -> usize {
        self.ends.len()
    }

    pub fn get(&self, i: usize) -> &str {
        let start = if i == 0 { 0 } else { self.ends[i - 1] as usize };
        // only ever filled from &str, so the slice is valid UTF-8
        std::str::from_utf8(&self.bytes[start..self.ends[i] as usize]).expect("utf-8 by construction")
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        (0..self.len()).map(|i| self.get(i))
    }
}

/// `StrVec` that returns the existing id for a repeated string.
#[derive(Default, Debug)]
pub(crate) struct DedupStrVec {
    pub vec: StrVec,
    map: HashMap<String, u32, RandomState>,
}

impl DedupStrVec {
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.map.get(s) {
            return i;
        }
        let i = self.vec.push(s);
        self.map.insert(s.to_owned(), i);
        i
    }
}

/// The snapshot string table: `offsets` has n+1 entries.
pub(crate) struct StrTable {
    pub offsets: Vec<u32>,
    pub bytes: Vec<u8>,
    dedup: HashMap<String, u32, RandomState>,
}

impl Default for StrTable {
    fn default() -> Self {
        StrTable::with_capacity(0, 0)
    }
}

impl StrTable {
    pub fn with_capacity(strings: usize, bytes: usize) -> Self {
        let mut offsets = Vec::with_capacity(strings + 1);
        offsets.push(0);
        StrTable { offsets, bytes: Vec::with_capacity(bytes), dedup: HashMap::default() }
    }

    pub fn len(&self) -> usize {
        self.offsets.len() - 1
    }

    fn check(&self) -> Result<()> {
        if self.bytes.len() > u32::MAX as usize || self.offsets.len() > u32::MAX as usize {
            return Err(Error::Registry("string table exceeds 4 GiB / 2^32 strings (format v1 limit)".into()));
        }
        Ok(())
    }

    pub fn push(&mut self, s: &str) -> Result<u32> {
        self.bytes.extend_from_slice(s.as_bytes());
        self.check()?;
        self.offsets.push(self.bytes.len() as u32);
        Ok((self.offsets.len() - 2) as u32)
    }

    /// Push every string of `v`; they get consecutive ids starting at the returned base.
    pub fn append(&mut self, v: &StrVec) -> Result<u32> {
        let base = self.len() as u32;
        let shift = self.bytes.len() as u64;
        if shift + v.bytes.len() as u64 > u32::MAX as u64 {
            return Err(Error::Registry("string table exceeds 4 GiB (format v1 limit)".into()));
        }
        self.bytes.extend_from_slice(&v.bytes);
        self.offsets.extend(v.ends.iter().map(|e| e + shift as u32));
        self.check()?;
        Ok(base)
    }

    /// Deduplicated push (for locators, licenses, assumptions: few distinct values).
    pub fn intern(&mut self, s: &str) -> Result<u32> {
        if let Some(&i) = self.dedup.get(s) {
            return Ok(i);
        }
        let i = self.push(s)?;
        self.dedup.insert(s.to_owned(), i);
        Ok(i)
    }

    pub fn get(&self, i: u32) -> &str {
        let (s, e) = (self.offsets[i as usize] as usize, self.offsets[i as usize + 1] as usize);
        std::str::from_utf8(&self.bytes[s..e]).expect("utf-8 by construction")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_shifts_offsets_and_intern_dedups() {
        let mut v = StrVec::default();
        v.push("ab");
        v.push("");
        v.push("ñ");
        let mut t = StrTable::with_capacity(4, 8);
        let first = t.intern("x").unwrap();
        let base = t.append(&v).unwrap();
        assert_eq!((first, base), (0, 1));
        assert_eq!([t.get(1), t.get(2), t.get(3)], ["ab", "", "ñ"]);
        assert_eq!(t.intern("x").unwrap(), 0);
        let mut d = DedupStrVec::default();
        assert_eq!((d.intern("a"), d.intern("b"), d.intern("a")), (0, 1, 0));
    }
}
