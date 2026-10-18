import json
import os
import random
import sqlite3
import statistics
import time
from pathlib import Path

import pytest

import atlas.store as store_mod
from atlas import (
    Entity,
    EntityType,
    EventKind,
    EvidenceState,
    FormalArtifact,
    LangString,
    Provenance,
    SCHEMA_VERSION,
    RelationType,
    Role,
    Source,
    Store,
    ValidationError,
    binary,
    conjunction,
    load_bundle,
    validate_jsonld,
)

FIXTURE = Path(__file__).parent / "fixtures" / "mini_atlas.json"
T = "2026-10-16T00:00:00+00:00"
P = Provenance("src:fixture-hand", "test", "unit-test", "1", "CC0-1.0", T)


@pytest.fixture()
def st():
    s = Store()
    load_bundle(s, json.loads(FIXTURE.read_text(encoding="utf-8")))
    yield s
    s.close()


def _stmt(eid: str, en: str, es: str | None = None) -> Entity:
    labels = (LangString(en, "en"),) + ((LangString(es, "es"),) if es else ())
    return Entity(eid, EntityType.STATEMENT, labels)


# ------------------------------------------------------------------ conjunctions & alternatives


def test_conjunction_of_premises_is_preserved(st):
    c = st.get_claim("c:pyth-euclid")
    assert c.is_conjunction and set(c.targets) == {"s:sas-congruence", "s:parallelogram-area"}
    assert c.ids(Role.VIA) == ("p:euclid-i47",) and c.ids(Role.ASSUMPTION) == ("s:parallel-postulate",)
    edges = [e for e in st.iter_edges(relations=[RelationType.DEPENDS_ON_LOGICALLY]) if e.claim_id == c.id]
    assert len(edges) == 2 and all(e.conjunct_count == 2 for e in edges)
    assert {e.target for e in edges} == {"s:sas-congruence", "s:parallelogram-area"}


def test_alternative_proofs_are_kept_as_alternatives(st):
    alts = st.alternatives("g:pythagoras-proofs")
    assert [c.id for c in alts] == ["c:pyth-euclid", "c:pyth-similarity"]
    assert alts[1].targets == ("s:similar-triangles",)
    deps = st.claims_for("s:pythagoras", roles=[Role.SUBJECT], relations=[RelationType.DEPENDS_ON_LOGICALLY])
    assert {c.alternative_group for c in deps} == {"g:pythagoras-proofs"}


def test_alternative_group_must_share_subject_and_relation(st):
    bad = conjunction("c:bad-alt", "depends_on_logically", "s:law-of-cosines", ["s:pythagoras"], [P],
                      alternative_group="g:pythagoras-proofs")
    with pytest.raises(ValidationError, match="alternative_group"):
        st.add_claim(bad)
    assert st.get_claim("c:bad-alt") is None  # rolled back


# ------------------------------------------------------------------ provenance & validation


def test_unregistered_source_rolls_back_whole_batch(st):
    good = binary("c:ok", "generalizes", "s:law-of-cosines", "s:sas-congruence", [P])
    orphan = binary("c:orphan", "generalizes", "s:parseval", "s:law-of-cosines",
                    [Provenance("src:nowhere", "x", "t", "1", "CC0-1.0", T)])
    with pytest.raises(ValidationError, match="unregistered source"):
        st.add_claims([good, orphan])
    assert st.get_claim("c:ok") is None and st.get_claim("c:orphan") is None


def test_unknown_entity_and_wrong_endpoint_types_are_rejected(st):
    with pytest.raises(ValidationError, match="unknown entity"):
        st.add_claim(binary("c:x", "generalizes", "s:pythagoras", "s:missing", [P]))
    with pytest.raises(ValidationError, match="endpoint type"):
        st.add_claim(binary("c:y", "named_after", "s:pythagoras", "pub:elements", [P]))
    with pytest.raises(ValidationError, match="duplicate"):
        st.add_claim(binary("c:named", "named_after", "s:pythagoras", "per:pythagoras", [P]))


def test_every_stored_claim_has_provenance_and_state(st):
    for c in st.iter_claims():
        assert c.provenance and c.evidence_state
    rows = st._conn.execute("SELECT COUNT(*) FROM claims c WHERE NOT EXISTS "
                            "(SELECT 1 FROM provenance p WHERE p.claim_id = c.id)").fetchone()[0]
    assert rows == 0


def test_existing_claim_ids_returns_only_stored_ids_across_chunks(st):
    stored = [c.id for c in st.iter_claims()]
    missing = [f"c:missing-{i}" for i in range(2000)]  # forces several IN chunks
    assert st.existing_claim_ids(missing + stored + stored[:1]) == set(stored)
    assert st.existing_claim_ids([]) == set()


