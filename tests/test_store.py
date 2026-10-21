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
        assert s.schema_version == SCHEMA_VERSION == 4
        assert s._conn.execute("PRAGMA journal_mode").fetchone()[0] == "wal"
        s.add_source(Source("src:x", "X", "CC0-1.0"))
    monkeypatch.setitem(store_mod.MIGRATIONS, 5, "CREATE TABLE later(id TEXT PRIMARY KEY);")
    with Store(db) as s:
        assert s.schema_version == 5
        assert s._conn.execute("SELECT COUNT(*) FROM later").fetchone()[0] == 0
        assert any(e.payload == {"from": 4, "to": 5} for e in s.events(kinds=[EventKind.MIGRATION]))
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


# ------------------------------------------------------------------ v4 (ADR-0005)

import hashlib  # noqa: E402
import shutil  # noqa: E402

from atlas.model import (  # noqa: E402
    CoverageCell,
    CurationKind,
    DateKind,
    FichaSection,
    HistoricalDate,
    NoteInfo,
    stable_id,
)

V3_FIXTURE = Path(__file__).parent / "fixtures" / "registry_v3.db"  # written by the v3 Store, 2026-10-19
HIST = Source("src:hist", "A history of mathematics (test)", "CC-BY-4.0", kind="historiography")
PH = Provenance("src:hist", "p. 12", "unit-test", "1", "CC-BY-4.0", T)
_V3_TABLES = {  # columns that existed in v3, in a fixed order
    "claims": "id, relation, evidence_state, alternative_group, supersedes, superseded_by, formal, data, created_at",
    "claim_participants": "claim_id, role, entity_id, position",
    "provenance": "claim_id, entity_id, source_id, locator, extractor, extractor_version, license, retrieved_at, quote",
    "merges": "*",
    "events": "*",
}


def _table_digest(con: sqlite3.Connection, table: str, where: str = "") -> tuple[int, str]:
    rows = sorted(map(repr, con.execute(f"SELECT {_V3_TABLES[table]} FROM {table} {where}")))
    return len(rows), hashlib.sha256(repr(rows).encode()).hexdigest()


def test_migration_v3_to_v4_keeps_every_row(tmp_path):
    db = tmp_path / "v3.db"
    shutil.copy(V3_FIXTURE, db)
    con = sqlite3.connect(db)
    assert con.execute("PRAGMA user_version").fetchone()[0] == 3
    max_seq = con.execute("SELECT MAX(seq) FROM events").fetchone()[0]
    before = {t: _table_digest(con, t) for t in _V3_TABLES}
    con.close()
    with Store(db) as s:
        assert s.schema_version == 4
        con = s._conn
        after = {t: _table_digest(con, t, f"WHERE seq <= {max_seq}" if t == "events" else "") for t in _V3_TABLES}
        assert after == before
        new = list(s.events(since_seq=max_seq))
        assert [(e.kind, e.payload) for e in new] == [(EventKind.MIGRATION, {"from": 3, "to": 4})]
        assert con.execute("SELECT COUNT(*) FROM relation_endpoints WHERE relation='cites' AND entity_type='witness'"
                           ).fetchone()[0] == 2
        assert s.resolve("s:pythagoras-dup") == "s:pythagoras"  # v3 merges still resolve
        assert s.get_claim("c:rt-prereq").evidence_state is EvidenceState.DISPUTED


def test_introduced_in_needs_a_historiography_source(st):
    st.add_entity(Entity("c:prime", "concept", (LangString("Número primo", "es"),)))
    with pytest.raises(ValidationError, match="historiography"):
        st.add_claim(binary("c:intro", "introduced_in", "c:prime", "pub:elements", [P]))
    assert st.get_claim("c:intro") is None
    st.add_source(HIST)
    st.add_claim(binary("c:intro", "introduced_in", "c:prime", "pub:elements", [P, PH]))
    assert st.get_claim("c:intro").relation is RelationType.INTRODUCED_IN


