//! Memory-mapped, zero-copy view of a snapshot file.
//!
//! `Snapshot::open` maps the file and checks the header and the section table
//! (O(number of sections)); every accessor then slices the map in place.  A
//! corrupted file can make accessors panic (bounds checks) but never causes
//! undefined behavior.  `verify()` recomputes the content hash and checks every
//! reference, for use after copying a snapshot or before serving it.

use bytemuck::Pod;
use memmap2::Mmap;
use rayon::prelude::*;
use std::fs::File;
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::format::{
    ENTRY_SIZE, FORMAT_VERSION, HASH_FIELD, HEADER_SIZE, Header, MAGIC, MAX_SECTION_CODE, NONE, SECTION_ALIGN,
    SUPPORTED_SCHEMA, Section, SectionEntry,
};
use crate::hashidx::{self, REGIONS};
use crate::vocab::{EntityType, EvidenceState, PRESENTATION_SCOPES, RelationType, Role, scope_bit, vocab_hash};

pub type NodeIx = u32;
pub type ClaimIx = u32;
pub type GroupIx = u32;
pub type SourceIx = u32;
pub type StrId = u32;

pub struct Snapshot {
    map: Mmap,
    path: PathBuf,
    header: Header,
    table: [(usize, usize); MAX_SECTION_CODE],
}

/// Edge direction in the CSR: `Out` is subject -> target, `In` is target -> subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    Out,
    In,
}

/// Raw typed CSR of one direction; slices point into the memory map.
#[derive(Clone, Copy)]
pub struct Csr<'a> {
    pub offsets: &'a [u32],
    pub nbr: &'a [u32],
    pub claim: &'a [u32],
    pub rel: &'a [u8],
    pub state: &'a [u8],
}

/// The edges of one node (optionally of one relation): parallel slices.
#[derive(Clone, Copy)]
pub struct EdgeSlice<'a> {
    pub nbr: &'a [u32],
    pub claim: &'a [u32],
    pub rel: &'a [u8],
    pub state: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeRef {
    pub nbr: NodeIx,
    pub claim: ClaimIx,
    pub relation: RelationType,
    pub state: EvidenceState,
}

impl<'a> Csr<'a> {
    pub fn num_nodes(&self) -> usize {
        self.offsets.len() - 1
    }

    pub fn range(&self, n: NodeIx) -> Range<usize> {
        self.offsets[n as usize] as usize..self.offsets[n as usize + 1] as usize
    }

    pub fn degree(&self, n: NodeIx) -> usize {
        self.range(n).len()
    }

    pub fn edges(&self, n: NodeIx) -> EdgeSlice<'a> {
        self.slice(self.range(n))
    }

    /// Edges of `n` with relation `rel` (edges are sorted by relation within a node).
    pub fn edges_rel(&self, n: NodeIx, rel: RelationType) -> EdgeSlice<'a> {
        let r = self.range(n);
        let rels = &self.rel[r.clone()];
        let lo = rels.partition_point(|&x| x < rel.code());
        let hi = lo + rels[lo..].partition_point(|&x| x == rel.code());
        self.slice(r.start + lo..r.start + hi)
    }

    fn slice(&self, r: Range<usize>) -> EdgeSlice<'a> {
        EdgeSlice {
            nbr: &self.nbr[r.clone()],
            claim: &self.claim[r.clone()],
            rel: &self.rel[r.clone()],
            state: &self.state[r],
        }
    }
}

impl<'a> EdgeSlice<'a> {
    pub fn len(&self) -> usize {
        self.nbr.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nbr.is_empty()
    }

    pub fn get(&self, i: usize) -> EdgeRef {
        EdgeRef {
            nbr: self.nbr[i],
            claim: self.claim[i],
            relation: RelationType::from_code(self.rel[i]).expect("relation code checked by verify()"),
            state: EvidenceState::from_code(self.state[i]).expect("state code checked by verify()"),
        }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = EdgeRef> + 'a {
        let s = *self;
        (0..s.len()).map(move |i| s.get(i))
    }
}

/// Claims and evidence states touching one node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvidenceSummary {
    /// Claims of any state the node takes part in (any role).  0 = coverage "unknown".
    pub claims: u32,
    /// Bitmask of `EvidenceState::bit()` over those claims.
    pub states: u8,
}

