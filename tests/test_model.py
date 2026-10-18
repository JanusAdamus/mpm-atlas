import pytest

from atlas.model import (
    RELATIONS,
    Claim,
    Confidence,
    Entity,
    EntityType,
    EvidenceState,
    FormalArtifact,
    LangString,
    Participant,
    Provenance,
    RelationType,
    Role,
    Source,
    ValidationError,
    binary,
    conjunction,
    replace,
    stable_id,
)

T = "2026-10-16T00:00:00+00:00"
P = Provenance("src:x", "p. 3", "unit-test", "1", "CC0-1.0", T)


def test_every_relation_has_a_spec_with_chain_semantics():
    assert set(RELATIONS) == set(RelationType)
    for spec in RELATIONS.values():
        assert spec.chain_authorizes
        if spec.inverse is not None:
            assert RELATIONS[spec.inverse].inverse is spec.name
    assert not RELATIONS[RelationType.ANALOGOUS_TO].transitive
    assert RELATIONS[RelationType.EQUIVALENT_TO].symmetric
    assert [r for r, s in RELATIONS.items() if s.allows_premises] == [RelationType.DEPENDS_ON_LOGICALLY]


def test_conjunction_claim_exposes_premise_set():
    c = conjunction("c:1", "depends_on_logically", "s:C", ["s:A", "s:B"], [P])
    assert c.subject == "s:C" and c.targets == ("s:A", "s:B") and c.is_conjunction


def test_shape_rules_reject_malformed_claims():
    with pytest.raises(ValidationError, match="provenance"):
        binary("c:1", "generalizes", "s:A", "s:B", [])
    with pytest.raises(ValidationError, match="premises"):
        conjunction("c:1", "generalizes", "s:A", ["s:B"], [P])
    with pytest.raises(ValidationError, match="exactly one subject"):
        Claim("c:1", RelationType.CITES, (Participant(Role.OBJECT, "p:1"),), (P,))
    with pytest.raises(ValidationError, match="either one object"):
        Claim("c:1", RelationType.DEPENDS_ON_LOGICALLY,
              (Participant(Role.SUBJECT, "s:C"), Participant(Role.OBJECT, "s:A"), Participant(Role.PREMISE, "s:B")),
              (P,))
    with pytest.raises(ValidationError, match="own object"):
        binary("c:1", "equivalent_to", "s:A", "s:A", [P])
    with pytest.raises(ValidationError, match="formal artifact"):
        binary("c:1", "formalized_as", "s:A", "s:B", [P], evidence_state=EvidenceState.FORMALLY_LINKED)
    with pytest.raises(ValidationError, match="superseded_by"):
        binary("c:1", "generalizes", "s:A", "s:B", [P], evidence_state="superseded")
    with pytest.raises(ValidationError, match="repeated"):
        conjunction("c:1", "depends_on_logically", "s:C", ["s:A", "s:A"], [P])


def test_formally_linked_with_artifact_is_valid():
    f = FormalArtifact("mathlib4", "abc123", "Nat.Prime.two_le", ("propext",))
    c = binary("c:1", "formalized_as", "s:A", "s:B", [P], evidence_state="formally_linked", formal=f)
    assert c.formal.axioms == ("propext",)


def test_provenance_and_source_validation():
    with pytest.raises(ValidationError, match="ISO-8601"):
        Provenance("src:x", "p. 3", "t", "1", "CC0-1.0", "yesterday")
    with pytest.raises(ValidationError, match="extractor_version"):
        Provenance("src:x", "p. 3", "t", " ", "CC0-1.0", T)
    with pytest.raises(ValidationError, match="declared"):
        Source("src:x", "X", "unknown")


def test_confidence_must_say_what_it_estimates():
    with pytest.raises(ValidationError):
        Confidence(0.9, "", "isotonic")
    with pytest.raises(ValidationError):
        Confidence(1.2, "P(accept)", "isotonic")


def test_entity_labels_and_fallback():
    e = Entity("s:A", EntityType.STATEMENT, (LangString("Pythagorean theorem", "en"),))
    assert e.label("es") == "Pythagorean theorem"
    with pytest.raises(ValidationError, match="two preferred"):
        Entity("s:A", "statement", (LangString("a", "en"), LangString("b", "en")))
    with pytest.raises(ValidationError, match="language tag"):
        LangString("x", "English")
    with pytest.raises(ValidationError, match="at least one label"):
        Entity("s:A", "statement", ())


def test_json_roundtrip_is_lossless():
    e = Entity("s:A", "statement", (LangString("Teorema", "es"), LangString("Theorem", "en")),
               aliases=(LangString("T1", "en"), LangString("T2", "en")),
               statements=(LangString("a^2+b^2=c^2", "en"),), formula="a^2+b^2=c^2",
               external_ids=(("wikidata", "Q1"),), provenance=(P,))
    assert Entity.from_json(e.to_json()) == e
    c = conjunction("c:1", "depends_on_logically", "s:C", ["s:A", "s:B"], [P],
                    extra=(Participant(Role.VIA, "p:1"),), assumptions=("char K = 0",), validity_domain="fields",
                    alternative_group="g:1", confidence=Confidence(0.5, "P(accept)", "none"),
                    explanation=(LangString("porque", "es"),),
                    formal=FormalArtifact("mathlib4", "v", "X.y", ("Classical.choice",)))
    assert Claim.from_json(c.to_json()) == c


def test_claims_are_immutable_and_replace_revalidates():
    c = binary("c:1", "generalizes", "s:A", "s:B", [P])
    with pytest.raises(AttributeError):
        c.relation = RelationType.CITES  # type: ignore[misc]
    with pytest.raises(ValidationError):
        replace(c, provenance=())


def test_stable_id_is_deterministic():
    assert stable_id("c", "a", "b") == stable_id("c", "a", "b") != stable_id("c", "ab")
    assert stable_id("c", "x").startswith("c:")