def test_formally_linked_is_refused_on_insert_and_transition(st):
    f = FormalArtifact("mathlib4", "v", "X.y")
    with pytest.raises(ValidationError, match="formal_allowed"):
        st.add_claim(binary("c:pa", "part_of_area", "s:pythagoras", "a:euclidean-geometry", [P],
                            evidence_state="formally_linked", formal=f))
    (pa,) = st.claims_for("s:pythagoras", roles=[Role.SUBJECT], relations=[RelationType.PART_OF_AREA])
    with pytest.raises(ValidationError, match="formal_allowed"):
        st.set_evidence_state(pa.id, "formally_linked", actor="AJ", reason="no", formal=f)


def test_curation_kinds_create_undo_and_are_idempotent(st):
    rule = dict(rule="out-of-domain", rule_version="1", actor="curation:out-of-domain/1", reason="not mathematics")
    with pytest.raises(ValidationError, match="identified actor"):
        st.add_curation("exclude_presentation", "app:surveying", payload={"scopes": ["map:general"]},
                        **{**rule, "actor": "system"})
    with pytest.raises(ValidationError, match="scopes"):
        st.add_curation("exclude_presentation", "app:surveying", payload={"scopes": ["map:moon"]}, **rule)
    ex = st.add_curation("exclude_presentation", "app:surveying",
                         payload={"scopes": ["search:default", "map:general"]}, **rule)
    assert ex.payload == {"scopes": ["map:general", "search:default"]}
    again = st.add_curation("exclude_presentation", "app:surveying",
                            payload={"scopes": ["map:general", "search:default"]}, **rule)
    assert again.id == ex.id and len(st.curations(target_id="app:surveying")) == 1
    with pytest.raises(ValidationError, match="another payload"):
        st.add_curation("exclude_presentation", "app:surveying", payload={"scopes": ["map:all"]}, **rule)
    undone = st.undo_curation(ex.id, actor="AJ", reason="it is applied mathematics")
    assert undone.undone_at and undone.undone_by == "AJ" and st.curations(target_id="app:surveying") == []
    with pytest.raises(ValidationError, match="already undone"):
        st.undo_curation(ex.id, actor="AJ", reason="twice")
    lab = st.add_curation("presentation_label", "s:pythagoras", rule="qualifier", rule_version="1", actor="AJ",
                          reason="homonym",
                          payload={"lang": "es", "text": "Pitágoras (teorema)", "provenance": P.__dict__})
    assert lab.payload["text"] == "Pitágoras (teorema)"
    assert st.get_entity("s:pythagoras").label("es") != lab.payload["text"]  # the source label is not touched
    st.undo_curation(lab.id, actor="AJ", reason="test")
    kinds = [e.kind for e in st.events("app:surveying")][-2:]
    assert kinds == [EventKind.CURATION_DECIDED, EventKind.CURATION_UNDONE]
    assert {k.value for k in CurationKind} == {"exclude_presentation", "presentation_label", "retire_claims",
                                               "propose_merge"}


def test_retire_claims_retracts_and_undo_restores_copies(st):
    cur = st.add_curation("retire_claims", "per:pythagoras", rule="homonym", rule_version="1", actor="AJ",
                          reason="homonymous membership", payload={"claims": ["c:rt-prereq"]})
    assert st.get_claim("c:rt-prereq").evidence_state is EvidenceState.RETRACTED
    ev = next(e for e in st.events("c:rt-prereq", kinds=[EventKind.EVIDENCE_CHANGED]))
    assert ev.payload["reason"] == f"curation:{cur.id}"
    st.undo_curation(cur.id, actor="AJ", reason="was right")
    new_id = stable_id("restored", "c:rt-prereq", str(cur.id))
    restored = st.get_claim(new_id)
    assert restored.evidence_state is EvidenceState.EXTRACTED
    assert restored.targets == st.get_claim("c:rt-prereq").targets
    assert restored.provenance[-1].locator == f"curation:{cur.id}"
    assert st.get_claim("c:rt-prereq").evidence_state is EvidenceState.RETRACTED  # terminal stays terminal
    (done,) = list(st.events("per:pythagoras", kinds=[EventKind.CURATION_UNDONE]))
    assert done.payload["restored"] == [new_id]


