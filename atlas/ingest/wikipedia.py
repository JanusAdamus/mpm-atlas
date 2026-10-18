"""English and Spanish Wikipedia adapter (CC BY-SA 4.0, attribution recorded).

Input: entities already in the store that carry a sitelink external id
(`enwiki` / `eswiki`, written by the Wikidata adapter).  For each article the
MediaWiki API gives, in one request per batch of 50 titles (`prop=cirrusdoc|info`),
the lead section as plain text, the article's links, its redirects and the current
revision id.

Politeness (https://www.mediawiki.org/wiki/Wikimedia_APIs/Rate_limits, read
2026-10-16): anonymous clients get 200 requests/min with a User-Agent that carries
contact info (e-mail or full URL, `ATLAS_CONTACT`) and 10/min without it, at most 3
concurrent.  `polite_settings()` picks request spacing and concurrency from that,
so a run without a contact is slow but never hammers the API with 429s.  Titles
are fetched in priority order (degree of the item in the store), so a run cut by
`budget_s` keeps the best-connected articles.  Titles whose item already has an
article of that language in the store go last (re-fetching them only adds links), so a
re-run extends the set; the HTTP cache makes repeated batches free.

Output:
- one `source_fragment` entity per article (lead extract as description); its
  provenance cites the exact revision, which is the CC BY-SA attribution;
- `<item> appears_in <article>` (extracted);
- `<article> cites <article>` for links between two articles of the same language
  that are both in the store (redirects followed), as `model_proposed`: a hyperlink is a relatedness candidate
  only, never a prerequisite or a dependency.
"""

from __future__ import annotations

import gzip
import json
import logging
import time
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from urllib.parse import quote

from ..model import EntityType, EvidenceState, LangString, Provenance, RelationType, Source, Entity, binary, now_iso, stable_id
from ..store import Store
from .common import IngestReport, Timer, Writer, batched, ensure_source
from .http import CachedClient, OfflineMiss, has_contact

LICENSE = "CC-BY-SA-4.0"
EXTRACTOR = "atlas.ingest.wikipedia"
ADAPTER_VERSION = "wikipedia/2"
TITLES_PER_REQUEST = 50
MAX_EXTRACT = 1500
# non-article namespaces that appear in `outgoing_link` (EN and ES names)
NAMESPACES = frozenset({"Wikipedia", "Category", "File", "Template", "Help", "Portal", "Draft", "Module", "MOS",
                        "Special", "Talk", "User", "Image", "WP", "Categoría", "Archivo", "Plantilla", "Ayuda",
                        "Anexo", "Portal", "Usuario", "Imagen", "Wikiproyecto", "Especial"})
WORKERS = 3  # Wikimedia: no more than 3 concurrent requests
INTERVAL_IDENTIFIED = 0.35  # s between request starts: ~170/min, under the 200/min bucket
INTERVAL_ANONYMOUS = 6.5  # ~9/min, under the 10/min bucket for a UA without contact info
LANGS = ("en", "es")
SKIP_TYPES = frozenset({EntityType.PERSON, EntityType.APPLICATION})
log = logging.getLogger("atlas.ingest.wikipedia")
LINK_NOTE = {"en": "Hyperlink between Wikipedia articles: a relatedness candidate only, never a prerequisite.",
             "es": "Enlace entre artículos de Wikipedia: solo candidato de relación, nunca un prerrequisito."}


def api_url(lang: str) -> str:
    return f"https://{lang}.wikipedia.org/w/api.php"


def source_id(lang: str) -> str:
    return f"src:wikipedia-{lang}"


def fragment_id(lang: str, title: str) -> str:
    return stable_id("wp", lang, title)


def page_url(lang: str, title: str, revid: int | None = None) -> str:
    base = f"https://{lang}.wikipedia.org/w/index.php?title={quote(title.replace(' ', '_'), safe='')}"
    return base + (f"&oldid={revid}" if revid else "")


@dataclass
class Page:
    title: str
    revid: int | None = None
    extract: str = ""
    links: list[str] = field(default_factory=list)
    redirects: list[str] = field(default_factory=list)
    fetched_at: str = ""
    missing: bool = False


def _accept(body: bytes) -> bool:
    try:
        err = json.loads(body).get("error", {})
    except ValueError:
        return False
    return err.get("code") not in ("maxlag", "ratelimited", "readonly")


def polite_settings(identified: bool | None = None) -> tuple[float, int]:
    """(min seconds between request starts, concurrent workers) for the Wikimedia API."""
    identified = has_contact() if identified is None else identified
    return (INTERVAL_IDENTIFIED, WORKERS) if identified else (INTERVAL_ANONYMOUS, 1)


