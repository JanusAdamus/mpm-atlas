"""Re-record the offline HTTP fixtures (needs network; tests never run this).

    python tests/fixtures/ingest/record_fixtures.py

It runs the real pipeline online (requests already recorded are replayed) against the Mathlib fixture tree and a tiny set
of Wikidata seeds (FIXTURE_SEEDS), then copies the HTTP cache it produced into
tests/fixtures/ingest/cache/.  The offline tests replay exactly these requests.
Wikidata responses are CC0; Wikipedia responses are CC BY-SA 4.0 (attribution:
the article titles and revision ids inside each response; see README.md).
"""

from __future__ import annotations

import shutil
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[2]))
sys.path.insert(0, str(HERE.parents[1]))

from atlas import EntityType, Store  # noqa: E402
from atlas.ingest import run_ingest, wikidata  # noqa: E402

FIXTURE_SEEDS = (
    wikidata.Seed("fixture-statements", EntityType.STATEMENT, "VALUES ?item { wd:Q11518 wd:Q26708 }"),
    wikidata.Seed("fixture-areas", EntityType.AREA, "VALUES ?item { wd:Q12479 wd:Q8087 }"),
    wikidata.Seed("fixture-concepts", EntityType.CONCEPT, "VALUES ?item { wd:Q49008 wd:Q37555 }"),
)


def main() -> None:
    from test_ingest_mathlib import build_tarball

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        build_tarball(root)
        # keep recorded responses (the Wikidata answers drift); only requests not in the fixtures go online
        for name in ("wikidata", "wikipedia"):
            if (HERE / "cache" / name).exists():
                shutil.copytree(HERE / "cache" / name, root / name)
        wikidata.SEEDS = FIXTURE_SEEDS
        with Store() as store:
            out = run_ingest(store, cache_root=root, workers=1)
        print(out["coverage"])
        for name in ("wikidata", "wikipedia"):
            dest = HERE / "cache" / name
            shutil.rmtree(dest, ignore_errors=True)
            shutil.copytree(root / name, dest, dirs_exist_ok=True)


if __name__ == "__main__":
    main()
