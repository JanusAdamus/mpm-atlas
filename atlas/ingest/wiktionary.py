"""Spanish names from the English Wiktionary's translation tables (r5, plan point 2).

Licence: the entries are CC BY-SA 4.0 + GFDL (siteinfo `rightsinfo` of en.wiktionary and
https://en.wiktionary.org/wiki/Wiktionary:Copyrights, both read 2026-10-19).  This adapter reads the
wikitext through the MediaWiki API (50 titles per request, cached, ≥1 s between requests, identified
User-Agent); it does not use a third-party extraction.

Wiktionary has no QIDs: the page is found by the item's English label, so a match is a string match.
To keep homonyms out, a translation is used only when the page's English section has exactly ONE
translation table whose gloss names a mathematical sense (`MATH_GLOSS`) and that table has Spanish
translations.  The result is never written as the item's own label: it is a `presentation_label`
curation (`curation:wiktionary-es/1`), reversible, with the revision URL, the gloss and the Spanish
line as provenance.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass

from ..model import CurationKind, Provenance, Source
from ..store import Store
from .common import IngestReport, batched, ensure_source
from .http import CachedClient
from .wikipedia import _accept  # rejects maxlag/ratelimited/readonly bodies: retried, never cached

API = "https://en.wiktionary.org/w/api.php"
SOURCE = Source("src:wiktionary-en", "English Wiktionary (translation tables)", "CC-BY-SA-4.0",
                uri="https://en.wiktionary.org/", version="live API, revision per page in provenance",
                terms="Wiktionary contributors; CC BY-SA 4.0 and GFDL. Attribution via the revision URL.",
                kind="encyclopedia")
LICENSE = "CC-BY-SA-4.0"
RULE, VERSION = "wiktionary-es", "2"  # 2: no bare "set"/"number"/"graph" in the gloss test
ACTOR = f"curation:{RULE}/{VERSION}"
TITLES = 50
MATH_GLOSS = re.compile(
    r"mathemat|geometr|algebra|topolog|arithmetic|number theory|set theory|graph theory|category theory|"
    r"\blogic\b|statistic|probabilit|calculus|analysis|matri|vector|polygon|polyhedr|group theory|ring theory|"
    r"\bfunction\b|equation|integer|polynomial|combinator|trigonometr|manifold|eigen|integra|differentia",
    re.I)
_LANG_HEAD = re.compile(r"^==([^=].*?)==\s*$", re.M)
_TRANS_TOP = re.compile(r"\{\{trans-top(?:-also)?\|(?:id=[^|}]*\|)?([^}]*)\}\}(.*?)\{\{trans-bottom\}\}", re.S)
_ES_LINE = re.compile(r"^\*\s*Spanish:(.*)$", re.M)
_ES_TERM = re.compile(r"\{\{tt?\+?\|es\|([^|}]+)")


@dataclass(frozen=True)
class Pick:
    text: str
    gloss: str
    line: str


def english_section(wikitext: str) -> str:
    heads = list(_LANG_HEAD.finditer(wikitext))
    for i, h in enumerate(heads):
        if h.group(1).strip() == "English":
            end = heads[i + 1].start() if i + 1 < len(heads) else len(wikitext)
            return wikitext[h.end():end]
    return ""


def spanish_from(wikitext: str) -> tuple[Pick | None, str]:
    """(pick, reason).  reason ∈ ok | no_math_table | ambiguous_math_tables | no_spanish."""
    tables = [(g.strip(), body) for g, body in _TRANS_TOP.findall(english_section(wikitext))]
    math = [(g, b) for g, b in tables if MATH_GLOSS.search(g)]
    if not math:
        return None, "no_math_table"
    with_es = [(g, m.group(1).strip()) for g, b in math if (m := _ES_LINE.search(b)) and _ES_TERM.search(m.group(1))]
    if len(math) > 1:
        return None, "ambiguous_math_tables"
    if not with_es:
        return None, "no_spanish"
    gloss, line = with_es[0]
    term = _ES_TERM.search(line).group(1).strip()  # first listed Spanish term
    term = re.sub(r"\[\[(?:[^|\]]*\|)?([^\]]*)\]\]", r"\1", term)
    return Pick(term, gloss, line[:300]), "ok"


def fetch(client: CachedClient, titles: list[str]) -> tuple[dict[str, tuple[str, int, str]], str]:
    """({requested title: (page title, revid, wikitext)}, fetched_at); redirects and normalisation followed."""
    resp = client.get(API, {"action": "query", "format": "json", "formatversion": "2", "prop": "revisions",
                            "rvprop": "content|ids", "rvslots": "main", "redirects": "1", "maxlag": "5",
                            "titles": "|".join(titles)}, accept=_accept)
    d = resp.json()
    if isinstance(d, dict) and "error" in d:  # an error body is not "no page"
        raise RuntimeError(f"Wiktionary API error {d['error'].get('code')}: {d['error'].get('info', '')[:200]}")
    q = d.get("query", {})  # type: ignore[union-attr]
    alias = {t: t for t in titles}
    for hop in q.get("normalized", []) + q.get("redirects", []):
        for t, final in alias.items():
            if final == hop["from"]:
                alias[t] = hop["to"]
    pages = {}
    for p in q.get("pages", []):
        revs = p.get("revisions") or []
        if revs:
            pages[p["title"]] = (p["title"], revs[0]["revid"], revs[0]["slots"]["main"].get("content", ""))
    return {t: pages[f] for t, f in alias.items() if f in pages}, resp.fetched_at


def targets(store: Store, ids: list[str] | None = None) -> dict[str, list[str]]:
    """English label -> entity ids, for Wikidata items without a Spanish label nor an active ES
    presentation label."""
    sql = ("SELECT id, json_extract(data,'$.labels.en') FROM entities WHERE id LIKE 'wd:Q%' AND merged_into IS NULL "
           "AND type != 'person' AND json_extract(data,'$.labels.es') IS NULL AND json_extract(data,'$.labels.en') IS NOT NULL")
    labelled = {r[0] for r in store._conn.execute(
        "SELECT target_id FROM curation WHERE kind='presentation_label' AND undone_at IS NULL "
        "AND json_extract(payload,'$.lang')='es'")}
    wanted = None if ids is None else set(ids)
    out: dict[str, list[str]] = {}
    for eid, en in store._conn.execute(sql):
        if eid in labelled or (wanted is not None and eid not in wanted) or len(en) > 80:
            continue
        out.setdefault(en, []).append(eid)
    return out


def ingest(store: Store, client: CachedClient, *, ids: list[str] | None = None,
           report: IngestReport | None = None) -> IngestReport:
    report = report or IngestReport("wiktionary-es")
    ensure_source(store, SOURCE)
    want = targets(store, ids)
    report.counters["labels_looked_up"] = len(want)
    for part in batched(sorted(want), TITLES):
        pages, fetched_at = fetch(client, part)
        for en in part:
            if en not in pages:
                report.counters["no_page"] += 1
                continue
            title, revid, text = pages[en]
            pick, why = spanish_from(text)
            report.counters[why] += 1
            if pick is None:
                continue
            if len(want[en]) > 1:  # two items share the English label: the page cannot tell them apart
                report.counters["ambiguous_items"] += 1
                continue
            prov = Provenance(SOURCE.id, f"https://en.wiktionary.org/w/index.php?title={title.replace(' ', '_')}"
                              f"&oldid={revid}#Translations", "curation:wiktionary-es", VERSION, LICENSE,
                              fetched_at, quote=f"{pick.gloss} — Spanish:{pick.line}")
            store.add_curation(CurationKind.PRESENTATION_LABEL, want[en][0], rule=RULE, rule_version=VERSION,
                               payload={"lang": "es", "text": pick.text, "provenance": dict(prov.__dict__)},
                               actor=ACTOR, reason="traducción de Wiktionary (única tabla con sentido matemático)")
            report.counters["labels_written"] += 1
    return report


def main() -> int:
    import sys

    from .common import DEFAULT_CACHE

    with Store(DEFAULT_CACHE.parent / "atlas.db") as store:
        store._conn.execute("PRAGMA busy_timeout = 60000")
        client = CachedClient(DEFAULT_CACHE / "wiktionary", offline="--offline" in sys.argv, min_interval=1.0)
        rep = ingest(store, client)
    rep.counters.update({f"http:{k}": v for k, v in client.stats.items()})
    sys.stdout.reconfigure(encoding="utf-8")
    print(json.dumps(rep.to_json(), ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

