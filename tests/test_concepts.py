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


def item(qid: str, label: str, etype: EntityType = EntityType.CONCEPT, aliases: tuple[str, ...] = (),
         desc: str | None = None) -> Entity:
    prov = (Provenance("src:test", qid, "hand", "1", "CC0-1.0", TS),)
    return Entity("wd:" + qid, etype, (LangString(label, "en"), LangString(label + " (es)", "es")),
                  aliases=tuple(LangString(a, "en") for a in aliases),
                  descriptions=(LangString(desc, "en"),) if desc else (), provenance=prov)


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


# ---------------------------------------------------------------------- round 3 (plan 2026-10-18-ronda-3-correcciones)
# Each test below fails with concept-projection/5 and resolve/2 (commit d290e32) and passes with /6 and /3.

FILLERS = [decl(f"Filler.f{i}", "Mathlib/Order/Filler") for i in range(4)]  # so one-off labels are specific


def members_of(entities: list[Entity], claims: tuple = ()) -> tuple[set[tuple[str, str]], C.Inputs]:
    s = Store()
    s.add_source(SRC)
    s.add_entities(entities + FILLERS)
    s.add_claims(list(claims))
    inp = C.load(s, with_resolve=False)
    members = {(a.decl, a.concept) for a in C.assign(inp)[0]}
    s.close()
    return members, inp


def test_homonym_ball_is_not_snooker_ball() -> None:
    members, _ = members_of([
        decl("ball_mul", "Mathlib/Analysis/Normed/Group/Pointwise"),
        item("Q55398354", "snooker ball", aliases=("ball",), desc="usually numberless ball used in snooker"),
        item("Q838611", "ball", desc="solid figure bounded by a sphere"),
    ])
    assert ("mathlib:ball_mul", "wd:Q838611") in members
    assert ("mathlib:ball_mul", "wd:Q55398354") not in members


def test_homonym_one_letter_name_is_not_mandelbrot_set() -> None:
    members, _ = members_of([
        decl("MvPFunctor.M.dest", "Mathlib/Data/PFunctor/Multivariate/M"),
        item("Q257", "Mandelbrot set", aliases=("M set",), desc="fractal set in the complex plane"),
    ])
    assert ("mathlib:MvPFunctor.M.dest", "wd:Q257") not in members


def test_homonym_at_prime_is_not_prime_number() -> None:
    """The domain guard lets prime number into RingTheory (number theory's directories), so it is the one-token
    alias rule that keeps `IsLocalization.AtPrime` (localization at a prime ideal) away from it."""
    members, inp = members_of([
        decl("IsLocalization.AtPrime", "Mathlib/RingTheory/Localization/AtPrime/Basic", EntityType.DEFINITION),
        item("Q49008", "prime number", aliases=("prime",)),
        item("Q12479", "number theory", EntityType.AREA),
    ], (binary("c:pa", RelationType.PART_OF_AREA, "wd:Q49008", "wd:Q12479", PROV),))
    assert "RingTheory" in inp.domains["wd:Q49008"]
    assert ("mathlib:IsLocalization.AtPrime", "wd:Q49008") not in members


def test_short_labels_do_not_match_through_abbreviations() -> None:
    """The label "NE" has two letters; read through rv.ABBREV it used to become "unequal" and take `ne_zero`."""
    members, _ = members_of([
        decl("RootPairing.ne_zero", "Mathlib/LinearAlgebra/RootSystem/Defs"),
        item("Q6953653", "NE", desc="class in computational complexity theory"),
    ])
    assert ("mathlib:RootPairing.ne_zero", "wd:Q6953653") not in members