def fetch_batch(client: CachedClient, lang: str, titles: list[str]) -> dict[str, Page]:
    """Pages keyed by the *requested* title (normalisation and redirects followed).

    One request per batch: `prop=cirrusdoc` returns the search-index document of each page,
    whose `opening_text` is the lead section in plain text and `outgoing_link` its article
    links, and `prop=info` the current revision id.  (wikipedia/1 used `prop=extracts|links`,
    which allows only 20 intro extracts per request: three requests per 50 titles.)"""
    base = {"action": "query", "format": "json", "formatversion": "2", "prop": "cirrusdoc|info",
            "cdincludes": "opening_text|outgoing_link|redirect", "redirects": "1", "maxlag": "5",
            "titles": "|".join(titles)}
    pages: dict[str, Page] = {}
    alias: dict[str, str] = {t: t for t in titles}
    cont: dict[str, str] = {}
    while True:
        resp = client.get(api_url(lang), {**base, **cont}, accept=_accept)
        data = resp.json()
        if "error" in data:
            raise RuntimeError(f"MediaWiki error: {data['error']}")
        q = data.get("query", {})
        for hop in q.get("normalized", []) + q.get("redirects", []):
            for t, final in alias.items():
                if final == hop["from"]:
                    alias[t] = hop["to"]
        _merge_pages(q, resp.fetched_at, pages)
        if "continue" not in data:
            break
        cont = {k: str(v) for k, v in data["continue"].items()}
    return {t: pages[f] for t, f in alias.items() if f in pages}


def _merge_pages(q: dict, fetched_at: str, pages: dict[str, Page]) -> None:
    """Fold one response's `query.pages` into `pages` (continuations add links to pages already seen).
    Reads wikipedia/2 (`cirrusdoc`) and wikipedia/1 (`extracts|links`) responses."""
    for p in q.get("pages", []):
        page = pages.setdefault(p["title"], Page(p["title"], fetched_at=fetched_at))
        page.missing = page.missing or bool(p.get("missing") or p.get("invalid"))
        page.revid = max(page.revid or 0, p.get("lastrevid") or 0) or None
        page.extract = page.extract or (p.get("extract") or "")
        page.links.extend(l["title"] for l in p.get("links", []) if l.get("ns") == 0)
        for doc in p.get("cirrusdoc", []):
            src = doc.get("source", {})
            page.extract = page.extract or (src.get("opening_text") or "")
            page.links.extend(_article_title(l) for l in src.get("outgoing_link", []) if _is_article(l))
            page.redirects.extend(r["title"] for r in src.get("redirect", []) if r.get("namespace") == 0)


def cached_pages(client: CachedClient, lang: str) -> dict[str, Page]:
    """Every page of `lang` in the HTTP cache, i.e. fetched by any earlier run (either response format).

    Lets a run write links from pages fetched before it to pages fetched now; without it a link
    between two runs' pages is lost unless its source page happens to be re-fetched."""
    prefix = api_url(lang) + "?"
    pages: dict[str, Page] = {}
    for path in sorted(client.cache_dir.glob("*/*.gz")):
        with gzip.open(path, "rb") as fh:
            header = json.loads(fh.readline())
            if not header.get("url", "").startswith(prefix):
                continue
            try:
                data = json.loads(fh.read())
            except ValueError:
                continue
        _merge_pages(data.get("query", {}), header["fetched_at"], pages)
    return pages


def _is_article(link: str) -> bool:
    """`outgoing_link` mixes namespaces ("Wikipedia:Good_articles*", "Category:..."); keep main-namespace links."""
    head, sep, _ = link.partition(":")
    return not (sep and head.replace("_", " ") in NAMESPACES) and not link.endswith("*")


def _article_title(link: str) -> str:
    return link.replace("_", " ")


def targets_from_store(store: Store) -> dict[str, dict[str, str]]:
    """lang -> {title: entity id} from the sitelink external ids in the store."""
    out: dict[str, dict[str, str]] = {lang: {} for lang in LANGS}
    for e in store.iter_entities():
        if e.type in SKIP_TYPES:
            continue
        for scheme, value in e.external_ids:
            lang = scheme[:-4] if scheme.endswith("wiki") else None
            if lang in out:
                out[lang].setdefault(value, e.id)
    return out