impl EvidenceSummary {
    pub fn is_unknown(&self) -> bool {
        self.claims == 0
    }

    pub fn has(&self, s: EvidenceState) -> bool {
        self.states as u32 & s.bit() != 0
    }

    /// Strongest current state present: formally_linked > editorially_reviewed >
    /// extracted > model_proposed > disputed.
    pub fn strongest(&self) -> Option<EvidenceState> {
        use EvidenceState as S;
        [S::FormallyLinked, S::EditoriallyReviewed, S::Extracted, S::ModelProposed, S::Disputed]
            .into_iter()
            .find(|s| self.has(*s))
    }
}

/// One (relation, evidence state) count of an aggregated concept edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConceptEdgeStat {
    pub relation: RelationType,
    pub state: EvidenceState,
    /// Declaration-level edges (primary member of the source -> primary member of the target).
    pub count: u32,
    /// The smallest claim behind this count: open it to trace the aggregated edge to its source.
    pub example: ClaimIx,
}

/// Aggregated edge concept -> `target` concept; slices point into the memory map.
#[derive(Clone, Copy, Debug)]
pub struct ConceptEdge<'a> {
    pub target: NodeIx,
    rel: &'a [u8],
    state: &'a [u8],
    count: &'a [u32],
    example: &'a [u32],
}

impl<'a> ConceptEdge<'a> {
    /// Every aggregated edge is a candidate (ADR-0005 §7a): it is inferred from memberships, so it
    /// stays out of the beginner summary and of "strength", whatever its state.
    pub const fn is_candidate(&self) -> bool {
        true
    }

    /// Declaration-level edges behind this aggregated edge, all relations and states.
    pub fn total(&self) -> u32 {
        self.count.iter().sum()
    }

