//! Concept layer end to end: membership claims in a registry -> `atlas compile` -> snapshot API.
//! Starts from the committed fixture (tests/fixtures/mini_atlas.db) plus three `member_of_concept` claims.

use atlas_core::{CompileOptions, Dir, EvidenceState, RelationType, Snapshot, compile};
use rusqlite::{Connection, params};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("atlas-core-concepts-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// (claim, declaration, concept, confidence)
const MEMBERS: [(&str, &str, &str, f64); 4] = [
    ("c:m1", "s:pythagoras", "c:right-triangle", 0.9),
    ("c:m2", "s:pythagoras", "c:lonely", 0.5),
    ("c:m3", "s:sas-congruence", "c:lonely", 0.8),
    ("c:m4", "s:parallelogram-area", "c:lonely", 0.7),
];

fn registry_with_memberships(dir: &std::path::Path) -> PathBuf {
    let db = dir.join("atlas.db");
    std::fs::copy(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini_atlas.db"), &db).unwrap();
    let c = Connection::open(&db).unwrap();
    for (id, s, o, conf) in MEMBERS {
        let data = format!(
            r#"{{"id":"{id}","relation":"member_of_concept","participants":[{{"role":"subject","entity":"{s}"}},{{"role":"object","entity":"{o}"}}],"evidence_state":"model_proposed","confidence":{{"value":{conf},"estimates":"e","calibration":"not calibrated"}}}}"#
        );
        c.execute("INSERT INTO claims VALUES (?1,'member_of_concept','model_proposed',NULL,NULL,NULL,NULL,?2,'t')",
                  params![id, data]).unwrap();
        c.execute("INSERT INTO claim_participants VALUES (?1,'subject',?2,0)", params![id, s]).unwrap();
        c.execute("INSERT INTO claim_participants VALUES (?1,'object',?2,1)", params![id, o]).unwrap();
        c.execute("INSERT INTO provenance VALUES (?1,NULL,'src:fixture-hand','test','concept-projection','2','CC0-1.0','t',NULL)",
                  params![id]).unwrap();
    }
    c.execute_batch("PRAGMA user_version = 2").unwrap();
    db
}

fn build(name: &str) -> (Snapshot, atlas_core::CompileReport) {
    let dir = scratch(name);
    let db = registry_with_memberships(&dir);
    let mut opts = CompileOptions::new(db, dir.join("snap"));
    opts.threads = 4;
    let r = compile(&opts).expect("compile");
    (Snapshot::open(&opts.out_dir).expect("open"), r)
}

#[test]
fn members_primary_concepts_and_aggregated_edges() {
    let (s, r) = build("api");
    s.verify().expect("verifies");
    let n = |id: &str| s.resolve(id).unwrap();
    let (rt, lonely) = (n("c:right-triangle"), n("c:lonely"));
    assert_eq!((r.concepts, r.declarations_with_concept), (2, 3));
    assert_eq!(s.concepts(), &{ let mut v = vec![rt, lonely]; v.sort(); v }[..]);
    assert_eq!(s.primary_concept(n("s:pythagoras")), Some(rt), "highest confidence wins");
    assert_eq!(s.primary_concept(n("s:law-of-cosines")), None);
    assert_eq!(s.concept_members(rt), &[n("s:pythagoras")]);
    assert_eq!(s.concept_members(lonely).len(), 2);
    assert_eq!(s.concept_memberships(lonely).len(), 3, "secondary membership of s:pythagoras is kept");
    assert!(!s.is_derived_concept(rt));

    // aggregated edges recounted independently from the CSR: primary member -> primary member, other concept,
    // with the weakest of the edge and the two primary memberships (ADR-0005 §7a)
    let membership = |m: u32| {
        let c = s.primary_concept(m).unwrap();
        s.csr(Dir::Out).edges_rel(m, RelationType::MemberOfConcept).iter().find(|e| e.nbr == c).unwrap().state
    };
    let mut want: BTreeMap<(u32, u32, RelationType, EvidenceState), u32> = BTreeMap::new();
    for &c in s.concepts() {
        for &m in s.concept_members(c) {
            for e in s.csr(Dir::Out).edges(m).iter() {
                if e.relation == RelationType::MemberOfConcept { continue; }
                if let Some(t) = s.primary_concept(e.nbr).filter(|&t| t != c) {
                    let state = e.state.weakest(membership(m)).weakest(membership(e.nbr));
                    *want.entry((c, t, e.relation, state)).or_default() += 1;
                }
            }
        }
    }
    let mut got = BTreeMap::new();
    for &c in s.concepts() {
        for e in s.concept_edges(c) {
            assert_eq!(e.total(), e.stats().map(|x| x.count).sum::<u32>());
            for st in e.stats() {
                got.insert((c, e.target, st.relation, st.state), st.count);
                let ex = s.claim_relation(st.example);
                assert_eq!(ex, st.relation, "the example claim carries the counted relation");
            }
        }
    }
    assert_eq!(got, want);
    assert_eq!(got.get(&(rt, lonely, RelationType::DependsOnLogically, EvidenceState::ModelProposed)), Some(&2),
               "Euclid's proof: two extracted premises in c:lonely, through model_proposed memberships");
    assert!(got.keys().all(|k| k.3 == EvidenceState::ModelProposed), "never stronger than the memberships");
    assert!(s.concept_edges(rt).all(|e| e.is_candidate()));
    assert_eq!(s.num_concept_edges(), 1);
}

#[test]
fn concept_layer_is_deterministic() {
    let (_, a) = build("det-a");
    let (_, b) = build("det-b");
    assert_eq!(a.content_hash, b.content_hash);
}
