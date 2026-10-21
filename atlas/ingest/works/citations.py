"""Citations from OpenCitations (CC0, REST v2) and Crossref (DOI references), for a seeded sample of
the corpus' works with a DOI.  ADR-0005 §8.1: these sources only add pairs between canonical works
that the registry does not have yet; the pairs already stated by the primary record are counted.

The full ingest reads the OpenCitations dump instead (36 GB compressed); the pilot measures on a
sample what share of internal pairs each source adds on top of zbMATH and OpenAlex.
"""

from __future__ import annotations

import random
import re
import time
from typing import Any

from ...model import RelationType
from ...store import Store
from ..http import CachedClient, HttpError
from .common import (CONTACT, USER_AGENT, WORKS_CACHE, by_external, cache_path, claim, norm_doi, prov, read_cache,
                     register_sources, run_batch, write_cache)

OC_API = "https://api.opencitations.net/index/v2/references/doi:"
CR_API = "https://api.crossref.org/works/"
EXTRACTOR, VERSION = "atlas.ingest.works.citations", "works-cit/1"
_DOI_IN = re.compile(r"doi:(10\.\S+)")


def sample_dois(store: Store, prefix: str, k: int, seed: int) -> list[str]:
    dois = [r[0] for r in store._conn.execute(  # noqa: SLF001
        "SELECT DISTINCT x.value FROM external_ids x JOIN entities e ON e.id = x.entity_id "
        "WHERE x.scheme='doi' AND x.entity_id GLOB ? AND e.merged_into IS NULL ORDER BY x.value", (prefix + "*",))]
    return sorted(random.Random(seed).sample(dois, min(k, len(dois))))


def fetch(source: str, name: str, dois: list[str]) -> dict[str, Any]:
    path = cache_path(source, name)
    if path.exists():
        return {"records": len(read_cache(path).rows), "cached": True}
    client = CachedClient(WORKS_CACHE / source / "_http", min_interval=1.0, user_agent=USER_AGENT, timeout=60)
    rows, t0, errors = [], time.perf_counter(), 0
    for d in dois:
        try:
            if source == "opencitations":
                resp = client.get(OC_API + d, use_cache=False)
                refs = sorted({norm_doi(m) for r in resp.json() for m in _DOI_IN.findall(r.get("cited", ""))})
            else:
                resp = client.get(CR_API + d, {"mailto": CONTACT}, use_cache=False)
                refs = sorted({norm_doi(r["DOI"]) for r in resp.json()["message"].get("reference", ()) if r.get("DOI")})
        except HttpError as exc:
            errors += exc.status != 404
            rows.append({"doi": d, "refs": [], "status": exc.status, "retrieved_at": None})
            continue
        rows.append({"doi": d, "refs": [r for r in refs if r], "status": 200, "retrieved_at": resp.fetched_at})
    if errors:  # 429/5xx are not "no references": nothing is cached, a rerun fetches again
        raise RuntimeError(f"{source} {name}: {errors} HTTP errors other than 404; not cached, rerun later")
    write_cache(path, rows)
    return {"records": len(rows), "errors": errors, "elapsed_s": round(time.perf_counter() - t0, 1)}


def _present_pairs(store: Store, subjects: list[str]) -> set[tuple[str, str]]:
    """Canonical (subject, object) pairs of current `cites` whose subject resolves to one of `subjects`."""
    out = set()
    for s in subjects:
        ids = store.merged_ids(s)
        q = (f"SELECT o.entity_id FROM claims c JOIN claim_participants p ON p.claim_id=c.id AND p.role='subject' "
             f"JOIN claim_participants o ON o.claim_id=c.id AND o.role='object' WHERE c.relation='cites' "
             f"AND p.entity_id IN ({','.join('?' * len(ids))})")
        out.update((s, store.resolve(o)) for (o,) in store._conn.execute(q, ids))  # noqa: SLF001
    return out


def ingest(store: Store, source: str, name: str) -> dict[str, Any]:
    register_sources(store, [source])
    path = cache_path(source, name)
    batch = read_cache(path)
    t = time.perf_counter()
    doi_of = by_external(store, "doi", {d for r in batch.rows for d in [r["doi"], *r["refs"]]})
    canon = {d: store.resolve(e) for d, e in doi_of.items()}
    subjects = sorted({canon[r["doi"]] for r in batch.rows if r["doi"] in canon})
    present = _present_pairs(store, subjects)
    st = {"works": len(batch.rows), "with_refs": 0, "refs": 0, "internal": 0, "already_present": 0, "added": 0,
          "http_errors": sum(r["status"] not in (200, 404) for r in batch.rows)}
    if st["http_errors"]:
        raise RuntimeError(f"{source} {name}: cache holds {st['http_errors']} HTTP error rows; refetch it")
    claims = []
    for r in batch.rows:
        st["with_refs"] += bool(r["refs"])
        src = canon.get(r["doi"])
        for d in r["refs"]:
            st["refs"] += 1
            tgt = canon.get(d)
            if not src or not tgt or tgt == src:
                continue
            st["internal"] += 1
            if (src, tgt) in present:
                st["already_present"] += 1
                continue
            present.add((src, tgt))
            p = prov(source, f"{OC_API if source == 'opencitations' else CR_API}{r['doi']}", EXTRACTOR, VERSION,
                     r["retrieved_at"])
            claims.append(claim(RelationType.CITES, src, tgt, p, source))
            st["added"] += 1
    bid = run_batch(store, source, f"{VERSION}@{source}:cites", name, batch, [], claims, st)
    return {"batch": bid, **st, "seconds": round(time.perf_counter() - t, 2)}


__all__ = ["sample_dois", "fetch", "ingest"]
