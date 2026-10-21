"""Exact-identifier merges of the works layer (ADR-0005 §8.1): `merge_many`, actor resolver:works/1,
evidence = the key.  Survivor priority: Wikidata > zbMATH > OpenAlex.  No strong id, no merge."""

from __future__ import annotations

import time
from typing import Any

from ...store import Store
from . import canon

ACTOR = "resolver:works/1"
RANK = {"wd:": 0, "wdw:": 0, "zb:": 1, "oa:": 2}


def _rank(eid: str) -> int:
    return next((r for p, r in RANK.items() if eid.startswith(p)), 9)


def doi_merges(store: Store) -> list[tuple[str, str, str]]:
    rows = store._conn.execute(  # noqa: SLF001
        "SELECT x.value, x.entity_id FROM external_ids x JOIN entities e ON e.id = x.entity_id "
        "WHERE x.scheme='doi' AND e.type='publication' AND e.merged_into IS NULL ORDER BY x.value, x.entity_id")
    by_doi: dict[str, list[str]] = {}
    for doi, eid in rows:
        by_doi.setdefault(doi, []).append(eid)
    out, merged = [], set()
    for doi, ids in by_doi.items():
        if len(ids) < 2:
            continue
        ids = sorted(ids, key=lambda e: (_rank(e), e))
        survivor = ids[0]
        for m in ids[1:]:
            if _rank(m) > _rank(survivor) and m not in merged:  # two records of one source are not merged
                merged.add(m)
                out.append((survivor, m, f"doi:{doi}"))
    return out


def run(store: Store) -> dict[str, Any]:
    t = time.perf_counter()
    out: dict[str, Any] = {}
    cur = canon.load_curated()
    steps = (("edition_of", lambda: canon.edition_merges(store, cur)), ("cdli", lambda: canon.cdli_merges(store)),
             ("doi", lambda: doi_merges(store)), ("person_ids", lambda: canon.person_merges(store)))
    for name, compute in steps:  # each list is read after the previous merges, never against a stale registry
        items = compute()
        survivors = {s for s, _, _ in items}
        items = [x for x in items if x[1] not in survivors]  # a survivor of this call cannot be merged in it
        if items:
            store.merge_many(items, actor=ACTOR, reason=f"exact identifier match ({name})")
        out[name] = len(items)
    out["seconds"] = round(time.perf_counter() - t, 2)
    return out
