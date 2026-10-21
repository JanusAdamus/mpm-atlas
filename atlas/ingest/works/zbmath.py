"""zbMATH Open adapter (CC BY-SA 4.0), REST API `api.zbmath.org/v1`.

Fetch: one partition = (MSC top class, publication year), paged at 100 per request.  The REST search
window stops at 10,000 hits, so a partition above it is an error, never a silent cut.  Each partition's
records are filtered to metadata (no review, no keyword, no reference text) and written as one
`data/cache/works/zbmath/<key>.jsonl.gz`; the raw responses are never cached.

Ingest, two passes (ADR-0005 §8.5):
1. works: one `ingest_batch` per partition with publications `zb:<document id>`, authors
   `zba:<author code>`, `authored_by` and `part_of_area` (MSC code -> the registry's area with that
   MSC id, falling back to `NNxxx` then `NN-XX`);
2. citations: one batch per partition with the `cites` whose target is a work already in the
   registry; external references are counted, not stored.
"""

from __future__ import annotations

import logging
import time
from concurrent.futures import ThreadPoolExecutor
from typing import Any

from ...model import DateKind, Entity, EntityType, HistoricalDate, LangString, RelationType
from ...store import Store
from ..http import CachedClient, HttpError
from .common import (LANGS, USER_AGENT, WORKS_CACHE, by_external, cache_path, claim, existing, norm_doi, prov,
                     read_cache, register_sources, run_batch, write_cache)

log = logging.getLogger(__name__)
API = "https://api.zbmath.org/v1/document/_search"
PER_PAGE = 100
WINDOW = 10_000
EXTRACTOR, VERSION = "atlas.ingest.works.zbmath", "works-zb/1"
RUN = f"{VERSION}@zbmath-rest-2026-10"
R = RelationType


def partition_key(msc: str, year: int) -> str:
    return f"cc={msc};py={year}"


def _filter(rec: dict[str, Any]) -> dict[str, Any]:
    """Metadata only.  `editorial_contributions` (reviews), keywords and reference texts are dropped here."""
    title = rec.get("title") or {}
    links = [l.get("identifier") for l in rec.get("links") or () if l.get("type") == "doi"]
    authors = [(a.get("codes") or [None])[0] for a in (rec.get("contributors") or {}).get("authors") or ()]
    names = [a.get("name") for a in (rec.get("contributors") or {}).get("authors") or ()]
    editors = [((e.get("codes") or [None])[0], e.get("name")) for e in (rec.get("contributors") or {}).get("editors") or ()]
    refs = [{"zb": (r.get("zbmath") or {}).get("document_id"), "doi": norm_doi(r.get("doi"))}
            for r in rec.get("references") or ()]
    langs = (rec.get("language") or {}).get("languages") or []
    return {
        "id": rec["id"], "zbl": rec.get("identifier"), "title": title.get("title"), "subtitle": title.get("subtitle"),
        "addition": title.get("addition"), "year": rec.get("year"), "type": (rec.get("document_type") or {}).get("code"),
        "language": langs[0] if len(langs) == 1 else None, "doi": [norm_doi(d) for d in links if d],
        "authors": [[c, n] for c, n in zip(authors, names)], "editors": editors,
        "msc": [m.get("code") for m in rec.get("msc") or () if m.get("code")],
        "venue": (rec.get("source") or {}).get("source"), "database": rec.get("database"), "refs": refs,
    }


def fetch_partition(client: CachedClient, msc: str, year: int) -> int:
    """Fetch one partition into the filtered cache (skipped when present).  Returns its record count."""
    path = cache_path("zbmath", partition_key(msc, year))
    if path.exists():
        return len(read_cache(path).rows)
    query = f"cc:{msc}* py:{year}"
    rows: list[dict[str, Any]] = []
    page, total = 0, None
    fetched_at = None
    while total is None or len(rows) < total:
        try:
            resp = client.get(API, {"search_string": query, "page": str(page), "results_per_page": str(PER_PAGE)},
                              use_cache=False)
        except HttpError as exc:  # the API answers "no results" with a 404 envelope
            if exc.status == 404 and b"No results found" in exc.body and page == 0:
                break
            raise
        d = resp.json()
        st = d["status"]
        if not st.get("execution_bool"):
            raise RuntimeError(f"zbMATH {query} page {page}: {st.get('internal_code')}")
        total = st["nr_total_results"]
        if total > WINDOW:
            raise RuntimeError(f"zbMATH partition {query} has {total} hits > REST window {WINDOW}: split it")
        batch = d.get("result") or []
        fetched_at = fetched_at or resp.fetched_at
        rows.extend({**_filter(r), "retrieved_at": resp.fetched_at} for r in batch)
        if not batch:
            break
        page += 1
    if total is not None and len(rows) != total:  # a short page must not be frozen as a complete partition
        raise RuntimeError(f"zbMATH {query}: got {len(rows)} records, API reported {total}; not cached")
    write_cache(path, rows)
    # One line per partition: a run that dies leaves a trail in its stderr log (the 2026-10-19 fetch did not).
    log.info("zbmath %s: %d records, %d pages", partition_key(msc, year), len(rows), page)
    return len(rows)


