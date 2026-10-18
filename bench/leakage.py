"""Lexical leakage check for retrieval qrels (PROTOCOL.md §7).

A qrel is *trivial* when every query token (the last one as a prefix, like
Store.search) occurs in one indexed field of the relevant entity: the lexical
index then finds it by construction, so the item measures storage, not search.
Tokenization mirrors FTS5 unicode61 with remove_diacritics: alphanumeric runs,
accents folded, case folded.
"""

from __future__ import annotations

import re
import unicodedata
from collections import Counter, defaultdict
from typing import Iterable, Mapping, Sequence

from atlas.model import Entity

from .items import Item
from .metrics import proportion
from .registry import Registry

TOKEN = re.compile(r"[^\W_]+")


def tokens(text: str) -> list[str]:
    folded = "".join(c for c in unicodedata.normalize("NFKD", text.casefold()) if not unicodedata.combining(c))
    return TOKEN.findall(folded)


def indexed_fields(e: Entity) -> list[tuple[str, str]]:
    """(field, text) rows the lexical index holds for an entity (SPEC §10)."""
    return ([("label", x.text) for x in e.labels] + [("alias", x.text) for x in e.aliases]
            + [("description", x.text) for x in e.descriptions] + [("statement", x.text) for x in e.statements]
            + [("external_id", v) for _, v in e.external_ids])


def field_match(query: str, text: str) -> bool:
    q, f = tokens(query), tokens(text)
    if not q or not f:
        return False
    fs = set(f)
    return all(t in fs for t in q[:-1]) and any(x.startswith(q[-1]) for x in f)


def jaccard(query: str, text: str) -> float:
    a, b = set(tokens(query)), set(tokens(text))
    return len(a & b) / len(a | b) if a | b else 0.0


def token_coverage(query: str, texts: Iterable[str]) -> float:
    """Share of query tokens present anywhere in the entity's indexed text (graded, unlike `trivial`)."""
    q = tokens(query)
    if not q:
        return 0.0
    vocab = {t for txt in texts for t in tokens(txt)}
    hit = sum(t in vocab for t in q[:-1]) + any(v.startswith(q[-1]) for v in vocab)
    return hit / len(q)


def check_qrel(query: str, entity: Entity) -> dict:
    rows = indexed_fields(entity)
    hit = next((fld for fld, txt in rows if field_match(query, txt)), None)
    return {"trivial": hit is not None, "field": hit,
            "max_jaccard": max((jaccard(query, txt) for _, txt in rows), default=0.0),
            "token_coverage": token_coverage(query, (txt for _, txt in rows))}


def annotate(items: Sequence[Item], resolve: Registry) -> dict[str, list[dict]]:
    """Per retrieval item id, the leakage verdict of each qrel with grade >= 2 that resolves."""
    out: dict[str, list[dict]] = {}
    for it in items:
        if it.task != "retrieval":
            continue
        rows = []
        for q in it.payload["qrels"]:
            if q["grade"] < 2:
                continue
            for eid in resolve.ids(q["ref"]):
                ent = resolve.entity(eid)
                if ent is not None:
                    rows.append({"entity": eid, **check_qrel(it.payload["query"], ent)})
        out[it.id] = rows
    return out


def report(items: Sequence[Item], verdicts: Mapping[str, list[dict]]) -> dict:
    """Trivial share by generator/origin, with Wilson intervals; items with any trivial qrel count as trivial."""
    groups: dict[str, list[bool]] = defaultdict(list)
    jac: dict[str, list[float]] = defaultdict(list)
    cov: dict[str, list[float]] = defaultdict(list)
    fields: dict[str, Counter] = defaultdict(Counter)
    for it in items:
        rows = verdicts.get(it.id) or []
        if not rows:
            continue
        key = f"{it.origin}:{it.strata.get('generator', it.origin)}:{it.payload.get('modality')}"
        groups[key].append(any(r["trivial"] for r in rows))
        jac[key].append(max(r["max_jaccard"] for r in rows))
        cov[key].append(max(r["token_coverage"] for r in rows))
        fields[key].update(r["field"] for r in rows if r["trivial"])
    out = {}
    for key, flags in sorted(groups.items()):
        share = proportion(sum(flags), len(flags))
        out[key] = {"trivial": share, "mean_max_jaccard": sum(jac[key]) / len(jac[key]),
                    "mean_token_coverage": sum(cov[key]) / len(cov[key]),
                    "full_token_coverage": proportion(sum(c == 1.0 for c in cov[key]), len(cov[key])),
                    "trivial_field": dict(fields[key].most_common()), "regression_only": share["value"] > 0.5}
    return out


def trivial_ids(verdicts: Mapping[str, Iterable[dict]]) -> set[str]:
    return {iid for iid, rows in verdicts.items() if any(r["trivial"] for r in rows)}
