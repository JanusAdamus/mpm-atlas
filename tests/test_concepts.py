"""Concept projection: assignment signals, derived zone concepts, reversible claims, evaluation split."""

from __future__ import annotations

import pytest

from atlas import (
    Entity,
    EntityType,
    EvidenceState,
    LangString,
    Provenance,
    RelationType,
    Role,
    Source,
    Store,
    binary,
    conjunction,
)
from atlas import concepts as C

TS = "2026-10-17T00:00:00+00:00"
SRC = Source("src:test", "concept test data", "CC0-1.0")


def decl(name: str, module: str, etype: EntityType = EntityType.STATEMENT, doc: str | None = None) -> Entity:
    prov = (Provenance("src:test", f"{module}.lean:1", "hand", "1", "CC0-1.0", TS),)
    return Entity("mathlib:" + name, etype, (LangString(name, "und"),),
                  descriptions=(LangString(doc, "en"),) if doc else (), external_ids=(("mathlib", name),),
                  provenance=prov)


def item(qid: str, label: str, etype: EntityType = EntityType.CONCEPT, aliases: tuple[str, ...] = ()) -> Entity:
    prov = (Provenance("src:test", qid, "hand", "1", "CC0-1.0", TS),)
    return Entity("wd:" + qid, etype, (LangString(label, "en"), LangString(label + " (es)", "es")),
                  aliases=tuple(LangString(a, "en") for a in aliases), provenance=prov)


PROV = (Provenance("src:test", "t", "hand", "1", "CC0-1.0", TS),)
ZONE = "Mathlib/Combinatorics/Widgets"


@pytest.fixture()
def store():
    s = Store()
    s.add_source(SRC)
    s.add_entities([
        decl("Nat.Prime", "Mathlib/Data/Nat/Prime/Defs", EntityType.DEFINITION,
             "`Nat.Prime p` means that `p` is a prime number."),
        decl("Nat.Prime.two_le", "Mathlib/Data/Nat/Prime/Defs"),
        decl("Nat.Prime.pos", "Mathlib/Data/Nat/Prime/Defs"),
        decl("EuclideanGeometry.dist_sq_eq", "Mathlib/Geometry/Euclidean/Angle",
             doc="**Pythagorean theorem**: the square of the hypotenuse equals the sum of the squares."),
        *[decl(f"Widget.w{i}", ZONE) for i in range(6)],
        decl("Mathlib.Tactic.foo", "Mathlib/Tactic/Foo", EntityType.RESOURCE),
        item("Q49008", "prime number", aliases=("prime",)),
        item("Q11518", "Pythagorean theorem", EntityType.STATEMENT),
        item("Q1", "monster group"),
    ])
    s.add_claims([
        binary("c:fa", RelationType.FORMALIZED_AS, "wd:Q49008", "mathlib:Nat.Prime", PROV),
        conjunction("c:dep", RelationType.DEPENDS_ON_LOGICALLY, "mathlib:Nat.Prime.pos", ["mathlib:Nat.Prime"], PROV),
    ])
    yield s
    s.close()


def by_decl(asg: list[C.Assignment]) -> dict[str, list[C.Assignment]]:
    out: dict[str, list[C.Assignment]] = {}
    for a in sorted(asg, key=lambda a: (a.decl, a.rank)):
        out.setdefault(a.decl, []).append(a)
    return out


