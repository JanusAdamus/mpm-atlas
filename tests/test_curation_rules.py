"""Round-5 name curation rules (atlas.ingest.curation): reversible, idempotent, sourced."""

import pytest

from atlas import Entity, EntityType, EvidenceState, LangString, Provenance, RelationType, Source, Store, binary
from atlas.ingest import curation as cur

T = "2026-10-16T00:00:00+00:00"
WD = Provenance("src:wikidata", "https://www.wikidata.org/wiki/Q1", "atlas.ingest.wikidata", "wd-map/2", "CC0-1.0", T)
ML = Provenance("src:mathlib4", "Mathlib/X.lean:1", "atlas.ingest.mathlib", "1", "Apache-2.0", T)


def _wd(qid: str, es: str | None, en: str, desc_en: str | None = None, t=EntityType.CONCEPT) -> Entity:
    labels = (LangString(en, "en"),) + ((LangString(es, "es"),) if es else ())
    descs = (LangString(desc_en, "en"),) if desc_en else ()
    return Entity("wd:" + qid, t, labels, descriptions=descs, provenance=(WD,))


@pytest.fixture()
def st():
    s = Store()
    s.add_source(Source("src:wikidata", "Wikidata", "CC0-1.0"))
    s.add_source(Source("src:mathlib4", "Mathlib", "Apache-2.0"))
    s.add_entities([
        _wd("Q258", "Sudáfrica", "South Africa", "country in southern Africa"),
        _wd("Q4587955", "sextillón", "sextillion"),
        _wd("Q1096885", "red", "lattice", "subgroup of a real vector space"),
        _wd("Q3117934", "Grupo especial ortogonal", "special orthogonal group"),
        _wd("Q999", "Teorema de Pitágoras", "Pythagorean theorem", t=EntityType.STATEMENT),
        _wd("Q140092320", "head", "head"), _wd("Q105225393", "head", "head", "vertex which an directed edge enters"),
        Entity("concept:Mathlib/Algebra/Squarefree", EntityType.CONCEPT, (LangString("algebra › squarefree", "en"),)),
        Entity("mathlib:RingHom.snd", EntityType.STATEMENT, (LangString("RingHom.snd", "und"),), provenance=(ML,)),
        Entity("mathlib:Finsupp.x", EntityType.STATEMENT, (LangString("Finsupp.x", "und"),), provenance=(ML,)),
    ])
    s.add_claims([binary(f"c:m{i}", RelationType.MEMBER_OF_CONCEPT, d, "wd:Q258", (ML,),
                         evidence_state=EvidenceState.MODEL_PROPOSED)
                  for i, d in enumerate(["mathlib:RingHom.snd", "mathlib:Finsupp.x"])])
    yield s
    s.close()


def test_out_of_domain_excludes_retires_and_undo_restores(st):
    n = cur.out_of_domain(st)
    assert n["excluded"] == 2 and n["claims_retired"] == 2
    ex = st.curations(target_id="wd:Q258", kind="exclude_presentation")[0]
    assert ex.payload["scopes"] == cur.ALL_SCOPES and ex.actor == "curation:out-of-domain/1"
    assert st.curations(target_id="wd:Q4587955")[0].payload["scopes"] == cur.NUMBER_SCOPES
    assert {st.get_claim(c).evidence_state for c in ("c:m0", "c:m1")} == {EvidenceState.RETRACTED}
    again = cur.out_of_domain(st)  # idempotent: same rows, nothing retired twice
    assert again["claims_retired"] == 0 and len(st.curations()) == 3
    assert cur.undo_rule(st, "out-of-domain", "1", reason="test") == 3
    assert st.curations() == []
    restored = [c for c in st.iter_claims(RelationType.MEMBER_OF_CONCEPT)
                if c.evidence_state is EvidenceState.MODEL_PROPOSED]
    assert len(restored) == 2 and all(c.id.startswith("restored:") for c in restored)


def test_derived_zones_leave_only_the_general_map(st):
    cur.derived_out_of_general(st)
    (row,) = st.curations(target_id="concept:Mathlib/Algebra/Squarefree")
    assert row.payload["scopes"] == ["map:general"]
    cur.derived_out_of_general(st)
    assert len(st.curations()) == 1


def test_qualifier_is_sourced_from_the_items_own_description(st):
    cur.qualifiers(st)
    (row,) = st.curations(target_id="wd:Q1096885")
    assert row.payload["text"] == "red (subgrupo de un espacio vectorial real)"
    assert row.payload["provenance"]["quote"] == "subgroup of a real vector space"
    assert st.get_entity("wd:Q1096885").label("es") == "red"  # the source label is untouched


def test_lower_initial_keeps_proper_nouns_and_acronyms():
    assert cur.lower_initial("Grupo especial ortogonal", "special orthogonal group") == "grupo especial ortogonal"
    assert cur.lower_initial("Pitágoras", "pythagoras tree") == "pitágoras"  # EN lower: rule applies
    assert cur.lower_initial("Teorema de Pitágoras", "Pythagorean theorem") is None  # EN capitalized
    assert cur.lower_initial("Lie algebra", "Lie algebra") is None
    assert cur.lower_initial("Grupo de Lie", "Lie group") is None  # EN capitalized: rule does not apply
    assert cur.lower_initial("Grupo de tipo Lie", "group of Lie type") == "grupo de tipo Lie"
    assert cur.lower_initial("Lie", "lie group of Lie") is None  # capitalized in EN: proper noun
    assert cur.lower_initial("ALL", "all") is None


