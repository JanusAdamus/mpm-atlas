"""Ingest adapters: Mathlib4, Wikidata, Wikipedia (ES/EN), then entity resolution.

`run_ingest(store, sources, limit=None, offline=False)` runs them in dependency
order (Mathlib first so Wikidata can seed from and link to its declarations;
Wikipedia reads the sitelinks Wikidata wrote; resolution last).  Every adapter is
idempotent: HTTP responses are cached under `cache_root`, entity inserts ignore
existing ids and claim ids are deterministic, so an interrupted run is resumed by
running it again.  `offline=True` reads only the cache (tests point `cache_root`
at tests/fixtures/ingest/).
"""

from __future__ import annotations

import time
from pathlib import Path
from typing import Iterable

from ..store import Store
from . import mathlib, wikidata, wikipedia
from .common import DEFAULT_CACHE, IngestReport, Timer
from .http import CachedClient

SOURCES = ("mathlib", "wikidata", "wikipedia", "resolve")


def run_ingest(store: Store, sources: Iterable[str] = SOURCES, limit: int | None = None, offline: bool = False, *,
               cache_root: Path | str = DEFAULT_CACHE, workers: int | None = None, budget_s: float | None = None) -> dict:
    """Run the selected adapters; returns per-source reports, timings and the store's coverage.

    `budget_s` bounds the Wikipedia stage (see wikipedia.ingest), the only
    stage whose speed is set by a remote rate limit rather than by this machine."""
    wanted = set(sources)
    unknown = wanted - set(SOURCES)
    if unknown:
        raise ValueError(f"unknown sources {sorted(unknown)}; choose from {SOURCES}")
    root = Path(cache_root)
    t0 = time.perf_counter()
    reports: list[IngestReport] = []
    if "mathlib" in wanted:
        reports.append(mathlib.ingest(store, cache_root=root, offline=offline, limit=limit, workers=workers))
    if "wikidata" in wanted:
        # a truncated/timed-out SPARQL answer is split, not retried (retrying a 60 s query wastes minutes)
        client = CachedClient(root / "wikidata", offline=offline, min_interval=0.15, max_reject_retries=0)
        extra = [link["qid"] for link in mathlib.curated_links(root)]
        rep = wikidata.ingest(store, client=client, limit=limit, extra_qids=extra)
        rep.counters.update({f"http:{k}": v for k, v in client.stats.items()})
        reports.append(rep)
    if wanted & {"mathlib", "wikidata"}:
        rep = IngestReport("mathlib-wikidata-links")
        with Timer(rep, "link"):
            mathlib.link_wikidata(store, cache_root=root, report=rep)
        reports.append(rep)
    if "wikipedia" in wanted:
        interval, wp_workers = wikipedia.polite_settings()
        client = CachedClient(root / "wikipedia", offline=offline, min_interval=0.0 if offline else interval)
        rep = wikipedia.ingest(store, client=client, limit=limit, budget_s=budget_s,
                               workers=wp_workers)
        rep.counters.update({f"http:{k}": v for k, v in client.stats.items()})
        reports.append(rep)
    out: dict = {"reports": [r.to_json() for r in reports]}
    if "resolve" in wanted:
        from .. import resolve  # late import: resolve depends on ingest.lean

        t = time.perf_counter()
        out["resolve"] = resolve.run(store, queue_path=root / "resolve" / f"review-queue-{resolve.RESOLVER_VERSION.replace('/', '-')}.jsonl")
        out["resolve"]["elapsed_s"] = round(time.perf_counter() - t, 2)
    out["elapsed_s"] = round(time.perf_counter() - t0, 2)
    out["coverage"] = store.coverage()
    return out


__all__ = ["run_ingest", "SOURCES"]
