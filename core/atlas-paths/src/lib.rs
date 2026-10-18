//! atlas-paths: typed routes between two entities of the snapshot, with what each
//! step and the whole route authorize.
//!
//! - `algebra`: shapes/composition.json as a checked monoid (associative, monotone
//!   non-strengthening, R1–R11).  A route's conclusion is the product of its steps.
//! - `derive`: AND/OR closure for conjunctive premises (R7) and alternative proofs.
//! - `search`: bidirectional label-setting A* over (node, kind) with ALT potentials,
//!   hop limits and an expansion budget; k routes by the penalty method.
//! - `explain`: per-step justification, conclusion, abstention with reasons.
//! - `concept`: concept-first routes over the aggregated concept edges, with the
//!   declaration-level witness of each concept step (drill-down).
//!
//! Floor rules (SPEC §3): only current claims; `disputed` only on request;
//! `analogous_to` degrades to hypothesis; a conjunction counts only when every
//! premise is satisfied; the score is a weighted sum of ordinal terms, never a
//! product of edge probabilities.  Abstain rather than invent a route.

pub mod algebra;
pub mod concept;
pub mod derive;
mod explain;
pub mod index;
pub mod search;

use atlas_core::{Error, EvidenceState, RelationType, Result, Snapshot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use algebra::{CompositionTable, Way};
pub use concept::{ConceptGraph, ConceptLevel};
pub use index::GraphIndex;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    FewestNew,
    BestSupported,
    MostIntuitive,
    MostFormal,
    MostDiverse,
    Shortest,
}

impl Objective {
    pub const ALL: [Objective; 6] = [
        Objective::FewestNew,
        Objective::BestSupported,
        Objective::MostIntuitive,
        Objective::MostFormal,
        Objective::MostDiverse,
        Objective::Shortest,
    ];
}

/// Parsed `/api/path?from=&to=&objective=best_supported&max_hops=6&lang=es`.
#[derive(Clone, Debug, Deserialize)]
pub struct PathQuery {
    pub from: String,
    pub to: String,
    #[serde(default = "default_objective")]
    pub objective: Objective,
    #[serde(default = "default_max_hops")]
    pub max_hops: u32,
    /// Use `disputed` claims (shown as such).  Off by default (rule R8).
    #[serde(default)]
    pub include_disputed: bool,
    #[serde(default = "default_lang")]
    pub lang: String,
    /// Visible, adjustable weights; empty = the objective's defaults.  Given keys
    /// override the defaults one by one (composition.json `weights.terms`).
    #[serde(default)]
    pub weights: BTreeMap<String, f64>,
}

fn default_objective() -> Objective {
    Objective::BestSupported
}

fn default_max_hops() -> u32 {
    6
}

fn default_lang() -> String {
    "es".into()
}

/// Options beyond the SPEC §15 parameters (same query string; all optional).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct PathOptions {
    /// Number of alternative routes (penalty method).
    pub k: usize,
    /// Weakest conclusion worth returning (0 shows bibliographic/historical reach).
    pub min_strength: u8,
    /// R7 strict: reject a conjunctive step with an unsatisfied premise instead of
    /// returning the route as incomplete.
    pub strict_conjunctions: bool,
    /// Entity ids the reader already knows (cost 0 under `new`; count as satisfied premises).
    pub known: Vec<String>,
    /// Popped labels per search run before giving up (the answer then says so).
    pub max_expansions: usize,
    /// Edge scans for the forward closure of what the start establishes (R7); past
    /// it, premises are decided by bounded backward chaining (see `derive`).
    pub derive_budget: usize,
    /// Alternative routes (2nd..k-th) cost at most this times the best route's cost
    /// (penalized cost, as the penalty method ranks them).  Bounds the work of the
    /// later runs; the response says when it cut alternatives.
    pub max_stretch: f64,
    /// Expansions for each alternative run (2nd..k-th): this factor times the
    /// first run's, at least `alt_min_expansions`, at most `max_expansions`.  A run
    /// cut by it sets `alternatives_capped`; the first route stays exact.
    pub alt_budget_factor: f64,
    pub alt_min_expansions: usize,
    /// Ablation switches for benchmarks; both on by default.
    pub bidirectional: bool,
    pub heuristic: bool,
    /// Search the concept graph first (aggregated concept edges) and answer at
    /// concept level in `PathResponse.concepts`, when both endpoints map to concepts.
    pub concepts: bool,
    /// Drill-down: explain the declaration-level claim that witnesses each concept
    /// step as a full route (`ConceptStep.drill_down`).  Off: witness id, sources and
    /// memberships only.
    pub drill_down: bool,
}