def test_only_mathematical_items_are_candidates() -> None:
    members, inp = members_of([
        decl("Nat.wiki_card", "Mathlib/Data/Nat/Card", doc="As explained on **Wikipedia**, a count."),
        decl("Nat.ramanujan", "Mathlib/NumberTheory/Taxicab", doc="The number **1728** is a cube."),
        decl("Calendar.leap", "Mathlib/Data/Calendar", doc="A **leap year starting on Wednesday** has 366 days."),
        decl("Snooker.cue", "Mathlib/Geometry/Billiard", doc="A **snooker ball** rolls."),
        decl("Square.cipher", "Mathlib/Combinatorics/Square", doc="The **Square** cipher on a square grid."),
        item("Q52", "Wikipedia", desc="free multilingual online encyclopedia"),
        item("Q2997789", "1728", desc="natural number"),
        item("Q217015", "leap year starting on Wednesday", desc="type of year on a solar calendar"),
        item("Q55398354", "snooker ball", desc="usually numberless ball used in snooker"),
        item("Q2755328", "Square", desc="block cipher invented by Joan Daemen and Vincent Rijmen"),
        item("Q11518", "Pythagorean theorem", EntityType.STATEMENT, desc="relation in Euclidean geometry"),
    ])
    assert inp.excluded == {"wd:Q52": "source", "wd:Q2997789": "number", "wd:Q217015": "date",
                            "wd:Q55398354": "other_field", "wd:Q2755328": "other_field"}
    assert "wd:Q11518" in inp.items
    assert not {c for _, c in members} & set(inp.excluded)


def test_a_curated_link_keeps_an_item_whatever_its_description() -> None:
    _, inp = members_of([
        decl("Polynomial.schwartz_zippel", "Mathlib/Algebra/Polynomial/SchwartzZippel"),
        item("Q7433182", "Schwartz-Zippel lemma", desc="tool used in software testing"),
    ], (binary("c:fa", RelationType.FORMALIZED_AS, "wd:Q7433182", "mathlib:Polynomial.schwartz_zippel", PROV),))
    assert "wd:Q7433182" in inp.items and not inp.excluded


def test_non_math_reason() -> None:
    from atlas import resolve as rv
    assert rv.non_math_reason(EntityType.CONCEPT, ["10"], ["natural number"]) == "number"
    assert rv.non_math_reason(EntityType.CONCEPT, ["Sunday"], ["day of the week"]) == "date"
    # a century in the description dates a mathematical problem; it does not make it a date (run 6 excluded it)
    assert rv.non_math_reason(EntityType.CONCEPT, ["Archimedes' cattle problem"],
                              ["a problem in number theory", "problema matemático propuesto por Arquímedes en el siglo I"]) is None
    assert rv.non_math_reason(EntityType.CONCEPT, ["Ars Magna"], ["book", "libro de matemáticas del siglo XVI"]) == "source"
    # the typographic prime ′ took `IsLocalization.AtPrime` in run 7; a character of a group is still mathematics
    assert rv.non_math_reason(EntityType.CONCEPT, ["prime"], ["character, used to name a new variable"]) == "other_field"
    assert rv.non_math_reason(EntityType.CONCEPT, ["beth number"], ["transfinite numbers, carácter Unicode"]) is None
    assert rv.non_math_reason(EntityType.CONCEPT, ["digit"], ["numerical symbol used in numeral systems"]) is None
    assert rv.non_math_reason(EntityType.CONCEPT, ["mobile app"], ["software application"]) == "other_field"
    assert rv.non_math_reason(EntityType.CONCEPT, ["domain"], ["ring where no zero divisors coincide"]) is None
    # the description also says it is mathematics: stays
    assert rv.non_math_reason(EntityType.CONCEPT, ["idempotence"],
                              ["property of operations in mathematics and computer science"]) is None
    # a theorem is a theorem whatever field the description names
    assert rv.non_math_reason(EntityType.STATEMENT, ["Nash theorem"], ["result in economics"]) is None


def test_resolve_does_not_match_names_on_a_one_token_core() -> None:
    """The label "open formula" minus its generic word is "open": not a candidate for `IsOpen`."""
    from atlas import resolve as rv
    s = Store()
    s.add_source(SRC)
    s.add_entities([decl("IsOpen", "Mathlib/Topology/Defs/Basic", EntityType.DEFINITION), *FILLERS])
    idx = rv.MathlibIndex.from_store(s)
    assert rv.item_candidates(idx, "wd:Q830486", EntityType.CONCEPT, [("open formula", 1.0)]) == []
    assert [c.right for c in rv.item_candidates(idx, "wd:Q213363", EntityType.CONCEPT, [("open", 1.0)])] == [
        "mathlib:IsOpen"]
    s.close()


