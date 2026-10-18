//! The path engine on the adversarial fixture (tests/fixtures/paths_fixture.db, built
//! by make_fixture.py through the Python Store).  Each test is one way a route can
//! pretend to authorize more than it does; the engine must reject or downgrade it.

use atlas_core::{CompileOptions, EvidenceState, RelationType, Snapshot, compile};
use atlas_paths::{CompositionTable, Direction, Objective, PathEngine, PathOptions, PathQuery, PathResponse};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn manifest() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn snapshot() -> &'static Snapshot {
    static SNAP: OnceLock<Snapshot> = OnceLock::new();
    SNAP.get_or_init(|| {
        let out = std::env::temp_dir().join(format!("atlas-paths-engine-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let opts = CompileOptions::new(manifest().join("tests/fixtures/paths_fixture.db"), &out);
        compile(&opts).expect("compile fixture");
        let s = Snapshot::open(&out).expect("open snapshot");
        s.verify().expect("fixture snapshot verifies");
        s
    })
}

fn table() -> CompositionTable {
    CompositionTable::load(&composition_path()).expect("shipped table loads")
}

fn composition_path() -> PathBuf {
    manifest().join("../../shapes/composition.json")
}

fn engine() -> PathEngine<'static> {
    PathEngine::new(snapshot(), table())
}

fn q(from: &str, to: &str, objective: Objective) -> PathQuery {
    PathQuery {
        from: from.into(),
        to: to.into(),
        objective,
        max_hops: 6,
        include_disputed: false,
        lang: "en".into(),
        weights: Default::default(),
    }
}

fn run(from: &str, to: &str) -> PathResponse {
    engine().find(&q(from, to, Objective::BestSupported)).expect("query runs")
}

fn run_with(from: &str, to: &str, opts: &PathOptions) -> PathResponse {
    engine().find_with(&q(from, to, Objective::BestSupported), opts).expect("query runs")
}

fn kinds(r: &PathResponse) -> Vec<String> {
    r.paths.iter().map(|p| p.conclusion.kind.clone()).collect()
}

fn claims(r: &PathResponse, i: usize) -> Vec<String> {
    r.paths[i].steps.iter().map(|s| s.claim_id.clone()).collect()
}

// ---------------------------------------------------------------- analogies

#[test]
fn analogy_then_application_is_only_a_transfer_hypothesis() {
    let r = run("s:x-analog", "app:surveying");
    assert!(!r.abstained, "{:?}", r.reason);
    assert!(!kinds(&r).iter().any(|k| k == "application" || k == "application_via_special_case"), "{:?}", kinds(&r));
    let c = &r.paths[0].conclusion;
    assert_eq!(c.kind, "transfer_hypothesis");
    assert!(c.is_hypothesis && !c.mathematical);
    assert_eq!(c.strength, 1);
    assert_eq!(claims(&r, 0), ["c:x-analog-pyth", "c:pyth-surveying"]);
}

#[test]
fn two_analogies_conclude_nothing() {
    let r = run("s:x-analog2", "app:surveying");
    assert!(r.abstained && r.paths.is_empty());
    assert_eq!(r.abstention.as_deref(), Some("unauthorized"));
    assert!(r.reason.as_deref().unwrap().contains("R10"), "{:?}", r.reason);
    let e = r.exploration.as_ref().expect("raw connection shown as exploration");
    assert_eq!(e.conclusion.kind, "none");
    assert_eq!(e.conclusion.strength, 0);
}

#[test]
fn analogy_degrades_a_logical_chain_to_hypothesis() {
    // x-analog ~ pythagoras, pythagoras depends_on (similar triangles): a hypothesis, not a dependency
    let r = run("s:x-analog", "s:similar-triangles");
    assert!(!r.abstained, "{:?}", r.reason);
    for p in &r.paths {
        assert!(p.conclusion.is_hypothesis && p.conclusion.strength <= 1, "{:?}", p.conclusion);
        assert!(!p.conclusion.mathematical);
    }
}

// ---------------------------------------------------------------- citations