impl Default for PathOptions {
    fn default() -> Self {
        PathOptions {
            k: 3,
            min_strength: 1,
            strict_conjunctions: false,
            known: Vec::new(),
            max_expansions: 1_000_000,
            derive_budget: 200_000,
            max_stretch: 2.0,
            alt_budget_factor: 4.0,
            alt_min_expansions: 5_000,
            bidirectional: true,
            heuristic: true,
            concepts: true,
            drill_down: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct PathResponse {
    pub from: String,
    pub to: String,
    pub objective: Objective,
    /// The weights actually used (objective defaults plus overrides).
    pub weights: BTreeMap<String, f64>,
    /// Concept-first answer (searched before the declaration/entity level), or None
    /// when `PathOptions.concepts` is off.  `abstained`, `paths` and `exploration`
    /// below are the entity-level answer and do not depend on it.
    pub concepts: Option<ConceptLevel>,
    pub abstained: bool,
    pub reason: Option<String>,
    /// Machine-readable abstention cause: disconnected, coverage_unknown,
    /// only_weak (only conclusions below min_strength), unauthorized (connected
    /// but no composition authorizes a route; it may name a step walked against its
    /// direction), disputed_only (needs include_disputed), hop_limit, budget,
    /// strict_conjunction.
    pub abstention: Option<String>,
    pub paths: Vec<FoundPath>,
    /// When abstaining: the closest connection found, shown as exploration only.
    pub exploration: Option<FoundPath>,
    pub composition_version: u32,
    pub stats: SearchStats,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchStats {
    pub algorithm: &'static str,
    pub runs: usize,
    pub expanded: usize,
    pub relaxed: usize,
    pub labels: usize,
    pub truncated: bool,
    /// Fewer than k routes because no further one was within `max_stretch` or
    /// within the alternative runs' expansion budget (`alt_budget_factor`).
    pub alternatives_capped: bool,
    pub conjunctions_rejected: usize,
    /// Size of the set established from the start (R7), if it was needed.
    pub derivable: Option<usize>,
    pub derivable_complete: Option<bool>,
    /// Backward AND/OR searches run for premises outside a partial forward closure.
    pub premise_proofs: usize,
    /// Microseconds spent deciding premises (forward closure plus backward searches).
    pub premise_micros: u64,
    pub micros: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FoundPath {
    /// Route cost (lower is better): sum of weighted step terms plus the conclusion term.
    pub score: f64,
    /// Unweighted totals per cost term, so the ranking can be read.
    pub cost_terms: BTreeMap<String, f64>,
    pub conclusion: Conclusion,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Conclusion {
    pub authorizes: String,
    pub assumptions: Vec<String>,
    pub weakest_evidence: EvidenceState,
    pub is_hypothesis: bool,
    /// Name in shapes/composition.json `conclusions`.
    pub kind: String,
    /// What the reader may call the route, from the conclusion's family:
    /// mathematical, structural, application, pedagogical, candidate_hypothesis,
    /// historical, documentary, none.  A `historical` route is an answer only as a
    /// historical connection, never a dependency, prerequisite or application.
    pub category: &'static str,
    /// 0..=5 (composition.json `strength_scale`).
    pub strength: u8,
    pub mathematical: bool,
    /// False when a conjunctive step has unsatisfied premises (R7).
    pub complete: bool,
    pub missing_premises: Vec<String>,
    pub caveats: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Forward,
    /// Walked against the stored direction, via `RelationSpec.inverse`, symmetry
    /// or a converse reading declared in composition.json.
    Inverse,
}

#[derive(Clone, Debug, Serialize)]
pub struct Step {
    pub claim_id: String,
    pub relation: RelationType,
    pub from: String,
    pub to: String,
    pub from_label: String,
    pub to_label: String,
    pub direction: Direction,
    pub evidence_state: EvidenceState,
    pub authorizes: String,
    /// Conclusion of the route up to and including this step.
    pub route_so_far: String,
    pub assumptions: Vec<String>,
    pub premises: Vec<String>,
    pub premises_missing: Vec<String>,
    /// How each satisfied premise is established: start, known, or derived (claims).
    pub premises_satisfied: Vec<PremiseSupport>,
    pub via: Vec<String>,
    pub context: Vec<String>,
    pub validity_domain: Option<String>,
    /// Other current claims of the same alternative group (OR-branches).
    pub alternatives: Vec<String>,
    pub sources: Vec<SourceRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PremiseSupport {
    pub premise: String,
    pub how: &'static str,
    pub claims: Vec<String>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceRef {
    pub source_id: String,
    pub locator: String,
    pub license: String,
}

/// Engine construction options.
#[derive(Clone, Copy, Debug)]
pub struct EngineOptions {
    /// ALT landmarks (0 disables the heuristic's lower bounds).
    pub landmarks: usize,
}

impl Default for EngineOptions {
    fn default() -> Self {
        EngineOptions { landmarks: 16 }
    }
}

pub struct PathEngine<'s> {
    pub snapshot: &'s Snapshot,
    pub table: CompositionTable,
    pub index: GraphIndex,
    pub concepts: ConceptGraph,
}

impl<'s> PathEngine<'s> {
    pub fn new(snapshot: &'s Snapshot, table: CompositionTable) -> Self {
        Self::with_options(snapshot, table, EngineOptions::default())
    }

    pub fn with_options(snapshot: &'s Snapshot, table: CompositionTable, opts: EngineOptions) -> Self {
        let (index, concepts) =
            rayon::join(|| GraphIndex::build_typed(snapshot, opts.landmarks, &table), || ConceptGraph::build(snapshot));
        PathEngine { snapshot, table, index, concepts }
    }

    pub fn find(&self, query: &PathQuery) -> Result<PathResponse> {
        self.find_with(query, &PathOptions::default())
    }

    pub fn find_with(&self, query: &PathQuery, opts: &PathOptions) -> Result<PathResponse> {
        explain::run(self, query, opts)
    }

    /// Effective weights: the objective's defaults, overridden key by key.
    pub fn weights(&self, query: &PathQuery) -> Result<BTreeMap<String, f64>> {
        let mut w = self.table.objective_weights(query.objective).clone();
        for (k, v) in &query.weights {
            if !algebra::WEIGHT_TERMS.contains(&k.as_str()) {
                return Err(Error::BadRequest(format!(
                    "unknown weight {k:?}; known: {}",
                    algebra::WEIGHT_TERMS.join(", ")
                )));
            }
            if !v.is_finite() || *v < 0.0 {
                return Err(Error::BadRequest(format!("weight {k} must be finite and >= 0")));
            }
            w.insert(k.clone(), *v);
        }
        Ok(w)
    }
}