# ------------------------------------------------------------------ evidence states & versions


def test_evidence_transitions(st):
    with pytest.raises(ValidationError, match="identified reviewer"):
        st.set_evidence_state("c:cosines-gen", "editorially_reviewed", actor="system", reason="ok")
    c = st.set_evidence_state("c:cosines-gen", "editorially_reviewed", actor="AJ", reason="checked II.12-13")
    assert c.evidence_state is EvidenceState.EDITORIALLY_REVIEWED
    with pytest.raises(ValidationError, match="FormalArtifact"):
        st.set_evidence_state("c:cosines-gen", "formally_linked", actor="AJ", reason="lean")
    f = FormalArtifact("mathlib4", "v4.0.0", "EuclideanGeometry.law_cos", ("propext", "Quot.sound"))
    c = st.set_evidence_state("c:cosines-gen", "formally_linked", actor="AJ", reason="lean", formal=f)
    assert c.formal == f
    st.set_evidence_state("c:cosines-gen", "retracted", actor="AJ", reason="wrong decl")
    with pytest.raises(ValidationError, match="not allowed"):
        st.set_evidence_state("c:cosines-gen", "extracted", actor="AJ", reason="undo")
    kinds = [(e.payload["from"], e.payload["to"]) for e in st.events("c:cosines-gen", kinds=[EventKind.EVIDENCE_CHANGED])]
    assert kinds == [("extracted", "editorially_reviewed"), ("editorially_reviewed", "formally_linked"),
                     ("formally_linked", "retracted")]
    assert st.get_claim("c:cosines-gen") not in st.claims_for("s:pythagoras")


def test_supersede_keeps_history(st):
    with pytest.raises(ValidationError, match="supersede"):
        st.add_claim(binary("c:v2", "generalizes", "s:parseval", "s:pythagoras", [P], supersedes="c:parseval-gen"))
    new = binary("c:parseval-gen-v2", "generalizes", "s:parseval", "s:pythagoras", [P],
                 validity_domain="orthonormal bases (finite or countable) of inner product spaces")
    out = st.supersede("c:parseval-gen", new, actor="AJ", reason="narrow validity domain")
    assert out.supersedes == "c:parseval-gen"
    old = st.get_claim("c:parseval-gen")
    assert old.evidence_state is EvidenceState.SUPERSEDED and old.superseded_by == "c:parseval-gen-v2"
    assert [c.id for c in st.history("c:parseval-gen-v2")] == ["c:parseval-gen", "c:parseval-gen-v2"]
    current = {c.id for c in st.claims_for("s:parseval")}
    assert "c:parseval-gen-v2" in current and "c:parseval-gen" not in current
    with pytest.raises(ValidationError, match="only current"):
        st.supersede("c:parseval-gen", binary("c:v3", "generalizes", "s:parseval", "s:pythagoras", [P]),
                     actor="AJ", reason="again")
    assert [e.kind for e in st.events("c:parseval-gen")][-1] is EventKind.CLAIM_SUPERSEDED


# ------------------------------------------------------------------ merges


def test_merge_and_undo(st):
    st.add_entity(_stmt("s:pyth-dup", "Pythagoras's theorem", "Teorema pitagórico"))
    st.add_claim(binary("c:dup-cites", "appears_in", "s:pyth-dup", "pub:elements", [P]))
    with pytest.raises(ValidationError, match="identified actor"):
        st.merge("s:pythagoras", "s:pyth-dup", actor="system", reason="same")
    with pytest.raises(ValidationError, match="type mismatch"):
        st.merge("per:pythagoras", "s:pyth-dup", actor="AJ", reason="same")
    m = st.merge("s:pythagoras", "s:pyth-dup", actor="AJ", reason="same statement", evidence="labels+statement")
    assert st.resolve("s:pyth-dup") == "s:pythagoras"
    assert st.get_entity("s:pyth-dup").id == "s:pythagoras"
    assert "c:dup-cites" in {c.id for c in st.claims_for("s:pythagoras")}
    assert st.search("pitagórico")[0].entity_id == "s:pythagoras"
    edge = next(e for e in st.iter_edges() if e.claim_id == "c:dup-cites")
    assert edge.source == "s:pythagoras"
    with pytest.raises(ValidationError, match="already merged"):
        st.merge("s:pythagoras", "s:pyth-dup", actor="AJ", reason="again")
    assert st.coverage()["active_merges"] == 1

    st.undo_merge(m.id, actor="AJ", reason="different traditions")
    assert st.resolve("s:pyth-dup") == "s:pyth-dup"
    assert "c:dup-cites" not in {c.id for c in st.claims_for("s:pythagoras")}
    assert st.search("pitagórico")[0].entity_id == "s:pyth-dup"
    assert st.get_merge(m.id).undone_at is not None
    with pytest.raises(ValidationError, match="already undone"):
        st.undo_merge(m.id, actor="AJ", reason="twice")
    assert [e.kind for e in st.events("s:pyth-dup")][-2:] == [EventKind.MERGE, EventKind.MERGE_UNDONE]


