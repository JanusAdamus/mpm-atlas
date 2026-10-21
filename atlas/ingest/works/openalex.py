"""OpenAlex adapter (CC0), REST API.  Abstracts are never requested (`select` lists the fields).

Inputs (each one batch, cached filtered as `data/cache/works/openalex/<key>.jsonl.gz`):
- a seeded random sample of the Mathematics field (`primary_topic.field.id:26`), 200 per page;
- the works of a list of DOIs (used for the zbMATH ∩ OpenAlex overlap and merges), 50 per request.

Primary-record rule (ADR-0005 §8.1): a work whose DOI already belongs to a zbMATH or Wikidata
record is secondary: it gets its ids and date, no `authored_by` and no `cites` (those come from the
primary record); the overlap is measured in the batch stats and the merge is made by `resolve_works`.
"""

from __future__ import annotations

import time
from typing import Any, Iterable

from ...model import DateKind, Entity, EntityType, HistoricalDate, LangString, RelationType
from ...store import Store
from ..http import CachedClient
from .common import (CONTACT, USER_AGENT, WORKS_CACHE, by_external, cache_path, claim, existing, norm_doi, prov,
                     read_cache, register_sources, run_batch, write_cache)

API = "https://api.openalex.org/works"
FIELDS = ("id,doi,display_name,publication_year,publication_date,type,language,authorships,primary_topic,"
          "referenced_works,primary_location")
EXTRACTOR, VERSION = "atlas.ingest.works.openalex", "works-oa/1"
RUN = f"{VERSION}@openalex-api-2026-10"
PLAUSIBLE_FROM = 1450  # before printing: an OpenAlex year this early is flagged (calendar unknown), never rejected
R = RelationType


def _short(url: str | None) -> str | None:
    return url.rsplit("/", 1)[-1] if url else None


def _filter(w: dict[str, Any], fetched_at: str) -> dict[str, Any]:
    pt = w.get("primary_topic") or {}
    loc = (w.get("primary_location") or {}).get("source") or {}
    return {
        "id": _short(w["id"]), "doi": norm_doi(w.get("doi")), "title": w.get("display_name"),
        "year": w.get("publication_year"), "date": w.get("publication_date"), "type": w.get("type"),
        "language": w.get("language"),
        "authors": [[_short((a.get("author") or {}).get("id")), (a.get("author") or {}).get("display_name"),
                     _short((a.get("author") or {}).get("orcid"))] for a in w.get("authorships") or ()],
        "subfield": _short((pt.get("subfield") or {}).get("id")), "field": _short((pt.get("field") or {}).get("id")),
        "refs": [_short(x) for x in w.get("referenced_works") or ()], "venue": loc.get("display_name"),
        "retrieved_at": fetched_at,
    }


def _client() -> CachedClient:
    return CachedClient(WORKS_CACHE / "openalex" / "_http", min_interval=1.0, user_agent=USER_AGENT)