#[test]
fn citation_chain_authorizes_nothing_semantic() {
    let r = run("s:x-cited", "s:pythagoras");
    assert!(r.abstained);
    assert_eq!(r.abstention.as_deref(), Some("only_weak"));
    let e = r.exploration.as_ref().unwrap();
    assert_eq!(e.conclusion.kind, "bibliographic");
    assert_eq!(e.conclusion.strength, 0);
    assert!(!e.conclusion.mathematical);

    let r = run_with("s:x-cited", "s:pythagoras", &PathOptions { min_strength: 0, ..Default::default() });
    assert!(!r.abstained);
    let p = &r.paths[0];
    assert_eq!(p.conclusion.kind, "bibliographic");
    let rels: Vec<_> = p.steps.iter().map(|s| (s.relation.as_str(), s.direction)).collect();
    assert_eq!(
        rels,
        [
            ("appears_in", Direction::Forward),
            ("cites", Direction::Forward),
            ("cites", Direction::Forward),
            ("appears_in", Direction::Inverse)
        ]
    );
}

// ---------------------------------------------------------------- history (SPEC §3, 2026-10-17)

#[test]
fn historical_chain_is_an_answer_labelled_historical() {
    let r = run("s:x-nm1", "s:x-nm2");
    assert!(!r.abstained && r.abstention.is_none(), "{:?}", r.reason);
    assert!(r.exploration.is_none());
    let c = &r.paths[0].conclusion;
    assert_eq!((c.kind.as_str(), c.category, c.strength), ("historical", "historical", 0));
    assert!(!c.mathematical && !c.is_hypothesis);
    assert!(c.caveats[0].contains("not a mathematical dependency"), "{:?}", c.caveats);
    assert_eq!(claims(&r, 0), ["c:x-nm1-named", "c:x-nm2-named"]);
}

#[test]
fn historical_chain_never_upgrades_to_logic() {
    // nm1 -named-> namer <-named- nm2 -depends_on-> axiom: R4, no conclusion at all
    let r = run("s:x-nm1", "s:x-nm-axiom");
    assert!(r.abstained && r.paths.is_empty(), "{:?}", kinds(&r));
    assert_eq!(r.abstention.as_deref(), Some("unauthorized"));
    assert!(r.reason.as_deref().unwrap().contains("R4"), "{:?}", r.reason);
    assert_eq!(r.exploration.as_ref().unwrap().conclusion.category, "none");
}

#[test]
fn mathematical_route_outranks_an_equally_long_historical_one() {
    let r = run("s:x-nm3", "s:x-nm2");
    assert!(!r.abstained, "{:?}", r.reason);
    assert_eq!(r.paths[0].conclusion.kind, "logical_dependency");
    assert_eq!(r.paths[0].conclusion.category, "mathematical");
    assert!(r.paths.iter().all(|p| p.conclusion.category != "historical"), "{:?}", kinds(&r));
}

#[test]
fn bibliographic_reach_still_abstains() {
    let r = run("s:x-cited", "s:pythagoras");
    assert_eq!(r.abstention.as_deref(), Some("only_weak"));
    assert_eq!(r.exploration.as_ref().unwrap().conclusion.category, "documentary");
}

#[test]
fn citation_followed_by_logic_is_unauthorized() {
    let r = run_with("pub:x-survey-a", "s:sas-congruence", &PathOptions { min_strength: 0, ..Default::default() });
    assert!(r.abstained, "{:?}", kinds(&r));
    assert_eq!(r.abstention.as_deref(), Some("unauthorized"));
    assert!(r.reason.as_deref().unwrap().contains("R4"), "{:?}", r.reason);
}

// ---------------------------------------------------------------- prerequisite vs proof

#[test]
fn prerequisite_then_use_is_a_study_route_never_logic() {
    // SPEC section 15 example: right triangle -> converse of Pythagoras
    let r = run("c:right-triangle", "s:pythagoras-converse");
    assert!(!r.abstained, "{:?}", r.reason);
    for p in &r.paths {
        assert_eq!(p.conclusion.kind, "study_route");
        assert_eq!(p.conclusion.strength, 2);
        assert!(!p.conclusion.mathematical);
        assert!(p.conclusion.authorizes.contains("never a logical necessity"));
    }
    let s = &r.paths[0].steps;
    assert_eq!(s[0].relation.as_str(), "prerequisite_for");
    assert_eq!((s[1].relation.as_str(), s[1].direction), ("depends_on_logically", Direction::Inverse));
}

