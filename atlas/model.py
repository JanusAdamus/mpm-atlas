"""Domain model of the atlas: entities, n-ary claims, evidence, provenance, events.

Everything here is a frozen value object.  Construction validates; invalid
objects cannot exist.  `to_json()` / `from_json()` give the canonical JSON
shape used by the store's `data` column and by the HTTP API.
"""

from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass, field, replace
from datetime import datetime
from enum import StrEnum
from typing import Any, Iterable, Mapping

SCHEMA_VERSION = 4


class ValidationError(ValueError):
    """Raised when a domain object or a store operation breaks the contract."""


# --------------------------------------------------------------------------- enums


class EntityType(StrEnum):
    CONCEPT = "concept"
    DEFINITION = "definition"
    THEORY = "theory"
    STATEMENT = "statement"
    PROOF = "proof"
    CONJECTURE = "conjecture"
    COUNTEREXAMPLE = "counterexample"
    ALGORITHM = "algorithm"
    MODEL = "model"
    PROBLEM = "problem"
    PHENOMENON = "phenomenon"
    APPLICATION = "application"
    PUBLICATION = "publication"
    SOURCE_FRAGMENT = "source_fragment"
    PERSON = "person"
    RESOURCE = "resource"
    EXPERIMENT = "experiment"
    AREA = "area"  # MSC-like region of mathematics; target of part_of_area
    WITNESS = "witness"  # v4: physical witness of a work (tablet, papyrus, manuscript); ADR-0005 §2
    NOTE = "note"  # v4: one sentence of a reader's card (ficha), text in `statements`; ADR-0005 §6


class RelationType(StrEnum):
    DEPENDS_ON_LOGICALLY = "depends_on_logically"
    PROVES = "proves"
    PREREQUISITE_FOR = "prerequisite_for"
    GENERALIZES = "generalizes"
    SPECIAL_CASE_OF = "special_case_of"
    EQUIVALENT_TO = "equivalent_to"
    ANALOGOUS_TO = "analogous_to"
    APPLIED_IN = "applied_in"
    MODELS = "models"
    CITES = "cites"
    INFLUENCED = "influenced"
    PART_OF_AREA = "part_of_area"
    FORMALIZED_AS = "formalized_as"
    NAMED_AFTER = "named_after"
    COUNTEREXAMPLE_TO = "counterexample_to"
    APPEARS_IN = "appears_in"
    AUTHORED_BY = "authored_by"
    MEMBER_OF_CONCEPT = "member_of_concept"  # schema v2 (v3: areas as objects): concept projection
    # v4 (ADR-0005)
    SAID_EQUIVALENT_TO = "said_equivalent_to"  # Wikidata P460: a source says they may be the same; never composes
    INSTANCE_OF = "instance_of"  # Wikidata P31: x is an example of B (P279 stays special_case_of)
    INTRODUCED_IN = "introduced_in"  # needs a historiography source
    COPIED_BY = "copied_by"  # witness -> copyist
    EDITED_BY = "edited_by"
    TRANSLATED_BY = "translated_by"
    EXPLAINS = "explains"  # note -> what it explains


class EvidenceState(StrEnum):
    EXTRACTED = "extracted"
    MODEL_PROPOSED = "model_proposed"
    EDITORIALLY_REVIEWED = "editorially_reviewed"
    FORMALLY_LINKED = "formally_linked"
    DISPUTED = "disputed"
    SUPERSEDED = "superseded"
    RETRACTED = "retracted"


# States a claim is "current" in (shown by default, used by path engine).
CURRENT_STATES = frozenset(
    {
        EvidenceState.EXTRACTED,
        EvidenceState.MODEL_PROPOSED,
        EvidenceState.EDITORIALLY_REVIEWED,
        EvidenceState.FORMALLY_LINKED,
        EvidenceState.DISPUTED,
    }
)
# Strength order used for "weakest link" (ADR-0005 §7a): an aggregate is never stronger than its weakest part.
EVIDENCE_RANK: Mapping[EvidenceState, int] = {
    EvidenceState.FORMALLY_LINKED: 4,
    EvidenceState.EDITORIALLY_REVIEWED: 3,
    EvidenceState.EXTRACTED: 2,
    EvidenceState.MODEL_PROPOSED: 1,
    EvidenceState.DISPUTED: 0,
}
assert set(EVIDENCE_RANK) == CURRENT_STATES
# Allowed manual transitions via Store.set_evidence_state.  SUPERSEDED is only
# reachable through Store.supersede; RETRACTED is terminal.
TRANSITIONS: Mapping[EvidenceState, frozenset[EvidenceState]] = {
    s: frozenset(CURRENT_STATES | {EvidenceState.RETRACTED}) - {s} for s in CURRENT_STATES
} | {EvidenceState.SUPERSEDED: frozenset({EvidenceState.RETRACTED}), EvidenceState.RETRACTED: frozenset()}