def fetch_sample(seed: int, n: int = 10_000, per_page: int = 200) -> dict[str, Any]:
    client, t0, got = _client(), time.perf_counter(), 0
    for page in range(1, n // per_page + 1):
        path = cache_path("openalex", f"sample=f26;seed={seed};page={page:03d}")
        if path.exists():
            got += len(read_cache(path).rows)
            continue
        params = {"filter": "primary_topic.field.id:26", "sample": str(n), "seed": str(seed), "per_page": str(per_page),
                  "page": str(page), "select": FIELDS, "mailto": CONTACT}
        resp = client.get(API, params, use_cache=False)
        rows = [_filter(w, resp.fetched_at) for w in resp.json()["results"]]
        write_cache(path, rows)
        got += len(rows)
    return {"records": got, "elapsed_s": round(time.perf_counter() - t0, 1), "http": client.stats}


def fetch_dois(name: str, dois: list[str], chunk: int = 50) -> dict[str, Any]:
    client, t0, got = _client(), time.perf_counter(), 0
    for i in range(0, len(dois), chunk):
        path = cache_path("openalex", f"dois={name};chunk={i // chunk:04d}")
        if path.exists():
            got += len(read_cache(path).rows)
            continue
        params = {"filter": "doi:" + "|".join(dois[i:i + chunk]), "per_page": "200", "select": FIELDS,
                  "mailto": CONTACT}
        resp = client.get(API, params, use_cache=False)
        rows = [_filter(w, resp.fetched_at) for w in resp.json()["results"]]
        write_cache(path, rows)
        got += len(rows)
    return {"requested": len(dois), "records": got, "elapsed_s": round(time.perf_counter() - t0, 1),
            "http": client.stats}


def _lang(code: str | None) -> str:
    """OpenAlex `language` as a BCP 47 primary tag; it is sometimes upper case ('FR')."""
    c = (code or "").strip().lower()
    return c if len(c) == 2 and c.isalpha() else "und"


def _date(r: dict[str, Any], p) -> tuple[tuple[HistoricalDate, ...], bool]:
    """(dates, flagged).  An implausibly early year is kept with '?' and calendar 'unknown' (ADR §1)."""
    raw = r["date"] or (str(r["year"]) if r["year"] else None)
    if not r["year"]:
        return (), False
    year = int(r["year"])
    if year < PLAUSIBLE_FROM:
        return (HistoricalDate(DateKind.PUBLISHED, f"{year:04d}?", p, calendar="unknown", raw=raw),), True
    edtf = r["date"] if r["date"] and r["date"][:4] == f"{year:04d}" else f"{year:04d}"
    return (HistoricalDate(DateKind.PUBLISHED, edtf, p, raw=raw),), False


def ingest_works(store: Store, pattern: str = "*") -> dict[str, Any]:
    register_sources(store, ["openalex"])
    out: dict[str, Any] = {"batches": 0, "skipped_batches": 0, "works": 0, "secondary": 0, "claims": 0,
                           "authors": 0, "dates": 0, "dates_flagged": 0, "statistics_or_ml": 0, "seconds": 0.0}
    for path in sorted((WORKS_CACHE / "openalex").glob(f"{pattern}.jsonl.gz")):
        key = path.name.removesuffix(".jsonl.gz")
        batch = read_cache(path)
        t = time.perf_counter()
        primary_of = by_external(store, "doi", (r["doi"] for r in batch.rows if r["doi"]), skip_prefix="oa:")
        ents: dict[str, Entity] = {}
        claims = []
        st = {"works": 0, "secondary": 0, "authors": 0, "dates": 0, "dates_flagged": 0, "statistics_or_ml": 0}
        for r in batch.rows:
            p = prov("openalex", f"https://openalex.org/{r['id']}", EXTRACTOR, VERSION, r["retrieved_at"])
            dates, flagged = _date(r, p)
            ids = [("openalex", r["id"])] + ([("doi", r["doi"])] if r["doi"] else []) + (
                [("openalex_subfield", r["subfield"])] if r["subfield"] else [])
            lang = _lang(r["language"])
            doc = Entity(f"oa:{r['id']}", EntityType.PUBLICATION,
                         (LangString((r["title"] or f"OpenAlex {r['id']}")[:2000], lang),),
                         external_ids=tuple(ids), provenance=(p,), dates=dates)
            ents.setdefault(doc.id, doc)
            st["works"] += 1
            st["dates"] += len(dates)
            st["dates_flagged"] += flagged
            st["statistics_or_ml"] += r["subfield"] in ("2613", "2611")  # Statistics and Probability; Modeling
            if r["doi"] in primary_of:
                st["secondary"] += 1
                continue
            for aid, name, orcid in r["authors"]:
                if not aid:
                    continue
                ents.setdefault(f"oaa:{aid}", Entity(f"oaa:{aid}", EntityType.PERSON, (LangString(name or aid, "und"),),
                                                     external_ids=(("openalex_author", aid),) + (
                                                         (("orcid", orcid),) if orcid else ()), provenance=(p,)))
                claims.append(claim(R.AUTHORED_BY, doc.id, f"oaa:{aid}", p))
                st["authors"] += 1
        claims = list({c.id: c for c in claims}.values())
        bid = run_batch(store, "openalex", RUN + ":works", key, batch, list(ents.values()), claims, st)
        out["seconds"] += time.perf_counter() - t
        if bid is None:
            out["skipped_batches"] += 1
            continue
        out["batches"] += 1
        out["claims"] += len(claims)
        for k in st:
            out[k] += st[k]
    out["seconds"] = round(out["seconds"], 2)
    return out


def ingest_citations(store: Store, pattern: str = "*") -> dict[str, Any]:
    """`cites` from primary OpenAlex works to works in the registry (OpenAlex ids, resolved by reads)."""
    out = {"batches": 0, "skipped_batches": 0, "refs": 0, "internal": 0, "external": 0, "from_secondary": 0,
           "claims": 0, "seconds": 0.0}
    for path in sorted((WORKS_CACHE / "openalex").glob(f"{pattern}.jsonl.gz")):
        key = path.name.removesuffix(".jsonl.gz")
        batch = read_cache(path)
        t = time.perf_counter()
        targets = existing(store, (f"oa:{x}" for r in batch.rows for x in r["refs"]))
        merged = dict(store._conn.execute(  # noqa: SLF001 - secondary = merged into another record
            f"SELECT id, merged_into FROM entities WHERE id IN ({','.join('?' * len(batch.rows))}) "
            "AND merged_into IS NOT NULL", [f"oa:{r['id']}" for r in batch.rows])) if batch.rows else {}
        primary_of = by_external(store, "doi", (r["doi"] for r in batch.rows if r["doi"]), skip_prefix="oa:")
        claims, st = [], {"refs": 0, "internal": 0, "external": 0, "from_secondary": 0}
        for r in batch.rows:
            src = f"oa:{r['id']}"
            secondary = src in merged or r["doi"] in primary_of
            p = prov("openalex", f"https://openalex.org/{r['id']}#referenced_works", EXTRACTOR, VERSION,
                     r["retrieved_at"])
            for x in r["refs"]:
                st["refs"] += 1
                internal = f"oa:{x}" in targets
                st["internal" if internal else "external"] += 1
                if internal and secondary:
                    st["from_secondary"] += 1  # counted; the primary record states its own references
                elif internal and f"oa:{x}" != src:
                    claims.append(claim(R.CITES, src, f"oa:{x}", p))
        claims = list({c.id: c for c in claims}.values())
        bid = run_batch(store, "openalex", RUN + ":cites", key, batch, [], claims, st)
        out["seconds"] += time.perf_counter() - t
        if bid is None:
            out["skipped_batches"] += 1
            continue
        out["batches"] += 1
        out["claims"] += len(claims)
        for k in st:
            out[k] += st[k]
    out["seconds"] = round(out["seconds"], 2)
    return out


def dois_of(store: Store, prefix: str) -> list[str]:
    return [r[0] for r in store._conn.execute(  # noqa: SLF001
        "SELECT value FROM external_ids WHERE scheme='doi' AND entity_id GLOB ? ORDER BY value", (prefix + "*",))]


__all__ = ["fetch_sample", "fetch_dois", "ingest_works", "ingest_citations", "dois_of", "Iterable"]