#[test]
fn prerequisite_then_dependency_in_reading_order_is_rejected() {
    // angle before lemma, lemma needs axiom: says nothing about angle and axiom (R5)
    let r = run_with("d:x-angle", "s:x-axiom", &PathOptions { min_strength: 0, ..Default::default() });
    assert!(r.abstained, "{:?}", kinds(&r));
    assert_eq!(r.abstention.as_deref(), Some("unauthorized"));
    assert!(r.reason.as_deref().unwrap().contains("R5"), "{:?}", r.reason);
}

#[test]
fn prerequisite_is_not_walked_backwards() {
    let r = run_with("s:x-axiom", "d:x-angle", &PathOptions { min_strength: 0, ..Default::default() });
    assert!(r.abstained);
    assert_eq!(r.abstention.as_deref(), Some("unauthorized"));
    assert!(r.reason.as_deref().unwrap().contains("direction"), "{:?}", r.reason);
}

// ---------------------------------------------------------------- special cases

#[test]
fn generalization_transfers_application_but_special_case_does_not() {
    let r = run("s:law-of-cosines", "app:surveying");
    assert!(!r.abstained, "{:?}", r.reason);
    assert_eq!(r.paths[0].conclusion.kind, "application_via_special_case");

    let r = run_with("s:x-special", "app:surveying", &PathOptions { min_strength: 0, ..Default::default() });
    assert!(r.abstained, "{:?}", kinds(&r));
    assert_eq!(r.abstention.as_deref(), Some("unauthorized"));
}

// ---------------------------------------------------------------- conjunctions (R7)

#[test]
fn missing_conjunct_makes_the_route_incomplete() {
    // SAS alone does not give Euclid's I.47: the parallelogram premise is missing
    let r = run("s:sas-congruence", "s:pythagoras");
    assert!(!r.abstained, "{:?}", r.reason);
    let c = &r.paths[0].conclusion;
    assert!(!c.complete);
    assert_eq!(c.missing_premises, ["s:parallelogram-area"]);
    assert!(c.assumptions.iter().any(|a| a.contains("unsatisfied premise")), "{:?}", c.assumptions);
    assert_eq!(r.paths[0].steps[0].premises_missing, ["s:parallelogram-area"]);
    assert!(r.paths[0].cost_terms["missing_premise"] >= 1.0);
}

#[test]
fn strict_mode_rejects_a_missing_conjunct_and_explains() {
    let opts = PathOptions { strict_conjunctions: true, ..Default::default() };
    let r = run_with("s:sas-congruence", "s:pythagoras", &opts);
    assert!(r.abstained && r.paths.is_empty());
    assert_eq!(r.abstention.as_deref(), Some("strict_conjunction"));
    assert!(r.reason.as_deref().unwrap().contains("s:parallelogram-area"));
    assert!(r.stats.conjunctions_rejected > 0);
}

#[test]
fn known_premise_completes_the_conjunction() {
    let opts = PathOptions { known: vec!["s:parallelogram-area".into()], strict_conjunctions: true, ..Default::default() };
    let r = run_with("s:sas-congruence", "s:pythagoras", &opts);
    assert!(!r.abstained, "{:?}", r.reason);
    let c = &r.paths[0].conclusion;
    assert!(c.complete);
    assert_eq!(c.kind, "logical_support");
    let sat = &r.paths[0].steps[0].premises_satisfied;
    assert_eq!(sat.len(), 1);
    assert_eq!((sat[0].premise.as_str(), sat[0].how), ("s:parallelogram-area", "known"));
}

#[test]
fn derived_premise_completes_the_conjunction() {
    // P gives Q and R; Goal needs Q and R
    let r = run_with("s:x-p", "s:x-goal", &PathOptions { strict_conjunctions: true, ..Default::default() });
    assert!(!r.abstained, "{:?}", r.reason);
    let p = &r.paths[0];
    assert!(p.conclusion.complete);
    assert_eq!(p.conclusion.kind, "logical_support");
    let last = p.steps.last().unwrap();
    assert_eq!(last.claim_id, "c:x-goal");
    assert_eq!(last.premises_satisfied.len(), 1);
    assert_eq!(last.premises_satisfied[0].how, "derived");
    assert!(r.stats.derivable.unwrap() >= 4);

    // from Q alone, R is missing
    let r = run("s:x-q", "s:x-goal");
    assert_eq!(r.paths[0].conclusion.missing_premises, ["s:x-r"]);
}

