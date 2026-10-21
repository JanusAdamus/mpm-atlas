"""Mathlib topic lists `docs/undergrad.yaml` and `docs/overview.yaml` (Apache-2.0), r5 plan point 4.

Both files are in the Mathlib tarball already ingested (same commit, no network).  They nest three levels:
area › subarea › usual name: 'Lean.declaration'.  The adapter links the usual English name to the declaration
as an English ALIAS of the declaration entity (`entity_replaced`, previous kept), with one provenance row per
topic: locator `docs/<file>:<line>`, quote = the topic path.  Topics are a curriculum list, not a dependency:
no prerequisite and no area claim is derived from them.  Empty values ('' = not yet in Mathlib) and URLs are
counted, not written.  Names that do not resolve to an ingested declaration are counted.
"""

from __future__ import annotations

import json
import re
import tarfile
from dataclasses import dataclass, replace
from pathlib import Path

from ..model import LangString, Provenance, Source
from ..store import Store
from .common import DEFAULT_CACHE, IngestReport, ensure_source
from .mathlib import LICENSE, Snapshot, _tar_commit, decl_id

FILES = ("docs/undergrad.yaml", "docs/overview.yaml")
SOURCE_ID = "src:mathlib4-topics"
EXTRACTOR = "atlas.ingest.mathlib_topics"
VERSION = "mathlib-topics/1"
_KEY = re.compile(r"""^(?P<ind> *)(?P<key>"[^"]*"|[^:#][^:]*?)\s*:\s*(?P<val>.*?)\s*$""")
_QUOTED = re.compile(r"'((?:[^']|'')*)'|\"([^\"]*)\"")


@dataclass(frozen=True)
class Topic:
    path: tuple[str, ...]  # area, subarea, ..., usual name
    decl: str  # Lean name, '' when the topic is not formalized, or a URL
    line: int


def _strip_comment(v: str) -> str:
    out, quote = [], None
    for ch in v:
        if quote is None and ch == "#":
            break
        if ch in "'\"":
            quote = None if quote == ch else (quote or ch)
        out.append(ch)
    return "".join(out).strip()


def _values(v: str) -> list[str]:
    v = _strip_comment(v)
    if not v:
        return []
    found = [m[1].replace("''", "'") if m[1] is not None else m[2] for m in _QUOTED.finditer(v)]
    return found or [v]


def parse(text: str) -> list[Topic]:
    """Indentation reader for the three-level topic files (the stdlib has no YAML parser)."""
    stack: list[tuple[int, str]] = []
    topics: list[Topic] = []
    last: tuple[tuple[str, ...], int] | None = None  # leaf waiting for values on the next lines
    for n, raw in enumerate(text.splitlines(), 1):
        line = raw.rstrip()
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if line.lstrip()[0] in "'\"" and last is not None:  # continuation: value(s) on their own line
            topics.extend(Topic(last[0], d, n) for d in _values(line.strip()))
            continue
        m = _KEY.match(line)
        if m is None:
            continue
        ind, key, val = len(m["ind"]), m["key"].strip().strip('"'), m["val"]
        while stack and stack[-1][0] >= ind:
            stack.pop()
        path = tuple(k for _, k in stack) + (key,)
        values = _values(val)
        if values:
            topics.extend(Topic(path, d, n) for d in values)
            last = None
        else:
            stack.append((ind, key))
            last = (path, n)
    return [t for t in topics if len(t.path) >= 2]


def read_files(snap_path: Path) -> dict[str, str]:
    out = {}
    with tarfile.open(snap_path, "r:gz") as tf:
        for m in tf:
            rel = m.name.split("/", 1)[-1]
            if m.isfile() and rel in FILES:
                out[rel] = tf.extractfile(m).read().decode("utf-8")
    return out


def ingest(store: Store, snap: Snapshot, *, report: IngestReport | None = None) -> IngestReport:
    report = report or IngestReport("mathlib-topics")
    ensure_source(store, Source(SOURCE_ID, "Mathlib4 topic lists (docs/undergrad.yaml, docs/overview.yaml)", LICENSE,
                                uri="https://github.com/leanprover-community/mathlib4/tree/master/docs",
                                version=f"commit {snap.commit}", retrieved_at=snap.retrieved_at, kind="library",
                                terms="Apache-2.0, same repository and commit as src:mathlib4"))
    aliases: dict[str, list[tuple[str, Provenance]]] = {}
    for rel, text in read_files(snap.path).items():
        for t in parse(text):
            report.counters[f"{rel}:topics"] += 1
            if not t.decl:
                report.counters[f"{rel}:not_formalized"] += 1
                continue
            if t.decl.startswith("http"):
                report.counters[f"{rel}:url"] += 1
                continue
            name = re.sub(r"\$([^$]*)\$", r"\1", t.path[-1]).strip()
            prov = Provenance(SOURCE_ID, f"{rel}:{t.line}", EXTRACTOR, VERSION, LICENSE, snap.retrieved_at,
                              quote=" › ".join(t.path) + f": {t.decl}")
            aliases.setdefault(decl_id(t.decl), []).append((name, prov))
    changed = []
    for eid, items in sorted(aliases.items()):
        e = store.get_entity(eid, resolve=False)
        if e is None:
            report.counters["decl_not_in_registry"] += len(items)
            continue
        have = {(a.lang, a.text) for a in e.aliases} | {(l.lang, l.text) for l in e.labels}
        new = [LangString(n, "en") for n, _ in items if ("en", n) not in have]
        provs = [p for _, p in items if p not in e.provenance]
        if not new and not provs:
            report.counters["already_present"] += len(items)
            continue
        report.counters["aliases_added"] += len(dict.fromkeys(new))
        changed.append(replace(e, aliases=e.aliases + tuple(dict.fromkeys(new)), provenance=e.provenance + tuple(provs)))
    report.counters["declarations_linked"] = len(changed)
    if changed:
        store.add_entities(changed, actor=f"{EXTRACTOR}/{VERSION}", on_conflict="replace", batch_size=500)
    return report


def snapshot(cache_root: Path = DEFAULT_CACHE) -> Snapshot:
    cp = json.loads((cache_root / "checkpoints" / "mathlib.json").read_text(encoding="utf-8")).get("tarball")
    path = cache_root / "mathlib" / cp["file"]
    return Snapshot(path, _tar_commit(path), cp["toolchain"], cp["retrieved_at"])


def main() -> int:
    import sys

    with Store(DEFAULT_CACHE.parent / "atlas.db") as store:
        store._conn.execute("PRAGMA busy_timeout = 60000")
        rep = ingest(store, snapshot())
    sys.stdout.reconfigure(encoding="utf-8")
    print(json.dumps(rep.to_json(), ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

