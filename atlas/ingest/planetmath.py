"""PlanetMath (r5 plan point 4): entry metadata from the 64 per-MSC repositories of github.com/planetmath.

Licence: planetmath.org/about says CC BY-SA 2.5; every repository ships a LICENSE file with CC BY-SA 3.0
Unported (read 2026-10-19 in 03_Mathematical_logic_and_foundations@2599a6dc).  The provenance records the
repository licence, which is the one of the copy read, and the Source notes both.  The site is frozen
(entries dated ~2013), so every row carries the commit.

Read per entry (the LaTeX header up to `\\endmetadata`; the article text is not stored):
`\\pmcanonicalname`, `\\pmtitle`, `\\pmtype`, `\\pmclassification{msc}`, `\\pmsynonym`, `\\pmdefines`, `\\pmrelated`.

Written, only for entries whose canonical name is the PlanetMath id (Wikidata P7726) of an item already in the
registry (no matching by name, so no homonyms):
- one `source_fragment` `pm:<canonical>` per entry (title as label, the defined terms as aliases);
- `<item> appears_in <fragment>` (extracted);
- `\\pmsynonym` -> English aliases of the item (`entity_replaced`, previous kept);
- `\\pmrelated` between two written fragments -> `cites`, `model_proposed`: "related" is a see-also link,
  never a prerequisite nor a dependency (same reading as Wikipedia links).

Network: one GitHub API call (repository list) and one tarball per repository, ≥1 s apart, identified
User-Agent.  Only the filtered metadata is cached (`data/cache/planetmath/<repo>@<sha>.jsonl.gz`); offline
mode (tests) reads only that cache.
"""

from __future__ import annotations

import gzip
import io
import json
import re
import tarfile
import urllib.request
from dataclasses import dataclass, replace
from pathlib import Path

from ..model import EntityType, EvidenceState, LangString, Provenance, RelationType, Source, Entity, binary, stable_id
from ..store import Store
from .common import DEFAULT_CACHE, IngestReport, Writer, ensure_source
from .http import USER_AGENT, CachedClient

ORG = "planetmath"
SOURCE_ID = "src:planetmath"
LICENSE = "CC-BY-SA-3.0"
EXTRACTOR = "atlas.ingest.planetmath"
VERSION = "planetmath/1"
SKIP_REPOS = frozenset({"fem2016"})  # not an MSC section
_FIELD = re.compile(r"\\pm(canonicalname|title|type|classification|synonym|defines|related)\{([^}]*)\}(?:\{([^}]*)\})?")
RELATED_NOTE = {"en": "PlanetMath 'related' link: see-also only, never a prerequisite.",
                "es": "Enlace «relacionado» de PlanetMath: solo «véase también», nunca un prerrequisito."}


@dataclass(frozen=True)
class PMEntry:
    canonical: str
    title: str
    type: str
    msc: tuple[str, ...]
    synonyms: tuple[str, ...]
    defines: tuple[str, ...]
    related: tuple[str, ...]
    file: str

    @staticmethod
    def from_json(d: dict) -> PMEntry:
        return PMEntry(d["canonical"], d["title"], d["type"], tuple(d["msc"]), tuple(d["synonyms"]),
                       tuple(d["defines"]), tuple(d["related"]), d["file"])


def parse_header(text: str, file: str) -> PMEntry | None:
    head = text.split("\\endmetadata", 1)[0]
    head = "\n".join(l for l in head.splitlines() if not l.lstrip().startswith("%"))
    f: dict[str, list[str]] = {}
    for key, a, b in _FIELD.findall(head):
        value = b if key == "classification" else a
        if key == "classification" and a != "msc":
            continue
        if value.strip():
            f.setdefault(key, []).append(value.strip())
    if "canonicalname" not in f:
        return None
    return PMEntry(f["canonicalname"][0], (f.get("title") or f["canonicalname"])[0], (f.get("type") or [""])[0],
                   tuple(f.get("classification", ())), tuple(dict.fromkeys(f.get("synonym", ()))),
                   tuple(dict.fromkeys(f.get("defines", ()))), tuple(dict.fromkeys(f.get("related", ()))), file)


