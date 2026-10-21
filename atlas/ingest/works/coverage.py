"""Coverage cells (ADR-0005 §9): per source, period and area, what the source indexes (`indexed`,
counted on the source with the method named) against what the registry holds (`ingested`).
Append-only: measuring again adds rows with a new `measured_at`."""

from __future__ import annotations

from typing import Any

from ...model import CoverageCell, now_iso
from ...store import Store
from ..http import CachedClient, HttpError
from .common import ACTOR, CONTACT, USER_AGENT, WORKS_CACHE, read_cache
from .zbmath import API as ZB_API, _area_of, msc_areas

PERIODS = ((1600, 1799), (1800, 1867), (1868, 1899), (1900, 1949), (1950, 1999), (2000, 2026))


def _client() -> CachedClient:
    # few retries: a 429 from an exhausted daily budget does not clear by waiting (it is recorded as unknown)
    return CachedClient(WORKS_CACHE / "_coverage", min_interval=1.0, user_agent=USER_AGENT, max_retries=1)


def _zb_count(c: CachedClient, q: str) -> int | None:
    """0 only for the API's 'No results found' envelope; any other 404 is unknown (None)."""
    try:
        d = c.get(ZB_API, {"search_string": q, "page": "0", "results_per_page": "1"}, use_cache=False).json()
    except HttpError as exc:
        if exc.status == 404:
            return 0 if b"No results found" in exc.body else None
        raise
    return int(d["status"]["nr_total_results"])


def _cdli_indexed(genre: str = "Mathematical") -> tuple[int | None, str]:
    """(count, method) from the search pages `canon.fetch_cdli` cached, with their fetch date."""
    pages = sorted((WORKS_CACHE / "cdli").glob(f"genre={genre}_page=*.jsonl.gz"))
    if not pages:
        return None, "unknown: no CDLI search pages in the cache"
    rows = [r for p in pages for r in read_cache(p).rows]
    fetched = max((r.get("retrieved_at") or "") for r in rows)[:10] if rows else "?"
    return len(rows), f"rows of search?genre={genre} in the cache ({len(pages)} pages, fetched {fetched})"


def _oa_count(c: CachedClient, a: int, b: int) -> int | None:
    """None = unknown: OpenAlex refused the count (429, e.g. the keyless daily budget is used up)."""
    try:
        d = c.get("https://api.openalex.org/works", {"filter": f"primary_topic.field.id:26,publication_year:{a}-{b}",
                                                 "per_page": "1", "select": "id", "mailto": CONTACT},
              use_cache=False).json()
    except HttpError as exc:
        if exc.status == 429:
            return None
        raise
    return int(d["meta"]["count"])


def _ingested(store: Store, glob: str, batch_like: str, a: int, b: int) -> int:
    return store._conn.execute(  # noqa: SLF001
        "SELECT COUNT(DISTINCT e.id) FROM entities e JOIN ingest_batches g ON g.id = e.ingest_batch "
        "JOIN entity_dates d ON d.entity_id = e.id WHERE e.id GLOB ? AND g.batch_key LIKE ? "
        "AND d.lo BETWEEN ? AND ?", (glob, batch_like, a, b)).fetchone()[0]


def measure(store: Store, msc: str) -> dict[str, Any]:
    c, ts = _client(), now_iso()
    area = _area_of(f"{msc}-XX", msc_areas(store)) or "*"
    cells = []
    for a, b in PERIODS:
        zb_n = _zb_count(c, f"cc:{msc}* py:{a}-{b}")
        cells.append(CoverageCell("src:zbmath", "REST v1, 2026-10-19", f"{a:04d}/{b:04d}", area, zb_n,
                                  f"nr_total_results of 'cc:{msc}* py:{a}-{b}', zbMATH REST, {ts[:10]}"
                                  if zb_n is not None else f"unknown: zbMATH answered 404 without 'No results', {ts[:10]}",
                                  _ingested(store, "zb:*", f"cc={msc}_%", a, b), f"msc-class-{msc}-all", ts))
        oa_n = _oa_count(c, a, b)
        cells.append(CoverageCell("src:openalex", "API, 2026-10-19", f"{a:04d}/{b:04d}", "*", oa_n,
                                  f"meta.count of primary_topic.field.id:26,publication_year:{a}-{b}, {ts[:10]}"
                                  if oa_n is not None else f"unknown: OpenAlex answered 429 (keyless budget), {ts[:10]}",
                                  _ingested(store, "oa:*", "sample=%", a, b), "random-sample-10000-seed20261019", ts))
    cdli = store._conn.execute("SELECT COUNT(*) FROM entities WHERE id GLOB 'cdli:*'").fetchone()[0]  # noqa: SLF001
    cdli_n, cdli_method = _cdli_indexed()
    cells.append(CoverageCell("src:cdli", "cdli.earth search JSON, 2026-10-19", "../0000", "*", cdli_n,
                              cdli_method, cdli, "all", ts))
    canon_n = store._conn.execute(  # noqa: SLF001
        "SELECT COUNT(*) FROM entities e JOIN ingest_batches g ON g.id=e.ingest_batch "
        "WHERE g.batch_key IN ('canon-p50-sitelinks3','curated-v1') AND e.type IN ('publication','witness')").fetchone()[0]
    cells.append(CoverageCell("src:wikidata", "WDQS + wbgetentities, 2026-10-19", "../2026", "*", None,
                              "unknown: Wikidata has no catalogue of all mathematical works", canon_n,
                              "P50-registry-person-sitelinks>=3+curated", ts))
    store.add_coverage_cells(cells, actor=ACTOR)
    return {"cells": len(cells), "rows": [c.__dict__ for c in cells]}
