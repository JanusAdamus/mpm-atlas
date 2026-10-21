//! On-disk layout of a snapshot file (`atlas-<hash16>.snap`).
//!
//! ```text
//! [Header 256 B][SectionEntry 24 B x num_sections][pad][section 0][pad][section 1]...
//! ```
//! Everything is little-endian.  Each section starts on a 64-byte boundary so it
//! can be viewed in place as `&[u8]` / `&[u32]` from the memory map.  The content
//! hash is BLAKE3 over the whole file with the `content_hash` field zeroed.

use bytemuck::{Pod, Zeroable};

#[cfg(not(target_endian = "little"))]
compile_error!("the snapshot format is little-endian and is mapped in place");

pub const MAGIC: [u8; 8] = *b"ATLSNAP\0";
/// Bump on any change to sections or their meaning.
/// v3 (ADR-0005): u64 string offsets, NodeFlags, presentation labels, `core_content_hash`,
/// ConceptStatState = weakest link.
pub const FORMAT_VERSION: u32 = 3;
/// Highest registry `PRAGMA user_version` the compiler understands.
pub const SUPPORTED_SCHEMA: u32 = 4;
pub const SECTION_ALIGN: u64 = 64;
/// "No value" in any u32 column (no label, no area, no group, ...).
pub const NONE: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Header {
    pub magic: [u8; 8],
    pub format_version: u32,
    /// Registry schema version (`PRAGMA user_version`) the snapshot was compiled from.
    pub schema_version: u32,
    pub vocab_hash: [u8; 16],
    pub content_hash: [u8; 32],
    /// `MAX(events.seq)` of the registry at compile time: the snapshot is exactly
    /// the registry state after that event (the event log is append-only).
    pub source_event_seq: u64,
    pub num_sections: u32,
    pub num_nodes: u32,
    pub num_claims: u32,
    pub num_edges: u32,
    pub num_groups: u32,
    pub num_sources: u32,
    pub num_strings: u32,
    pub num_prov: u32,
    /// v3: zero in a core snapshot; in a layer snapshot (works), the `content_hash` of the core
    /// snapshot whose node indexes it refers to.  `Snapshot::open_layer` rejects a mismatch.
    pub core_content_hash: [u8; 32],
    pub reserved: [u64; 15],
}

pub const HEADER_SIZE: usize = std::mem::size_of::<Header>();
const _: () = assert!(HEADER_SIZE == 256);
/// Byte range of `content_hash` inside the header (zeroed while hashing).
pub const HASH_FIELD: std::ops::Range<usize> = 32..64;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SectionEntry {
    pub kind: u32,
    pub elem_size: u32,
    pub offset: u64,
    pub len: u64,
}

pub const ENTRY_SIZE: usize = std::mem::size_of::<SectionEntry>();
const _: () = assert!(ENTRY_SIZE == 24);