def fetch(msc: str, years: range, *, workers: int = 2, min_interval: float = 1.0) -> dict[str, Any]:
    client = CachedClient(WORKS_CACHE / "zbmath" / "_http", min_interval=min_interval, user_agent=USER_AGENT,
                          timeout=120)
    t0 = time.perf_counter()
    with ThreadPoolExecutor(workers) as ex:
        counts = dict(zip(years, ex.map(lambda y: fetch_partition(client, msc, y), years)))
    return {"records": sum(counts.values()), "by_year": {y: n for y, n in counts.items() if n},
            "elapsed_s": round(time.perf_counter() - t0, 1), "http": client.stats}


# ---------------------------------------------------------------------- ingest


def _area_of(code: str, areas: dict[str, str]) -> str | None:
    for k in (code, code[:3] + "xx", code[:2] + "-XX"):
        if k in areas:
            return areas[k]
    return None


def msc_areas(store: Store) -> dict[str, str]:
    rows = store._conn.execute(  # noqa: SLF001
        "SELECT x.value, x.entity_id FROM external_ids x JOIN entities e ON e.id = x.entity_id "
        "WHERE x.scheme='msc' AND e.type='area' ORDER BY x.entity_id")
    out: dict[str, str] = {}
    for code, eid in rows:
        out.setdefault(code, eid)
    return out


def _entities(r: dict[str, Any]) -> tuple[Entity, list[Entity]]:
    p = prov("zbmath", f"https://zbmath.org/{r['id']}", EXTRACTOR, VERSION, r["retrieved_at"])
    title = " ".join(x for x in (r["title"], r["subtitle"]) if x) or f"zbMATH {r['zbl'] or r['id']}"
    lang = LANGS.get(r["language"] or "", "und")
    ids = [("zbmath_doc", str(r["id"]))] + ([("zbl", r["zbl"])] if r["zbl"] else []) + [("doi", d) for d in r["doi"]]
    dates = ()
    if r["year"] and str(r["year"]).isdigit() and len(str(r["year"])) == 4:
        kind = DateKind.EDITION if r["addition"] and "ed" in r["addition"] else DateKind.PUBLISHED
        dates = (HistoricalDate(kind, str(r["year"]), p, raw=str(r["year"])),)
    doc = Entity(f"zb:{r['id']}", EntityType.PUBLICATION, (LangString(title[:2000], lang),),
                 external_ids=tuple(ids), provenance=(p,), dates=dates)
    people = [Entity(f"zba:{c}", EntityType.PERSON, (LangString(n or c, "und"),),
                     external_ids=(("zbmath_author", c),), provenance=(p,))
              for c, n in r["authors"] + [list(e) for e in r["editors"]] if c]
    return doc, people