# relations left out of the priority degree: module imports never touch items, and this adapter's own
# output must not reorder the plan between runs (that would change batch membership and miss the cache)
DEGREE_EXCLUDED = frozenset({RelationType.DEPENDS_ON_LOGICALLY, RelationType.APPEARS_IN, RelationType.CITES})


def item_degrees(store: Store, items: set[str]) -> Counter:
    """Number of current claims touching each item, over the relations that set the priority."""
    deg: Counter = Counter()
    for e in store.iter_edges(relations=[r for r in RelationType if r not in DEGREE_EXCLUDED]):
        if e.source in items:
            deg[e.source] += 1
        if e.target in items:
            deg[e.target] += 1
    return deg


def prioritized(titles: dict[str, str], degree: Counter) -> list[str]:
    """Best-connected items first; ties broken by title so the order is deterministic."""
    return sorted(titles, key=lambda t: (-degree.get(titles[t], 0), t))


def plan_batches(titles: list[str], done: set[str] = frozenset()) -> list[list[str]]:
    """Batches in priority order, titles not yet in the store first; each batch is sorted so its
    URL (the cache key) is stable."""
    todo = [t for t in titles if t not in done]
    again = [t for t in titles if t in done]
    return [sorted(b) for part in (todo, again) for b in batched(part, TITLES_PER_REQUEST)]


def existing(store: Store, lang: str) -> tuple[set[str], dict[str, str]]:
    """(items that already have an article of `lang`, {article title: fragment id}) from earlier runs."""
    sid = source_id(lang)
    items = {c.subject for c in store.iter_claims(RelationType.APPEARS_IN)
             if c.provenance and c.provenance[0].source_id == sid}
    prefix = f"Wikipedia ({lang}): "
    frags = {e.labels[0].text[len(prefix):]: e.id for e in store.iter_entities(EntityType.SOURCE_FRAGMENT)
             if e.labels and e.labels[0].text.startswith(prefix)}
    return items, frags


@dataclass
class _LangState:
    lang: str
    items: dict[str, str]
    by_final: dict[str, str] = field(default_factory=dict)  # article title (or redirect) -> fragment id
    fetched: dict[str, Page] = field(default_factory=dict)  # pages with their links: this run + HTTP cache


def ingest(store: Store, *, client: CachedClient, limit: int | None = None,
           report: IngestReport | None = None, langs: tuple[str, ...] = LANGS,
           budget_s: float | None = None, workers: int = WORKERS) -> IngestReport:
    """`limit`: titles per language; `budget_s`: stop issuing requests after this many seconds
    (everything fetched so far is written, links included)."""
    report = report or IngestReport("wikipedia")
    deadline = None if budget_s is None else time.monotonic() + budget_s
    with Timer(report, "targets"):
        targets = targets_from_store(store)
        degree = item_degrees(store, {i for t in targets.values() for i in t.values()})
    writer = Writer(store, report, actor=EXTRACTOR)
    plans = {lang: prioritized(targets.get(lang, {}), degree)[:limit] for lang in langs}
    known: dict[str, tuple[set[str], dict[str, str]]] = {}
    for lang in langs:
        if plans[lang]:
            with Timer(report, f"{lang}:targets"):
                known[lang] = existing(store, lang)
    todo = {lang: sum(targets[lang][t] not in known[lang][0] for t in plans[lang]) if lang in known else 0
            for lang in langs}
    left = sum(todo.values())
    for lang in langs:
        titles = plans[lang]
        report.counters[f"{lang}:titles_requested"] = len(titles)
        # the budget is shared out in proportion to the titles each language has not fetched yet
        lang_deadline = None if deadline is None else (
            time.monotonic() + max(0.0, deadline - time.monotonic()) * todo[lang] / max(1, left))
        left -= todo[lang]
        if not titles:
            continue
        done_items, frags = known[lang]
        state = _LangState(lang, targets[lang], by_final=dict(frags))
        done = {t for t in titles if state.items[t] in done_items}
        report.counters[f"{lang}:titles_already_in_store"] = len(done)
        _register(store, lang)
        plan = plan_batches(titles, done)
        step = workers  # one batch per worker per round: a budget overshoots by at most one batch
        with ThreadPoolExecutor(max_workers=workers) as pool:
            for k, rnd in enumerate(batched(plan, step)):
                if lang_deadline is not None and time.monotonic() > lang_deadline:
                    report.counters[f"{lang}:titles_skipped_budget"] += sum(len(b) for b in plan[k * step:])
                    break
                with Timer(report, f"{lang}:fetch"):
                    got = list(pool.map(lambda b: (_fetch_or_skip(client, lang, b), b), rnd))
                with Timer(report, f"{lang}:write"):
                    for pages, batch in got:
                        if pages is None:  # offline and never fetched: counted, not written
                            report.counters[f"{lang}:titles_not_cached_offline"] += len(batch)
                            continue
                        _write_pages(store, writer, state, batch, pages, report)
                    writer.flush()
                if k % 10 == 0:
                    log.info("%s: %d/%d batches, %s", lang, min((k + 1) * step, len(plan)), len(plan), client.stats)
        with Timer(report, f"{lang}:cached_pages"):
            for title, page in cached_pages(client, lang).items():
                if title in state.by_final and title not in state.fetched and page.revid:
                    state.fetched[title] = page
                    report.counters[f"{lang}:pages_from_earlier_runs"] += 1
        with Timer(report, f"{lang}:write"):
            _write_links(writer, state, report)
            writer.flush()
    return report