#[test]
fn backward_chaining_decides_premises_when_the_forward_closure_is_cut() {
    // derive_budget 0: the forward closure stops at once; premises are proven backwards
    let opts = PathOptions { derive_budget: 0, strict_conjunctions: true, ..Default::default() };
    let r = run_with("s:x-p", "s:x-goal", &opts);
    assert!(!r.abstained, "{:?}", r.reason);
    assert_eq!(r.stats.derivable_complete, Some(false));
    let last = r.paths[0].steps.last().unwrap();
    assert_eq!(last.premises_satisfied[0].how, "derived");
    assert_eq!(last.premises_satisfied[0].claims, ["c:x-r-dep"]);

    let r = run_with("s:x-q", "s:x-goal", &PathOptions { derive_budget: 0, ..Default::default() });
    assert_eq!(r.paths[0].conclusion.missing_premises, ["s:x-r"]);
    // (from Q the forward closure ends on its first scan, so it is exact: no "bounds" caveat)
    assert_eq!(r.stats.derivable_complete, Some(true));

    // the AND/OR backward search itself: the parallel postulate gives similar triangles
    // (forward, first scan), hence Pythagoras by the similarity proof (OR-branch), hence
    // its converse; SAS has no derivation, so Euclid's branch (AND) cannot fire
    let snap = snapshot();
    let n = |id: &str| snap.resolve(id).unwrap();
    let mask = atlas_core::CURRENT_STATES_MASK & !EvidenceState::Disputed.bit();
    let d = atlas_paths::derive::closure(snap, n("s:parallel-postulate"), &[], mask, 0);
    assert!(!d.complete && !d.in_forward(n("s:pythagoras")));
    assert!(d.contains(n("s:pythagoras")) && d.contains(n("s:pythagoras-converse")));
    assert!(!d.contains(n("s:sas-congruence")));
    let (claims, _) = d.derivation(n("s:pythagoras-converse"), 10);
    let ids: Vec<&str> = claims.iter().map(|&c| snap.claim_id(c)).collect();
    assert_eq!(ids, ["c:converse-uses", "c:pyth-similarity", "c:similar-parallel"]);
    assert!(d.backward_checks.get() >= 2);
}

#[test]
fn complete_alternative_proof_beats_incomplete_one() {
    // parallel postulate -> Pythagoras: through similarity (complete) or Euclid (SAS missing)
    let opts = PathOptions { k: 3, ..Default::default() };
    let r = run_with("s:parallel-postulate", "s:pythagoras", &opts);
    assert!(!r.abstained, "{:?}", r.reason);
    assert!(r.paths[0].conclusion.complete);
    assert_eq!(claims(&r, 0), ["c:similar-parallel", "c:pyth-similarity"]);
    let euclid = r.paths.iter().find(|p| p.steps.iter().any(|s| s.claim_id == "c:pyth-euclid")).expect("second route");
    assert!(!euclid.conclusion.complete);
    assert_eq!(euclid.conclusion.missing_premises, ["s:sas-congruence"]);
    assert!(euclid.steps.last().unwrap().alternatives.contains(&"c:pyth-similarity".to_string()));
}

#[test]
fn forward_dependency_through_an_alternative_proof_is_not_necessary() {
    let r = run("s:pythagoras", "s:sas-congruence");
    assert!(!r.abstained, "{:?}", r.reason);
    let c = &r.paths[0].conclusion;
    assert_eq!(c.kind, "logical_dependency");
    assert!(c.caveats.iter().any(|x| x.contains("R11") && x.contains("c:pyth-similarity")), "{:?}", c.caveats);
}

// ---------------------------------------------------------------- evidence states and objectives

