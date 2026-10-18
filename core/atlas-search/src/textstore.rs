//! Per-node texts on disk, for `matched` in hits and for the query-time match
//! explanation.  `texts.bin` (little-endian, memory-mapped):
//! `magic | num_nodes: u64 | offsets: [u64; num_nodes + 1] | bytes`
//! A node's record is a sequence of `field: u8 | lang_len: u8 | text_len: u32 | lang | text`.

use crate::corpus::{Doc, Field, Text};
use atlas_core::{Error, NodeIx, Result};
use memmap2::Mmap;
use std::io::{BufWriter, Write};
use std::path::Path;

const MAGIC: &[u8; 8] = b"ATLTXT01";
const HEADER: usize = 16;

pub fn write(docs: &[Doc], num_nodes: usize, path: &Path) -> Result<u64> {
    let io = |e| Error::io(path, e);
    let mut offsets = vec![0u64; num_nodes + 1];
    let mut bytes: Vec<u8> = Vec::new();
    let mut next = 0usize;
    for (n, off) in offsets.iter_mut().take(num_nodes).enumerate() {
        *off = bytes.len() as u64;
        if next < docs.len() && docs[next].node as usize == n {
            for t in &docs[next].texts {
                let lang = &t.lang.as_bytes()[..t.lang.len().min(255)];
                bytes.push(t.field.code());
                bytes.push(lang.len() as u8);
                bytes.extend_from_slice(&(t.text.len() as u32).to_le_bytes());
                bytes.extend_from_slice(lang);
                bytes.extend_from_slice(t.text.as_bytes());
            }
            next += 1;
        }
    }
    offsets[num_nodes] = bytes.len() as u64;
    let mut f = BufWriter::new(std::fs::File::create(path).map_err(io)?);
    f.write_all(MAGIC).map_err(io)?;
    f.write_all(&(num_nodes as u64).to_le_bytes()).map_err(io)?;
    f.write_all(bytemuck::cast_slice(&offsets)).map_err(io)?;
    f.write_all(&bytes).map_err(io)?;
    f.flush().map_err(io)?;
    Ok((HEADER + offsets.len() * 8 + bytes.len()) as u64)
}

pub struct TextStore {
    map: Mmap,
    num_nodes: usize,
}

impl TextStore {
    pub fn open(path: &Path) -> Result<TextStore> {
        let file = std::fs::File::open(path).map_err(|e| Error::io(path, e))?;
        // SAFETY: written once by `write`, never modified in place.
        let map = unsafe { Mmap::map(&file) }.map_err(|e| Error::io(path, e))?;
        if map.len() < HEADER || &map[..8] != MAGIC {
            return Err(Error::Format(format!("{}: not a text store", path.display())));
        }
        let num_nodes = u64::from_le_bytes(map[8..16].try_into().expect("8 bytes")) as usize;
        let store = TextStore { map, num_nodes };
        let end = HEADER + (num_nodes + 1) * 8;
        if store.map.len() < end {
            return Err(Error::Format(format!("{}: truncated offsets", path.display())));
        }
        let offs = store.offsets();
        if offs.windows(2).any(|w| w[0] > w[1]) || offs[num_nodes] as usize != store.map.len() - end {
            return Err(Error::Integrity(format!("{}: bad offsets", path.display())));
        }
        Ok(store)
    }

    fn offsets(&self) -> &[u64] {
        bytemuck::cast_slice(&self.map[HEADER..HEADER + (self.num_nodes + 1) * 8])
    }

    pub fn num_nodes(&self) -> usize {
        self.num_nodes
    }

    /// Texts of `n` in the order they were written; malformed records end the list.
    pub fn texts(&self, n: NodeIx) -> Vec<Text> {
        let n = n as usize;
        if n >= self.num_nodes {
            return Vec::new();
        }
        let base = HEADER + (self.num_nodes + 1) * 8;
        let offs = self.offsets();
        let rec = &self.map[base + offs[n] as usize..base + offs[n + 1] as usize];
        let mut out = Vec::new();
        let mut i = 0;
        while i + 6 <= rec.len() {
            let Some(field) = Field::from_code(rec[i]) else { break };
            let ll = rec[i + 1] as usize;
            let tl = u32::from_le_bytes(rec[i + 2..i + 6].try_into().expect("4 bytes")) as usize;
            let (a, b) = (i + 6, i + 6 + ll);
            if b + tl > rec.len() {
                break;
            }
            let (Ok(lang), Ok(text)) = (std::str::from_utf8(&rec[a..b]), std::str::from_utf8(&rec[b..b + tl])) else {
                break;
            };
            out.push(Text { field, lang: lang.to_string(), text: text.to_string() });
            i = b + tl;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_with_gaps() {
        let t = |field, lang: &str, text: &str| Text { field, lang: lang.into(), text: text.into() };
        let docs = vec![
            Doc { node: 1, texts: vec![t(Field::Label, "es", "Teorema de Pitágoras"), t(Field::Alias, "en", "x")] },
            Doc { node: 3, texts: vec![t(Field::Formula, "und", "a^2+b^2=c^2")] },
        ];
        let dir = std::env::temp_dir().join(format!("atlas-textstore-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("texts.bin");
        write(&docs, 4, &p).unwrap();
        let s = TextStore::open(&p).unwrap();
        assert!(s.texts(0).is_empty() && s.texts(2).is_empty() && s.texts(9).is_empty());
        assert_eq!(s.texts(1), docs[0].texts);
        assert_eq!(s.texts(3), docs[1].texts);
        std::fs::remove_dir_all(&dir).ok();
    }
}
