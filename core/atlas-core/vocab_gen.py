"""Write shapes/vocab.json from atlas.model (the Python contract).

    python core/atlas-core/vocab_gen.py          # regenerate
    python core/atlas-core/vocab_gen.py --check  # exit 1 if the file is stale

Rust (atlas-core) checks its enums against both this file and atlas/model.py;
the web client and other non-Python readers use the file.  Enum order is the
order of the Python declaration and is the u8 code used in the snapshot.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from atlas.model import (  # noqa: E402
    CURRENT_STATES,
    RELATIONS,
    SCHEMA_VERSION,
    TRANSITIONS,
    EntityType,
    EvidenceState,
    Projection,
    RelationType,
    Role,
)

OUT = ROOT / "shapes" / "vocab.json"


def _types(ts: frozenset[EntityType] | None) -> list[str] | None:
    return None if ts is None else [t.value for t in EntityType if t in ts]


def build() -> dict:
    return {
        "generated_from": "atlas/model.py (core/atlas-core/vocab_gen.py)",
        "schema_version": SCHEMA_VERSION,
        "entity_types": [t.value for t in EntityType],
        "relation_types": [r.value for r in RelationType],
        "evidence_states": [s.value for s in EvidenceState],
        "current_states": [s.value for s in EvidenceState if s in CURRENT_STATES],
        "roles": [r.value for r in Role],
        "projections": [p.value for p in Projection],
        "relations": {
            r.value: {
                "projection": RELATIONS[r].projection.value,
                "symmetric": RELATIONS[r].symmetric,
                "transitive": RELATIONS[r].transitive,
                "inverse": None if RELATIONS[r].inverse is None else RELATIONS[r].inverse.value,
                "allows_premises": RELATIONS[r].allows_premises,
                "subject_types": _types(RELATIONS[r].subject_types),
                "object_types": _types(RELATIONS[r].object_types),
                "chain_authorizes": RELATIONS[r].chain_authorizes,
            }
            for r in RelationType
        },
        "transitions": {s.value: [t.value for t in EvidenceState if t in TRANSITIONS[s]] for s in EvidenceState},
    }


def render() -> str:
    return json.dumps(build(), ensure_ascii=False, indent=2) + "\n"


def main(argv: list[str]) -> int:
    text = render()
    if "--check" in argv:
        current = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        if current != text:
            print(f"{OUT} is stale; run python core/atlas-core/vocab_gen.py", file=sys.stderr)
            return 1
        return 0
    OUT.write_text(text, encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