#[test]
fn disputed_claims_need_an_explicit_request() {
    let r = run("s:x-d1", "s:x-d2");
    assert!(r.abstained);
    assert_eq!(r.abstention.as_deref(), Some("disputed_only"));
    assert!(r.reason.as_deref().unwrap().contains("c:x-d"));

    let mut query = q("s:x-d1", "s:x-d2", Objective::BestSupported);
    query.include_disputed = true;
    let r = engine().find(&query).unwrap();
    assert!(!r.abstained);
    let c = &r.paths[0].conclusion;
    assert_eq!(c.weakest_evidence, EvidenceState::Disputed);
    assert!(c.caveats.iter().any(|x| x.contains("disputed")));
}

#[test]
fn objectives_choose_different_routes_and_weights_are_visible() {
    let e = engine();
    let direct = ["c:x-f13".to_string()];
    let formal = ["c:x-f12".to_string(), "c:x-f23".to_string()];
    let first = |o: Objective, w: &[(&str, f64)]| {
        let mut query = q("s:x-f1", "s:x-f3", o);
        query.weights = w.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        let r = e.find(&query).unwrap();
        for (k, v) in w {
            assert_eq!(r.weights[*k], *v, "override shown");
        }
        assert!(r.paths.iter().all(|p| p.conclusion.kind == "generalization"));
        claims(&r, 0)
    };
    assert_eq!(first(Objective::Shortest, &[]), direct);
    assert_eq!(first(Objective::MostFormal, &[]), formal);
    assert_eq!(first(Objective::BestSupported, &[]), formal);
    assert_eq!(first(Objective::BestSupported, &[("length", 5.0)]), direct);

    let mut bad = q("s:x-f1", "s:x-f3", Objective::Shortest);
    bad.weights.insert("speed".into(), 1.0);
    assert!(e.find(&bad).is_err());
    bad.weights.clear();
    bad.weights.insert("length".into(), -1.0);
    assert!(e.find(&bad).is_err());
}

#[test]
fn k_routes_are_distinct_and_ranked() {
    let opts = PathOptions { k: 3, ..Default::default() };
    let r = run_with("s:x-f1", "s:x-f3", &opts);
    assert_eq!(r.paths.len(), 2);
    assert_ne!(claims(&r, 0), claims(&r, 1));
    assert!(r.paths[0].score <= r.paths[1].score);
}

#[test]
fn alternative_budget_caps_alternatives_never_the_first_route() {
    let full = run_with("s:x-f1", "s:x-f3", &PathOptions { k: 3, ..Default::default() });
    let tight = PathOptions { k: 3, alt_budget_factor: 0.0, alt_min_expansions: 0, ..Default::default() };
    let r = run_with("s:x-f1", "s:x-f3", &tight);
    assert_eq!(r.paths.len(), 1);
    assert_eq!(claims(&r, 0), claims(&full, 0));
    assert!(r.stats.alternatives_capped && !r.stats.truncated && !r.abstained);
    assert!(r.paths[0].conclusion.caveats.iter().all(|c| !c.contains("budget")));
    let bad = PathOptions { alt_budget_factor: f64::NAN, ..Default::default() };
    assert!(engine().find_with(&q("s:x-f1", "s:x-f3", Objective::BestSupported), &bad).is_err());
}

#[test]
fn every_step_is_justified() {
    let r = run("s:law-of-cosines", "app:surveying");
    for s in &r.paths[0].steps {
        assert!(!s.claim_id.is_empty() && !s.authorizes.is_empty() && !s.route_so_far.is_empty());
        assert_eq!(s.sources.len(), 1);
        assert_eq!(s.sources[0].source_id, "src:fixture-hand");
        assert_eq!(s.sources[0].license, "CC0-1.0");
        assert_eq!(s.evidence_state, EvidenceState::Extracted);
    }
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["objective"], "best_supported");
    assert_eq!(json["paths"][0]["steps"][0]["direction"], "forward");
    assert!(json["paths"][0]["conclusion"]["strength"].is_u64());
}

// ---------------------------------------------------------------- coverage, merges, inputs

#[test]
fn unknown_coverage_abstains_without_calling_it_isolated() {
    let r = run("c:x-lonely", "s:pythagoras");
    assert!(r.abstained);
    assert_eq!(r.abstention.as_deref(), Some("coverage_unknown"));
    assert!(r.reason.as_deref().unwrap().contains("unknown, not isolated"));
}