class Role(StrEnum):
    SUBJECT = "subject"  # the claim is about this entity: SUBJECT --relation--> OBJECT
    OBJECT = "object"  # single other end of a binary relation
    PREMISE = "premise"  # conjunctive other end: ALL premises jointly (relations with allows_premises)
    ASSUMPTION = "assumption"  # entity-level hypothesis the claim holds under
    CONTEXT = "context"  # theory/area/framework in which the claim is stated
    VIA = "via"  # the proof/argument/method that realizes the relation


class Projection(StrEnum):
    STRUCTURE = "structure"
    PROOF = "proof"
    LEARNING = "learning"
    APPLICATION = "application"
    HISTORY = "history"
    BIBLIO = "biblio"


# --------------------------------------------------------------------------- relation registry

MATH = frozenset(
    {
        EntityType.CONCEPT,
        EntityType.DEFINITION,
        EntityType.THEORY,
        EntityType.STATEMENT,
        EntityType.PROOF,
        EntityType.CONJECTURE,
        EntityType.COUNTEREXAMPLE,
        EntityType.ALGORITHM,
        EntityType.MODEL,
        EntityType.PROBLEM,
    }
)
WORLD = frozenset({EntityType.PHENOMENON, EntityType.APPLICATION, EntityType.EXPERIMENT, EntityType.PROBLEM, EntityType.MODEL})
DOCS = frozenset({EntityType.PUBLICATION, EntityType.SOURCE_FRAGMENT, EntityType.RESOURCE, EntityType.WITNESS})
# Notes (ficha sentences) only take part in `explains`: never in the generic relations.
NON_PERSON = frozenset(EntityType) - {EntityType.PERSON, EntityType.NOTE}


@dataclass(frozen=True)
class RelationSpec:
    """Static semantics of a relation type.  The path engine composes on these."""

    name: RelationType
    projection: Projection
    symmetric: bool
    transitive: bool
    inverse: RelationType | None
    allows_premises: bool
    subject_types: frozenset[EntityType] | None  # None = any type
    object_types: frozenset[EntityType] | None
    chain_authorizes: str  # what a chain of this relation alone lets the reader conclude
    formal_allowed: bool = True  # v4: may a claim of this relation be formally_linked?


def _spec(name: RelationType, proj: Projection, *, sym: bool = False, trans: bool = False,
          inv: RelationType | None = None, premises: bool = False,
          subj: frozenset[EntityType] | None = None, obj: frozenset[EntityType] | None = None,
          formal: bool = True, chain: str) -> RelationSpec:
    return RelationSpec(name, proj, sym, trans, inv, premises, subj, obj, chain, formal)