def secondary_store() -> Store:
    s = Store()
    s.add_source(SRC)
    s.add_entities([
        decl("PrimeIdeal", "Mathlib/RingTheory/Ideal/Prime", EntityType.DEFINITION,
             "Every **ideal** that is prime has a **Krull dimension**."),
        *FILLERS,
        item("Q1", "prime ideal"), item("Q2", "ideal"), item("Q3", "Krull dimension"),
    ])
    return s


def test_secondary_concepts_need_an_independent_signal() -> None:
    s = secondary_store()
    inp = C.load(s, with_resolve=False)
    sc = C.Scorer(inp, use_curated=True)
    got = [(a.concept, a.rank) for a in C.assign(inp, scorer=sc)[0] if a.decl == "mathlib:PrimeIdeal"]
    # "ideal" is part of the phrase behind the primary, so it is no evidence of its own; "Krull dimension" is
    assert got == [("wd:Q1", 0), ("wd:Q3", 1)]
    assert sc.dropped == {("mathlib:PrimeIdeal", "wd:Q2")}
    assert C.independent({("phrase", ("unit", "circle"))}, {("phrase", ("circle",))})
    assert not C.independent({("phrase", ("abel",))}, {("phrase", ("abel",))})
    assert not C.independent({("namespace",)}, {("phrase", ("x",))})
    assert C.independent({("resolve", "wd:Q9")}, {("resolve", "wd:Q8")})
    s.close()


def test_retired_secondaries_are_superseded_with_history(monkeypatch) -> None:
    s = secondary_store()
    inp = C.load(s, with_resolve=False)
    monkeypatch.setattr(C, "SECONDARY_INDEPENDENT", False)  # what an older run wrote
    old, derived = C.assign(inp)
    C.write(s, old, derived)
    before = C.current_claim_ids(s)[("mathlib:PrimeIdeal", "wd:Q2")]
    monkeypatch.setattr(C, "SECONDARY_INDEPENDENT", True)
    sc = C.Scorer(inp, use_curated=True)
    new, derived = C.assign(inp, scorer=sc)
    rep = C.write(s, new, derived, retired=sc.dropped)
    assert rep["claims_retired_secondary"] == 1 and rep["claims_retracted_stale"] == 0
    assert ("mathlib:PrimeIdeal", "wd:Q2") not in C.current_claim_ids(s)
    versions = s.history(before)
    assert [v.evidence_state for v in versions] == [EvidenceState.SUPERSEDED, EvidenceState.RETRACTED]
    assert "Retired: secondary concept without a signal independent" in versions[-1].explanation[0].text
    s.close()


def test_formalizations_of_non_mathematical_items_are_homonyms() -> None:
    s = Store()
    s.add_source(SRC)
    s.add_entities([decl("Square", "Mathlib/Algebra/Group/Even", EntityType.DEFINITION), *FILLERS,
                    item("Q2755328", "Square", desc="block cipher invented by Joan Daemen and Vincent Rijmen")])
    s.add_claims([binary("c:fa-cipher", RelationType.FORMALIZED_AS, "wd:Q2755328", "mathlib:Square", PROV,
                         evidence_state=EvidenceState.MODEL_PROPOSED)])
    inp = C.load(s, with_resolve=False)
    assert C.homonym_formalizations(s, inp, C.Scorer(inp, use_curated=True).one_word) == ["c:fa-cipher"]
    s.close()


