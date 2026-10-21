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


# ------------------------------------------------------------------ v4 (ADR-0005)

from atlas.ingest.wikidata import edtf_from_wikidata  # noqa: E402
from atlas.model import (  # noqa: E402
    CURRENT_STATES,
    DOCS,
    EVIDENCE_RANK,
    MATH,
    DateKind,
    FichaSection,
    HistoricalDate,
    NoteInfo,
    check_state_allowed,
    parse_edtf,
)


@pytest.mark.parametrize("text,calendar,span", [
    ("-0299", "gregorian", (-299, -299)),
    ("-0299~", "gregorian", (-299, -299)),
    ("-1899~/-1599~", "gregorian", (-1899, -1599)),
    ("0000", "gregorian", (0, 0)),
    ("1801-07-15", "gregorian", (1801, 1801)),
    ("19XX", "gregorian", (1900, 1999)),
    ("180X", "gregorian", (1800, 1809)),
    ("../1600", "gregorian", (None, 1600)),
    ("1600/..", "gregorian", (1600, None)),
    ("1680?", "gregorian", (1680, 1680)),
    ("1680%", "gregorian", (1680, 1680)),
    ("0000-02-29", "gregorian", (0, 0)),  # proleptic Gregorian: year 0 is a leap year
    ("1900-02-29", "julian", (1900, 1900)),
])
def test_edtf_subset_accepts_and_spans(text, calendar, span):
    assert parse_edtf(text, calendar) == span


@pytest.mark.parametrize("text,calendar", [
    ("1801-13", "gregorian"), ("1900-02-29", "gregorian"), ("-02XX", "gregorian"), ("1600/1500", "gregorian"),
    ("", "gregorian"), ("abc", "gregorian"), ("-0000", "gregorian"), ("../..", "gregorian"), ("1801-04-31", "julian"),
    ("1801", "hijri"),
])
def test_edtf_subset_rejects(text, calendar):
    with pytest.raises(ValidationError):
        parse_edtf(text, calendar)


@pytest.mark.parametrize("args,edtf,calendar", [
    (("-0300-00-00T00:00:00Z", 9), "-0299", "gregorian"),
    (("-0300-00-00T00:00:00Z", 7), "-0299/-0200", "gregorian"),  # 3rd century BC
    (("-0001-00-00T00:00:00Z", 9), "0000", "gregorian"),
    (("+1801-00-00T00:00:00Z", 9), "1801", "gregorian"),
    (("+1801-07-15T00:00:00Z", 11), "1801-07-15", "gregorian"),
    (("+1801-07-00T00:00:00Z", 10), "1801-07", "gregorian"),
    (("+1850-00-00T00:00:00Z", 7), "1801/1900", "gregorian"),
    (("+1950-00-00T00:00:00Z", 8), "1950/1959", "gregorian"),
    (("-1799-00-00T00:00:00Z", 9), "-1798", "gregorian"),  # Plimpton 322 in Wikidata
    (("-0300-00-00T00:00:00Z", 9, "Q1985786"), "-0299", "julian"),
    (("+1403-00-00T00:00:00Z", 9, "Q12345"), "1403", "unknown"),
])
def test_edtf_from_wikidata(args, edtf, calendar):
    out = edtf_from_wikidata(*args)
    assert (out.edtf, out.calendar) == (edtf, calendar)
    assert out.raw.startswith(args[0])


def test_edtf_from_wikidata_qualifiers():
    t = "-0300-00-00T00:00:00Z"
    assert edtf_from_wikidata(t, 9, qualifiers={"P1480": "Q5727902"}).edtf == "-0299~"
    assert edtf_from_wikidata(t, 9, qualifiers={"P1480": ["Q18122778"]}).edtf == "-0299?"
    assert edtf_from_wikidata(t, 9, qualifiers={"P1480": ["Q5727902", "Q18122778"]}).edtf == "-0299%"
    span = {"P1319": ("-1900-00-00T00:00:00Z", 9), "P1326": ("-1600-00-00T00:00:00Z", 9), "P1480": "Q5727902"}
    assert edtf_from_wikidata("-1800-00-00T00:00:00Z", 9, qualifiers=span).edtf == "-1899~/-1599~"
    with pytest.raises(ValidationError, match="year 0"):
        edtf_from_wikidata("+0000-00-00T00:00:00Z", 9)
    assert edtf_from_wikidata("+0000-00-00T00:00:00Z", 9, channel="wdqs").edtf == "0000"


def test_historical_date_needs_provenance_and_valid_edtf():
    d = HistoricalDate(DateKind.COPIED, "-1899~/-1599~", P, raw="ca. 1900-1600 BC")
    assert d.span == (-1899, -1599)
    with pytest.raises(ValidationError, match="provenance"):
        HistoricalDate(DateKind.COPIED, "-1798", None)  # type: ignore[arg-type]
    with pytest.raises(ValidationError):
        HistoricalDate("copied", "1900-02-29", P)
    assert HistoricalDate("copied", "1900-02-29", P, calendar="julian").kind is DateKind.COPIED
    e = Entity("cdli:P254790", "witness", (LangString("Plimpton 322", "en"),),
               dates=(d, HistoricalDate("copied", "-1798", P, raw="-1799-00-00T00:00:00Z/9")))
    assert Entity.from_json(e.to_json()) == e