# ------------------------------------------------------------------ search


def test_fts_lookup_accents_prefix_lang_alias_external_id(st):
    assert st.search("pitagoras")[0].entity_id in {"s:pythagoras", "per:pythagoras"}
    assert st.search("teorema de pit", lang="es")[0].entity_id == "s:pythagoras"
    assert st.search("Pythagoras' theorem")[0].entity_id == "s:pythagoras"
    hyp = st.search("hipotenusa")
    assert hyp[0].entity_id == "s:pythagoras" and hyp[0].field == "statement"
    assert [h.entity_id for h in st.search("Pitágoras", types=[EntityType.PERSON])] == ["per:pythagoras"]
    assert st.search("   ") == [] and st.search('"); DROP TABLE claims; --') == []
    st.add_entity(Entity("s:nat-prime", "definition", (LangString("Número primo", "es"),),
                         external_ids=(("mathlib", "Nat.Prime"),)))
    assert st.search("Nat.Prime")[0].entity_id == "s:nat-prime"


def test_replace_entity_reindexes_and_rebuild_matches(st):
    e = st.get_entity("app:surveying")
    st.add_entity(Entity(e.id, e.type, e.labels + (LangString("Topografía", "pt"),)), on_conflict="replace")
    assert st.search("topografia")[0].entity_id == "app:surveying"
    with pytest.raises(ValidationError, match="change type"):
        st.add_entity(Entity(e.id, "concept", e.labels), on_conflict="replace")
    before = [(h.entity_id, h.field) for h in st.search("geometria")]
    assert st.rebuild_fts() > 0
    assert [(h.entity_id, h.field) for h in st.search("geometria")] == before
    assert st.add_entities([e], on_conflict="ignore") == 0


# ------------------------------------------------------------------ events, schema, files


def test_events_are_append_only(st):
    with pytest.raises(sqlite3.DatabaseError, match="append-only"):
        st._conn.execute("UPDATE events SET actor='x'")
    with pytest.raises(sqlite3.DatabaseError, match="append-only"):
        st._conn.execute("DELETE FROM events")
    kinds = {e.kind for e in st.events()}
    assert {EventKind.MIGRATION, EventKind.SOURCE_ADDED, EventKind.ENTITY_ADDED, EventKind.CLAIM_ADDED} <= kinds


def test_migration_hook_and_wal_and_read_only(tmp_path, monkeypatch):
    db = tmp_path / "atlas.db"
    with Store(db) as s:
        assert s.schema_version == SCHEMA_VERSION == 3
        assert s._conn.execute("PRAGMA journal_mode").fetchone()[0] == "wal"
        s.add_source(Source("src:x", "X", "CC0-1.0"))
    monkeypatch.setitem(store_mod.MIGRATIONS, 4, "CREATE TABLE notes(id TEXT PRIMARY KEY);")
    with Store(db) as s:
        assert s.schema_version == 4
        assert s._conn.execute("SELECT COUNT(*) FROM notes").fetchone()[0] == 0
        assert any(e.payload == {"from": 3, "to": 4} for e in s.events(kinds=[EventKind.MIGRATION]))
    with Store(db, read_only=True) as ro:
        assert ro.get_source("src:x").title == "X"
        with pytest.raises(sqlite3.OperationalError):
            ro.add_source(Source("src:y", "Y", "CC0-1.0"))


def test_coverage_reports_unknown_not_isolated(st):
    st.add_entity(Entity("c:lonely", "concept", (LangString("Concepto sin relaciones", "es"),)))
    cov = st.coverage()
    assert cov["unknown_by_type"] == {"concept": 1}
    assert cov["claims_by_state"]["extracted"] == 15


# ------------------------------------------------------------------ JSON-LD


