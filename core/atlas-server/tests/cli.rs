//! The `atlas` binary: compile, stats, layout and index end to end; errors are the JSON envelope.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn atlas(args: &[&str]) -> (bool, Value) {
    // ATLAS_ENCODER=lsi: no ONNX model load (tests stay offline and fast; with the
    // neural default, `atlas index` on the 19-node fixture ran 12.4 h in a debug build).
    let out = Command::new(env!("CARGO_BIN_EXE_atlas"))
        .args(args)
        .env("ATLAS_ENCODER", "lsi")
        .output()
        .expect("run atlas");
    let v = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
    (out.status.success(), v)
}

#[test]
fn compile_stats_layout_and_error_envelope() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../atlas-core/tests/fixtures/mini_atlas.db");
    let out = std::env::temp_dir().join(format!("atlas-cli-test-{}", std::process::id()));
    let (db, snap) = (fixture.to_str().unwrap(), out.to_str().unwrap());

    let (ok, c) = atlas(&["compile", "--db", db, "--out", snap, "--json"]);
    assert!(ok, "{c}");
    assert_eq!((c["nodes"].as_u64(), c["edges"].as_u64()), (Some(19), Some(18)));

    let (ok, s) = atlas(&["stats", "--snapshot", snap, "--verify", "--json"]);
    assert!(ok, "{s}");
    assert_eq!(s["content_hash"], c["content_hash"]);
    assert_eq!(s["verified"], true);
    assert_eq!(s["unknown_by_type"]["concept"], 1);

    let layout = out.join("layout");
    let (ok, l) = atlas(&["layout", "--snapshot", snap, "--out", layout.to_str().unwrap(), "--json"]);
    assert!(ok, "{l}");
    assert_eq!(l["nodes"], 19);
    assert!(l["tiles"].as_u64().unwrap() >= 1, "{l}");

    // `atlas index` wires atlas-search: a build report, or its not_implemented envelope
    // while that crate is still a skeleton.
    let index = out.join("index");
    let (ok, i) = atlas(&["index", "--snapshot", snap, "--db", db, "--out", index.to_str().unwrap(), "--json"]);
    if ok {
        assert!(i["documents"].as_u64().unwrap() > 0, "{i}");
    } else {
        assert_eq!(i["error"]["code"], "not_implemented", "{i}");
    }

    // `atlas path` (round 3): --concepts and --drill-down reach the engine as in /api/path.
    let comp = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../shapes/composition.json");
    let path = |extra: &[&str]| {
        let mut a = vec!["path", "s:pythagoras", "s:sas-congruence", "--snapshot", snap];
        a.extend(["--composition", comp.to_str().unwrap(), "--json"]);
        a.extend(extra);
        atlas(&a)
    };
    let (ok, p) = path(&["--concepts", "--drill-down"]);
    assert!(ok, "{p}");
    assert!(p["concepts"].is_object(), "concept level on: {p}");
    let (ok, off) = path(&["--concepts=false"]);
    assert!(ok, "{off}");
    assert!(off["concepts"].is_null(), "concept level off: {off}");
    assert_eq!(off["paths"], p["paths"], "the entity-level answer does not depend on it");

    let (ok, e) = atlas(&["stats", "--snapshot", "does/not/exist", "--json"]);
    assert!(!ok);
    assert_eq!(e["error"]["code"], "internal");
}