def _filtered_path(cache: Path, repo: str, sha: str) -> Path:
    return cache / f"{repo}@{sha}.jsonl.gz"


def fetch_repo(cache: Path, repo: str, branch: str, client: CachedClient) -> tuple[str, list[PMEntry]]:
    """(commit, entries) of one repository; the tarball is streamed, only the metadata is kept."""
    done = sorted(cache.glob(f"{repo}@*.jsonl.gz"))
    if done:
        return done[-1].name.split("@", 1)[1][:-9], load_filtered(done[-1])
    if client.offline:
        raise FileNotFoundError(f"offline and no filtered cache for {repo}")
    client._wait_slot()
    url = f"https://codeload.github.com/{ORG}/{repo}/tar.gz/refs/heads/{branch}"
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=600) as resp:
        data = resp.read()
    entries = []
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as tf:
        sha = None
        for m in tf:
            sha = sha or tf.pax_headers.get("comment")
            if m.isfile() and m.name.endswith(".tex"):
                e = parse_header(tf.extractfile(m).read().decode("utf-8", "replace"), m.name.split("/", 1)[-1])
                if e:
                    entries.append(e)
    sha = sha or "unknown"
    out = _filtered_path(cache, repo, sha)
    out.parent.mkdir(parents=True, exist_ok=True)
    tmp = out.with_suffix(".tmp")
    with gzip.open(tmp, "wt", encoding="utf-8") as fh:
        for e in entries:
            fh.write(json.dumps(e.__dict__, ensure_ascii=False) + "\n")
    tmp.replace(out)
    return sha, entries


def load_filtered(path: Path) -> list[PMEntry]:
    with gzip.open(path, "rt", encoding="utf-8") as fh:
        return [PMEntry.from_json(json.loads(l)) for l in fh if l.strip()]


def list_repos(client: CachedClient) -> list[tuple[str, str]]:
    r = client.get(f"https://api.github.com/orgs/{ORG}/repos", {"per_page": "100"},
                   headers={"Accept": "application/vnd.github+json"})
    return sorted((x["name"], x["default_branch"]) for x in r.json() if x["name"] not in SKIP_REPOS)  # type: ignore[union-attr]


