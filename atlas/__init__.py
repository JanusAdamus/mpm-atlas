"""MPM atlas of mathematical connections.  Contract: docs/SPEC.md."""

from .model import (
    CURRENT_STATES,
    EVIDENCE_RANK,
    PRESENTATION_SCOPES,
    RELATIONS,
    SCHEMA_VERSION,
    Claim,
    Confidence,
    CoverageCell,
    Curation,
    CurationKind,
    DateKind,
    Edge,
    Entity,
    EntityType,
    Event,
    EventKind,
    EvidenceState,
    FichaSection,
    FormalArtifact,
    HistoricalDate,
    LangString,
    Merge,
    NoteInfo,
    Participant,
    Projection,
    Provenance,
    RelationSpec,
    RelationType,
    Role,
    SearchHit,
    Source,
    ValidationError,
    binary,
    conjunction,
    parse_edtf,
    stable_id,
)
from .store import Store, load_bundle, validate_jsonld

__version__ = "0.1.0"

# Child processes (cargo, atlas.exe) open no console window on Windows.
NO_WINDOW: dict = {"creationflags": __import__("subprocess").CREATE_NO_WINDOW} if __import__("sys").platform == "win32" else {}
