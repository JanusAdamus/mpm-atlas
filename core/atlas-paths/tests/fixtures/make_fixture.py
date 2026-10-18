"""Build the registries the atlas-paths tests compile, through the Python Store (the contract).

    python core/atlas-paths/tests/fixtures/make_fixture.py

- paths_fixture.db: tests/fixtures/mini_atlas.json (CC0, hand-written) plus adversarial
  clusters, each isolated so that the only routes are the ones a test is about:
    analogy:      s:x-analog ~ s:pythagoras -> app:surveying, s:x-analog2 ~ s:x-analog
    citations:    s:x-cited appears_in pub:x-survey-a cites pub:x-survey-b cites pub:elements
    prerequisite: d:x-angle prerequisite_for s:x-lemma depends_on_logically s:x-axiom
    special case: s:x-special special_case_of s:pythagoras (applied_in app:surveying)
    conjunction:  s:x-goal depends_on (s:x-q AND s:x-r); s:x-q, s:x-r depend_on s:x-p
    disputed:     s:x-d1 generalizes s:x-d2, disputed
    formal:       s:x-f1 generalizes s:x-f3 (extracted) and s:x-f1 -> s:x-f2 -> s:x-f3 (formally_linked)
    coverage:     c:x-lonely has no claims (unknown, not isolated)
    merge:        s:x-dup merged into s:pythagoras
  Nothing here is editorially reviewed: the states are the ones a test needs.
- empty_v1.db: an empty schema-v1 registry; tests fill it with atlas_core::synth.
"""

from __future__ import annotations

import json
import sqlite3
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(ROOT))

from atlas import (  # noqa: E402
    Entity,
    FormalArtifact,
    LangString,
    Provenance,
    Store,
    binary,
    conjunction,
    load_bundle,
)

ACTOR = "fixture-script"
PROV = [Provenance("src:fixture-hand", "adversarial fixture", "hand-written-fixture", "1", "CC0-1.0",
                   "2026-10-16T00:00:00+00:00")]

ENTITIES = [
    ("s:x-analog", "statement", "Teorema analogo", "Analogous theorem"),
    ("s:x-analog2", "statement", "Teorema analogo al analogo", "Analogue of the analogue"),
    ("pub:x-survey-a", "publication", "Panorama A", "Survey A"),
    ("pub:x-survey-b", "publication", "Panorama B", "Survey B"),
    ("s:x-cited", "statement", "Enunciado citado", "Cited statement"),
    ("d:x-angle", "definition", "Angulo", "Angle"),
    ("s:x-lemma", "statement", "Lema", "Lemma"),
    ("s:x-axiom", "statement", "Axioma", "Axiom"),
    ("s:x-special", "statement", "Caso particular", "Special case"),
    ("s:x-p", "statement", "P", "P"),
    ("s:x-q", "statement", "Q", "Q"),
    ("s:x-r", "statement", "R", "R"),
    ("s:x-goal", "statement", "Meta", "Goal"),
    ("s:x-d1", "statement", "D1", "D1"),
    ("s:x-d2", "statement", "D2", "D2"),
    ("s:x-f1", "statement", "F1", "F1"),
    ("s:x-f2", "statement", "F2", "F2"),
    ("s:x-f3", "statement", "F3", "F3"),
    ("c:x-lonely", "concept", "Concepto sin afirmaciones", "Concept without claims"),
    ("s:x-dup", "statement", "Pitagoras (duplicado)", "Pythagoras (duplicate)"),
]

BINARY = [
    ("c:x-analog-pyth", "analogous_to", "s:x-analog", "s:pythagoras"),
    ("c:x-analog2", "analogous_to", "s:x-analog2", "s:x-analog"),
    ("c:x-cited-in", "appears_in", "s:x-cited", "pub:x-survey-a"),
    ("c:x-a-cites-b", "cites", "pub:x-survey-a", "pub:x-survey-b"),
    ("c:x-b-cites-elements", "cites", "pub:x-survey-b", "pub:elements"),
    ("c:x-angle-prereq", "prerequisite_for", "d:x-angle", "s:x-lemma"),
    ("c:x-lemma-axiom", "depends_on_logically", "s:x-lemma", "s:x-axiom"),
    ("c:x-special", "special_case_of", "s:x-special", "s:pythagoras"),
    ("c:x-q-dep", "depends_on_logically", "s:x-q", "s:x-p"),
    ("c:x-r-dep", "depends_on_logically", "s:x-r", "s:x-p"),
    ("c:x-d", "generalizes", "s:x-d1", "s:x-d2"),
    ("c:x-f13", "generalizes", "s:x-f1", "s:x-f3"),
    ("c:x-f12", "generalizes", "s:x-f1", "s:x-f2"),
    ("c:x-f23", "generalizes", "s:x-f2", "s:x-f3"),
    ("c:x-dup-prereq", "prerequisite_for", "c:right-triangle", "s:x-dup"),
]


def build_fixture(out: Path) -> None:
    with Store(out) as s:
        load_bundle(s, json.loads((ROOT / "tests" / "fixtures" / "mini_atlas.json").read_text(encoding="utf-8")))
        s.add_entities([Entity(i, t, (LangString(es, "es"), LangString(en, "en"))) for i, t, es, en in ENTITIES])
        claims = [binary(cid, rel, a, b, PROV) for cid, rel, a, b in BINARY]
        claims.append(conjunction("c:x-goal", "depends_on_logically", "s:x-goal", ["s:x-q", "s:x-r"], PROV))
        s.add_claims(claims)
        s.set_evidence_state("c:x-d", "disputed", actor=ACTOR, reason="fixture: disputed generalization")
        lean = FormalArtifact("mathlib", "fixture", "X.F", ("propext",))
        for cid in ("c:x-f12", "c:x-f23"):
            s.set_evidence_state(cid, "formally_linked", actor=ACTOR, reason="fixture: formal link", formal=lean)
        s.merge("s:pythagoras", "s:x-dup", actor=ACTOR, reason="fixture: duplicate")


def build_empty(out: Path) -> None:
    with Store(out):
        pass


def main() -> None:
    for name, fn in (("paths_fixture.db", build_fixture), ("empty_v1.db", build_empty)):
        out = HERE / name
        for suffix in ("", "-wal", "-shm"):
            Path(str(out) + suffix).unlink(missing_ok=True)
        fn(out)
        con = sqlite3.connect(out)
        con.execute("PRAGMA journal_mode=DELETE")  # one self-contained file
        con.execute("VACUUM")
        con.close()


if __name__ == "__main__":
    main()