def test_signals_zones_and_infrastructure(store) -> None:
    inp = C.load(store, with_resolve=False)
    asg, derived = C.assign(inp)
    got = by_decl(asg)
    first = {d: v[0] for d, v in got.items()}
    assert first["mathlib:Nat.Prime"].concept == "wd:Q49008"
    assert first["mathlib:Nat.Prime"].signal == "curated"
    assert first["mathlib:Nat.Prime"].state is EvidenceState.EXTRACTED
    # namespace head (or signature premise) propagates the head's concept
    assert first["mathlib:Nat.Prime.two_le"].concept == "wd:Q49008"
    assert first["mathlib:Nat.Prime.two_le"].state is EvidenceState.MODEL_PROPOSED
    assert first["mathlib:EuclideanGeometry.dist_sq_eq"].concept == "wd:Q11518"
    # a zone with no Wikidata concept gets one derived concept, extracted from the module path
    assert derived == {C.DERIVED_PREFIX + ZONE: ZONE}
    assert {first[f"mathlib:Widget.w{i}"].concept for i in range(6)} == {C.DERIVED_PREFIX + ZONE}
    assert first["mathlib:Widget.w0"].signal == "zone_derived"
    # infrastructure is never assigned
    assert "mathlib:Mathlib.Tactic.foo" not in got
    cov = C.coverage(inp, asg)
    assert cov["content_declarations"] == 10 and cov["infrastructure_declarations"] == 1
    assert cov["unassigned"] == 0


def test_masked_evaluation_does_not_see_curated_links(store) -> None:
    inp = C.load(store, with_resolve=False)
    asg, _ = C.assign(inp, use_curated=False)
    assert all(a.signal != "curated" for a in asg)
    split = C.split_of("mathlib:Nat.Prime")
    assert split in ("dev", "test") and split == C.split_of("mathlib:Nat.Prime")  # deterministic
    ev = C.evaluate(asg, inp.curated, split=split)
    assert ev["gold_pairs"] == 1
    assert ev["recall_at_3"]["value"] == 1.0  # "prime number" is in the docstring


def test_ranks_follow_the_rust_rule() -> None:
    chosen = [("wd:Q20", (0.5, "name_doc", "")), ("wd:Q3", (0.5, "name_doc", "")), ("wd:Q9", (0.9, "resolve", ""))]
    assert [r[0] for r in C._ordered(chosen)] == ["wd:Q9", "wd:Q3", "wd:Q20"]
    assert C._q("concept:Mathlib/Q1") == 1 << 40


def test_write_is_idempotent_and_reversible(store) -> None:
    inp = C.load(store, with_resolve=False)
    asg, derived = C.assign(inp)
    rep = C.write(store, asg, derived)
    assert rep["claims_added"] == len(asg) and rep["derived_entities_added"] == 1
    cid = C.claim_id("mathlib:Nat.Prime.two_le", "wd:Q49008")
    claim = store.get_claim(cid)
    assert claim.relation is RelationType.MEMBER_OF_CONCEPT
    assert claim.provenance[0].extractor == C.EXTRACTOR and claim.provenance[0].extractor_version == C.VERSION
    assert claim.confidence is not None and "not calibrated" in claim.confidence.calibration
    assert store.get_entity(C.DERIVED_PREFIX + ZONE).label("es").endswith("(Mathlib, derivado)")
    # the source data is untouched: the curated link is still there, unchanged
    assert store.get_claim("c:fa").evidence_state is EvidenceState.EXTRACTED
    again = C.write(store, asg, derived)
    assert again["claims_added"] == 0 and again["claims_already_present"] == len(asg)
    # a later run that no longer produces an assignment retracts it
    rest = [a for a in asg if a.decl != "mathlib:Nat.Prime.two_le"]
    assert C.write(store, rest, derived)["claims_retracted_stale"] == 1
    assert store.get_claim(cid).evidence_state is EvidenceState.RETRACTED
    # a re-scored pair is superseded by a new version; the old claim stays, superseded
    a0 = next(a for a in rest if a.decl == "mathlib:Nat.Prime.pos")
    rescored = [a if a is not a0 else C.Assignment(a.decl, a.concept, 0.1234, a.rank, a.signal, a.evidence, a.state)
                for a in rest]
    rep = C.write(store, rescored, derived)
    assert (rep["claims_superseded"], rep["claims_added"], rep["claims_retracted_stale"]) == (1, 0, 0)
    assert store.get_claim(C.claim_id(a0.decl, a0.concept)).evidence_state is EvidenceState.SUPERSEDED
    assert C.retract_all(store, reason="test undo") == len(rest)
    assert C.write(store, rest, derived)["claims_added"] == len(rest)  # redo after undo: fresh ids, same content
    assert C.retract_all(store, reason="test undo again") == len(rest)
    assert not store.claims_for("mathlib:Nat.Prime", relations=[RelationType.MEMBER_OF_CONCEPT])
    assert [p.entity_id for p in store.get_claim("c:fa").participants if p.role is Role.OBJECT] == ["mathlib:Nat.Prime"]


