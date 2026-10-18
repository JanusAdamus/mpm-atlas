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

SCHEMA_VERSION = 1


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
DOCS = frozenset({EntityType.PUBLICATION, EntityType.SOURCE_FRAGMENT, EntityType.RESOURCE})
NON_PERSON = frozenset(EntityType) - {EntityType.PERSON}


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


def _spec(name: RelationType, proj: Projection, *, sym: bool = False, trans: bool = False,
          inv: RelationType | None = None, premises: bool = False,
          subj: frozenset[EntityType] | None = None, obj: frozenset[EntityType] | None = None,
          chain: str) -> RelationSpec:
    return RelationSpec(name, proj, sym, trans, inv, premises, subj, obj, chain)


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
              chain="bibliographic reach only; implies neither dependency nor agreement"),
        _spec(R.INFLUENCED, Projection.HISTORY, trans=False, subj=None, obj=None,
              chain="historical influence; each hop needs its own historical source"),
        _spec(R.PART_OF_AREA, Projection.STRUCTURE, trans=True, subj=NON_PERSON, obj=frozenset({E.AREA}),
              chain="area membership (area hierarchy is transitive)"),
        _spec(R.FORMALIZED_AS, Projection.PROOF, subj=MATH, obj=MATH | frozenset({E.RESOURCE}),
              chain="translation to a formal artifact; fidelity is a separate assessment from compilation"),
        _spec(R.NAMED_AFTER, Projection.HISTORY, subj=NON_PERSON, obj=frozenset({E.PERSON}),
              chain="naming only; implies no authorship or priority"),
        _spec(R.COUNTEREXAMPLE_TO, Projection.PROOF,
              subj=frozenset({E.COUNTEREXAMPLE, E.MODEL, E.CONCEPT, E.STATEMENT, E.DEFINITION}),
              obj=frozenset({E.STATEMENT, E.CONJECTURE}),
              chain="single hop: refutes the target under the stated assumptions"),
        _spec(R.APPEARS_IN, Projection.BIBLIO, subj=NON_PERSON, obj=DOCS,
              chain="location in literature; not a citation of correctness"),
        _spec(R.AUTHORED_BY, Projection.HISTORY, subj=DOCS | MATH, obj=frozenset({E.PERSON}),
              chain="authorship attribution"),
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


# --------------------------------------------------------------------------- value objects


@dataclass(frozen=True)
class LangString:
    text: str
    lang: str

    def __post_init__(self) -> None:
        _check_text(self.text, "text")
        if not LANG_RE.match(self.lang):
            raise ValidationError(f"invalid language tag {self.lang!r}")


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

    def __post_init__(self) -> None:
        _check_id(self.id, "source")
        _check_text(self.title, "source title")
        _check_text(self.license, "source license")
        if self.license.strip().lower() == "unknown":
            raise ValidationError("source license must be declared, not 'unknown'")
        if self.retrieved_at is not None:
            _check_iso(self.retrieved_at, "retrieved_at")


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

    def __post_init__(self) -> None:
        _check_id(self.id, "entity")
        object.__setattr__(self, "type", EntityType(self.type))
        if not self.labels:
            raise ValidationError(f"entity {self.id} needs at least one label")
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
        }

    @staticmethod
    def from_json(d: Mapping[str, Any]) -> Entity:
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
    historical_date: str | None = None  # when the connection was made historically (free ISO-ish)
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
    return errors


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
