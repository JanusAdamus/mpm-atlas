"""Build tests/fixtures/mini_atlas.db for the Rust tests, through the Python Store (the contract).

    python core/atlas-core/tests/fixtures/make_fixture.py

Content: tests/fixtures/mini_atlas.json (CC0, hand-written) plus, to exercise the
snapshot compiler, a duplicate entity merged into s:pythagoras, an entity with
no claims (coverage "unknown"), and one superseded claim.  Nothing here is
editorially reviewed.
"""

from __future__ import annotations

import json
import sqlite3
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(ROOT))

from atlas import Entity, LangString, Provenance, Store, binary, load_bundle  # noqa: E402

OUT = HERE / "mini_atlas.db"
ACTOR = "fixture-script"
PROV = [Provenance("src:fixture-hand", "fixture", "hand-written-fixture", "1", "CC0-1.0", "2026-10-16T00:00:00+00:00")]


def main() -> None:
    for suffix in ("", "-wal", "-shm"):
        Path(str(OUT) + suffix).unlink(missing_ok=True)
    with Store(OUT) as s:
        load_bundle(s, json.loads((ROOT / "tests" / "fixtures" / "mini_atlas.json").read_text(encoding="utf-8")))
        s.add_entity(Entity("s:pythagoras-dup", "statement", (LangString("Pythagoras theorem (duplicate)", "en"),)))
        s.add_entity(Entity("c:lonely", "concept", (LangString("Concepto sin afirmaciones", "es"),)))
        s.add_claim(binary("c:dup-prereq", "prerequisite_for", "c:right-triangle", "s:pythagoras-dup", PROV))
        s.merge("s:pythagoras", "s:pythagoras-dup", actor=ACTOR, reason="test fixture: duplicate")
        new = binary("c:cosines-gen-v2", "generalizes", "s:law-of-cosines", "s:pythagoras", PROV,
                     assumptions=("Euclidean plane",), validity_domain="real inner product spaces")
        s.supersede("c:cosines-gen", new, actor=ACTOR, reason="test fixture: add assumptions")
    con = sqlite3.connect(OUT)
    con.execute("PRAGMA journal_mode=DELETE")  # single self-contained file
    con.close()


if __name__ == "__main__":
    main()
