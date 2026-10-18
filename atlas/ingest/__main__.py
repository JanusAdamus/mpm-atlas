"""`python -m atlas.ingest [--db data/atlas.db] [--sources mathlib,wikidata,wikipedia,resolve] [--limit N] [--offline]`"""

from __future__ import annotations

import argparse
import json
import logging
import sys
from pathlib import Path

from ..store import Store
from . import SOURCES, run_ingest
from .common import DEFAULT_CACHE


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="python -m atlas.ingest")
    ap.add_argument("--db", default=str(DEFAULT_CACHE.parent / "atlas.db"))
    ap.add_argument("--sources", default=",".join(SOURCES))
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument("--offline", action="store_true")
    ap.add_argument("--cache", default=str(DEFAULT_CACHE))
    ap.add_argument("--workers", type=int, default=None)
    ap.add_argument("--report", default=None, help="write the JSON report here")
    ap.add_argument("--budget-min", type=float, default=None, help="stop Wikipedia requests after N minutes")
    args = ap.parse_args(argv)
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(name)s %(message)s", stream=sys.stderr)
    with Store(args.db) as store:
        out = run_ingest(store, [s for s in args.sources.split(",") if s], limit=args.limit, offline=args.offline,
                         cache_root=Path(args.cache), workers=args.workers,
                         budget_s=None if args.budget_min is None else args.budget_min * 60)
    text = json.dumps(out, ensure_ascii=False, indent=1)
    if args.report:
        Path(args.report).parent.mkdir(parents=True, exist_ok=True)
        Path(args.report).write_text(text, encoding="utf-8")
    sys.stdout.reconfigure(encoding="utf-8")
    print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
