"""`python -m atlas.ingest.works --db data/pilot/atlas-pilot.db STEP [STEP ...]`

Steps (ADR-0005 orden 2c; each one resumable: a finished batch key is skipped):
  canon      curated list + Wikidata canon of registry authors (needs the fetched item caches)
  cdli       CDLI mathematical tablets as witness
  zb-works   zbMATH partitions of --msc (pass 1)        oa-works   OpenAlex inputs (pass 1)
  merges     exact-identifier merges                     zb-cites / oa-cites   pass 2
  oc-cites / cr-cites   OpenCitations / Crossref sample  coverage   coverage_cells
Prints one JSON object with each step's stats and wall time."""

from __future__ import annotations

import argparse
import json
import logging
import sys
import time

from ...store import Store
from . import canon, citations, coverage, openalex, resolve, zbmath
from .common import cache_path


def _canon(st: Store, a: argparse.Namespace) -> dict:
    cur = canon.load_curated()
    return {"curated": canon.ingest_canon(st, cache_path("wikidata", "curated-v1"), curated=cur, key="curated-v1"),
            "canon": canon.ingest_canon(st, cache_path("wikidata", "canon-p50-sitelinks3"), key="canon-p50-sitelinks3")}


STEPS = {
    "canon": _canon,
    "cdli": lambda st, a: canon.ingest_cdli(st),
    "zb-works": lambda st, a: zbmath.ingest_works(st, a.msc),
    "oa-works": lambda st, a: openalex.ingest_works(st),
    "merges": lambda st, a: resolve.run(st),
    "zb-cites": lambda st, a: zbmath.ingest_citations(st, a.msc),
    "oa-cites": lambda st, a: openalex.ingest_citations(st),
    "oc-cites": lambda st, a: citations.ingest(st, "opencitations", a.cit_sample),
    "cr-cites": lambda st, a: citations.ingest(st, "crossref", a.cit_sample),
    "coverage": lambda st, a: coverage.measure(st, a.msc),
}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="python -m atlas.ingest.works")
    ap.add_argument("--db", required=True)
    ap.add_argument("--msc", default="30")
    ap.add_argument("--cit-sample", default="zb30-doi-300-seed20261019")
    ap.add_argument("steps", nargs="+", choices=sorted(STEPS))
    a = ap.parse_args(argv)
    logging.basicConfig(level=logging.INFO, stream=sys.stderr)
    out: dict = {}
    with Store(a.db) as st:
        for s in a.steps:
            t = time.perf_counter()
            out[s] = STEPS[s](st, a)
            out[s + ":wall_s"] = round(time.perf_counter() - t, 2)
    sys.stdout.reconfigure(encoding="utf-8")
    print(json.dumps(out, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