#[test]
fn disconnected_components_abstain_instantly() {
    let r = run("s:x-f1", "s:pythagoras");
    assert!(r.abstained);
    assert_eq!(r.abstention.as_deref(), Some("disconnected"));
    assert_eq!(r.stats.runs, 0);
}

#[test]
fn merged_ids_resolve_to_the_survivor() {
    let r = run("c:right-triangle", "s:x-dup");
    assert_eq!(r.to, "s:pythagoras");
    assert!(!r.abstained, "{:?}", r.reason);
    assert_eq!(r.paths[0].conclusion.kind, "study_route");
}

#[test]
fn bad_inputs_are_errors_not_routes() {
    let e = engine();
    assert!(matches!(e.find(&q("s:nope", "s:pythagoras", Objective::Shortest)), Err(atlas_core::Error::NotFound(_))));
    assert!(matches!(e.find(&q("s:pythagoras", "s:x-dup", Objective::Shortest)), Err(atlas_core::Error::BadRequest(_))));
    let mut zero = q("s:x-f1", "s:x-f3", Objective::Shortest);
    zero.max_hops = 0;
    assert!(e.find(&zero).is_err());
}

#[test]
fn hop_limit_is_respected() {
    let mut query = q("s:x-cited", "s:pythagoras", Objective::Shortest);
    query.max_hops = 3; // the only connection has 4 hops
    let r = engine().find_with(&query, &PathOptions { min_strength: 0, ..Default::default() }).unwrap();
    assert!(r.abstained);
    assert_eq!(r.abstention.as_deref(), Some("hop_limit"));
}

#[test]
fn all_objectives_and_search_modes_agree_on_the_fixture() {
    let e = engine();
    let pairs = [
        ("s:law-of-cosines", "app:surveying"),
        ("s:parallel-postulate", "s:pythagoras"),
        ("c:right-triangle", "s:pythagoras-converse"),
        ("s:x-p", "s:x-goal"),
        ("s:x-analog", "app:surveying"),
    ];
    for (a, b) in pairs {
        for o in Objective::ALL {
            let mut scores = Vec::new();
            for (bidirectional, heuristic) in [(true, true), (true, false), (false, true), (false, false)] {
                let opts = PathOptions { k: 1, bidirectional, heuristic, ..Default::default() };
                let r = e.find_with(&q(a, b, o), &opts).unwrap();
                assert!(!r.abstained, "{a} -> {b} {o:?}: {:?}", r.reason);
                scores.push(r.paths[0].score);
            }
            for s in &scores {
                assert!((s - scores[0]).abs() < 1e-9, "{a} -> {b} {o:?}: {scores:?}");
            }
        }
    }
}

// ------------------------------------------------------------------ concept-first routes

fn concepts(from: &str, to: &str, opts: &PathOptions) -> (PathResponse, atlas_paths::ConceptLevel) {
    let r = run_with(from, to, opts);
    let c = r.concepts.clone().expect("concept search on by default");
    (r, c)
}

#[test]
fn concept_route_aggregates_dependencies_but_is_not_a_declaration_chain() {
    let (r, c) = concepts("c:x-ca", "c:x-cc", &PathOptions::default());
    assert!(!c.abstained, "{:?}", c.reason);
    let route = c.route.unwrap();
    assert_eq!(route.conclusion.kind, "logical_dependency");
    let hops: Vec<(&str, &str, &str)> =
        route.steps.iter().map(|s| (s.from.as_str(), s.to.as_str(), s.witness.claim_id.as_str())).collect();
    assert_eq!(hops, [("c:x-ca", "c:x-cb", "c:x-a1-b1"), ("c:x-cb", "c:x-cc", "c:x-b2-c1")]);
    assert!(route.steps.iter().all(|s| s.relation == RelationType::DependsOnLogically && s.direction == Direction::Forward));
    // b1 != b2: the caveat says so; heuristic memberships make the steps model_proposed
    assert!(route.conclusion.caveats.iter().any(|x| x.contains("not a chain")), "{:?}", route.conclusion.caveats);
    assert_eq!(route.conclusion.weakest_evidence, EvidenceState::ModelProposed);
    for s in &route.steps {
        assert!(!s.witness.sources.is_empty());
        assert_eq!(s.witness.memberships.len(), 2);
        assert!(s.drill_down.is_none(), "drill-down is opt-in");
    }
    // no declaration-level route exists: membership never stands in for one
    assert!(r.abstained, "{:?}", r.paths.first().map(|p| &p.conclusion.kind));
}