def test_propose_merge_accepted_becomes_a_merge(st):
    st.add_entity(_stmt("s:pyth-dup", "Pythagoras theorem, duplicate"))
    prop = st.add_curation("propose_merge", "s:pyth-dup", rule="near-duplicate", rule_version="1",
                           actor="curation:near-duplicate/1", reason="same label",
                           payload={"survivor": "s:pythagoras", "merged": "s:pyth-dup", "evidence": "label"})
    assert st.resolve("s:pyth-dup") == "s:pyth-dup"  # a proposal does nothing
    m = st.accept_merge_proposal(prop.id, actor="AJ", reason="checked")
    assert m.evidence == f"curation:{prop.id}" and st.resolve("s:pyth-dup") == "s:pythagoras"
    with pytest.raises(ValidationError, match="accepted"):
        st.undo_curation(prop.id, actor="AJ", reason="reject")
    st.undo_merge(m.id, actor="AJ", reason="back")
    assert st.undo_curation(prop.id, actor="AJ", reason="rejected after all").undone_at


def test_entity_replaced_keeps_previous_and_identical_replace_is_a_noop(st):
    e = st.get_entity("app:surveying")
    n_events = len(list(st.events("app:surveying")))
    assert st.add_entities([e], on_conflict="replace") == 0
    assert len(list(st.events("app:surveying"))) == n_events
    st.add_entity(Entity(e.id, e.type, e.labels + (LangString("Topografía", "pt"),)), on_conflict="replace")
    ev = list(st.events("app:surveying", kinds=[EventKind.ENTITY_REPLACED]))
    assert len(ev) == 1 and Entity.from_json(ev[0].payload["previous"]) == e


def test_notes_are_immutable_and_not_searchable(st):
    note = Entity("note:pyth-1", "note", (LangString("En un triángulo rectángulo", "es"),),
                  statements=(LangString("En un triángulo rectángulo, el cuadrado de la hipotenusa...", "es"),),
                  note=NoteInfo(FichaSection.QUE_ES, 0))
    st.add_entity(note)
    assert all(h.entity_id != "note:pyth-1" for h in st.search("triángulo rectángulo"))
    st.add_claim(binary("c:explains", "explains", "note:pyth-1", "s:pythagoras", [P], evidence_state="model_proposed"))
    with pytest.raises(ValidationError, match="endpoint type"):
        st.add_claim(binary("c:bad-note", "appears_in", "note:pyth-1", "pub:elements", [P]))
    changed = Entity(note.id, note.type, note.labels, statements=(LangString("Otro texto", "es"),), note=note.note)
    with pytest.raises(ValidationError, match="immutable"):
        st.add_entity(changed, on_conflict="replace")
    st.rebuild_fts()
    assert all(h.entity_id != "note:pyth-1" for h in st.search("hipotenusa"))


def _work(i: int) -> Entity:
    return Entity(f"zb:{i}", "publication", (LangString(f"Work {i}", "en"),), external_ids=(("zbmath", str(i)),))


