//! Column store assembled by `compile` and its serialization to one snapshot file.

use memmap2::MmapMut;
use rayon::prelude::*;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::format::{
    ENTRY_SIZE, FORMAT_VERSION, HASH_FIELD, HEADER_SIZE, Header, MAGIC, Section, SectionEntry, align_up,
};
use crate::strings::StrTable;
use crate::vocab::vocab_hash;

/// Every column of the snapshot, as built in memory.
#[derive(Default)]
pub(crate) struct Columns {
    pub strings: StrTable,
    pub node_id: Vec<u32>,
    pub node_type: Vec<u8>,
    pub node_canon: Vec<u32>,
    pub node_label_es: Vec<u32>,
    pub node_label_en: Vec<u32>,
    pub node_area: Vec<u32>,
    pub node_claim_count: Vec<u32>,
    pub node_state_mask: Vec<u8>,
    pub node_hash: Vec<u32>,
    pub merged_nodes: Vec<u32>,
    pub claim_id: Vec<u32>,
    pub claim_rel: Vec<u8>,
    pub claim_state: Vec<u8>,
    pub claim_subject: Vec<u32>,
    pub claim_group: Vec<u32>,
    pub claim_tgt_off: Vec<u32>,
    pub claim_tgt: Vec<u32>,
    pub claim_ctx_off: Vec<u32>,
    pub claim_ctx_node: Vec<u32>,
    pub claim_ctx_role: Vec<u8>,
    pub claim_hash: Vec<u32>,
    pub claim_asm_off: Vec<u32>,
    pub claim_asm: Vec<u32>,
    pub claim_validity: Vec<u32>,
    pub group_id: Vec<u32>,
    pub group_off: Vec<u32>,
    pub group_claims: Vec<u32>,
    pub prov_off: Vec<u32>,
    pub prov_src: Vec<u32>,
    pub prov_locator: Vec<u32>,
    pub prov_license: Vec<u32>,
    pub source_id: Vec<u32>,
    pub source_title: Vec<u32>,
    pub source_license: Vec<u32>,
    pub out_off: Vec<u32>,
    pub out_nbr: Vec<u32>,
    pub out_claim: Vec<u32>,
    pub out_rel: Vec<u8>,
    pub out_state: Vec<u8>,
    pub in_off: Vec<u32>,
    pub in_nbr: Vec<u32>,
    pub in_claim: Vec<u32>,
    pub in_rel: Vec<u8>,
    pub in_state: Vec<u8>,
}

impl Columns {
    pub fn num_nodes(&self) -> u32 {
        self.node_type.len() as u32
    }

    pub fn num_claims(&self) -> u32 {
        self.claim_rel.len() as u32
    }

    fn sections(&self) -> Vec<(Section, &[u8])> {
        use Section as S;
        fn b(v: &[u32]) -> &[u8] {
            bytemuck::cast_slice(v)
        }
        vec![
            (S::StrOffsets, b(&self.strings.offsets)),
            (S::StrBytes, &self.strings.bytes),
            (S::NodeId, b(&self.node_id)),
            (S::NodeType, &self.node_type),
            (S::NodeCanon, b(&self.node_canon)),
            (S::NodeLabelEs, b(&self.node_label_es)),
            (S::NodeLabelEn, b(&self.node_label_en)),
            (S::NodeArea, b(&self.node_area)),
            (S::NodeClaimCount, b(&self.node_claim_count)),
            (S::NodeStateMask, &self.node_state_mask),
            (S::NodeHash, b(&self.node_hash)),
            (S::MergedNodes, b(&self.merged_nodes)),
            (S::ClaimId, b(&self.claim_id)),
            (S::ClaimRel, &self.claim_rel),
            (S::ClaimState, &self.claim_state),
            (S::ClaimSubject, b(&self.claim_subject)),
            (S::ClaimGroup, b(&self.claim_group)),
            (S::ClaimTgtOff, b(&self.claim_tgt_off)),
            (S::ClaimTgt, b(&self.claim_tgt)),
            (S::ClaimCtxOff, b(&self.claim_ctx_off)),
            (S::ClaimCtxNode, b(&self.claim_ctx_node)),
            (S::ClaimCtxRole, &self.claim_ctx_role),
            (S::ClaimHash, b(&self.claim_hash)),
            (S::ClaimAsmOff, b(&self.claim_asm_off)),
            (S::ClaimAsm, b(&self.claim_asm)),
            (S::ClaimValidity, b(&self.claim_validity)),
            (S::GroupId, b(&self.group_id)),
            (S::GroupOff, b(&self.group_off)),
            (S::GroupClaims, b(&self.group_claims)),
            (S::ProvOff, b(&self.prov_off)),
            (S::ProvSource, b(&self.prov_src)),
            (S::ProvLocator, b(&self.prov_locator)),
            (S::ProvLicense, b(&self.prov_license)),
            (S::SourceId, b(&self.source_id)),
            (S::SourceTitle, b(&self.source_title)),
            (S::SourceLicense, b(&self.source_license)),
            (S::OutOff, b(&self.out_off)),
            (S::OutNbr, b(&self.out_nbr)),
            (S::OutClaim, b(&self.out_claim)),
            (S::OutRel, &self.out_rel),
            (S::OutState, &self.out_state),
            (S::InOff, b(&self.in_off)),
            (S::InNbr, b(&self.in_nbr)),
            (S::InClaim, b(&self.in_claim)),
            (S::InRel, &self.in_rel),
            (S::InState, &self.in_state),
        ]
    }