    /// Counts per (relation, state), sorted by relation then state.
    pub fn stats(&self) -> impl ExactSizeIterator<Item = ConceptEdgeStat> + 'a {
        let s = *self;
        (0..s.rel.len()).map(move |i| ConceptEdgeStat {
            relation: RelationType::from_code(s.rel[i]).expect("relation code checked by verify()"),
            state: EvidenceState::from_code(s.state[i]).expect("state code checked by verify()"),
            count: s.count[i],
            example: s.example[i],
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProvRef<'a> {
    pub source: SourceIx,
    pub source_id: &'a str,
    pub locator: &'a str,
    pub license: &'a str,
}

impl Snapshot {
    /// Open a snapshot file, or a directory holding `CURRENT`.
    pub fn open(path: impl AsRef<Path>) -> Result<Snapshot> {
        let path = path.as_ref();
        let file_path = if path.is_dir() { crate::compile::current_snapshot(path)? } else { path.to_path_buf() };
        let file = File::open(&file_path).map_err(|e| Error::io(&file_path, e))?;
        // SAFETY: snapshot files are written once under a fresh name and never modified
        // in place (compile writes a temp file and renames it), so the mapping is stable.
        let map = unsafe { Mmap::map(&file) }.map_err(|e| Error::io(&file_path, e))?;
        Self::from_map(map, file_path)
    }

    /// Open a layer snapshot (the works layer, ADR-0005 §8.4) built over `core`: its header must name
    /// the content hash of `core`, else the node indexes it holds would point at other nodes.
    pub fn open_layer(path: impl AsRef<Path>, core: &Snapshot) -> Result<Snapshot> {
        let layer = Self::open(path)?;
        if layer.header.core_content_hash != core.header.content_hash {
            return Err(Error::Format(format!(
                "{}: layer built over core {}, but the open core is {}",
                layer.path.display(),
                blake3::Hash::from_bytes(layer.header.core_content_hash).to_hex(),
                core.content_hash_hex()
            )));
        }
        Ok(layer)
    }

    /// A layer snapshot names the core it was built over; a core snapshot has a zero `core_content_hash`.
    pub fn is_layer(&self) -> bool {
        self.header.core_content_hash != [0u8; 32]
    }

    fn from_map(map: Mmap, path: PathBuf) -> Result<Snapshot> {
        let name = path.display().to_string();
        let fail = |m: String| Err(Error::Format(format!("{name}: {m}")));
        if map.len() < HEADER_SIZE {
            return fail("shorter than a header".into());
        }
        let header: Header = bytemuck::pod_read_unaligned(&map[..HEADER_SIZE]);
        if header.magic != MAGIC {
            return fail("not an atlas snapshot (bad magic)".into());
        }
        if header.format_version != FORMAT_VERSION {
            return fail(format!("format v{}, this build reads v{FORMAT_VERSION}", header.format_version));
        }
        if header.schema_version == 0 || header.schema_version > SUPPORTED_SCHEMA {
            return fail(format!("registry schema v{}", header.schema_version));
        }
        if header.vocab_hash != vocab_hash() {
            return fail("built with another relation/evidence vocabulary; recompile".into());
        }
        let n_sec = header.num_sections as usize;
        let table_end = HEADER_SIZE + n_sec * ENTRY_SIZE;
        if map.len() < table_end {
            return fail("truncated section table".into());
        }
        let mut table = [(usize::MAX, 0usize); MAX_SECTION_CODE];
        for raw in map[HEADER_SIZE..table_end].chunks_exact(ENTRY_SIZE) {
            let e: SectionEntry = bytemuck::pod_read_unaligned(raw);
            let Some(kind) = Section::from_code(e.kind) else { return fail(format!("unknown section {}", e.kind)) };
            let end = e.offset.checked_add(e.len);
            if e.elem_size != kind.elem_size()
                || e.offset % SECTION_ALIGN != 0
                || e.len % e.elem_size as u64 != 0
                || end.is_none_or(|end| end > map.len() as u64)
                || table[e.kind as usize].0 != usize::MAX
            {
                return fail(format!("bad entry for section {kind:?}"));
            }
            table[e.kind as usize] = (e.offset as usize, e.len as usize);
        }
        let snap = Snapshot { map, path, header, table };
        if let Some(missing) = Section::ALL.iter().find(|s| snap.table[s.code() as usize].0 == usize::MAX) {
            return fail(format!("missing section {missing:?}"));
        }
        snap.check_lengths().map_err(|m| Error::Format(format!("{}: {m}", snap.path.display())))?;
        Ok(snap)
    }

    fn sec<T: Pod>(&self, s: Section) -> &[T] {
        let (off, len) = self.table[s.code() as usize];
        bytemuck::cast_slice(&self.map[off..off + len])
    }

    fn u32s(&self, s: Section) -> &[u32] {
        self.sec(s)
    }

    fn u8s(&self, s: Section) -> &[u8] {
        self.sec(s)
    }

    fn u64s(&self, s: Section) -> &[u64] {
        self.sec(s)
    }

    fn check_lengths(&self) -> std::result::Result<(), String> {
        use Section as S;
        let h = &self.header;
        let (n, c, e, g, src, st, p) = (
            h.num_nodes as usize,
            h.num_claims as usize,
            h.num_edges as usize,
            h.num_groups as usize,
            h.num_sources as usize,
            h.num_strings as usize,
            h.num_prov as usize,
        );
        let len = |s: Section| self.table[s.code() as usize].1 / s.elem_size() as usize;
        let expect: &[(&[Section], usize)] = &[
            (&[S::StrOffsets], st + 1),
            (&[S::NodeId, S::NodeType, S::NodeCanon, S::NodeLabelEs, S::NodeLabelEn, S::NodeArea], n),
            (&[S::NodeClaimCount, S::NodeStateMask, S::NodeFlags, S::NodeShowEs, S::NodeShowEn], n),
            (&[S::ClaimId, S::ClaimRel, S::ClaimState, S::ClaimSubject, S::ClaimGroup], c),
            (&[S::ClaimValidity], c),
            (&[S::ClaimTgtOff, S::ClaimCtxOff, S::ClaimAsmOff, S::ProvOff], c + 1),
            (&[S::GroupId], g),
            (&[S::GroupOff], g + 1),
            (&[S::ProvSource, S::ProvLocator, S::ProvLicense], p),
            (&[S::SourceId, S::SourceTitle, S::SourceLicense], src),
            (&[S::OutOff, S::InOff], n + 1),
            (&[S::OutNbr, S::OutClaim, S::OutRel, S::OutState, S::InNbr, S::InClaim, S::InRel, S::InState], e),
            (&[S::NodeConcept], n),
            (&[S::ConceptMemberOff, S::ConceptEdgeOff], len(S::ConceptNode) + 1),
            (&[S::ConceptStatOff], len(S::ConceptEdgeDst) + 1),
            (&[S::ConceptStatState, S::ConceptStatCount, S::ConceptStatExample], len(S::ConceptStatRel)),
        ];
        for (secs, want) in expect {
            for s in *secs {
                if len(*s) != *want {
                    return Err(format!("section {s:?} has {} elements, header implies {want}", len(*s)));
                }
            }
        }
        // offsets arrays must start at 0 and end at the length of what they index
        let so = self.u64s(S::StrOffsets);
        if so.first() != Some(&0) || so.last().map(|&x| x as usize) != Some(len(S::StrBytes)) {
            return Err("offsets StrOffsets do not span the string bytes".into());
        }
        let ends: &[(Section, usize)] = &[
            (S::ClaimTgtOff, len(S::ClaimTgt)),
            (S::ClaimCtxOff, len(S::ClaimCtxNode)),
            (S::ClaimAsmOff, len(S::ClaimAsm)),
            (S::GroupOff, len(S::GroupClaims)),
            (S::ProvOff, p),
            (S::OutOff, e),
            (S::InOff, e),
            (S::ConceptMemberOff, len(S::ConceptMember)),
            (S::ConceptEdgeOff, len(S::ConceptEdgeDst)),
            (S::ConceptStatOff, len(S::ConceptStatRel)),
        ];
        for (s, want) in ends {
            let o = self.u32s(*s);
            if o.first() != Some(&0) || o.last().map(|&x| x as usize) != Some(*want) {
                return Err(format!("offsets {s:?} do not span 0..{want}"));
            }
        }
        for (s, keys) in [(S::NodeHash, n), (S::ClaimHash, c)] {
            let slots = len(s);
            if slots % REGIONS != 0 || !(slots / REGIONS).is_power_of_two() || slots < 2 * keys {
                return Err(format!("hash table {s:?} has a bad size ({slots} slots for {keys} keys)"));
            }
        }
        if len(S::ClaimCtxRole) != len(S::ClaimCtxNode) {
            return Err("ClaimCtxRole and ClaimCtxNode differ in length".into());
        }
        Ok(())
    }

    // ------------------------------------------------------------ metadata

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_len(&self) -> u64 {
        self.map.len() as u64
    }

    pub fn content_hash_hex(&self) -> String {
        blake3::Hash::from_bytes(self.header.content_hash).to_hex().to_string()
    }

    pub fn num_nodes(&self) -> u32 {
        self.header.num_nodes
    }

    pub fn num_claims(&self) -> u32 {
        self.header.num_claims
    }

    pub fn num_edges(&self) -> u32 {
        self.header.num_edges
    }

    pub fn num_groups(&self) -> u32 {
        self.header.num_groups
    }

    pub fn num_sources(&self) -> u32 {
        self.header.num_sources
    }

    // ------------------------------------------------------------ strings

    /// String `id` of the table (`""` for NONE; U+FFFD if the bytes are corrupt).
    pub fn str(&self, id: StrId) -> &str {
        if id == NONE {
            return "";
        }
        let o = self.u64s(Section::StrOffsets);
        let (s, e) = (o[id as usize] as usize, o[id as usize + 1] as usize);
        self.u8s(Section::StrBytes).get(s..e).and_then(|b| std::str::from_utf8(b).ok()).unwrap_or("\u{FFFD}")
    }

    // ------------------------------------------------------------ nodes

    pub fn node_id(&self, n: NodeIx) -> &str {
        self.str(self.u32s(Section::NodeId)[n as usize])
    }

    pub fn node_type(&self, n: NodeIx) -> EntityType {
        EntityType::from_code(self.u8s(Section::NodeType)[n as usize]).expect("type code checked by verify()")
    }

    /// Preferred label with `Entity.label` semantics: `es` falls back to `en`, then to the first label.
    pub fn label(&self, n: NodeIx, lang: &str) -> &str {
        let es = self.u32s(Section::NodeLabelEs)[n as usize];
        if lang == "es" && es != NONE {
            return self.str(es);
        }
        self.str(self.u32s(Section::NodeLabelEn)[n as usize])
    }

    /// Label to show: the active presentation label of `lang` (curation `presentation_label`), else
    /// `label(n, lang)`.  The registry label is never replaced.
    pub fn display_label(&self, n: NodeIx, lang: &str) -> &str {
        let show = match lang {
            "es" => self.u32s(Section::NodeShowEs)[n as usize],
            "en" => self.u32s(Section::NodeShowEn)[n as usize],
            _ => NONE,
        };
        if show != NONE { self.str(show) } else { self.label(n, lang) }
    }

    /// Bits of the presentation scopes `n` is excluded from (`vocab::PRESENTATION_SCOPES` order).
    pub fn node_flags(&self, n: NodeIx) -> u8 {
        self.u8s(Section::NodeFlags)[n as usize]
    }

    /// Is `n` excluded from `scope` ("map:general", "map:all", "search:default", "concept_layer")?
    pub fn is_excluded(&self, n: NodeIx, scope: &str) -> bool {
        scope_bit(scope).is_some_and(|b| self.node_flags(n) & b != 0)
    }

    pub fn canonical(&self, n: NodeIx) -> NodeIx {
        self.u32s(Section::NodeCanon)[n as usize]
    }

    pub fn is_merged(&self, n: NodeIx) -> bool {
        self.canonical(n) != n
    }

    /// Nodes merged into `n` (directly or through a chain), ascending.
    pub fn merged_from(&self, n: NodeIx) -> Vec<NodeIx> {
        let canon = self.u32s(Section::NodeCanon);
        self.u32s(Section::MergedNodes).iter().copied().filter(|&m| canon[m as usize] == n).collect()
    }

    pub fn area(&self, n: NodeIx) -> Option<NodeIx> {
        Some(self.u32s(Section::NodeArea)[n as usize]).filter(|&a| a != NONE)
    }

    pub fn evidence(&self, n: NodeIx) -> EvidenceSummary {
        EvidenceSummary {
            claims: self.u32s(Section::NodeClaimCount)[n as usize],
            states: self.u8s(Section::NodeStateMask)[n as usize],
        }
    }

    /// Node of an entity id, exactly as stored (may be a merged node).
    pub fn find_node(&self, id: &str) -> Option<NodeIx> {
        let ids = self.u32s(Section::NodeId);
        hashidx::lookup(self.u32s(Section::NodeHash), id, |n| self.str(ids[n as usize]))
    }

    /// Canonical node of an entity id (follows merges, like `Store.resolve`).
    pub fn resolve(&self, id: &str) -> Option<NodeIx> {
        self.find_node(id).map(|n| self.canonical(n))
    }

    pub fn node_types(&self) -> &[u8] {
        self.u8s(Section::NodeType)
    }

    // ------------------------------------------------------------ edges

    /// Typed CSR over current claims.  Merged nodes have no edges; their edges
    /// belong to the canonical node.
    pub fn csr(&self, dir: Dir) -> Csr<'_> {
        use Section as S;
        let s = match dir {
            Dir::Out => [S::OutOff, S::OutNbr, S::OutClaim, S::OutRel, S::OutState],
            Dir::In => [S::InOff, S::InNbr, S::InClaim, S::InRel, S::InState],
        };
        Csr {
            offsets: self.u32s(s[0]),
            nbr: self.u32s(s[1]),
            claim: self.u32s(s[2]),
            rel: self.u8s(s[3]),
            state: self.u8s(s[4]),
        }
    }

    pub fn out_edges(&self, n: NodeIx) -> EdgeSlice<'_> {
        self.csr(Dir::Out).edges(n)
    }