def test_zone_fallback_is_labelled_and_not_an_own_signal() -> None:
    """A zone fallback says so in its claim (plan ronda 3), and coverage does not count it as a signal."""
    a = C.Assignment("mathlib:x", "wd:Q1", 0.1, 0, "zone", "zone Mathlib/Order: 3/4", EvidenceState.MODEL_PROPOSED)
    c = C.assignment_claim(a, TS)
    assert "Respaldo por zona, sin señal propia" in c.explanation[1].text
    assert "Zone fallback, not a signal of its own" in c.explanation[0].text
    own = C.assignment_claim(C.Assignment("mathlib:x", "wd:Q1", 0.5, 0, "name_doc", "bold:exact 'x'",
                                          EvidenceState.MODEL_PROPOSED), TS)
    assert "Respaldo por zona" not in own.explanation[1].text
    s = Store()
    s.add_source(SRC)
    s.add_entities([decl("Order.x", "Mathlib/Order/Zone"), *FILLERS])
    inp = C.load(s, with_resolve=False)
    cov = C.coverage(inp, [C.Assignment("mathlib:Order.x", "wd:Q1", 0.1, 0, "zone", "z", EvidenceState.MODEL_PROPOSED)])
    assert cov["assigned"] == 1 and cov["assigned_by_own_signals"] == 0
    s.close()


def test_resolve_ignores_one_and_two_character_labels() -> None:
    """Run 6 sent `MvPFunctor.M` to «a.m.», «nautical mile» and «mega» through resolve.py's alias "M"."""
    from atlas import resolve as rv
    s = Store()
    s.add_source(SRC)
    s.add_entities([decl("MvPFunctor.M", "Mathlib/Data/PFunctor/Multivariate/M", EntityType.DEFINITION), *FILLERS])
    idx = rv.MathlibIndex.from_store(s)
    assert rv.item_candidates(idx, "wd:Q1", EntityType.CONCEPT, [("M", 0.9)]) == []
    assert rv.item_candidates(idx, "wd:Q2", EntityType.CONCEPT, [("a.m.", 0.9)]) == []
    s.close()


def test_resolve_keeps_pi_despite_the_short_label_rule() -> None:
    """resolve/3 left `Real.pi` without «pi» (Q167): the label has two letters.  SHORT_LABEL_OK exempts it."""
    from atlas import resolve as rv
    s = Store()
    s.add_source(SRC)
    s.add_entities([decl("Real.pi", "Mathlib/Analysis/SpecialFunctions/Trigonometric/Basic", EntityType.DEFINITION),
                    *FILLERS])
    idx = rv.MathlibIndex.from_store(s)
    cands = rv.item_candidates(idx, "wd:Q167", EntityType.CONCEPT, [("pi", 0.9)])
    assert any("mathlib:Real.pi" in (c.left, c.right) for c in cands)
    s.close()


# ---------------------------------------------------------------------- round 5 (ADR-0005 §5)


def test_items_curated_out_of_the_concept_layer_get_no_members_until_undone() -> None:
    """An editor's exclude_presentation (scope concept_layer) keeps a homonym item (South Africa, alias «r s»)
    out of the projection, so a re-run does not recreate the memberships the curation retired; undoing the
    curation brings it back.  Exclusions in other scopes do not touch the layer."""
    s = Store()
    s.add_source(SRC)
    s.add_entities([decl("Matrix.rs_mul", "Mathlib/LinearAlgebra/Matrix/RS"),
                    item("Q258", "South Africa", aliases=("r s",), desc="country in southern Africa"),
                    item("Q9999", "r s", desc="rank of a matrix")] + FILLERS)

    def members() -> set[tuple[str, str]]:
        return {(a.decl, a.concept) for a in C.assign(C.load(s, with_resolve=False))[0]}

    cur = s.add_curation("exclude_presentation", "wd:Q258", rule="curation:out-of-domain", rule_version="1",
                         payload={"scopes": ["map:general", "concept_layer"]}, actor="curation:out-of-domain/1",
                         reason="test: country, not mathematics")
    s.add_curation("exclude_presentation", "wd:Q9999", rule="curation:map-only", rule_version="1",
                   payload={"scopes": ["map:general"]}, actor="curation:map-only/1", reason="test: map scope only")
    inp = C.load(s, with_resolve=False)
    assert inp.curated_out == {"wd:Q258": "curation:out-of-domain/1"} and "wd:Q258" not in inp.items
    assert "wd:Q9999" in inp.items  # another scope does not take it out of the layer
    assert not any(concept == "wd:Q258" for _, concept in members())
    s.undo_curation(cur.id, actor="editor:test", reason="test: undo")
    assert "wd:Q258" in C.load(s, with_resolve=False).items
    s.close()
