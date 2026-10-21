//! atlas-core: the registry (`data/atlas.db`, SQLite schema v1 written by
//! `atlas/store.py`) compiled into one memory-mapped binary snapshot that the hot
//! paths (search, paths, layout, server) read without copying.
//!
//! ```no_run
//! use atlas_core::{compile, CompileOptions, Snapshot, Dir, RelationType};
//! let report = compile(&CompileOptions::new("data/atlas.db", "data/snapshot"))?;
//! let snap = Snapshot::open("data/snapshot")?;          // reads data/snapshot/CURRENT
//! let n = snap.resolve("s:pythagoras").unwrap();
//! for e in snap.csr(Dir::Out).edges_rel(n, RelationType::DependsOnLogically).iter() {
//!     println!("{} -> {} via claim {}", snap.node_id(n), snap.node_id(e.nbr), snap.claim_id(e.claim));
//! }
//! # Ok::<(), atlas_core::Error>(())
//! ```

mod compile;
mod concepts;
mod error;
pub mod format;
pub mod hashidx;
mod layer;
mod read;
mod snapshot;
mod stats;
mod strings;
pub mod synth;
pub mod vocab;
mod write;

pub use compile::{CompileOptions, CompileReport, Phase, compile, current_snapshot};
pub use error::{Error, Result};
pub use format::NONE;
pub use snapshot::{
    ClaimIx, ConceptEdge, ConceptEdgeStat, Csr, Dir, EdgeRef, EdgeSlice, EvidenceSummary, GroupIx, NodeIx, ProvRef,
    Snapshot, SourceIx, StrId,
};
pub use stats::SnapshotStats;
pub use vocab::{CURRENT_STATES_MASK, EntityType, EvidenceState, Projection, RELATIONS, RelationSpec, RelationType, Role};