R = RelationType
E = EntityType
RELATIONS: Mapping[RelationType, RelationSpec] = {
    s.name: s
    for s in (
        _spec(R.DEPENDS_ON_LOGICALLY, Projection.PROOF, trans=True, premises=True, subj=MATH, obj=MATH,
              chain="logical dependency; assumptions of every hop accumulate (union)"),
        _spec(R.PROVES, Projection.PROOF, subj=frozenset({E.PROOF, E.ALGORITHM}),
              obj=frozenset({E.STATEMENT, E.CONJECTURE, E.PROBLEM}),
              chain="not chainable; a single hop says this proof establishes that statement"),
        _spec(R.PREREQUISITE_FOR, Projection.LEARNING, trans=True, subj=MATH | DOCS, obj=MATH | DOCS,
              chain="a study order suggestion; never a logical necessity"),
        _spec(R.GENERALIZES, Projection.STRUCTURE, trans=True, inv=R.SPECIAL_CASE_OF, subj=MATH, obj=MATH,
              chain="generalization (transitive, same validity domain or wider)"),
        _spec(R.SPECIAL_CASE_OF, Projection.STRUCTURE, trans=True, inv=R.GENERALIZES, subj=MATH, obj=MATH,
              chain="specialization (transitive)"),
        _spec(R.EQUIVALENT_TO, Projection.STRUCTURE, sym=True, trans=True, subj=MATH, obj=MATH,
              chain="equivalence under the intersection of all hops' assumptions"),
        _spec(R.ANALOGOUS_TO, Projection.STRUCTURE, sym=True, subj=NON_PERSON, obj=NON_PERSON,
              chain="nothing beyond a hypothesis; analogy chains never conclude application or equivalence"),
        _spec(R.APPLIED_IN, Projection.APPLICATION, subj=MATH, obj=WORLD,
              chain="single hop: documented use; chained only after equivalence/specialization on the math side"),
        _spec(R.MODELS, Projection.APPLICATION, subj=MATH, obj=WORLD,
              chain="single hop: a model of a phenomenon; empirical adequacy needs its own evidence"),
        _spec(R.CITES, Projection.BIBLIO, subj=DOCS, obj=DOCS,
              formal=False,
              chain="bibliographic reach only; implies neither dependency nor agreement"),
        _spec(R.INFLUENCED, Projection.HISTORY, trans=False, subj=None, obj=None,
              formal=False,
              chain="historical influence; each hop needs its own historical source"),
        _spec(R.PART_OF_AREA, Projection.STRUCTURE, trans=True, subj=NON_PERSON, obj=frozenset({E.AREA}),
              formal=False,
              chain="area membership (area hierarchy is transitive)"),
        _spec(R.FORMALIZED_AS, Projection.PROOF, subj=MATH, obj=MATH | frozenset({E.RESOURCE}),
              chain="translation to a formal artifact; fidelity is a separate assessment from compilation"),
        _spec(R.NAMED_AFTER, Projection.HISTORY, subj=NON_PERSON, obj=frozenset({E.PERSON}),
              formal=False,
              chain="naming only; implies no authorship or priority"),
        _spec(R.COUNTEREXAMPLE_TO, Projection.PROOF,
              subj=frozenset({E.COUNTEREXAMPLE, E.MODEL, E.CONCEPT, E.STATEMENT, E.DEFINITION}),
              obj=frozenset({E.STATEMENT, E.CONJECTURE}),
              chain="single hop: refutes the target under the stated assumptions"),
        _spec(R.APPEARS_IN, Projection.BIBLIO, subj=NON_PERSON, obj=DOCS,
              formal=False,
              chain="location in literature; not a citation of correctness"),
        _spec(R.AUTHORED_BY, Projection.HISTORY, subj=DOCS | MATH, obj=frozenset({E.PERSON}),
              formal=False,
              chain="authorship attribution"),
        _spec(R.MEMBER_OF_CONCEPT, Projection.STRUCTURE, subj=MATH | frozenset({E.RESOURCE}), obj=MATH | frozenset({E.AREA}),
              formal=False,
              chain="grouping only: the declaration is about the concept; implies no dependency, no equivalence "
                    "and no formalization"),
        # v4 (ADR-0005)
        _spec(R.SAID_EQUIVALENT_TO, Projection.STRUCTURE, sym=True, subj=MATH, obj=MATH,
              chain="a source says they may be the same; does not compose"),
        _spec(R.INSTANCE_OF, Projection.STRUCTURE, subj=MATH, obj=MATH,
              chain="x is an example of B; only instance_of then special_case_of* composes (to instance_of)"),
        _spec(R.INTRODUCED_IN, Projection.HISTORY, subj=MATH, obj=frozenset({E.PUBLICATION, E.WITNESS}),
              formal=False,
              chain="a historiographic source says it was introduced here; does not chain and implies no proof "
                    "or prerequisite"),
        _spec(R.COPIED_BY, Projection.HISTORY, subj=frozenset({E.WITNESS}), obj=frozenset({E.PERSON}), formal=False,
              chain="copied this witness; implies neither authorship nor priority"),
        _spec(R.EDITED_BY, Projection.HISTORY, subj=frozenset({E.PUBLICATION}), obj=frozenset({E.PERSON}),
              formal=False, chain="edited this edition; implies no authorship"),
        _spec(R.TRANSLATED_BY, Projection.HISTORY, subj=frozenset({E.PUBLICATION}), obj=frozenset({E.PERSON}),
              formal=False, chain="translated this edition; implies no authorship"),
        _spec(R.EXPLAINS, Projection.LEARNING, subj=frozenset({E.NOTE}), obj=MATH | frozenset({E.AREA}), formal=False,
              chain="text for the reader; does not chain and asserts nothing mathematical beyond itself"),
    )
}
del R, E
assert set(RELATIONS) == set(RelationType)

# --------------------------------------------------------------------------- helpers

ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:\-/#]{0,255}$")
LANG_RE = re.compile(r"^[a-z]{2,3}(-[A-Za-z0-9]{1,8})*$")


def stable_id(prefix: str, *parts: str) -> str:
    """Deterministic id for idempotent ingest: `prefix:` + 16 hex of sha1(parts)."""
    digest = hashlib.sha1("\x1f".join(parts).encode("utf-8")).hexdigest()[:16]
    return f"{prefix}:{digest}"


def _check_id(value: str, what: str) -> None:
    if not isinstance(value, str) or not ID_RE.match(value):
        raise ValidationError(f"invalid {what} id: {value!r}")


def _check_text(value: str, what: str) -> None:
    if not isinstance(value, str) or not value.strip():
        raise ValidationError(f"{what} must be a non-empty string")


def _check_iso(value: str, what: str) -> None:
    try:
        datetime.fromisoformat(value)
    except (TypeError, ValueError) as exc:
        raise ValidationError(f"{what} must be ISO-8601, got {value!r}") from exc