def ingest_works(store: Store, msc: str, *, primary_skip: set[str] | None = None) -> dict[str, Any]:
    """Pass 1 over every cached partition of `msc`.  `primary_skip`: zb ids whose canonical record is a
    higher-priority source (Wikidata): they get ids and dates but no authored_by/part_of_area."""
    register_sources(store, ["zbmath"])
    areas = msc_areas(store)
    out: dict[str, Any] = {"batches": 0, "skipped_batches": 0, "works": 0, "claims": 0, "seconds": 0.0,
                           "authors_per_work": 0, "msc_per_work": 0, "area_unmapped_codes": 0, "dates": 0,
                           "no_date": 0}
    skip = primary_skip or set()
    for path in sorted((WORKS_CACHE / "zbmath").glob(f"cc={msc}*.jsonl.gz")):
        key = path.name.removesuffix(".jsonl.gz")
        batch = read_cache(path)
        t = time.perf_counter()
        ents: dict[str, Entity] = {}
        claims = []
        st = {"works": 0, "authors": 0, "msc_codes": 0, "area_unmapped": 0, "dates": 0, "dates_flagged": 0,
              "secondary": 0}
        for r in batch.rows:
            doc, people = _entities(r)
            ents.setdefault(doc.id, doc)
            st["works"] += 1
            st["dates"] += len(doc.dates)
            st["msc_codes"] += len(r["msc"])
            if doc.id in skip:
                st["secondary"] += 1
                continue
            p = doc.provenance[0]
            for e in people:
                ents.setdefault(e.id, e)
            authors = [c for c, _ in r["authors"] if c]
            st["authors"] += len(authors)
            for c in dict.fromkeys(authors):
                claims.append(claim(R.AUTHORED_BY, doc.id, f"zba:{c}", p))
            for c, _ in r["editors"]:
                if c and c not in authors:
                    claims.append(claim(R.EDITED_BY, doc.id, f"zba:{c}", p))
            for a in dict.fromkeys(filter(None, (_area_of(m, areas) for m in r["msc"]))):
                claims.append(claim(R.PART_OF_AREA, doc.id, a, p))
            st["area_unmapped"] += sum(_area_of(m, areas) is None for m in r["msc"])
        bid = run_batch(store, "zbmath", RUN + ":works", key, batch, list(ents.values()), claims, st)
        out["seconds"] += time.perf_counter() - t
        if bid is None:
            out["skipped_batches"] += 1
            continue
        out["batches"] += 1
        out["works"] += st["works"]
        out["claims"] += len(claims)
        out["authors_per_work"] += st["authors"]
        out["msc_per_work"] += st["msc_codes"]
        out["area_unmapped_codes"] += st["area_unmapped"]
        out["dates"] += st["dates"]
        out["no_date"] += st["works"] - st["dates"]
    out["seconds"] = round(out["seconds"], 2)
    return out


def ingest_citations(store: Store, msc: str) -> dict[str, Any]:
    """Pass 2: `cites` to works already in the registry (by zbMATH document id, else DOI)."""
    out = {"batches": 0, "skipped_batches": 0, "refs": 0, "internal": 0, "external": 0, "claims": 0, "seconds": 0.0}
    for path in sorted((WORKS_CACHE / "zbmath").glob(f"cc={msc}*.jsonl.gz")):
        key = path.name.removesuffix(".jsonl.gz")
        batch = read_cache(path)
        t = time.perf_counter()
        targets = existing(store, (f"zb:{x['zb']}" for r in batch.rows for x in r["refs"] if x["zb"]))
        doi_map = by_external(store, "doi", (x["doi"] for r in batch.rows for x in r["refs"] if x["doi"] and not x["zb"]))
        sources = existing(store, (f"zb:{r['id']}" for r in batch.rows))
        claims, st = [], {"refs": 0, "internal": 0, "external": 0}
        for r in batch.rows:
            src = f"zb:{r['id']}"
            if src not in sources:
                continue
            p = prov("zbmath", f"https://zbmath.org/{r['id']}#references", EXTRACTOR, VERSION, r["retrieved_at"])
            for x in r["refs"]:
                st["refs"] += 1
                tgt = f"zb:{x['zb']}" if x["zb"] and f"zb:{x['zb']}" in targets else doi_map.get(x["doi"] or "")
                if tgt and tgt != src:
                    st["internal"] += 1
                    claims.append(claim(R.CITES, src, tgt, p))
                else:
                    st["external"] += 1
        claims = list({c.id: c for c in claims}.values())
        bid = run_batch(store, "zbmath", RUN + ":cites", key, batch, [], claims, st)
        out["seconds"] += time.perf_counter() - t
        if bid is None:
            out["skipped_batches"] += 1
            continue
        out["batches"] += 1
        out["claims"] += len(claims)
        for k in ("refs", "internal", "external"):
            out[k] += st[k]
    out["seconds"] = round(out["seconds"], 2)
    return out


def count(msc: str, client: CachedClient) -> int:
    """`nr_total_results` of `cc:<msc>*` (the coverage cell's `indexed`)."""
    d = client.get(API, {"search_string": f"cc:{msc}*", "page": "0", "results_per_page": "1"}, use_cache=False).json()
    return int(d["status"]["nr_total_results"])


__all__ = ["fetch", "fetch_partition", "ingest_works", "ingest_citations", "count", "partition_key", "msc_areas"]