macro_rules! sections {
    ($($name:ident = $code:literal : $ty:ty, $doc:literal;)+) => {
        /// Every section of the current format.  `elem_size` is fixed per kind.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u32)]
        pub enum Section { $(#[doc = $doc] $name = $code),+ }

        impl Section {
            pub const ALL: &'static [Section] = &[$(Section::$name),+];
            pub const COUNT: usize = Self::ALL.len();
            pub const fn elem_size(self) -> u32 {
                match self { $(Section::$name => std::mem::size_of::<$ty>() as u32),+ }
            }
            pub fn from_code(code: u32) -> Option<Section> {
                match code { $($code => Some(Section::$name),)+ _ => None }
            }
            pub const fn code(self) -> u32 { self as u32 }
        }
    };
}

sections! {
    StrOffsets = 1: u64, "string table: n+1 byte offsets into StrBytes (u64 since format v3)";
    StrBytes = 2: u8, "string table: UTF-8 bytes";

    NodeId = 10: u32, "node -> string id of the entity id";
    NodeType = 11: u8, "node -> EntityType code";
    NodeCanon = 12: u32, "node -> canonical node after merges (itself if not merged)";
    NodeLabelEs = 13: u32, "node -> string id of the `es` label, or NONE";
    NodeLabelEn = 14: u32, "node -> string id of the `en` label, else the first label";
    NodeArea = 15: u32, "node -> area node of its first current part_of_area claim, or NONE";
    NodeClaimCount = 16: u32, "node -> number of claims (any state) it takes part in, merges resolved; 0 = coverage unknown";
    NodeStateMask = 17: u8, "node -> bitmask of EvidenceState bits of those claims";
    NodeHash = 18: u32, "id -> node hash table (see hashidx.rs)";
    MergedNodes = 19: u32, "nodes with NodeCanon != self, ascending";
    // v3: presentation curation (ADR-0005 §5), from active `curation` rows; the registry labels are untouched
    NodeFlags = 20: u8, "node -> bits of excluded presentation scopes (vocab::PRESENTATION_SCOPES order)";
    NodeShowEs = 21: u32, "node -> string id of the active `es` presentation label, or NONE";
    NodeShowEn = 22: u32, "node -> string id of the active `en` presentation label, or NONE";

    ClaimId = 30: u32, "claim -> string id";
    ClaimRel = 31: u8, "claim -> RelationType code";
    ClaimState = 32: u8, "claim -> EvidenceState code";
    ClaimSubject = 33: u32, "claim -> canonical subject node";
    ClaimGroup = 34: u32, "claim -> alternative group (OR-node) index, or NONE";
    ClaimTgtOff = 35: u32, "claim -> range in ClaimTgt (n+1)";
    ClaimTgt = 36: u32, "object, or the conjunctive premise set (AND-node), canonical nodes";
    ClaimCtxOff = 37: u32, "claim -> range in ClaimCtxNode/Role (n+1)";
    ClaimCtxNode = 38: u32, "assumption/context/via participants, canonical nodes";
    ClaimCtxRole = 39: u8, "Role code of each ClaimCtxNode";
    ClaimHash = 40: u32, "id -> claim hash table (see hashidx.rs)";
    ClaimAsmOff = 41: u32, "claim -> range in ClaimAsm (n+1)";
    ClaimAsm = 42: u32, "string ids of free-text assumptions";
    ClaimValidity = 43: u32, "claim -> string id of validity_domain, or NONE";

    GroupId = 50: u32, "group -> string id; groups are sorted by id";
    GroupOff = 51: u32, "group -> range in GroupClaims (n+1)";
    GroupClaims = 52: u32, "claims of each alternative group (OR-node), in claim order";

    ProvOff = 60: u32, "claim -> range in Prov* (n+1)";
    ProvSource = 61: u32, "provenance row -> source index";
    ProvLocator = 62: u32, "provenance row -> string id of locator";
    ProvLicense = 63: u32, "provenance row -> string id of license";

    SourceId = 70: u32, "source -> string id; sources sorted by id";
    SourceTitle = 71: u32, "source -> string id of title";
    SourceLicense = 72: u32, "source -> string id of license";

    OutOff = 80: u32, "node -> range in Out* (n+1); current claims only";
    OutNbr = 81: u32, "subject -> target node; sorted by (node, relation, nbr, claim)";
    OutClaim = 82: u32, "claim of each out edge";
    OutRel = 83: u8, "RelationType code of each out edge";
    OutState = 84: u8, "EvidenceState code of each out edge";
    InOff = 90: u32, "node -> range in In* (n+1)";
    InNbr = 91: u32, "target -> subject node; sorted by (node, relation, nbr, claim)";
    InClaim = 92: u32, "claim of each in edge";
    InRel = 93: u8, "RelationType code of each in edge";
    InState = 94: u8, "EvidenceState code of each in edge";

    // v2: concept layer (src/concepts.rs), derived from current member_of_concept claims
    NodeConcept = 100: u32, "node -> primary concept node, or NONE";
    ConceptNode = 101: u32, "concept nodes (objects of a current membership), ascending";
    ConceptMemberOff = 102: u32, "concept -> range in ConceptMember (C+1)";
    ConceptMember = 103: u32, "primary members of each concept, ascending";
    ConceptEdgeOff = 104: u32, "concept -> range in ConceptEdgeDst (C+1)";
    ConceptEdgeDst = 105: u32, "target concept node of each aggregated edge, ascending per concept";
    ConceptStatOff = 106: u32, "aggregated edge -> range in ConceptStat* (E+1)";
    ConceptStatRel = 107: u8, "RelationType code of each (relation, state) count";
    ConceptStatState = 108: u8, "weakest EvidenceState of edge, source membership and target membership (v3)";
    ConceptStatCount = 109: u32, "declaration-level edges behind each (relation, state)";
    ConceptStatExample = 110: u32, "smallest claim behind each (relation, state): a traceable example";
}

/// Section codes are below this; loaders index a table by code.
pub const MAX_SECTION_CODE: usize = 128;

pub fn align_up(x: u64) -> u64 {
    x.div_ceil(SECTION_ALIGN) * SECTION_ALIGN
}