def _fetch_or_skip(client: CachedClient, lang: str, batch: list[str]) -> dict[str, Page] | None:
    """Offline, a batch no earlier run fetched (e.g. cut by a time budget) is skipped instead of failing
    the whole replay; online, misses cannot happen and errors propagate."""
    try:
        return fetch_batch(client, lang, batch)
    except OfflineMiss:
        if not client.offline:
            raise
        return None


def _register(store: Store, lang: str) -> None:
    ensure_source(store, Source(source_id(lang), f"Wikipedia ({lang})", LICENSE, uri=f"https://{lang}.wikipedia.org/",
                                version="live API, revision per page in provenance", retrieved_at=now_iso(),
                                terms="Text by Wikipedia contributors under CC BY-SA 4.0. Reuse requires attribution "
                                      "(article title, revision URL, contributors via page history) and share-alike."))


def _prov(lang: str, locator: str, page: Page) -> tuple[Provenance, ...]:
    return (Provenance(source_id(lang), locator, EXTRACTOR, ADAPTER_VERSION, LICENSE, page.fetched_at),)


def _write_pages(store: Store, writer: Writer, st: _LangState, titles: list[str], pages: dict[str, Page],
                 report: IngestReport) -> None:
    lang = st.lang
    for t in titles:
        page = pages.get(t)
        if page is None or page.missing or not page.revid:
            report.counters[f"{lang}:pages_missing"] += 1
            continue
        fid = fragment_id(lang, page.title)
        prov = _prov(lang, page_url(lang, page.title, page.revid), page)
        if page.title not in st.fetched:
            text = " ".join(page.extract.split())[:MAX_EXTRACT]
            if page.title not in st.by_final:
                report.counters[f"{lang}:fragments"] += 1
                report.counters[f"{lang}:with_extract"] += bool(text)
            writer.entity(Entity(fid, EntityType.SOURCE_FRAGMENT,
                                 (LangString(f"Wikipedia ({lang}): {page.title}", lang),),
                                 descriptions=(LangString(text, lang),) if text else (), provenance=prov))
            st.fetched[page.title] = page
            st.by_final[page.title] = fid
            for r in page.redirects:
                st.by_final.setdefault(r, fid)
        item = st.items[t]
        writer.know(item, store.get_entity(item).type)
        if writer.claim(binary(stable_id("c", "wikipedia", lang, item, page.title), RelationType.APPEARS_IN,
                               item, fid, prov)):
            report.counters[f"{lang}:appears_in"] += 1


def _write_links(writer: Writer, st: _LangState, report: IngestReport) -> None:
    """Links from every known page (this run's and the HTTP cache's) to any article of the store;
    a link to an article outside the store is only counted."""
    lang = st.lang
    for fid in set(st.by_final.values()):
        writer.know(fid, EntityType.SOURCE_FRAGMENT)
    for title, page in st.fetched.items():
        fid = st.by_final[title]
        prov = _prov(lang, page_url(lang, title, page.revid) + "#links", page)
        for target in dict.fromkeys(page.links):
            tid = st.by_final.get(target)
            if tid is None:
                report.counters[f"{lang}:links_outside_set"] += 1
                continue
            if tid == fid:
                continue
            if writer.claim(binary(stable_id("c", "wikipedia", lang, "link", title, target), RelationType.CITES,
                                   fid, tid, prov, evidence_state=EvidenceState.MODEL_PROPOSED,
                                   explanation=(LangString(LINK_NOTE["en"], "en"), LangString(LINK_NOTE["es"], "es")))):
                report.counters[f"{lang}:cites_candidates"] += 1