def test_audit_sample_is_stratified_and_deterministic(store) -> None:
    inp = C.load(store, with_resolve=False)
    asg, derived = C.assign(inp)
    a = C.audit_sample(inp, asg, size=6)
    assert a == C.audit_sample(inp, asg, size=6)
    assert len(a) == 6
    assert {r["stratum"] for r in a} >= {"Mathlib/Data", "Mathlib/Combinatorics", "Mathlib/Geometry"}
    assert all(r["verdict"] is None for r in a)
    C.write(store, asg, derived)
    traced = C.audit_sample(inp, asg, size=6, registry_ids=C.current_claim_ids(store))
    assert len(store.existing_claim_ids([r["claim_id"] for r in traced])) == 6, "every row names a registry claim"


def test_homonym_guard_integral_is_not_isintegral() -> None:
    """Plan P0 (E01/E06): the calculus item "integral" never takes `IsIntegral`, as member or formalization."""
    s = Store()
    s.add_source(SRC)
    s.add_entities([
        decl("IsIntegral", "Mathlib/RingTheory/IntegralClosure/IsIntegral/Defs", EntityType.DEFINITION,
             "An element `x` of an algebra is *integral* if it is a root of a monic polynomial."),
        decl("MeasureTheory.integral", "Mathlib/MeasureTheory/Integral/Bochner/Basic", EntityType.DEFINITION,
             "The Bochner **integral** of a function."),
        decl("Complex.isIntegral_I", "Mathlib/Analysis/Complex/IsIntegral"),  # in the item's domain: `Is` rule
        *[decl(f"Filler.f{i}", "Mathlib/Order/Filler") for i in range(4)],  # so "integral" is specific
        item("Q80091", "integral"),
        item("Q149972", "calculus", EntityType.AREA),
    ])
    s.add_claims([
        binary("c:area", RelationType.PART_OF_AREA, "wd:Q80091", "wd:Q149972", PROV),
        binary("c:fa-proposed", RelationType.FORMALIZED_AS, "wd:Q80091", "mathlib:IsIntegral", PROV,
               evidence_state=EvidenceState.MODEL_PROPOSED),
    ])
    inp = C.load(s, with_resolve=False)
    assert "MeasureTheory" in inp.domains["wd:Q80091"] and "RingTheory" not in inp.domains["wd:Q80091"]
    sc = C.Scorer(inp, use_curated=True)
    asg, _ = C.assign(inp, scorer=sc)
    members = {(a.decl, a.concept) for a in asg}
    assert ("mathlib:MeasureTheory.integral", "wd:Q80091") in members
    assert ("mathlib:IsIntegral", "wd:Q80091") not in members
    assert ("mathlib:Complex.isIntegral_I", "wd:Q80091") not in members
    assert C.predicate_adjectives("Algebra.IsIntegral.isIntegral") == {"integral"}
    assert C.homonym_formalizations(s, inp, sc.one_word) == ["c:fa-proposed"]
    assert C.retract_ids(s, ["c:fa-proposed"], reason="test") == 1
    assert s.get_claim("c:fa-proposed").evidence_state is EvidenceState.RETRACTED
    # regression (2026-10-18): a retracted candidate is not a curated link on the next run
    again = C.load(s, with_resolve=False)
    assert not again.curated
    members = {(a.decl, a.concept) for a in C.assign(again)[0]}
    assert ("mathlib:IsIntegral", "wd:Q80091") not in members
    s.close()
