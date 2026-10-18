"""Independent oracle of what a chain of typed hops authorizes (SPEC §3 rules R1-R8).

It reads the same table the path engine loads (shapes/composition.json) plus
atlas.model.RELATIONS.  That shared table is a named shared assumption: agreement
with this oracle shows conformance to the SPEC, not mathematical validity
(PROTOCOL.md §6).
"""

from __future__ import annotations

import json
import unicodedata
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Sequence

from atlas.model import RELATIONS, RelationType

ROOT = Path(__file__).resolve().parents[1]
TABLE = ROOT / "shapes" / "composition.json"


@dataclass(frozen=True)
class Hop:
    relation: str
    direction: str = "forward"


@lru_cache(maxsize=4)
def table(path: str = str(TABLE)) -> dict:
    t = json.loads(Path(path).read_text(encoding="utf-8"))
    known = {r.value for r in RelationType}
    unknown = set(t["relations"]) - known
    if unknown:
        raise ValueError(f"composition table names unknown relations {sorted(unknown)}")
    return t


def strength(conclusion: str) -> int:
    return table()["conclusions"].get(conclusion, {"strength": -1})["strength"]


def is_mathematical(conclusion: str) -> bool:
    return bool(table()["conclusions"].get(conclusion, {}).get("mathematical"))


def read_as(hop: Hop) -> str | None:
    """The relation as read in the walking direction (inverse via the vocabulary or symmetry), or None."""
    spec = RELATIONS[RelationType(hop.relation)]
    if hop.direction == "forward" or spec.symmetric:
        return hop.relation
    return spec.inverse.value if spec.inverse is not None else None


def step(hop: Hop) -> tuple[str, str | None]:
    """(base conclusion, modality) of one hop: the generator of the monoid in composition.json."""
    rel = read_as(hop)
    if rel is not None:
        rule = table()["relations"][rel]
        st = rule.get("step") or {}
        return st.get("kind", rule["single_hop"]), st.get("modality")
    converse = table()["relations"][hop.relation].get("converse")  # walked against its direction (v2)
    return (converse["kind"], None) if converse else ("none", None)


def is_conjunctive_converse(hop: Hop) -> bool:
    """R7: walking this hop backwards only counts if every premise of the claim is satisfied."""
    if read_as(hop) is not None:
        return False
    return (table()["relations"][hop.relation].get("converse") or {}).get("conjunction") == "require_all"


@lru_cache(maxsize=1)
def _products() -> dict[tuple[str, str], str]:
    out: dict[tuple[str, str], str] = {}
    for e in table()["compose"]:
        for a in e["left"]:
            for b in e["right"]:
                out[a, b] = b if e["result"] == "=right" else a if e["result"] == "=left" else e["result"]
    return out


def conclude(hops: Sequence[Hop]) -> str:
    """What a chain of typed hops authorizes: the product of its steps in the monoid of
    shapes/composition.json (unlisted products are none, none absorbs), then the modality:
    more than `max_per_path` hypothetical steps conclude nothing (R10), one renames the result (R3)."""
    if not hops:
        return "none"
    steps = [step(h) for h in hops]
    prod = _products()
    base = steps[0][0]
    for kind, _ in steps[1:]:
        if base == "none":
            break
        base = prod.get((base, kind), "none")
    hypo = sum(m == "hypothetical" for _, m in steps)
    if base == "none" or not hypo:
        return base
    mod = table()["modalities"]["hypothetical"]
    if hypo > mod["max_per_path"]:
        return "none"
    return mod["names"].get(base, mod["names"]["*"])


def parse_path(word: Sequence[str]) -> list[Hop]:
    """composition.json 'expect' notation: 'rel' or 'rel^-1'."""
    return [Hop(w[:-3], "inverse") if w.endswith("^-1") else Hop(w) for w in word]


def _fold(s: str) -> str:
    return "".join(c for c in unicodedata.normalize("NFKD", s.casefold()) if not unicodedata.combining(c)).strip()


@lru_cache(maxsize=1)
def _text_index() -> dict[str, str]:
    idx: dict[str, str] = {}
    for key, c in table()["conclusions"].items():
        for txt in (key, c.get("es", ""), c.get("en", "")):
            if txt:
                idx[_fold(txt)] = key
    for rel, spec in RELATIONS.items():
        rule = table()["relations"].get(rel.value)
        if rule:
            idx.setdefault(_fold(spec.chain_authorizes), rule["chain_result"])
    return idx


def normalize_conclusion(authorizes: str | None, is_hypothesis: bool = False) -> str:
    """Map an engine's `conclusion.authorizes` text to a conclusion key ('unparsed' if unknown)."""
    if is_hypothesis:
        return "hypothesis"
    if not authorizes:
        return "none"
    f = _fold(authorizes)
    idx = _text_index()
    if f in idx:
        return idx[f]
    for txt, key in sorted(idx.items(), key=lambda kv: -len(kv[0])):
        if len(txt) >= 6 and txt in f:
            return key
    if "study" in f or "estudio" in f:
        return "study_route"
    return "unparsed"