def now_iso() -> str:
    return datetime.now().astimezone().isoformat(timespec="seconds")


# --------------------------------------------------------------------------- EDTF subset (ADR-0005 §1)

CALENDARS = frozenset({"gregorian", "julian", "unknown"})
_EDTF_DATE = re.compile(r"^(-?)(\d{4}|\d{3}X|\d{2}XX)(?:-(\d{2})(?:-(\d{2}))?)?([?~%])?$")
_MONTH_DAYS = (31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31)


def _is_leap(year: int, calendar: str) -> bool:
    """Astronomical year numbering (0 = 1 BC).  Proleptic Gregorian, or Julian; 'unknown' takes the
    Julian rule, the more permissive of the two (every Gregorian leap year is a Julian one)."""
    if calendar == "gregorian":
        return year % 4 == 0 and (year % 100 != 0 or year % 400 == 0)
    return year % 4 == 0


def _edtf_point(text: str, calendar: str) -> tuple[tuple[int, int, int], tuple[int, int, int]]:
    """Earliest and latest (year, month, day) of one EDTF date of the subset."""
    m = _EDTF_DATE.match(text)
    if not m:
        raise ValidationError(f"not an EDTF date of the supported subset: {text!r}")
    sign, year_s, month_s, day_s, _ = m.groups()
    if "X" in year_s:
        if sign or month_s:
            raise ValidationError(f"unspecified digits only in years >= 0, without month: {text!r}")
        return (int(year_s.replace("X", "0")), 1, 1), (int(year_s.replace("X", "9")), 12, 31)
    year = int(year_s)
    if sign and year == 0:
        raise ValidationError(f"-0000 is not a year; 1 BC is 0000: {text!r}")
    year = -year if sign else year
    if month_s is None:
        return (year, 1, 1), (year, 12, 31)
    month = int(month_s)
    if not 1 <= month <= 12:
        raise ValidationError(f"month out of range in {text!r}")
    last = _MONTH_DAYS[month - 1] + int(month == 2 and _is_leap(year, calendar))
    if day_s is None:
        return (year, month, 1), (year, month, last)
    day = int(day_s)
    if not 1 <= day <= last:
        raise ValidationError(f"day out of range in {text!r} ({calendar} calendar)")
    return (year, month, day), (year, month, day)


def parse_edtf(text: str, calendar: str = "gregorian") -> tuple[int | None, int | None]:
    """Validate a date of the EDTF level 0/1 subset of ADR-0005 §1 and return (lo_year, hi_year):
    inclusive astronomical years, None for an open end.  No `datetime`: years <= 0 are valid."""
    if calendar not in CALENDARS:
        raise ValidationError(f"calendar must be one of {sorted(CALENDARS)}, got {calendar!r}")
    if not isinstance(text, str) or not text:
        raise ValidationError("an EDTF date must be a non-empty string")
    parts = text.split("/")
    if len(parts) == 1:
        lo, hi = _edtf_point(text, calendar)
        return lo[0], hi[0]
    if len(parts) != 2 or parts == ["..", ".."]:
        raise ValidationError(f"not an EDTF interval of the supported subset: {text!r}")
    start = None if parts[0] == ".." else _edtf_point(parts[0], calendar)[0]
    end = None if parts[1] == ".." else _edtf_point(parts[1], calendar)[1]
    if start is not None and end is not None and start > end:
        raise ValidationError(f"interval start after its end: {text!r}")
    return (None if start is None else start[0]), (None if end is None else end[0])


# --------------------------------------------------------------------------- value objects


@dataclass(frozen=True)
class LangString:
    text: str
    lang: str

    def __post_init__(self) -> None:
        _check_text(self.text, "text")
        if not LANG_RE.match(self.lang):
            raise ValidationError(f"invalid language tag {self.lang!r}")


SOURCE_KINDS = frozenset({"dataset", "library", "encyclopedia", "historiography", "editorial"})


@dataclass(frozen=True)
class Source:
    """A dataset, library or document the atlas reads from (WP02 registry row)."""

    id: str
    title: str
    license: str  # SPDX id or URL; "unknown" is not accepted
    uri: str | None = None
    version: str | None = None
    retrieved_at: str | None = None
    terms: str | None = None  # free-text reuse conditions
    kind: str | None = None  # v4: one of SOURCE_KINDS; introduced_in needs a "historiography" source

    def __post_init__(self) -> None:
        _check_id(self.id, "source")
        _check_text(self.title, "source title")
        _check_text(self.license, "source license")
        if self.license.strip().lower() == "unknown":
            raise ValidationError("source license must be declared, not 'unknown'")
        if self.retrieved_at is not None:
            _check_iso(self.retrieved_at, "retrieved_at")
        if self.kind is not None and self.kind not in SOURCE_KINDS:
            raise ValidationError(f"source kind must be one of {sorted(SOURCE_KINDS)}, got {self.kind!r}")