#[test]
fn concept_route_walked_backwards_is_support_and_flags_conjunctive_witnesses() {
    let (_, c) = concepts("c:x-cc", "c:x-ca", &PathOptions::default());
    let route = c.route.unwrap();
    assert_eq!(route.conclusion.kind, "logical_support");
    assert!(route.steps.iter().all(|s| s.direction == Direction::Inverse));
    let (_, c) = concepts("c:x-cb", "c:x-cd", &PathOptions::default());
    let route = c.route.unwrap();
    assert_eq!(route.steps[0].witness.claim_id, "c:x-dc1-dep");
    assert_eq!(route.steps[0].witness.other_premises, 1);
    assert!(route.conclusion.caveats.iter().any(|x| x.contains("R7")), "{:?}", route.conclusion.caveats);
}

#[test]
fn concept_route_never_mixes_dependency_and_support_through_a_shared_concept() {
    // A -> B <- D (and A -> B -> C <- D): dependency then support composes to none
    let (_, c) = concepts("c:x-ca", "c:x-cd", &PathOptions::default());
    assert!(c.abstained && c.route.is_none());
    assert_eq!(c.abstention.as_deref(), Some("no_route"));
}

#[test]
fn concept_endpoints_map_declarations_to_their_primary_concept() {
    let (_, c) = concepts("s:x-a2", "c:x-cc", &PathOptions::default());
    assert_eq!((c.from.via, c.from.concept.as_deref()), ("primary_concept", Some("c:x-ca")));
    assert_eq!((c.to.via, c.to.concept.as_deref()), ("concept", Some("c:x-cc")));
    assert!(!c.abstained);
    let (_, c) = concepts("s:x-a1", "s:x-a2", &PathOptions::default());
    assert_eq!(c.abstention.as_deref(), Some("same_concept"));
    let (_, c) = concepts("s:pythagoras", "s:x-axiom", &PathOptions::default());
    assert_eq!(c.abstention.as_deref(), Some("not_a_concept"));
    let r = run_with("s:x-a1", "s:x-b1", &PathOptions { concepts: false, ..Default::default() });
    assert!(r.concepts.is_none());
}

#[test]
fn drill_down_explains_each_witness_as_a_declaration_route_with_sources() {
    let (_, c) = concepts("c:x-ca", "c:x-cc", &PathOptions { drill_down: true, ..Default::default() });
    for s in c.route.unwrap().steps {
        let d = s.drill_down.expect("drill-down requested");
        assert_eq!(d.steps.len(), 1);
        assert_eq!(d.steps[0].claim_id, s.witness.claim_id);
        assert_eq!((d.steps[0].from.as_str(), d.steps[0].to.as_str()), (s.witness.from.as_str(), s.witness.to.as_str()));
        assert_eq!(d.conclusion.kind, "logical_dependency");
        assert!(!d.steps[0].sources.is_empty());
    }
}

#[test]
fn membership_alone_never_authorizes_an_entity_level_conclusion() {
    // s:x-a1 member_of_concept c:x-ca: concludes none, so it is never walked; the
    // membership shows only as an unauthorized exploration, and at concept level
    let r = run("s:x-a1", "c:x-ca");
    assert!(r.abstained);
    assert_eq!(r.abstention.as_deref(), Some("unauthorized"));
    assert!(r.reason.as_deref().unwrap().contains("never walked"), "{:?}", r.reason);
    assert_eq!(r.exploration.unwrap().conclusion.category, "none");
    assert_eq!(r.concepts.unwrap().abstention.as_deref(), Some("same_concept"));
    let r = run("s:x-a2", "s:x-b1");
    assert!(r.abstained, "{:?}", r.paths.first().map(|p| &p.conclusion.kind));
}