def test_capitalization_labels_and_undo(st):
    n = cur.capitalization(st)
    assert n["created"] == 1
    assert st.curations(target_id="wd:Q3117934")[0].payload["text"] == "grupo especial ortogonal"
    cur.undo_rule(st, "capitalization", "1", reason="test")
    assert st.curations(target_id="wd:Q3117934") == []


def test_duplicate_groups_only_propose_never_merge(st):
    n = cur.duplicate_proposals(st)
    assert n["probable_duplicate"] == 1
    (p,) = st.curations(kind="propose_merge")
    assert p.payload["survivor"] == "wd:Q105225393" and p.payload["merged"] == "wd:Q140092320"
    assert st.merges() == [] and st.resolve("wd:Q140092320") == "wd:Q140092320"


def _decl(name: str, module: str) -> Entity:
    prov = Provenance("src:mathlib4", f"{module}.lean:1", "atlas.ingest.mathlib", "1", "Apache-2.0", T)
    return Entity("mathlib:" + name, EntityType.STATEMENT, (LangString(name, "und"),),
                  external_ids=(("mathlib", name),), provenance=(prov,))


def _member(cid: str, decl: str, item: str, signal: str) -> object:
    p = Provenance("src:mathlib4", f"{decl} -> {item}", "concept-projection", "9", "CC0-1.0", T,
                   quote='{"signal": "%s", "evidence": "x", "rank": 0}' % signal)
    state = EvidenceState.EXTRACTED if signal == "curated" else EvidenceState.MODEL_PROPOSED
    return binary(cid, RelationType.MEMBER_OF_CONCEPT, decl, item, (p,), evidence_state=state)


def test_homonym_memberships_follow_the_keep_rule_and_undo_restores():
    """B5-04: `Disjoint` leaves «exclusive or» while `xor_self` stays; a curated link is never touched; the item
    whose sense is not mathematics (lift in data mining) also leaves the concept layer."""
    s = Store()
    s.add_source(Source("src:wikidata", "Wikidata", "CC0-1.0"))
    s.add_source(Source("src:mathlib4", "Mathlib", "Apache-2.0"))
    s.add_entities([_wd("Q498186", "disyunción exclusiva", "exclusive or"), _wd("Q3001212", "lift", "lift"),
                    _decl("Disjoint", "Mathlib/Order/Disjoint"), _decl("xor_self", "Mathlib/Logic/Basic"),
                    _decl("Bool.xor", "Mathlib/Data/Bool"), _decl("Quotient.lift", "Mathlib/Data/Quot")])
    s.add_claims([_member("c:dis", "mathlib:Disjoint", "wd:Q498186", "resolve"),
                  _member("c:xor", "mathlib:xor_self", "wd:Q498186", "signature"),
                  _member("c:cur", "mathlib:Bool.xor", "wd:Q498186", "curated"),
                  _member("c:lift", "mathlib:Quotient.lift", "wd:Q3001212", "name_doc")])
    n = cur.homonym_memberships(s)
    assert n["claims_retired"] == 2 and n["kept"] == 1 and n["out_of_layer"] == 1
    state = {c: s.get_claim(c).evidence_state for c in ("c:dis", "c:xor", "c:cur", "c:lift")}
    assert state == {"c:dis": EvidenceState.RETRACTED, "c:xor": EvidenceState.MODEL_PROPOSED,
                     "c:cur": EvidenceState.EXTRACTED, "c:lift": EvidenceState.RETRACTED}
    assert s.curations(target_id="wd:Q3001212", kind="exclude_presentation")[0].payload["scopes"] == ["concept_layer"]
    assert cur.homonym_memberships(s)["claims_retired"] == 0  # idempotent
    from atlas import concepts as C
    assert C.curated_retired_pairs(s) == {("mathlib:Disjoint", "wd:Q498186"), ("mathlib:Quotient.lift", "wd:Q3001212")}
    assert cur.undo_rule(s, "homonym-membership", "1", reason="test") == 3
    assert C.curated_retired_pairs(s) == frozenset()
    live = {(c.subject, c.evidence_state) for c in s.iter_claims(RelationType.MEMBER_OF_CONCEPT)
            if c.evidence_state is EvidenceState.MODEL_PROPOSED}
    assert ("mathlib:Disjoint", EvidenceState.MODEL_PROPOSED) in live
    s.close()


def test_named_after_ludolph_is_retired_with_its_reason():
    s = Store()
    s.add_source(Source("src:wikidata", "Wikidata", "CC0-1.0"))
    s.add_entities([_wd("Q167", "pi", "pi"), _wd("Q310771", None, "Ludolph van Ceulen", t=EntityType.PERSON)])
    s.add_claims([binary("c:c77bbb29d006dccd", RelationType.NAMED_AFTER, "wd:Q167", "wd:Q310771", (WD,))])
    assert cur.named_after(s)["claims_retired"] == 1
    assert s.get_claim("c:c77bbb29d006dccd").evidence_state is EvidenceState.RETRACTED
    (row,) = s.curations(target_id="wd:Q167")
    assert "ludolfino" in row.reason and row.actor == "curation:named-after/1"
    assert cur.named_after(s)["not_current"] == 1
    s.close()
