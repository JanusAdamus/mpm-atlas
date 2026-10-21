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
    said (v4):    c:x-se1 said_equivalent_to c:x-se2 said_equivalent_to c:x-se3 (never composes)
    instance (v4): c:x-i0 instance_of c:x-i1 instance_of c:x-i2 special_case_of c:x-i3 special_case_of c:x-i4
    history:      s:x-nm1, s:x-nm2, s:x-nm3 named_after per:x-namer; s:x-nm2 depends_on s:x-nm-axiom;
                  s:x-nm3 depends_on s:x-nm-mid depends_on s:x-nm2
    concepts:     member_of_concept groups declarations under c:x-ca, c:x-cb, c:x-cc, c:x-cd;
                  s:x-a1 depends_on s:x-b1 and s:x-b2 depends_on s:x-c1 (A -> B -> C at concept
                  level, but no declaration chain: b1 != b2); s:x-dc1 depends_on (s:x-b1 AND s:x-c1)
                  (D -> B, D -> C); s:x-a1 in A is extracted (curated), every other membership
                  model_proposed
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
    Confidence,
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
    ("per:x-namer", "person", "Epónimo", "Namesake"),
    ("s:x-nm1", "statement", "Teorema epónimo 1", "Eponymous theorem 1"),
    ("s:x-nm2", "statement", "Teorema epónimo 2", "Eponymous theorem 2"),
    ("s:x-nm3", "statement", "Teorema epónimo 3", "Eponymous theorem 3"),
    ("s:x-nm-mid", "statement", "Lema intermedio", "Intermediate lemma"),
    ("s:x-nm-axiom", "statement", "Axioma del epónimo", "Namesake axiom"),
    ("c:x-ca", "concept", "Concepto A", "Concept A"),
    ("c:x-cb", "concept", "Concepto B", "Concept B"),
    ("c:x-cc", "concept", "Concepto C", "Concept C"),
    ("c:x-cd", "concept", "Concepto D", "Concept D"),
    ("s:x-a1", "statement", "A1", "A1"),
    ("s:x-a2", "statement", "A2", "A2"),
    ("s:x-b1", "statement", "B1", "B1"),
    ("s:x-b2", "statement", "B2", "B2"),
    ("s:x-c1", "statement", "C1", "C1"),
    ("s:x-dc1", "statement", "DC1", "DC1"),
    ("c:x-se1", "concept", "Grafo (dicho igual)", "Graph (said same)"),
    ("c:x-se2", "concept", "Grafo finito", "Finite graph"),
    ("c:x-se3", "concept", "Grafo dirigido (dicho igual)", "Directed graph (said same)"),
    ("c:x-i0", "concept", "Ejemplo de un ejemplo", "Example of an example"),
    ("c:x-i1", "concept", "Numero aureo", "Golden ratio"),
    ("c:x-i2", "concept", "Numero construible", "Constructible number"),
    ("c:x-i3", "concept", "Numero algebraico", "Algebraic number"),
    ("c:x-i4", "concept", "Numero complejo", "Complex number"),
]

# (claim, declaration, concept): the concept cluster's memberships
MEMBERS = [
    ("c:x-m-a1", "s:x-a1", "c:x-ca"),
    ("c:x-m-a2", "s:x-a2", "c:x-ca"),
    ("c:x-m-b1", "s:x-b1", "c:x-cb"),
    ("c:x-m-b2", "s:x-b2", "c:x-cb"),
    ("c:x-m-c1", "s:x-c1", "c:x-cc"),
    ("c:x-m-dc1", "s:x-dc1", "c:x-cd"),
]
MEMBER_CONFIDENCE = Confidence(0.6, "heuristic membership score (fixture)", "not calibrated")

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
    ("c:x-nm1-named", "named_after", "s:x-nm1", "per:x-namer"),
    ("c:x-nm2-named", "named_after", "s:x-nm2", "per:x-namer"),
    ("c:x-nm3-named", "named_after", "s:x-nm3", "per:x-namer"),
    ("c:x-nm2-axiom", "depends_on_logically", "s:x-nm2", "s:x-nm-axiom"),
    ("c:x-nm3-mid", "depends_on_logically", "s:x-nm3", "s:x-nm-mid"),
    ("c:x-nm-mid-2", "depends_on_logically", "s:x-nm-mid", "s:x-nm2"),
    ("c:x-a1-b1", "depends_on_logically", "s:x-a1", "s:x-b1"),
    ("c:x-b2-c1", "depends_on_logically", "s:x-b2", "s:x-c1"),
    ("c:x-se12", "said_equivalent_to", "c:x-se1", "c:x-se2"),
    ("c:x-se23", "said_equivalent_to", "c:x-se2", "c:x-se3"),
    ("c:x-i01", "instance_of", "c:x-i0", "c:x-i1"),
    ("c:x-i12", "instance_of", "c:x-i1", "c:x-i2"),
    ("c:x-i23", "special_case_of", "c:x-i2", "c:x-i3"),
    ("c:x-i34", "special_case_of", "c:x-i3", "c:x-i4"),
]


def build_fixture(out: Path) -> None:
    with Store(out) as s:
        load_bundle(s, json.loads((ROOT / "tests" / "fixtures" / "mini_atlas.json").read_text(encoding="utf-8")))
        s.add_entities([Entity(i, t, (LangString(es, "es"), LangString(en, "en"))) for i, t, es, en in ENTITIES])
        claims = [binary(cid, rel, a, b, PROV) for cid, rel, a, b in BINARY]
        claims.append(conjunction("c:x-goal", "depends_on_logically", "s:x-goal", ["s:x-q", "s:x-r"], PROV))
        claims.append(conjunction("c:x-dc1-dep", "depends_on_logically", "s:x-dc1", ["s:x-b1", "s:x-c1"], PROV))
        claims += [binary(cid, "member_of_concept", d, c, PROV, evidence_state="extracted" if d == "s:x-a1"
                          else "model_proposed", confidence=MEMBER_CONFIDENCE) for cid, d, c in MEMBERS]
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