def test_batches_are_atomic_idempotent_and_leave_one_event(st):
    st.add_source(Source("src:zbmath", "zbMATH Open", "CC-BY-SA-4.0", kind="dataset"))
    pz = Provenance("src:zbmath", "oai", "unit-test", "1", "CC-BY-SA-4.0", T)
    seq0 = max(e.seq for e in st.events())
    kw = dict(source_id="src:zbmath", run="zb@1", input_sha256="0" * 64, actor="ingest:zbmath/1")
    cites = [binary(stable_id("c", "cites", f"zb:{i}", f"zb:{i - 1}"), "cites", f"zb:{i}", f"zb:{i - 1}", [pz])
             for i in range(1, 5)]
    bid = st.ingest_batch(batch_key="2026-01", entities=[_work(i) for i in range(5)], claims=cites,
                          stats={"citations_internal": 4, "citations_external": 7}, **kw)
    events = list(st.events(since_seq=seq0))
    assert [e.kind for e in events] == [EventKind.BATCH_INGESTED]
    assert events[0].payload["stats"] == {"citations_internal": 4, "citations_external": 7,
                                          "entities_inserted": 5, "claims_inserted": 4}
    assert st._conn.execute("SELECT COUNT(*) FROM claims WHERE ingest_batch=?", (bid,)).fetchone()[0] == 4
    assert st.ingest_batch(batch_key="2026-01", entities=[_work(i) for i in range(5)], claims=cites, **kw) is None
    assert list(st.events(since_seq=events[0].seq)) == []

    def boom():
        yield _work(10)
        raise RuntimeError("killed mid-batch")

    with pytest.raises(RuntimeError):
        st.ingest_batch(batch_key="2026-02", entities=boom(), **kw)
    assert st.get_entity("zb:10") is None
    assert st._conn.execute("SELECT COUNT(*) FROM ingest_batches WHERE batch_key='2026-02'").fetchone()[0] == 0
    orphans = st._conn.execute(
        """SELECT COUNT(*) FROM claims c WHERE NOT EXISTS (SELECT 1 FROM events e WHERE e.kind='claim_added'
           AND e.target_id=c.id) AND (c.ingest_batch IS NULL OR NOT EXISTS (SELECT 1 FROM events e
           WHERE e.kind='batch_ingested' AND e.target_id='batch:' || c.ingest_batch))""").fetchone()[0]
    assert orphans == 0


def test_entity_dates_fill_rebuild_and_range(st):
    st.add_source(HIST)
    plimpton = Entity("cdli:P254790", "witness", (LangString("Plimpton 322", "en"),),
                      dates=(HistoricalDate(DateKind.COPIED, "-1899~/-1599~", PH),
                             HistoricalDate(DateKind.COPIED, "-1798", PH, raw="-1799-00-00T00:00:00Z/9")))
    gauss = Entity("wd:Q1", "publication", (LangString("Disquisitiones", "la"),),
                   dates=(HistoricalDate(DateKind.PUBLISHED, "1801", PH), HistoricalDate("edition", "1966/..", PH)))
    st.add_entities([plimpton, gauss])
    rows = st._conn.execute("SELECT entity_id, position, lo, hi FROM entity_dates ORDER BY 1, 2").fetchall()
    assert rows == [("cdli:P254790", 0, -1899, -1599), ("cdli:P254790", 1, -1798, -1798),
                    ("wd:Q1", 0, 1801, 1801), ("wd:Q1", 1, 1966, None)]
    assert st.dated_entities(-2000, -1000) == ["cdli:P254790"]
    assert st.dated_entities(2000, None) == ["wd:Q1"]  # an open end reaches the present
    assert st.dated_entities(1800, 1900, kinds=["published"]) == ["wd:Q1"]
    st._conn.execute("DELETE FROM entity_dates")
    assert st.rebuild_entity_dates() == 4
    assert st._conn.execute("SELECT COUNT(*) FROM entity_dates").fetchone()[0] == 4
    st.add_entity(Entity(gauss.id, gauss.type, gauss.labels, dates=gauss.dates[:1]), on_conflict="replace")
    assert st.dated_entities(2000, None) == []