@dataclass(frozen=True)
class Provenance:
    source_id: str
    locator: str  # page/section/line/xpath/decl name: where in the source
    extractor: str  # tool or person that produced the record
    extractor_version: str
    license: str
    retrieved_at: str  # ISO-8601
    quote: str | None = None  # justifying fragment, when license allows

    def __post_init__(self) -> None:
        _check_id(self.source_id, "source")
        for name in ("locator", "extractor", "extractor_version", "license"):
            _check_text(getattr(self, name), name)
        _check_iso(self.retrieved_at, "retrieved_at")


class DateKind(StrEnum):
    COMPOSED = "composed"  # composition of the work (estimated)
    COPIED = "copied"  # production of the physical witness (copy or autograph)
    PUBLISHED = "published"  # first publication
    EDITION = "edition"  # date of one edition or translation
    BORN = "born"
    DIED = "died"
    FLORUIT = "floruit"


@dataclass(frozen=True)
class HistoricalDate:
    """One sourced date of an entity.  `edtf` is the ADR-0005 §1 subset in ASTRONOMICAL numbering
    (0000 = 1 BC, -0299 = 300 BC); the calendar is never converted silently."""

    kind: DateKind
    edtf: str
    provenance: Provenance  # required: every date has its own source
    calendar: str = "gregorian"
    raw: str | None = None  # exact source value, e.g. Wikidata "-0300-00-00T00:00:00Z/7 Q1985786"

    def __post_init__(self) -> None:
        object.__setattr__(self, "kind", DateKind(self.kind))
        if not isinstance(self.provenance, Provenance):
            raise ValidationError("a historical date needs its provenance")
        parse_edtf(self.edtf, self.calendar)

    @property
    def span(self) -> tuple[int | None, int | None]:
        """(lo_year, hi_year): inclusive astronomical years, None = open end."""
        return parse_edtf(self.edtf, self.calendar)

    def to_json(self) -> dict[str, Any]:
        return {"kind": self.kind.value, "edtf": self.edtf, "provenance": _prov_json(self.provenance),
                "calendar": self.calendar, "raw": self.raw}

    @staticmethod
    def from_json(d: Mapping[str, Any]) -> HistoricalDate:
        return HistoricalDate(DateKind(d["kind"]), d["edtf"], Provenance(**d["provenance"]),
                              d.get("calendar", "gregorian"), d.get("raw"))


class FichaSection(StrEnum):
    QUE_ES = "que_es"
    POR_QUE_IMPORTA = "por_que_importa"
    COMPRUEBO = "compruebo"


@dataclass(frozen=True)
class NoteInfo:
    """Where a `note` entity (one sentence) sits in its ficha."""

    section: FichaSection
    ordinal: int

    def __post_init__(self) -> None:
        object.__setattr__(self, "section", FichaSection(self.section))
        if not isinstance(self.ordinal, int) or isinstance(self.ordinal, bool) or self.ordinal < 0:
            raise ValidationError("note ordinal must be an integer >= 0")


@dataclass(frozen=True)
class Confidence:
    value: float
    estimates: str  # the event the number estimates, e.g. "P(reviewer accepts relation)"
    calibration: str  # how it was calibrated, e.g. "isotonic on bench/v1 dev split"

    def __post_init__(self) -> None:
        if not 0.0 <= self.value <= 1.0:
            raise ValidationError("confidence value must be in [0, 1]")
        _check_text(self.estimates, "confidence.estimates")
        _check_text(self.calibration, "confidence.calibration")


@dataclass(frozen=True)
class FormalArtifact:
    library: str  # e.g. "mathlib4"
    version: str  # commit or release
    declaration: str  # e.g. "Nat.Prime.two_le"
    axioms: tuple[str, ...] = ()  # e.g. ("propext", "Classical.choice", "Quot.sound")

    def __post_init__(self) -> None:
        for name in ("library", "version", "declaration"):
            _check_text(getattr(self, name), f"formal.{name}")


@dataclass(frozen=True)
class Participant:
    role: Role
    entity_id: str

    def __post_init__(self) -> None:
        object.__setattr__(self, "role", Role(self.role))
        _check_id(self.entity_id, "entity")