def test_jsonld_export_conforms_and_tampering_is_caught(st):
    doc = st.export_jsonld(["s:pythagoras"], depth=1)
    assert validate_jsonld(doc) == []
    ids = {n["@id"] for n in doc["@graph"]}
    assert "aid:c%3Apyth-euclid" in ids and "aid:src%3Afixture-hand" in ids
    claim = next(n for n in doc["@graph"] if n["@id"] == "aid:c%3Apyth-euclid")
    assert [p["role"] for p in claim["participant"]].count("premise") == 2
    assert doc["@context"]["label"]["@container"] == "@language"
    claim["provenance"][0]["extractorVersion"] = ""
    claim["evidenceState"] = "formally_linked"
    claim["participant"].append({"role": "object", "entity": "aid:s%3Asas-congruence"})
    errs = validate_jsonld(doc)
    assert any("extractorVersion" in e for e in errs)
    assert any("formalArtifact" in e for e in errs)
    assert any("premises only" in e for e in errs)


def test_shacl_file_lists_every_relation_and_state():
    ttl = store_mod.SHACL_PATH.read_text(encoding="utf-8")
    for r in RelationType:
        assert f"atlas:{r.value}" in ttl
    for s in EvidenceState:
        assert f"atlas:{s.value}" in ttl


# ------------------------------------------------------------------ scale


def test_bulk_insert_100k_claims(tmp_path, capsys):
    """Scale check.  Default 100k claims; ATLAS_BULK_CLAIMS=1000000 for the 1M run reported in SPEC."""
    n_claims = int(os.environ.get("ATLAS_BULK_CLAIMS", "100000"))
    n_entities = max(20_000, n_claims // 5)
    db = tmp_path / "bulk.db"
    with Store(db) as s:
        s.add_source(Source("src:bulk", "Synthetic bulk", "CC0-1.0"))
        prov = [Provenance("src:bulk", "row", "bulk-test", "1", "CC0-1.0", T)]
        t0 = time.perf_counter()
        s.add_entities(Entity(f"s:{i}", "statement", (LangString(f"Enunciado {i}", "es"),
                                                       LangString(f"Statement {i}", "en")))
                       for i in range(n_entities))
        t1 = time.perf_counter()

        def claims():
            for i in range(n_claims):
                a, b, c = f"s:{i % n_entities}", f"s:{(i * 7 + 1) % n_entities}", f"s:{(i * 13 + 2) % n_entities}"
                if i % 3 == 0 and len({a, b, c}) == 3:
                    yield conjunction(f"c:{i}", "depends_on_logically", a, [b, c], prov)
                elif a != b:
                    yield binary(f"c:{i}", "generalizes", a, b, prov)

        inserted = s.add_claims(claims())
        t2 = time.perf_counter()
        n_edges = sum(1 for _ in s.iter_edges())
        t3 = time.perf_counter()
        rnd = random.Random(0)
        lat_nb = _timed(lambda: s.claims_for(f"s:{rnd.randrange(n_entities)}"), 200)
        lat_fts = _timed(lambda: s.search(f"statement {rnd.randrange(n_entities)}", limit=10), 200)
        assert inserted >= n_claims * 0.99
        assert s.coverage()["claims_by_source"]["src:bulk"] == inserted
    with capsys.disabled():
        print(f"\n[bulk] entities={n_entities} in {t1 - t0:.2f}s; claims={inserted} in {t2 - t1:.2f}s "
              f"({inserted / (t2 - t1):,.0f}/s); iter_edges={n_edges} in {t3 - t2:.2f}s; "
              f"claims_for p50/p95={lat_nb[0]:.2f}/{lat_nb[1]:.2f} ms; "
              f"search p50/p95={lat_fts[0]:.2f}/{lat_fts[1]:.2f} ms; db={db.stat().st_size / 2**20:.0f} MiB")
    assert (t2 - t1) < n_claims / 1000  # generous ceiling (>=1k claims/s); measured value printed above


def _timed(fn, n: int) -> tuple[float, float]:
    ms = []
    for _ in range(n):
        t = time.perf_counter()
        fn()
        ms.append((time.perf_counter() - t) * 1000)
    q = statistics.quantiles(ms, n=20)
    return statistics.median(ms), q[18]


def test_reader_sees_merge_committed_by_another_connection(tmp_path):
    db = tmp_path / "shared.db"
    with Store(db) as w:
        load_bundle(w, json.loads(FIXTURE.read_text(encoding="utf-8")))
        w.add_entity(_stmt("s:dup", "Pythagoras theorem duplicate"))
        with Store(db, read_only=True) as r:
            assert r.resolve("s:dup") == "s:dup"
            w.merge("s:pythagoras", "s:dup", actor="AJ", reason="duplicate")
            assert r.resolve("s:dup") == "s:pythagoras"