def test_resolve_scales_with_sql_not_a_full_load(tmp_path, monkeypatch, capsys):
    """Synthetic 1M merges (written directly; the API path is tested above): p95 of resolve < 1 ms."""
    n = int(os.environ.get("ATLAS_MERGES", "1000000"))
    with Store(tmp_path / "merges.db") as s:
        s._conn.execute("BEGIN")
        s._conn.executemany("INSERT INTO entities(id, type, data, created_at) VALUES (?, 'publication', '{}', 't')",
                            ((f"s:{i}",) for i in range(n // 10)))
        s._conn.executemany("INSERT INTO entities(id, type, data, merged_into, created_at) "
                            "VALUES (?, 'publication', '{}', ?, 't')",
                            ((f"m:{i}", f"s:{i % (n // 10)}") for i in range(n)))
        s._conn.execute("COMMIT")
        monkeypatch.setattr(Store, "_redirects", lambda self: pytest.fail("loaded every redirect"))
        rnd = random.Random(1)
        lat_r = _timed(lambda: s.resolve(f"m:{rnd.randrange(n)}"), 2000)
        lat_m = _timed(lambda: s.merged_ids(f"s:{rnd.randrange(n // 10)}"), 500)
        assert s.resolve("m:12") == "s:12" and len(s.merged_ids("m:12")) == 11
    with capsys.disabled():
        print(f"\n[merges] n={n} resolve p50/p95={lat_r[0]:.3f}/{lat_r[1]:.3f} ms; "
              f"merged_ids p50/p95={lat_m[0]:.3f}/{lat_m[1]:.3f} ms")
    assert lat_r[1] < 1.0


def test_merge_many_is_atomic_and_undoable(st):
    st.add_entities([_stmt("s:d1", "dup 1"), _stmt("s:d2", "dup 2")])
    with pytest.raises(ValidationError, match="unknown entity"):
        st.merge_many([("s:pythagoras", "s:d1", "doi:1"), ("s:pythagoras", "s:missing", "doi:2")],
                      actor="resolver:works/1", reason="exact id")
    assert st.resolve("s:d1") == "s:d1"  # nothing applied
    ms = st.merge_many([("s:pythagoras", "s:d1", "doi:1"), ("s:pythagoras", "s:d2", "doi:2")],
                       actor="resolver:works/1", reason="exact id")
    assert [m.evidence for m in ms] == ["doi:1", "doi:2"]
    assert st.merged_ids("s:pythagoras")[1:] == ["s:d1", "s:d2"]
    with pytest.raises(ValidationError, match="survivor"):
        st.merge_many([("s:d1", "s:x", None), ("s:y", "s:d1", None)], actor="AJ", reason="chain")
    for m in ms:
        st.undo_merge(m.id, actor="AJ", reason="undo batch")
    assert st.resolve("s:d1") == "s:d1" and st.merged_ids("s:pythagoras") == ["s:pythagoras"]


def test_coverage_cells_are_append_only(st):
    st.add_source(Source("src:zbmath", "zbMATH Open", "CC-BY-SA-4.0", kind="dataset"))
    cell = CoverageCell("src:zbmath", "2026-10", "1900/1949", "*", None, "unknown", 0, "all", T)
    assert st.add_coverage_cells([cell], actor="ingest:zbmath/1") == 1
    assert st.coverage_cells("src:zbmath") == [cell]
    with pytest.raises(ValidationError, match="duplicate"):
        st.add_coverage_cells([cell], actor="ingest:zbmath/1")
    with pytest.raises(sqlite3.DatabaseError, match="append-only"):
        st._conn.execute("UPDATE coverage_cells SET ingested=1")
    with pytest.raises(sqlite3.DatabaseError, match="append-only"):
        st._conn.execute("DELETE FROM coverage_cells")
    with pytest.raises(ValidationError):
        CoverageCell("src:zbmath", "2026-10", "first half", "*", None, "x", 0, "all", T)


def test_replace_uses_the_external_ids_entity_index(st):
    plan = st._conn.execute("EXPLAIN QUERY PLAN DELETE FROM external_ids WHERE entity_id=?", ("x",)).fetchall()
    assert any("external_ids_entity" in row[-1] for row in plan), plan


def test_jsonld_exports_dates_and_notes(st):
    st.add_source(HIST)
    st.add_entity(Entity("cdli:P254790", "witness", (LangString("Plimpton 322", "en"),),
                         dates=(HistoricalDate(DateKind.COPIED, "-1798", PH),)))
    st.add_claim(binary("c:plimpton-in", "appears_in", "s:pythagoras", "cdli:P254790", [P]))
    doc = st.export_jsonld(["cdli:P254790"], depth=1)
    assert validate_jsonld(doc) == []
    node = next(n for n in doc["@graph"] if n.get("atlasId") == "cdli:P254790")
    assert node["@type"] == "Witness" and node["date"][0]["edtf"] == "-1798"
    node["date"][0]["edtf"] = "-02XX"
    assert any("EDTF" in e for e in validate_jsonld(doc))