@dataclass(frozen=True)
class Entity:
    id: str
    type: EntityType
    labels: tuple[LangString, ...]  # one preferred label per language
    aliases: tuple[LangString, ...] = ()
    descriptions: tuple[LangString, ...] = ()
    statements: tuple[LangString, ...] = ()  # informal statement text (statements, definitions, conjectures)
    formula: str | None = None  # LaTeX source as written in the source
    external_ids: tuple[tuple[str, str], ...] = ()  # (scheme, value): ("wikidata","Q49008"), ("msc","11A41")
    provenance: tuple[Provenance, ...] = ()
    dates: tuple[HistoricalDate, ...] = ()  # v4: each with its own provenance; disagreeing dates are all kept
    note: NoteInfo | None = None  # v4: required iff type == note

    def __post_init__(self) -> None:
        _check_id(self.id, "entity")
        object.__setattr__(self, "type", EntityType(self.type))
        if not self.labels:
            raise ValidationError(f"entity {self.id} needs at least one label")
        if (self.note is None) == (self.type is EntityType.NOTE):
            raise ValidationError(f"entity {self.id}: `note` is required for, and only for, type note")
        if not all(isinstance(d, HistoricalDate) for d in self.dates):
            raise ValidationError(f"entity {self.id}: dates must be HistoricalDate values")
        langs = [l.lang for l in self.labels]
        if len(langs) != len(set(langs)):
            raise ValidationError(f"entity {self.id} has two preferred labels in one language")
        for scheme, value in self.external_ids:
            _check_text(scheme, "external id scheme")
            _check_text(value, "external id value")

    def label(self, lang: str = "es") -> str:
        """Preferred label in `lang`, falling back to en, then to the first label."""
        by_lang = {l.lang: l.text for l in self.labels}
        return by_lang.get(lang) or by_lang.get("en") or self.labels[0].text

    def to_json(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "type": self.type.value,
            "labels": {l.lang: l.text for l in self.labels},
            "aliases": [[a.lang, a.text] for a in self.aliases],
            "descriptions": {d.lang: d.text for d in self.descriptions},
            "statements": {s.lang: s.text for s in self.statements},
            "formula": self.formula,
            "external_ids": [list(x) for x in self.external_ids],
            "provenance": [_prov_json(p) for p in self.provenance],
            "dates": [x.to_json() for x in self.dates],
            "note": None if self.note is None else {"section": self.note.section.value, "ordinal": self.note.ordinal},
        }

    @staticmethod
    def from_json(d: Mapping[str, Any]) -> Entity:
        note = d.get("note")
        return Entity(
            id=d["id"],
            type=EntityType(d["type"]),
            labels=tuple(LangString(t, l) for l, t in d["labels"].items()),
            aliases=tuple(LangString(t, l) for l, t in d.get("aliases", ())),
            descriptions=tuple(LangString(t, l) for l, t in d.get("descriptions", {}).items()),
            statements=tuple(LangString(t, l) for l, t in d.get("statements", {}).items()),
            formula=d.get("formula"),
            external_ids=tuple((s, v) for s, v in d.get("external_ids", ())),
            provenance=tuple(Provenance(**p) for p in d.get("provenance", ())),
            dates=tuple(HistoricalDate.from_json(x) for x in d.get("dates", ())),
            note=None if not note else NoteInfo(FichaSection(note["section"]), note["ordinal"]),
        )