def test_claim_historical_date_is_validated_and_old_json_reads():
    assert binary("c:1", "influenced", "s:A", "s:B", [P], historical_date="1801").historical_date == "1801"
    with pytest.raises(ValidationError, match="historical_date"):
        binary("c:1", "influenced", "s:A", "s:B", [P], historical_date="early 1800s")
    old = binary("c:1", "generalizes", "s:A", "s:B", [P]).to_json()
    del old["historical_date"]
    assert Claim.from_json(old).historical_date is None


def test_new_relation_invariants():
    R = RelationType
    s = RELATIONS[R.SAID_EQUIVALENT_TO]
    assert s.symmetric and not s.transitive and s.inverse is None
    assert not RELATIONS[R.INSTANCE_OF].transitive and not RELATIONS[R.INTRODUCED_IN].transitive
    assert RELATIONS[R.COPIED_BY].subject_types == frozenset({EntityType.WITNESS})
    assert RELATIONS[R.COPIED_BY].object_types == frozenset({EntityType.PERSON})
    assert RELATIONS[R.EXPLAINS].subject_types == frozenset({EntityType.NOTE})
    assert RELATIONS[R.EXPLAINS].object_types == MATH | {EntityType.AREA}
    assert EntityType.WITNESS in DOCS and EntityType.WITNESS in RELATIONS[R.AUTHORED_BY].subject_types
    for rel in (R.PART_OF_AREA, R.ANALOGOUS_TO, R.APPEARS_IN):  # notes never take part in generic relations
        assert EntityType.NOTE not in RELATIONS[rel].subject_types
    for rel in (R.INTRODUCED_IN, R.COPIED_BY, R.EDITED_BY, R.TRANSLATED_BY):
        assert RELATIONS[rel].projection.value == "history"
    assert RELATIONS[R.EXPLAINS].projection.value == "learning"


def test_formally_linked_is_rejected_where_not_allowed():
    for rel, spec in RELATIONS.items():
        if spec.projection.value in ("biblio", "history") or rel in (RelationType.PART_OF_AREA,
                                                                      RelationType.MEMBER_OF_CONCEPT):
            assert not spec.formal_allowed, rel
            with pytest.raises(ValidationError, match="formally_linked"):
                check_state_allowed(rel, EvidenceState.FORMALLY_LINKED)
            check_state_allowed(rel, EvidenceState.EXTRACTED)
    check_state_allowed(RelationType.FORMALIZED_AS, EvidenceState.FORMALLY_LINKED)


def test_note_info_iff_note_type():
    info = NoteInfo(FichaSection.QUE_ES, 0)
    n = Entity("note:x", "note", (LangString("Un número primo…", "es"),), statements=(LangString("…", "es"),),
               note=info)
    assert Entity.from_json(n.to_json()) == n
    with pytest.raises(ValidationError, match="only for, type note"):
        Entity("note:y", "note", (LangString("x", "es"),))
    with pytest.raises(ValidationError, match="only for, type note"):
        Entity("s:y", "statement", (LangString("x", "es"),), note=info)
    with pytest.raises(ValidationError):
        NoteInfo("que_es", -1)
    with pytest.raises(ValueError):
        NoteInfo("resumen", 0)


def test_evidence_rank_covers_current_states():
    assert set(EVIDENCE_RANK) == CURRENT_STATES
    order = sorted(EVIDENCE_RANK, key=EVIDENCE_RANK.get, reverse=True)
    assert [s.value for s in order] == ["formally_linked", "editorially_reviewed", "extracted", "model_proposed",
                                        "disputed"]


def test_source_kind_is_checked():
    assert Source("src:h", "History of X", "CC-BY-4.0", kind="historiography").kind == "historiography"
    with pytest.raises(ValidationError, match="kind"):
        Source("src:h", "History of X", "CC-BY-4.0", kind="blog")


def test_vocabulary_parity_with_rust():
    """Python side of the parity check: the enums of core/atlas-core/src/vocab.rs, in code order, are the
    Python enums in declaration order (the Rust side, tests/vocab.rs, parses model.py)."""
    import re
    from pathlib import Path

    from atlas.model import PRESENTATION_SCOPES, EvidenceState, Projection, Role

    src = (Path(__file__).resolve().parents[1] / "core" / "atlas-core" / "src" / "vocab.rs").read_text(encoding="utf-8")

    def rust_enum(name: str) -> list[str]:
        body = src.split(f"    {name}, \"")[1].split("}")[0]
        return re.findall(r'= "([a-z_]+)"', body)

    for enum in (EntityType, RelationType, EvidenceState, Role, Projection):
        assert rust_enum(enum.__name__) == [x.value for x in enum], enum.__name__
    scopes = re.search(r"PRESENTATION_SCOPES: \[&str; \d+\] = \[([^\]]*)\]", src).group(1)
    assert re.findall(r'"([^"]+)"', scopes) == list(PRESENTATION_SCOPES)
