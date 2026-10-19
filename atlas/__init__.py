"""MPM atlas of mathematical connections.  Contract: docs/SPEC.md."""

from .model import (
    CURRENT_STATES,
    RELATIONS,
    SCHEMA_VERSION,
    Claim,
    Confidence,
    Edge,
    Entity,
    EntityType,
    Event,
    EventKind,
    EvidenceState,
    FormalArtifact,
    LangString,
    Merge,
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
    stable_id,
)
from .store import Store, load_bundle, validate_jsonld

__version__ = "0.1.0"

# Child processes (cargo, atlas.exe) open no console window on Windows.
NO_WINDOW: dict = {"creationflags": __import__("subprocess").CREATE_NO_WINDOW} if __import__("sys").platform == "win32" else {}