@dataclass(frozen=True)
class Claim:
    """An n-ary, contextual, sourced assertion.  The unit of truth of the atlas.

    Shape rules (enforced here and mirrored in shapes/atlas.shacl.ttl):
    - exactly one SUBJECT;
    - exactly one OBJECT, or (if the relation allows_premises) one or more
      PREMISE participants and no OBJECT: the premises are a conjunction;
    - at least one Provenance; an EvidenceState;
    - FORMALLY_LINKED requires `formal`; SUPERSEDED requires `superseded_by`;
    - an entity appears at most once per role.
    """

    id: str
    relation: RelationType
    participants: tuple[Participant, ...]
    provenance: tuple[Provenance, ...]
    evidence_state: EvidenceState = EvidenceState.EXTRACTED
    assumptions: tuple[str, ...] = ()  # free-text hypotheses not (yet) entities
    validity_domain: str | None = None  # e.g. "finite groups", "char K = 0"
    alternative_group: str | None = None  # claims sharing it are alternative justifications
    supersedes: str | None = None  # previous version of this claim
    superseded_by: str | None = None  # state, filled by the store
    formal: FormalArtifact | None = None  # state, required for FORMALLY_LINKED
    confidence: Confidence | None = None
    historical_date: str | None = None  # when the connection was made historically (EDTF subset, v4)
    explanation: tuple[LangString, ...] = ()  # reader-facing text
    disagreements: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _check_id(self.id, "claim")
        object.__setattr__(self, "relation", RelationType(self.relation))
        object.__setattr__(self, "evidence_state", EvidenceState(self.evidence_state))
        for ref in (self.alternative_group, self.supersedes, self.superseded_by):
            if ref is not None:
                _check_id(ref, "claim reference")
        for problem in claim_shape_errors(self):
            raise ValidationError(f"claim {self.id}: {problem}")

    # -- convenient views
    def ids(self, role: Role) -> tuple[str, ...]:
        return tuple(p.entity_id for p in self.participants if p.role is role)

    @property
    def subject(self) -> str:
        return self.ids(Role.SUBJECT)[0]

    @property
    def targets(self) -> tuple[str, ...]:
        """The OBJECT, or the conjunctive PREMISE set."""
        return self.ids(Role.OBJECT) or self.ids(Role.PREMISE)

    @property
    def is_conjunction(self) -> bool:
        return bool(self.ids(Role.PREMISE))

    def to_json(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "relation": self.relation.value,
            "participants": [{"role": p.role.value, "entity": p.entity_id} for p in self.participants],
            "provenance": [_prov_json(p) for p in self.provenance],
            "evidence_state": self.evidence_state.value,
            "assumptions": list(self.assumptions),
            "validity_domain": self.validity_domain,
            "alternative_group": self.alternative_group,
            "supersedes": self.supersedes,
            "superseded_by": self.superseded_by,
            "formal": None if self.formal is None else {**self.formal.__dict__, "axioms": list(self.formal.axioms)},
            "confidence": None if self.confidence is None else dict(self.confidence.__dict__),
            "historical_date": self.historical_date,
            "explanation": {e.lang: e.text for e in self.explanation},
            "disagreements": list(self.disagreements),
        }

    @staticmethod
    def from_json(d: Mapping[str, Any]) -> Claim:
        formal = d.get("formal")
        conf = d.get("confidence")
        return Claim(
            id=d["id"],
            relation=RelationType(d["relation"]),
            participants=tuple(Participant(Role(p["role"]), p["entity"]) for p in d["participants"]),
            provenance=tuple(Provenance(**p) for p in d["provenance"]),
            evidence_state=EvidenceState(d.get("evidence_state", "extracted")),
            assumptions=tuple(d.get("assumptions", ())),
            validity_domain=d.get("validity_domain"),
            alternative_group=d.get("alternative_group"),
            supersedes=d.get("supersedes"),
            superseded_by=d.get("superseded_by"),
            formal=None if not formal else FormalArtifact(**{**formal, "axioms": tuple(formal.get("axioms", ()))}),
            confidence=None if not conf else Confidence(**conf),
            historical_date=d.get("historical_date"),
            explanation=tuple(LangString(t, l) for l, t in d.get("explanation", {}).items()),
            disagreements=tuple(d.get("disagreements", ())),
        )


def binary(claim_id: str, relation: RelationType | str, subject: str, obj: str,
           provenance: Iterable[Provenance], **kw: Any) -> Claim:
    """Shorthand for the common SUBJECT --relation--> OBJECT claim."""
    parts = (Participant(Role.SUBJECT, subject), Participant(Role.OBJECT, obj)) + tuple(kw.pop("extra", ()))
    return Claim(claim_id, RelationType(relation), parts, tuple(provenance), **kw)


def conjunction(claim_id: str, relation: RelationType | str, subject: str, premises: Iterable[str],
                provenance: Iterable[Provenance], **kw: Any) -> Claim:
    """SUBJECT --relation--> (P1 AND P2 AND ...), e.g. C depends_on_logically A and B."""
    parts = (Participant(Role.SUBJECT, subject),) + tuple(Participant(Role.PREMISE, p) for p in premises)
    return Claim(claim_id, RelationType(relation), parts + tuple(kw.pop("extra", ())), tuple(provenance), **kw)


def claim_shape_errors(c: Claim) -> list[str]:
    """Pure shape rules of a claim (no store access).  Same rules as ClaimShape in SHACL."""
    errors: list[str] = []
    spec = RELATIONS[c.relation]
    counts = {r: 0 for r in Role}
    seen: set[tuple[Role, str]] = set()
    for p in c.participants:
        counts[p.role] += 1
        if (p.role, p.entity_id) in seen:
            errors.append(f"entity {p.entity_id} repeated in role {p.role.value}")
        seen.add((p.role, p.entity_id))
    if counts[Role.SUBJECT] != 1:
        errors.append("needs exactly one subject")
    if counts[Role.PREMISE] and not spec.allows_premises:
        errors.append(f"relation {c.relation.value} does not take conjunctive premises")
    if counts[Role.PREMISE] and counts[Role.OBJECT]:
        errors.append("use either one object or a premise set, not both")
    if not counts[Role.PREMISE] and counts[Role.OBJECT] != 1:
        errors.append("needs exactly one object (or premises)")
    if set(c.ids(Role.SUBJECT)) & set(c.ids(Role.OBJECT) + c.ids(Role.PREMISE)):
        errors.append("subject cannot also be its own object/premise")
    if not c.provenance:
        errors.append("no provenance: every claim needs source, locator, extractor+version, license, retrieved_at")
    if c.evidence_state is EvidenceState.FORMALLY_LINKED and c.formal is None:
        errors.append("formally_linked requires a formal artifact (library, version, declaration, axioms)")
    if c.evidence_state is EvidenceState.SUPERSEDED and c.superseded_by is None:
        errors.append("superseded requires superseded_by")
    if c.supersedes is not None and c.supersedes == c.id:
        errors.append("a claim cannot supersede itself")
    if c.historical_date is not None:
        try:
            parse_edtf(c.historical_date)
        except ValidationError as exc:
            errors.append(f"historical_date: {exc}")
    return errors


