//! The Rust vocabulary must match atlas/model.py (parsed as text) and
//! shapes/vocab.json (generated from it); shapes/composition.json may only name
//! known relations, states and conclusions.

use atlas_core::vocab::PRESENTATION_SCOPES;
use atlas_core::{EntityType, EvidenceState, Projection, RelationType, Role};
use serde_json::Value;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Values of `class <name>(StrEnum):` in model.py, in declaration order.
fn py_enum(src: &str, name: &str) -> Vec<String> {
    let head = format!("class {name}(StrEnum):");
    let body = src.split(&head).nth(1).unwrap_or_else(|| panic!("{name} not found in model.py"));
    body.lines()
        .skip(1)
        .take_while(|l| l.is_empty() || l.starts_with(' '))
        .filter_map(|l| {
            let (_, rhs) = l.split_once(" = \"")?;
            Some(rhs.split('"').next()?.to_owned())
        })
        .collect()
}

fn strs<T: Copy>(all: &[T], f: fn(T) -> &'static str) -> Vec<String> {
    all.iter().map(|x| f(*x).to_owned()).collect()
}

#[test]
fn enums_match_model_py() {
    let src = read("atlas/model.py");
    assert_eq!(py_enum(&src, "EntityType"), strs(EntityType::ALL, EntityType::as_str));
    assert_eq!(py_enum(&src, "RelationType"), strs(RelationType::ALL, RelationType::as_str));
    assert_eq!(py_enum(&src, "EvidenceState"), strs(EvidenceState::ALL, EvidenceState::as_str));
    assert_eq!(py_enum(&src, "Role"), strs(Role::ALL, Role::as_str));
    assert_eq!(py_enum(&src, "Projection"), strs(Projection::ALL, Projection::as_str));
}

#[test]
fn relation_specs_match_model_py() {
    let src = read("atlas/model.py");
    let calls: Vec<&str> = src.split("_spec(R.").skip(1).map(|c| c.split("chain=").next().unwrap()).collect();
    assert_eq!(calls.len(), RelationType::COUNT, "one _spec(...) per relation");
    for call in calls {
        let name = call.split(',').next().unwrap().trim().to_lowercase();
        let rel: RelationType = name.parse().unwrap();
        let spec = rel.spec();
        let proj = call.split("Projection.").nth(1).unwrap().split(',').next().unwrap().to_lowercase();
        assert_eq!(spec.projection.as_str(), proj, "{name} projection");
        assert_eq!(spec.symmetric, call.contains("sym=True"), "{name} symmetric");
        assert_eq!(spec.transitive, call.contains("trans=True"), "{name} transitive");
        assert_eq!(spec.allows_premises, call.contains("premises=True"), "{name} premises");
        assert_eq!(spec.formal_allowed, !call.contains("formal=False"), "{name} formal_allowed");
        let inv = call.split("inv=R.").nth(1).map(|s| s.split(',').next().unwrap().trim().to_lowercase());
        assert_eq!(spec.inverse.map(|r| r.as_str().to_owned()), inv, "{name} inverse");
    }
}

fn names(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_owned()).collect()
}

#[test]
fn vocab_json_matches_rust() {
    let v: Value = serde_json::from_str(&read("shapes/vocab.json")).unwrap();
    assert_eq!(names(&v["entity_types"]), strs(EntityType::ALL, EntityType::as_str));
    assert_eq!(names(&v["relation_types"]), strs(RelationType::ALL, RelationType::as_str));
    assert_eq!(names(&v["evidence_states"]), strs(EvidenceState::ALL, EvidenceState::as_str));
    assert_eq!(names(&v["roles"]), strs(Role::ALL, Role::as_str));
    assert_eq!(names(&v["projections"]), strs(Projection::ALL, Projection::as_str));
    let current: Vec<String> =
        EvidenceState::ALL.iter().filter(|s| s.is_current()).map(|s| s.as_str().to_owned()).collect();
    assert_eq!(names(&v["current_states"]), current);
    for r in RelationType::ALL {
        let j = &v["relations"][r.as_str()];
        let s = r.spec();
        assert_eq!(j["projection"], s.projection.as_str(), "{r}");
        assert_eq!(j["symmetric"], s.symmetric, "{r}");
        assert_eq!(j["transitive"], s.transitive, "{r}");
        assert_eq!(j["allows_premises"], s.allows_premises, "{r}");
        assert_eq!(j["inverse"].as_str(), s.inverse.map(RelationType::as_str), "{r}");
        assert_eq!(j["formal_allowed"], s.formal_allowed, "{r}");
    }
    for st in EvidenceState::ALL {
        assert_eq!(v["evidence_rank"][st.as_str()].as_u64(), st.rank().map(u64::from), "rank of {st}");
    }
    assert_eq!(names(&v["presentation_scopes"]), PRESENTATION_SCOPES.map(str::to_owned).to_vec());
}

#[test]
fn composition_json_names_only_known_things() {
    let c: Value = serde_json::from_str(&read("shapes/composition.json")).unwrap();
    let conclusions = c["conclusions"].as_object().unwrap();
    let rels = c["relations"].as_object().unwrap();
    assert_eq!(rels.len(), RelationType::COUNT, "every relation has a composition entry");
    for (name, spec) in rels {
        let r: RelationType = name.parse().unwrap();
        for k in ["single_hop", "chain_result"] {
            assert!(conclusions.contains_key(spec[k].as_str().unwrap()), "{name}.{k}");
        }
        let chain = spec["chain"].as_str().unwrap();
        assert_eq!(chain == "transitive", r.spec().transitive, "{name}: chain must follow RELATIONS.transitive");
    }
    for p in c["pairs"].as_array().unwrap() {
        p["first"].as_str().unwrap().parse::<RelationType>().unwrap();
        p["then"].as_str().unwrap().parse::<RelationType>().unwrap();
        assert!(conclusions.contains_key(p["result"].as_str().unwrap()));
    }
    for rule in c["rules"].as_array().unwrap() {
        for key in ["exclude", "opt_in"] {
            if let Some(states) = rule.get(key) {
                for s in names(states) {
                    s.parse::<EvidenceState>().unwrap();
                }
            }
        }
        if let Some(list) = rule.get("relations") {
            for r in names(list) {
                r.parse::<RelationType>().unwrap();
            }
        }
    }
}
