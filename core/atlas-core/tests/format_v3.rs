//! Format v3 / schema v4 (ADR-0005): u64 string offsets, NodeFlags and presentation labels from the
//! active curation, layer snapshots bound to their core, and the bibliographic-source audit.
//! Registries start from the committed fixture (tests/fixtures/mini_atlas.db) plus rows written here.

use atlas_core::format::{FORMAT_VERSION, SUPPORTED_SCHEMA, Section};
use atlas_core::{CompileOptions, Error, Snapshot, compile};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("atlas-core-v3-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The fixture registry raised to schema v4 with the `curation` table of `atlas/store.py` (ADR-0005).
fn registry_v4(dir: &Path) -> PathBuf {
    let db = dir.join("atlas.db");
    std::fs::copy(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini_atlas.db"), &db).unwrap();
    Connection::open(&db)
        .unwrap()
        .execute_batch(
            "CREATE TABLE curation(id INTEGER PRIMARY KEY, kind TEXT NOT NULL, target_id TEXT NOT NULL,
               rule TEXT NOT NULL, rule_version TEXT NOT NULL, payload TEXT NOT NULL, actor TEXT NOT NULL,
               reason TEXT NOT NULL, created_at TEXT NOT NULL, undone_at TEXT, undone_by TEXT, undo_reason TEXT);
             PRAGMA user_version = 4;",
        )
        .unwrap();
    db
}

fn curate(db: &Path, kind: &str, target: &str, payload: &str, undone: bool) {
    Connection::open(db)
        .unwrap()
        .execute(
            "INSERT INTO curation(kind, target_id, rule, rule_version, payload, actor, reason, created_at, undone_at)
             VALUES (?1, ?2, 'test', '1', ?3, 'curation:test/1', 'test', 't', ?4)",
            params![kind, target, payload, undone.then_some("t2")],
        )
        .unwrap();
}

fn build(db: &Path, out: &Path, core: Option<[u8; 32]>) -> atlas_core::Result<Snapshot> {
    let mut opts = CompileOptions::new(db, out);
    opts.threads = 2;
    opts.core_content_hash = core;
    compile(&opts)?;
    Snapshot::open(out)
}

#[test]
fn format_v3_has_u64_string_offsets_and_reads_back() {
    assert_eq!((FORMAT_VERSION, SUPPORTED_SCHEMA), (3, 4));
    assert_eq!(Section::StrOffsets.elem_size(), 8, "byte offsets are u64 in format v3");
    let dir = scratch("u64");
    let s = build(&registry_v4(&dir), &dir.join("snap"), None).unwrap();
    s.verify().unwrap();
    assert_eq!(s.header().format_version, 3);
    let p = s.resolve("s:pythagoras").unwrap();
    assert_eq!((s.label(p, "es"), s.node_id(p)), ("Teorema de Pitágoras", "s:pythagoras"));
    assert!(!s.is_layer());
}

#[test]
fn node_flags_apply_active_curation_and_ignore_undone() {
    let dir = scratch("flags");
    let db = registry_v4(&dir);
    curate(&db, "exclude_presentation", "app:surveying", r#"{"scopes":["map:general","search:default"]}"#, false);
    curate(&db, "exclude_presentation", "per:euclid", r#"{"scopes":["map:all"]}"#, true);
    curate(&db, "exclude_presentation", "s:pythagoras-dup", r#"{"scopes":["concept_layer"]}"#, false);
    curate(&db, "presentation_label", "s:pythagoras", r#"{"lang":"es","text":"Pitágoras (teorema)","provenance":{}}"#, false);
    curate(&db, "presentation_label", "s:pythagoras", r#"{"lang":"en","text":"Undone label","provenance":{}}"#, true);
    let s = build(&db, &dir.join("snap"), None).unwrap();
    s.verify().unwrap();
    let n = |id: &str| s.resolve(id).unwrap();
    let surveying = n("app:surveying");
    assert!(s.is_excluded(surveying, "map:general") && s.is_excluded(surveying, "search:default"));
    assert!(!s.is_excluded(surveying, "map:all") && !s.is_excluded(surveying, "concept_layer"));
    assert_eq!(s.node_flags(n("per:euclid")), 0, "an undone curation is ignored");
    assert!(s.is_excluded(n("s:pythagoras"), "concept_layer"), "a curation on a merged id lands on the canonical node");
    let p = n("s:pythagoras");
    assert_eq!(s.display_label(p, "es"), "Pitágoras (teorema)");
    assert_eq!(s.label(p, "es"), "Teorema de Pitágoras", "the registry label is not replaced");
    assert_eq!(s.display_label(p, "en"), "Pythagorean theorem", "undone label ignored");

    curate(&db, "exclude_presentation", "nope:missing", r#"{"scopes":["map:all"]}"#, false);
    let err = build(&db, &dir.join("snap2"), None).err().expect("unknown target fails");
    assert!(matches!(err, Error::Registry(ref m) if m.contains("unknown target")), "{err}");
}

#[test]
fn layer_snapshot_rejects_another_core() {
    let dir = scratch("layer");
    let db = registry_v4(&dir);
    let core = build(&db, &dir.join("core"), None).unwrap();
    let layer = build(&db, &dir.join("works"), Some(core.header().content_hash)).unwrap();
    assert!(layer.is_layer());
    Snapshot::open_layer(layer.path(), &core).expect("same core");
    let other = build(&db, &dir.join("works-other"), Some([7u8; 32])).unwrap();
    let err = Snapshot::open_layer(other.path(), &core).err().expect("mismatch is rejected");
    assert!(matches!(err, Error::Format(ref m) if m.contains("layer built over core")), "{err}");
}

#[test]
fn bibliographic_source_audit_fails_compile() {
    let dir = scratch("audit");
    let db = registry_v4(&dir);
    let c = Connection::open(&db).unwrap();
    c.execute("INSERT INTO sources VALUES ('src:zbmath', '{\"title\":\"zbMATH Open\",\"license\":\"CC-BY-SA-4.0\"}', 't')", [])
        .unwrap();
    let claim = |id: &str, rel: &str, s: &str, o: &str| {
        let data = format!(
            r#"{{"id":"{id}","relation":"{rel}","participants":[{{"role":"subject","entity":"{s}"}},{{"role":"object","entity":"{o}"}}],"evidence_state":"extracted"}}"#
        );
        c.execute("INSERT INTO claims VALUES (?1, ?2, 'extracted', NULL, NULL, NULL, NULL, ?3, 't')", params![id, rel, data])
            .unwrap();
        c.execute("INSERT INTO claim_participants VALUES (?1, 'subject', ?2, 0)", params![id, s]).unwrap();
        c.execute("INSERT INTO claim_participants VALUES (?1, 'object', ?2, 1)", params![id, o]).unwrap();
        c.execute(
            "INSERT INTO provenance VALUES (?1, NULL, 'src:zbmath', 'oai', 'test', '1', 'CC-BY-SA-4.0', 't', NULL)",
            params![id],
        )
        .unwrap();
    };
    claim("c:zb-appears", "appears_in", "s:pythagoras", "pub:elements");
    build(&db, &dir.join("ok"), None).expect("allowed relations compile");
    claim("c:zb-gen", "generalizes", "s:law-of-cosines", "s:pythagoras");
    let err = build(&db, &dir.join("bad"), None).err().expect("audit fails");
    assert!(matches!(err, Error::Registry(ref m) if m.contains("source audit") && m.contains("c:zb-gen")), "{err}");
}

#[test]
fn curation_on_a_works_id_compiles_in_both_scopes() {
    let dir = scratch("works-curation");
    let db = registry_v4(&dir);
    curate(&db, "exclude_presentation", "oa:W1", r#"{"scopes":["map:all"]}"#, false);
    curate(&db, "presentation_label", "zb:7", r#"{"lang":"es","text":"Obra","provenance":{}}"#, false);
    let core = build(&db, &dir.join("core"), None).expect("core skips curation of works ids");
    build(&db, &dir.join("works"), Some(core.header().content_hash)).expect("works layer skips absent targets");
}