def check_state_allowed(relation: RelationType | str, state: EvidenceState | str) -> None:
    """Rules on the state a claim may be written in (v4).  The store checks them on insert and on
    set_evidence_state, not on read: legacy rows (the formally_linked part_of_area of v3) stay
    readable until a named data operation moves them (ADR-0005 §7e)."""
    rel = RelationType(relation)
    if EvidenceState(state) is EvidenceState.FORMALLY_LINKED and not RELATIONS[rel].formal_allowed:
        raise ValidationError(f"relation {rel.value} cannot be formally_linked (formal_allowed is false)")


def _prov_json(p: Provenance) -> dict[str, Any]:
    return dict(p.__dict__)


# --------------------------------------------------------------------------- events & merges


class EventKind(StrEnum):
    SOURCE_ADDED = "source_added"
    ENTITY_ADDED = "entity_added"
    ENTITY_REPLACED = "entity_replaced"
    CLAIM_ADDED = "claim_added"
    EVIDENCE_CHANGED = "evidence_changed"
    CLAIM_SUPERSEDED = "claim_superseded"
    MERGE = "merge"
    MERGE_UNDONE = "merge_undone"
    MIGRATION = "migration"
    CURATION_DECIDED = "curation_decided"  # v4 (ADR-0005)
    CURATION_UNDONE = "curation_undone"
    BATCH_INGESTED = "batch_ingested"
    COVERAGE_MEASURED = "coverage_measured"


@dataclass(frozen=True)
class Event:
    seq: int
    ts: str
    actor: str
    kind: EventKind
    target_id: str
    payload: Mapping[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class Merge:
    id: int
    survivor_id: str
    merged_id: str
    actor: str
    reason: str
    evidence: str | None
    created_at: str
    undone_at: str | None = None


class CurationKind(StrEnum):
    EXCLUDE_PRESENTATION = "exclude_presentation"
    PRESENTATION_LABEL = "presentation_label"
    RETIRE_CLAIMS = "retire_claims"
    PROPOSE_MERGE = "propose_merge"


# Scopes of exclude_presentation; the position is the bit in the snapshot's NodeFlags section.
PRESENTATION_SCOPES = ("map:general", "map:all", "search:default", "concept_layer")


@dataclass(frozen=True)
class Curation:
    """One reversible curation decision (ADR-0005 §5).  Rows are never deleted: undoing sets undone_at."""

    id: int
    kind: CurationKind
    target_id: str
    rule: str
    rule_version: str
    payload: Mapping[str, Any]
    actor: str
    reason: str
    created_at: str
    undone_at: str | None = None
    undone_by: str | None = None
    undo_reason: str | None = None


@dataclass(frozen=True)
class CoverageCell:
    """One coverage measurement (ADR-0005 §9).  `indexed` None = unknown, shown as "desconocido"."""

    source_id: str
    source_version: str
    period: str  # EDTF interval, e.g. "1900/1949"
    area: str  # area id or "*"
    indexed: int | None
    indexed_method: str
    ingested: int
    cutoff: str  # "all", "top25-cited", ...
    measured_at: str

    def __post_init__(self) -> None:
        _check_id(self.source_id, "source")
        for name in ("source_version", "area", "indexed_method", "cutoff"):
            _check_text(getattr(self, name), name)
        parse_edtf(self.period)
        _check_iso(self.measured_at, "measured_at")
        for name in ("indexed", "ingested"):
            v = getattr(self, name)
            if v is not None and (not isinstance(v, int) or v < 0):
                raise ValidationError(f"{name} must be a count >= 0")
        if self.ingested is None:
            raise ValidationError("ingested must be a count")


@dataclass(frozen=True)
class SearchHit:
    entity_id: str  # already resolved through merges
    field: str  # label | alias | description | statement | explanation
    lang: str
    text: str
    score: float  # bm25, lower is better (SQLite convention), negated so higher = better


@dataclass(frozen=True)
class Edge:
    """Binary projection of a claim for graph algorithms (layout, paths).

    A conjunctive claim yields one Edge per premise, all with the same claim_id
    and `conjunct_count` > 1: consumers must not treat them as independent.
    """

    claim_id: str
    relation: RelationType
    source: str  # subject (resolved)
    target: str  # object or one premise (resolved)
    evidence_state: EvidenceState
    conjunct_count: int
    alternative_group: str | None


__all__ = [n for n in dir() if not n.startswith("_")] + ["replace"]