    pub fn in_edges(&self, n: NodeIx) -> EdgeSlice<'_> {
        self.csr(Dir::In).edges(n)
    }

    // ------------------------------------------------------------ concept layer (src/concepts.rs)

    /// Primary concept of a declaration (its membership with the highest confidence), if it has one.
    pub fn primary_concept(&self, n: NodeIx) -> Option<NodeIx> {
        Some(self.u32s(Section::NodeConcept)[n as usize]).filter(|&c| c != NONE)
    }

    /// Concept nodes (objects of at least one current `member_of_concept` claim), ascending.
    pub fn concepts(&self) -> &[NodeIx] {
        self.u32s(Section::ConceptNode)
    }

    /// Position of `c` in `concepts()`.
    pub fn concept_index(&self, c: NodeIx) -> Option<usize> {
        self.concepts().binary_search(&c).ok()
    }

    /// Declarations whose primary concept is `c`, ascending (empty if `c` is not a concept).
    pub fn concept_members(&self, c: NodeIx) -> &[NodeIx] {
        let Some(k) = self.concept_index(c) else { return &[] };
        let o = self.u32s(Section::ConceptMemberOff);
        &self.u32s(Section::ConceptMember)[o[k] as usize..o[k + 1] as usize]
    }

    /// Every current membership of `c` (primary and secondary): `nbr` is the declaration, `claim` the
    /// `member_of_concept` claim (its provenance and confidence are in the registry).
    pub fn concept_memberships(&self, c: NodeIx) -> EdgeSlice<'_> {
        self.csr(Dir::In).edges_rel(c, RelationType::MemberOfConcept)
    }

    /// Aggregated edges `c` -> other concepts, by target node ascending.
    pub fn concept_edges(&self, c: NodeIx) -> impl ExactSizeIterator<Item = ConceptEdge<'_>> + '_ {
        let r = self.concept_index(c).map_or(0..0, |k| {
            let o = self.u32s(Section::ConceptEdgeOff);
            o[k] as usize..o[k + 1] as usize
        });
        let (dst, so) = (self.u32s(Section::ConceptEdgeDst), self.u32s(Section::ConceptStatOff));
        r.map(move |e| {
            let s = so[e] as usize..so[e + 1] as usize;
            ConceptEdge {
                target: dst[e],
                rel: &self.u8s(Section::ConceptStatRel)[s.clone()],
                state: &self.u8s(Section::ConceptStatState)[s.clone()],
                count: &self.u32s(Section::ConceptStatCount)[s.clone()],
                example: &self.u32s(Section::ConceptStatExample)[s],
            }
        })
    }

    /// Number of aggregated concept -> concept edges.
    pub fn num_concept_edges(&self) -> usize {
        self.u32s(Section::ConceptEdgeDst).len()
    }

    /// A concept derived from a Mathlib zone (`concept:Mathlib/...`), not a Wikidata item.
    pub fn is_derived_concept(&self, n: NodeIx) -> bool {
        self.node_id(n).starts_with("concept:")
    }

    // ------------------------------------------------------------ claims

    pub fn claim_id(&self, c: ClaimIx) -> &str {
        self.str(self.u32s(Section::ClaimId)[c as usize])
    }

    pub fn find_claim(&self, id: &str) -> Option<ClaimIx> {
        let ids = self.u32s(Section::ClaimId);
        hashidx::lookup(self.u32s(Section::ClaimHash), id, |c| self.str(ids[c as usize]))
    }

    pub fn claim_relation(&self, c: ClaimIx) -> RelationType {
        RelationType::from_code(self.u8s(Section::ClaimRel)[c as usize]).expect("relation code checked by verify()")
    }

    pub fn claim_state(&self, c: ClaimIx) -> EvidenceState {
        EvidenceState::from_code(self.u8s(Section::ClaimState)[c as usize]).expect("state code checked by verify()")
    }

    pub fn claim_subject(&self, c: ClaimIx) -> NodeIx {
        self.u32s(Section::ClaimSubject)[c as usize]
    }

    /// The object, or the conjunctive premise set (AND-node): all are needed together.
    pub fn claim_targets(&self, c: ClaimIx) -> &[NodeIx] {
        let o = self.u32s(Section::ClaimTgtOff);
        &self.u32s(Section::ClaimTgt)[o[c as usize] as usize..o[c as usize + 1] as usize]
    }

    /// `conjunct_count` of the claim's edges (> 1 for a conjunction).
    pub fn conjunct_count(&self, c: ClaimIx) -> usize {
        self.claim_targets(c).len()
    }

    /// Assumption / context / via participants.
    pub fn claim_context(&self, c: ClaimIx) -> impl ExactSizeIterator<Item = (Role, NodeIx)> + '_ {
        let o = self.u32s(Section::ClaimCtxOff);
        let r = o[c as usize] as usize..o[c as usize + 1] as usize;
        let (nodes, roles) = (&self.u32s(Section::ClaimCtxNode)[r.clone()], &self.u8s(Section::ClaimCtxRole)[r]);
        roles.iter().zip(nodes).map(|(&r, &n)| (Role::from_code(r).expect("role code checked by verify()"), n))
    }

    /// Free-text assumptions (`Claim.assumptions`).
    pub fn claim_assumptions(&self, c: ClaimIx) -> impl ExactSizeIterator<Item = &str> + '_ {
        let o = self.u32s(Section::ClaimAsmOff);
        self.u32s(Section::ClaimAsm)[o[c as usize] as usize..o[c as usize + 1] as usize].iter().map(|&s| self.str(s))
    }

    pub fn claim_validity_domain(&self, c: ClaimIx) -> Option<&str> {
        Some(self.u32s(Section::ClaimValidity)[c as usize]).filter(|&s| s != NONE).map(|s| self.str(s))
    }

    pub fn claim_group(&self, c: ClaimIx) -> Option<GroupIx> {
        Some(self.u32s(Section::ClaimGroup)[c as usize]).filter(|&g| g != NONE)
    }

    pub fn claim_provenance(&self, c: ClaimIx) -> impl ExactSizeIterator<Item = ProvRef<'_>> + '_ {
        let o = self.u32s(Section::ProvOff);
        let r = o[c as usize] as usize..o[c as usize + 1] as usize;
        let (src, loc, lic) = (
            &self.u32s(Section::ProvSource)[r.clone()],
            &self.u32s(Section::ProvLocator)[r.clone()],
            &self.u32s(Section::ProvLicense)[r],
        );
        (0..src.len()).map(move |i| ProvRef {
            source: src[i],
            source_id: self.source_id(src[i]),
            locator: self.str(loc[i]),
            license: self.str(lic[i]),
        })
    }

    pub fn claim_states(&self) -> &[u8] {
        self.u8s(Section::ClaimState)
    }

    pub fn claim_relations(&self) -> &[u8] {
        self.u8s(Section::ClaimRel)
    }

    // ------------------------------------------------------------ alternative groups (OR-nodes)

    pub fn group_id(&self, g: GroupIx) -> &str {
        self.str(self.u32s(Section::GroupId)[g as usize])
    }

    pub fn find_group(&self, id: &str) -> Option<GroupIx> {
        let ids = self.u32s(Section::GroupId);
        ids.binary_search_by(|&s| self.str(s).cmp(id)).ok().map(|i| i as GroupIx)
    }

    /// The alternative claims of a group (any state; filter with `claim_state`).
    pub fn group_claims(&self, g: GroupIx) -> &[ClaimIx] {
        let o = self.u32s(Section::GroupOff);
        &self.u32s(Section::GroupClaims)[o[g as usize] as usize..o[g as usize + 1] as usize]
    }

    // ------------------------------------------------------------ sources

    pub fn source_id(&self, s: SourceIx) -> &str {
        self.str(self.u32s(Section::SourceId)[s as usize])
    }

    pub fn source_title(&self, s: SourceIx) -> &str {
        self.str(self.u32s(Section::SourceTitle)[s as usize])
    }

    pub fn source_license(&self, s: SourceIx) -> &str {
        self.str(self.u32s(Section::SourceLicense)[s as usize])
    }

    // ------------------------------------------------------------ integrity

    /// Recompute the BLAKE3 content hash (parallel) and compare with the header.
    pub fn verify_hash(&self) -> Result<()> {
        let mut h = blake3::Hasher::new();
        let mut head = self.map[..HEADER_SIZE].to_vec();
        head[HASH_FIELD].fill(0);
        h.update(&head);
        h.update_rayon(&self.map[HEADER_SIZE..]);
        if *h.finalize().as_bytes() != self.header.content_hash {
            return Err(Error::Integrity(format!("{}: content hash mismatch", self.path.display())));
        }
        Ok(())
    }

    /// Hash plus every reference: offsets monotone, node/claim/string ids in range,
    /// enum codes valid, strings valid UTF-8.
    pub fn verify(&self) -> Result<()> {
        self.verify_hash()?;
        self.verify_refs().map_err(|m| Error::Integrity(format!("{}: {m}", self.path.display())))
    }

    fn verify_refs(&self) -> std::result::Result<(), String> {
        use Section as S;
        let h = &self.header;
        let (n, c, g, src, st) = (h.num_nodes, h.num_claims, h.num_groups, h.num_sources, h.num_strings);
        if !self.u64s(S::StrOffsets).par_windows(2).all(|w| w[0] <= w[1]) {
            return Err("offsets StrOffsets not monotone".into());
        }
        let monotone = [
            S::ClaimTgtOff, S::ClaimCtxOff, S::ClaimAsmOff, S::GroupOff, S::ProvOff, S::OutOff, S::InOff,
            S::ConceptMemberOff, S::ConceptEdgeOff, S::ConceptStatOff,
        ];
        for s in monotone {
            if !self.u32s(s).par_windows(2).all(|w| w[0] <= w[1]) {
                return Err(format!("offsets {s:?} not monotone"));
            }
        }
        let refs: &[(&[Section], u32, bool)] = &[
            (&[S::NodeId, S::NodeLabelEn, S::ClaimId, S::ClaimAsm, S::GroupId], st, false),
            (&[S::ProvLocator, S::ProvLicense, S::SourceId, S::SourceTitle, S::SourceLicense], st, false),
            (&[S::NodeLabelEs, S::ClaimValidity, S::NodeShowEs, S::NodeShowEn], st, true),
            (&[S::NodeCanon, S::MergedNodes, S::ClaimSubject, S::ClaimTgt, S::ClaimCtxNode], n, false),
            (&[S::NodeHash], n, true),
            (&[S::OutNbr, S::InNbr, S::ConceptNode, S::ConceptMember, S::ConceptEdgeDst], n, false),
            (&[S::NodeArea, S::NodeConcept], n, true),
            (&[S::GroupClaims, S::OutClaim, S::InClaim, S::ConceptStatExample], c, false),
            (&[S::ClaimHash], c, true),
            (&[S::ClaimGroup], g, true),
            (&[S::ProvSource], src, false),
        ];
        for (secs, bound, none_ok) in refs {
            for s in *secs {
                if !self.u32s(*s).par_iter().all(|&x| x < *bound || (*none_ok && x == NONE)) {
                    return Err(format!("section {s:?} has an id out of range"));
                }
            }
        }
        let codes: &[(&[Section], usize)] = &[
            (&[S::NodeType], EntityType::COUNT),
            (&[S::ClaimRel, S::OutRel, S::InRel, S::ConceptStatRel], RelationType::COUNT),
            (&[S::ClaimState, S::OutState, S::InState, S::ConceptStatState], EvidenceState::COUNT),
            (&[S::ClaimCtxRole], Role::COUNT),
        ];
        for (secs, bound) in codes {
            for s in *secs {
                if !self.u8s(*s).par_iter().all(|&x| (x as usize) < *bound) {
                    return Err(format!("section {s:?} has an invalid code"));
                }
            }
        }
        if !self.u8s(S::NodeStateMask).par_iter().all(|&m| (m as u32) < (1 << EvidenceState::COUNT)) {
            return Err("NodeStateMask has unknown bits".into());
        }
        if !self.u8s(S::NodeFlags).par_iter().all(|&f| (f as u32) < (1 << PRESENTATION_SCOPES.len())) {
            return Err("NodeFlags has unknown bits".into());
        }
        let (o, b) = (self.u64s(S::StrOffsets), self.u8s(S::StrBytes));
        if !(0..st as usize).into_par_iter().all(|i| std::str::from_utf8(&b[o[i] as usize..o[i + 1] as usize]).is_ok()) {
            return Err("string table has invalid UTF-8".into());
        }
        Ok(())
    }
}