    fn header(&self, schema_version: u32, source_event_seq: u64, num_sections: u32) -> Header {
        let mut h: Header = bytemuck::Zeroable::zeroed();
        h.magic = MAGIC;
        h.format_version = FORMAT_VERSION;
        h.schema_version = schema_version;
        h.vocab_hash = vocab_hash();
        h.source_event_seq = source_event_seq;
        h.num_sections = num_sections;
        h.num_nodes = self.num_nodes();
        h.num_claims = self.num_claims();
        h.num_edges = self.out_nbr.len() as u32;
        h.num_groups = self.group_id.len() as u32;
        h.num_sources = self.source_id.len() as u32;
        h.num_strings = self.strings.len() as u32;
        h.num_prov = self.prov_src.len() as u32;
        h
    }
}

pub(crate) struct Written {
    pub path: PathBuf,
    pub hash_hex: String,
    pub bytes: u64,
}


/// Write `atlas-<hash16>.snap` into `dir` (temp file + rename), then point `CURRENT` at it.
pub(crate) fn write_snapshot(dir: &Path, cols: &Columns, schema: u32, seq: u64) -> Result<Written> {
    fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let sections = cols.sections();
    debug_assert_eq!(sections.len(), Section::COUNT);
    let mut pos = (HEADER_SIZE + ENTRY_SIZE * sections.len()) as u64;
    let mut table = Vec::with_capacity(sections.len());
    for (s, data) in &sections {
        pos = align_up(pos);
        table.push(SectionEntry { kind: s.code(), elem_size: s.elem_size(), offset: pos, len: data.len() as u64 });
        pos += data.len() as u64;
    }
    let header = cols.header(schema, seq, sections.len() as u32);

    let tmp = dir.join(format!("atlas.tmp-{}.snap", std::process::id()));
    let io = |e| Error::io(&tmp, e);
    let file = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&tmp).map_err(io)?;
    file.set_len(pos).map_err(io)?; // zero-filled: padding needs no writes
    // SAFETY: the temp file is private to this process until it is renamed below.
    let mut map = unsafe { MmapMut::map_mut(&file) }.map_err(io)?;
    let table_end = HEADER_SIZE + ENTRY_SIZE * sections.len();
    map[..HEADER_SIZE].copy_from_slice(bytemuck::bytes_of(&header));
    map[HEADER_SIZE..table_end].copy_from_slice(bytemuck::cast_slice(&table));
    let mut jobs: Vec<(&mut [u8], &[u8])> = Vec::with_capacity(sections.len());
    let mut rest: &mut [u8] = &mut map[..];
    let mut at = 0u64;
    for ((_, data), entry) in sections.iter().zip(&table) {
        let (_, tail) = rest.split_at_mut((entry.offset - at) as usize);
        let (dst, tail) = tail.split_at_mut(data.len());
        jobs.push((dst, data));
        rest = tail;
        at = entry.offset + entry.len;
    }
    jobs.into_par_iter().for_each(|(dst, src)| dst.copy_from_slice(src));
    let hash = blake3::Hasher::new().update_rayon(&map).finalize(); // hash field is still zero
    map[HASH_FIELD].copy_from_slice(hash.as_bytes());
    drop(map);
    drop(file);

    let hex = hash.to_hex().to_string();
    let name = format!("atlas-{}.snap", &hex[..16]);
    let path = dir.join(&name);
    if path.exists() {
        fs::remove_file(&tmp).map_err(io)?; // identical content already published
    } else {
        fs::rename(&tmp, &path).map_err(|e| Error::io(&path, e))?;
    }
    publish_current(dir, &name)?;
    Ok(Written { path, hash_hex: hex, bytes: pos })
}

fn publish_current(dir: &Path, name: &str) -> Result<()> {
    let tmp = dir.join("CURRENT.tmp");
    fs::write(&tmp, format!("{name}\n")).map_err(|e| Error::io(&tmp, e))?;
    let cur = dir.join("CURRENT");
    fs::rename(&tmp, &cur).map_err(|e| Error::io(&cur, e))?;
    // Older snapshots go; one still mapped by a running process (Windows) stays until next time.
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let f = e.file_name();
            let f = f.to_string_lossy();
            if f.starts_with("atlas-") && f.ends_with(".snap") && f != name {
                let _ = fs::remove_file(e.path());
            }
        }
    }
    Ok(())
}