def ingest(store: Store, client: CachedClient, *, cache: Path | None = None,
           report: IngestReport | None = None) -> IngestReport:
    report = report or IngestReport("planetmath")
    cache = cache or client.cache_dir
    ensure_source(store, Source(SOURCE_ID, "PlanetMath (entry metadata, github.com/planetmath)", LICENSE,
                                uri="https://github.com/planetmath", version="per-repository commit in provenance",
                                kind="encyclopedia",
                                terms="Repository LICENSE: CC BY-SA 3.0 Unported; planetmath.org/about: CC BY-SA 2.5. "
                                      "Attribution: PlanetMath authors (\\pmauthor), entry file at the commit."))
    items: dict[str, list[str]] = {}
    for v, eid in store._conn.execute("SELECT value, entity_id FROM external_ids WHERE scheme='planetmath'"):
        items.setdefault(v, []).append(eid)
    entries: dict[str, tuple[PMEntry, str, str]] = {}
    for repo, branch in list_repos(client):
        sha, got = fetch_repo(cache, repo, branch, client)
        report.counters["entries_read"] += len(got)
        for e in got:
            entries.setdefault(e.canonical, (e, repo, sha))
    writer = Writer(store, report, actor=f"{EXTRACTOR}/{VERSION}")
    frag_of: dict[str, str] = {}
    aliases: dict[str, list[tuple[str, Provenance]]] = {}
    for canon, (e, repo, sha) in sorted(entries.items()):
        targets = [store.resolve(i) for i in items.get(canon, [])]
        targets = list(dict.fromkeys(t for t in targets if (x := store.get_entity(t)) is not None
                                     and x.type is not EntityType.SOURCE_FRAGMENT))
        if not targets:
            report.counters["entries_without_item"] += 1
            continue
        if len(targets) > 1:
            report.counters["entries_with_several_items"] += 1
        loc = f"https://github.com/{ORG}/{repo}/blob/{sha}/{e.file}"
        prov = (Provenance(SOURCE_ID, loc, EXTRACTOR, VERSION, LICENSE, _retrieved(cache, repo, sha)),)
        fid = "pm:" + canon if re.fullmatch(r"[A-Za-z0-9_.\-]+", canon) else stable_id("pm", canon)
        writer.entity(Entity(fid, EntityType.SOURCE_FRAGMENT, (LangString(f"PlanetMath: {e.title}", "en"),),
                             aliases=tuple(LangString(d, "en") for d in e.defines),
                             descriptions=(LangString(f"PlanetMath {e.type or 'entry'}; MSC {', '.join(e.msc) or '—'}", "en"),),
                             provenance=prov))
        frag_of[canon] = fid
        for t in targets:
            writer.know(t, store.get_entity(t).type)
            if writer.claim(binary(stable_id("c", "planetmath", t, canon), RelationType.APPEARS_IN, t, fid, prov)):
                report.counters["appears_in"] += 1
            q = replace(prov[0], quote=f"\\pmsynonym: {', '.join(e.synonyms)}") if e.synonyms else None
            for s in e.synonyms:
                aliases.setdefault(t, []).append((s, q))
    writer.flush()
    for canon, fid in frag_of.items():
        e, repo, sha = entries[canon]
        prov = (Provenance(SOURCE_ID, f"https://github.com/{ORG}/{repo}/blob/{sha}/{e.file}#pmrelated", EXTRACTOR,
                           VERSION, LICENSE, _retrieved(cache, repo, sha)),)
        for r in e.related:
            tid = frag_of.get(r)
            if tid is None or tid == fid:
                report.counters["related_outside_set"] += tid is None
                continue
            if writer.claim(binary(stable_id("c", "planetmath", "related", canon, r), RelationType.CITES, fid, tid,
                                   prov, evidence_state=EvidenceState.MODEL_PROPOSED,
                                   explanation=(LangString(RELATED_NOTE["en"], "en"),
                                                LangString(RELATED_NOTE["es"], "es")))):
                report.counters["related_cites"] += 1
    writer.flush()
    _write_aliases(store, aliases, report)
    return report


def _write_aliases(store: Store, aliases: dict[str, list[tuple[str, Provenance]]], report: IngestReport) -> None:
    changed = []
    for eid, pairs in sorted(aliases.items()):
        e = store.get_entity(eid, resolve=False)
        have = {(a.lang, a.text.casefold()) for a in e.aliases} | {(l.lang, l.text.casefold()) for l in e.labels}
        new = [LangString(s, "en") for s, _ in pairs if ("en", s.casefold()) not in have]
        provs = [p for _, p in pairs if p not in e.provenance]
        if new or provs:
            report.counters["synonym_aliases"] += len(dict.fromkeys(new))
            changed.append(replace(e, aliases=e.aliases + tuple(dict.fromkeys(new)),
                                   provenance=e.provenance + tuple(dict.fromkeys(provs))))
    if changed:
        report.counters["items_with_new_aliases"] += store.add_entities(changed, actor=f"{EXTRACTOR}/{VERSION}",
                                                                        on_conflict="replace", batch_size=500)


def _retrieved(cache: Path, repo: str, sha: str) -> str:
    from datetime import datetime, timezone

    p = _filtered_path(cache, repo, sha)
    return datetime.fromtimestamp(p.stat().st_mtime, timezone.utc).isoformat(timespec="seconds")


def main() -> int:
    import sys

    with Store(DEFAULT_CACHE.parent / "atlas.db") as store:
        store._conn.execute("PRAGMA busy_timeout = 60000")
        client = CachedClient(DEFAULT_CACHE / "planetmath", offline="--offline" in sys.argv, min_interval=1.0)
        rep = ingest(store, client)
    sys.stdout.reconfigure(encoding="utf-8")
    print(json.dumps(rep.to_json(), ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
