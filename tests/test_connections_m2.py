"""Round 5 step 2a (connections): M2 transformations and the Wikidata mapping v3 (ADR-0005 §7b-§7e).

The registry is built the way wd-map/2 and mathlib v3 wrote it (P31 and P460 claims with their old ids,
part_of_area formally_linked), then M2 runs twice: the "Conexiones" tests of the ADR."""

from __future__ import annotations

import pytest

from atlas import EntityType, EvidenceState, RelationType, Store
from atlas.ingest import wikidata, wikidata_m2
from atlas.model import Entity, LangString, Provenance, Source, binary, stable_id

E, R, S = EntityType, RelationType, EvidenceState
TS = "2026-10-16T00:00:00+00:00"


def _wd(qid: str, pid: str, obj: str, rel: RelationType, *, version: str = "wd-map/2", note: tuple = ()):
    prov = Provenance("src:wikidata", f"https://www.wikidata.org/wiki/{qid}#{pid}", "atlas.ingest.wikidata",
                      version, "CC0-1.0", TS)
    return binary(stable_id("c", "wikidata", qid, pid, obj), rel, "wd:" + qid, "wd:" + obj, (prov,), assumptions=note)


@pytest.fixture()
def v3_store() -> Store:
    st = Store()
    st.add_source(Source("src:wikidata", "Wikidata", "CC0-1.0"))
    st.add_source(Source("src:mathlib", "Mathlib", "Apache-2.0"))
    for q in ("Q1", "Q2", "Q3", "Q4", "Q5", "Q6", "Q7"):
        st.add_entity(Entity("wd:" + q, E.CONCEPT, (LangString(q, "es"),)))
    st.add_entity(Entity("wd:A", E.AREA, (LangString("área", "es"),)))
    st.add_entity(Entity("m:d", E.DEFINITION, (LangString("d", "und"),)))
    st.add_claims([
        _wd("Q1", "P460", "Q2", R.EQUIVALENT_TO, note=("P460 note",)),   # graph = finite graph (said)
        _wd("Q3", "P31", "Q4", R.SPECIAL_CASE_OF),                       # golden ratio instance of constructible
        _wd("Q4", "P279", "Q5", R.SPECIAL_CASE_OF),                      # stays special_case_of
        _wd("Q6", "P7719", "Q7", R.GENERALIZES),                         # Q6 generalizes Q7 ...
        _wd("Q6", "P279", "Q7", R.SPECIAL_CASE_OF),                      # ... and Q6 special case of Q7
        _wd("Q1", "P2283", "Q5", R.PREREQUISITE_FOR),
        _wd("Q5", "P2283", "Q1", R.PREREQUISITE_FOR),
        _wd("Q3", "P31", "A", R.PART_OF_AREA),                           # P31 to an area stays part_of_area
        binary("c:mathlib:d", R.PART_OF_AREA, "m:d", "wd:A",
               (Provenance("src:mathlib", "Mathlib/X.lean:1", "atlas.ingest.mathlib", "1", "Apache-2.0", TS),)),
    ])
    # v3 wrote file locations as formally_linked; v4 refuses to insert it, so recreate the old row directly
    st._conn.execute("UPDATE claims SET evidence_state='formally_linked' WHERE id='c:mathlib:d'")
    return st


def _current(st: Store, rel: RelationType) -> list:
    return list(st.iter_claims(rel, states=[S.EXTRACTED, S.MODEL_PROPOSED, S.EDITORIALLY_REVIEWED,
                                            S.FORMALLY_LINKED, S.DISPUTED]))


def _from_pid(claims: list, pid: str) -> list:
    return [c for c in claims if any(p.locator.endswith("#" + pid) for p in c.provenance)]


def test_mapping_v3_separates_instance_and_sameness() -> None:
    assert wikidata.map_statement("P31", E.CONCEPT, E.CONCEPT)[0].relation is R.INSTANCE_OF
    assert wikidata.map_statement("P279", E.CONCEPT, E.CONCEPT)[0].relation is R.SPECIAL_CASE_OF
    assert wikidata.map_statement("P31", E.CONCEPT, E.AREA)[0].relation is R.PART_OF_AREA
    assert wikidata.map_statement("P460", E.CONCEPT, E.CONCEPT)[0].relation is R.SAID_EQUIVALENT_TO
    # new relations get new ids, so re-ingest never collides with the superseded wd-map/2 claim
    assert wikidata.claim_id("Q3", "P31", "Q4", R.INSTANCE_OF) != stable_id("c", "wikidata", "Q3", "P31", "Q4")
    assert wikidata.claim_id("Q4", "P279", "Q5", R.SPECIAL_CASE_OF) == stable_id("c", "wikidata", "Q4", "P279", "Q5")


def test_m2_conexiones(v3_store: Store) -> None:
    st = v3_store
    changed = wikidata_m2.run(st)
    assert changed == {"p460": 1, "p31": 1, "bidirectional": 4, "part_of_area": 1}
    assert _from_pid(_current(st, R.EQUIVALENT_TO), "P460") == []
    assert _from_pid(_current(st, R.SPECIAL_CASE_OF), "P31") == []
    assert [c for c in _current(st, R.PART_OF_AREA) if c.evidence_state is S.FORMALLY_LINKED] == []
    said = _current(st, R.SAID_EQUIVALENT_TO)
    assert [(c.subject, c.targets, c.assumptions) for c in said] == [("wd:Q1", ("wd:Q2",), ("P460 note",))]
    assert said[0].provenance[0].extractor_version == wikidata.MAPPING_VERSION
    inst = _current(st, R.INSTANCE_OF)
    assert [(c.subject, c.targets) for c in inst] == [("wd:Q3", ("wd:Q4",))]
    assert inst[0].id == wikidata.claim_id("Q3", "P31", "Q4", R.INSTANCE_OF)
    old = st.history(inst[0].id)
    assert [c.evidence_state for c in old] == [S.SUPERSEDED, S.EXTRACTED]
    # both directions -> disputed, with the partner in the event; the P279 chain Q4 -> Q5 is untouched
    disputed = {c.id for c in st.iter_claims(states=[S.DISPUTED])}
    assert disputed == {stable_id("c", "wikidata", "Q6", "P7719", "Q7"), stable_id("c", "wikidata", "Q6", "P279", "Q7"),
                        stable_id("c", "wikidata", "Q1", "P2283", "Q5"), stable_id("c", "wikidata", "Q5", "P2283", "Q1")}
    ev = list(st.events(stable_id("c", "wikidata", "Q6", "P7719", "Q7")))
    assert ev[-1].actor == "curation:bidirectional/1" and stable_id("c", "wikidata", "Q6", "P279", "Q7") in ev[-1].payload["reason"]
    assert st.get_claim(stable_id("c", "wikidata", "Q4", "P279", "Q5")).evidence_state is S.EXTRACTED


def test_m2_rerun_changes_nothing(v3_store: Store) -> None:
    st = v3_store
    wikidata_m2.run(st)
    n_events = len(list(st.events()))
    assert wikidata_m2.run(st) == {"p460": 0, "p31": 0, "bidirectional": 0, "part_of_area": 0}
    assert len(list(st.events())) == n_events
    assert wikidata_m2.run(st, dry_run=True) == {"p460": 0, "p31": 0, "bidirectional": 0, "part_of_area": 0}
