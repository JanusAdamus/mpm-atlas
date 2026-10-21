"""Small canon (ADR-0005 orden 2c.1): Wikidata works of registry authors, the curated list of ancient
and non-European works and witnesses, and the CDLI mathematical tablets as `witness`.

Dates are read ONLY from the Wikidata JSON channel (wbgetentities) and converted with
`edtf_from_wikidata(..., channel="json")`: the pilot found that WDQS returns century-precision values
unshifted but year-precision values in astronomical numbering (docs/plans/r5/piloto-obras.md §1), so
the `wdqs` channel is not used for dates.

Ids: a Wikidata work is `wdw:Q…` (the works-layer prefix of shapes/works_layer.json) with the QID as an
external id.  When `wd:Q…` already exists in the core (the Wikidata adapter typed Elements, Plimpton
322… as `concept`) it is counted: retyping it is a contract decision, not an ingest one.
"""

from __future__ import annotations

import html
import json
import re
import time
from pathlib import Path
from typing import Any, Iterable

from ...model import (DateKind, Entity, EntityType, HistoricalDate, LangString, Provenance, RelationType, Source,
                      ValidationError, binary, stable_id)
from ...store import Store
from ..common import ensure_source
from ..http import CachedClient
from ..wikidata import edtf_from_wikidata
from .common import (ACTOR, CLAIM_PREFIX, assert_metadata_only, USER_AGENT, WORKS_CACHE, cache_path, claim, existing, prov, read_cache,
                     register_sources, run_batch, write_cache)

WD_API = "https://www.wikidata.org/w/api.php"
WDQS = "https://query.wikidata.org/sparql"
WDQS_SCHOLARLY = "https://query-scholarly.wikidata.org/sparql"
CDLI_SEARCH = "https://cdli.earth/search"
EXTRACTOR, VERSION = "atlas.ingest.works.canon", "works-canon/1"
RUN = f"{VERSION}@2026-10"
CURATED = Path(__file__).with_name("curated.json")
MIN_SITELINKS = 3
PRINTING = 1450  # a witness 'publication' date before this is its copy date
WITNESS_CLASSES = frozenset({"Q87167", "Q1570005", "Q125576", "Q1820389"})  # manuscript, clay tablet, papyrus
R = RelationType

MACTUTOR = Source("src:mactutor", "MacTutor History of Mathematics", "CC-BY-SA-4.0",
                  uri="https://mathshistory.st-andrews.ac.uk/", version="live, read 2026-10-19", kind="historiography",
                  terms="'Except where otherwise noted, the text of MacTutor by JOC / EFR is licensed under a Creative "
                        "Commons Attribution-ShareAlike 4.0 International License' (Miscellaneous/copyright, "
                        "read 2026-10-19).  Only short quotes justifying a claim are stored.")


def _client(cache: str, interval: float = 1.0) -> CachedClient:
    # Wikidata/CDLI answers carry descriptions and transliterations: never cached raw (use_cache=False).
    return CachedClient(WORKS_CACHE / cache / "_http", min_interval=interval, user_agent=USER_AGENT, timeout=120)


def registry_persons(store: Store) -> dict[str, str]:
    """QID -> entity id of every person with a Wikidata id."""
    rows = store._conn.execute(  # noqa: SLF001
        "SELECT x.value, x.entity_id FROM external_ids x JOIN entities e ON e.id = x.entity_id "
        "WHERE x.scheme='wikidata' AND e.type='person' AND e.merged_into IS NULL ORDER BY x.value")
    return dict(rows)


def _sparql(client: CachedClient, endpoint: str, query: str) -> list[dict]:
    resp = client.post(endpoint, {"query": query, "format": "json"},
                       headers={"Accept": "application/sparql-results+json"}, use_cache=False)
    return json.loads(resp.body, strict=False)["results"]["bindings"]


def canon_qids(client: CachedClient, persons: Iterable[str], chunk: int = 400) -> list[str]:
    qids = sorted(persons)
    found: set[str] = set()
    for i in range(0, len(qids), chunk):
        vals = " ".join(f"wd:{q}" for q in qids[i:i + chunk])
        rows = _sparql(client, WDQS, f"SELECT DISTINCT ?w WHERE {{ VALUES ?a {{ {vals} }} ?w wdt:P50 ?a . "
                                     f"?w wikibase:sitelinks ?n . FILTER(?n >= {MIN_SITELINKS}) }}")
        found.update(r["w"]["value"].rsplit("/", 1)[-1] for r in rows)
    return sorted(found)


def scholarly_count(client: CachedClient, persons: Iterable[str], chunk: int = 400) -> int:
    """Articles in the WDQS scholarly graph with P50 = a registry person (what the canon leaves out)."""
    qids, n = sorted(persons), 0
    for i in range(0, len(qids), chunk):
        vals = " ".join(f"wd:{q}" for q in qids[i:i + chunk])
        rows = _sparql(client, WDQS_SCHOLARLY,
                       f"SELECT (COUNT(DISTINCT ?w) AS ?n) WHERE {{ VALUES ?a {{ {vals} }} ?w wdt:P50 ?a }}")
        n += int(rows[0]["n"]["value"])
    return n


# ---------------------------------------------------------------------- Wikidata JSON (dates channel)

PROPS = ("P31", "P50", "P98", "P655", "P629", "P571", "P577", "P921", "P356", "P894", "P2474")


def _val(snak: dict) -> Any:
    return (snak.get("datavalue") or {}).get("value")


def _filter_item(q: str, e: dict, fetched_at: str) -> dict[str, Any]:
    claims = e.get("claims") or {}
    out: dict[str, Any] = {"qid": q, "labels": {l: v["value"] for l, v in (e.get("labels") or {}).items()
                                                if l in ("en", "es")},
                           "sitelinks": len(e.get("sitelinks") or {}), "retrieved_at": fetched_at}
    for p in PROPS:
        vals = []
        for s in claims.get(p, ()):
            if s.get("rank") == "deprecated":
                continue
            v = _val(s["mainsnak"])
            if v is None:
                continue
            if isinstance(v, dict) and "time" in v:
                quals = {}
                for qp, qs in (s.get("qualifiers") or {}).items():
                    qv = [_val(x) for x in qs if _val(x) is not None]
                    if qp == "P1480":
                        quals[qp] = [x["id"] for x in qv]
                    elif qp in ("P1319", "P1326") and qv:
                        quals[qp] = [qv[0]["time"], qv[0]["precision"]]
                vals.append({"time": v["time"], "precision": v["precision"],
                             "calendar": v["calendarmodel"].rsplit("/", 1)[-1], "qualifiers": quals})
            else:
                vals.append(v["id"] if isinstance(v, dict) and "id" in v else v)
        if vals:
            out[p] = vals
    return out


def fetch_items(name: str, qids: list[str], chunk: int = 50) -> Path:
    path = cache_path("wikidata", name)
    if path.exists():
        return path
    client, rows = _client("wikidata", 0.5), []
    for i in range(0, len(qids), chunk):
        resp = client.get(WD_API, {"action": "wbgetentities", "ids": "|".join(qids[i:i + chunk]), "format": "json",
                                   "props": "labels|claims|sitelinks", "languages": "en|es"}, use_cache=False)
        for q, e in json.loads(resp.body)["entities"].items():
            if "missing" not in e and e.get("id") == q:  # a redirect answers under its target id
                rows.append(_filter_item(q, e, resp.fetched_at))
    write_cache(path, rows)
    return path


def wd_dates(it: dict[str, Any], p: Provenance, *, witness: bool) -> tuple[list[HistoricalDate], int]:
    """(dates, flagged).  P571 = composition, P577 = publication.  On a witness, P571 is the copy date and
    P577 is a copy date only before printing (YBC 7289 has P577 = 1945, its first publication)."""
    out, flagged = [], 0
    for pid, kind in (("P571", DateKind.COMPOSED), ("P577", DateKind.PUBLISHED)):
        for v in it.get(pid, ()):
            q = {k: (tuple(x) if k != "P1480" else x) for k, x in v["qualifiers"].items()}
            d = edtf_from_wikidata(v["time"], v["precision"], v["calendar"], q, channel="json")
            lo = HistoricalDate(kind, d.edtf, p, d.calendar).span[0]
            copied = witness and (pid == "P571" or (lo is not None and lo < PRINTING))
            out.append(HistoricalDate(DateKind.COPIED if copied else kind, d.edtf, p, d.calendar, d.raw))
            flagged += d.calendar == "unknown"
    return out, flagged


# ---------------------------------------------------------------------- canon + curated ingest


def _work_id(qid: str) -> str:
    """Every Wikidata work is `wdw:Q…` (works-layer prefix, shapes/works_layer.json); the QID is an external id."""
    return f"wdw:{qid}"


def ingest_canon(store: Store, items_path: Path, *, curated: dict[str, Any] | None = None,
                 key: str = "canon") -> dict[str, Any]:
    ensure_source(store, MACTUTOR)
    batch = read_cache(items_path)
    persons = registry_persons(store)
    roles = {i["qid"]: i for i in (curated or {}).get("items", ())}
    taken = dict(store._conn.execute(  # noqa: SLF001
        f"SELECT id, type FROM entities WHERE id IN ({','.join('?' * len(batch.rows))})",
        [f"wd:{r['qid']}" for r in batch.rows])) if batch.rows else {}
    ents: dict[str, Entity] = {}
    claims = []
    st: dict[str, int] = {"works": 0, "witnesses": 0, "editions": 0, "qid_also_core_entity": 0, "dates": 0,
                          "dates_flagged": 0, "authored_by": 0, "coauthor_not_in_registry": 0,
                          "p50_on_witness_not_authorship": 0, "edited_by": 0, "translated_by": 0, "appears_in": 0}
    work_of: dict[str, str] = {}
    for it in batch.rows:
        q = it["qid"]
        role = roles.get(q, {}).get("role")
        witness = role == "witness" or bool(WITNESS_CLASSES & set(it.get("P31", ())))
        etype = EntityType.WITNESS if witness else EntityType.PUBLICATION
        eid = _work_id(q)
        st["qid_also_core_entity"] += f"wd:{q}" in taken
        p = Provenance("src:wikidata", f"https://www.wikidata.org/wiki/{q}", EXTRACTOR, VERSION, "CC0-1.0",
                       it["retrieved_at"])
        dates, flagged = wd_dates(it, p, witness=witness)
        st["dates"] += len(dates)
        st["dates_flagged"] += flagged
        labels = tuple(LangString(t, l) for l, t in it["labels"].items()) or (LangString(roles.get(q, {}).get("label", q), "und"),)
        ids = [("wikidata", q)] + [("doi", d.lower()) for d in it.get("P356", ())] + \
              [("zbmath_doc_de", z) for z in it.get("P894", ())] + [("cdli", c) for c in it.get("P2474", ())]
        ents[eid] = Entity(eid, etype, labels, external_ids=tuple(ids), provenance=(p,), dates=tuple(dates))
        work_of[q] = eid
        st["witnesses" if witness else "works"] += 1
        st["editions"] += role == "edition"
        for a in it.get("P50", ()):
            if witness:
                st["p50_on_witness_not_authorship"] += 1
            elif a in persons:
                claims.append(claim(R.AUTHORED_BY, eid, persons[a], _at(p, "P50")))
                st["authored_by"] += 1
            else:
                st["coauthor_not_in_registry"] += 1
        for pid, rel, k in (("P98", R.EDITED_BY, "edited_by"), ("P655", R.TRANSLATED_BY, "translated_by")):
            for a in it.get(pid, ()):
                if a in persons and not witness:
                    claims.append(claim(rel, eid, persons[a], _at(p, pid)))
                    st[k] += 1
        for subj in it.get("P921", ()):
            if taken_type(store, f"wd:{subj}") not in (None, "person", "note"):
                claims.append(claim(R.APPEARS_IN, f"wd:{subj}", eid, _at(p, "P921")))
                st["appears_in"] += 1
    if curated:
        claims += _curated_claims(store, curated, work_of, persons, st)
    assert_metadata_only(ents.values())
    claims = list({c.id: c for c in claims}.values())  # a source may repeat a statement
    bid = store.ingest_batch(source_id="src:wikidata", run=RUN, batch_key=key, input_sha256=batch.sha256,
                             actor=ACTOR, entities=list(ents.values()), claims=claims, stats=st)
    return {"batch": bid, **st, "claims": len(claims)}


def _at(p: Provenance, pid: str) -> Provenance:
    return Provenance(p.source_id, f"{p.locator}#{pid}", p.extractor, p.extractor_version, p.license, p.retrieved_at)


def taken_type(store: Store, eid: str) -> str | None:
    row = store._conn.execute("SELECT type FROM entities WHERE id=?", (eid,)).fetchone()  # noqa: SLF001
    return row[0] if row else None


_WS = re.compile(r"\s+")


def page_text(url: str) -> tuple[str, str]:
    """Visible text of a page (whitespace-normalized) and its fetch time; used to check a quote."""
    resp = _client("mactutor", 1.0).get(url, use_cache=False)
    t = resp.body.decode("utf-8", "replace")
    t = re.sub(r"<script.*?</script>|<style.*?</style>", "", t, flags=re.S)
    t = html.unescape(re.sub(r"<[^>]+>", " ", t)).replace("Ⓣ", " ")
    return _WS.sub(" ", t), resp.fetched_at


def _curated_claims(store: Store, cur: dict[str, Any], work_of: dict[str, str], persons: dict[str, str],
                    st: dict[str, int]) -> list:
    """copied_by and introduced_in, each only if its quote is found verbatim on the cited page."""
    out, pages = [], {}
    for kind in ("copied_by", "introduced_in"):
        st[f"{kind}_emitted"] = st[f"{kind}_quote_not_found"] = 0
        for c in cur.get(kind, ()):
            if c["locator"] not in pages:
                pages[c["locator"]] = page_text(c["locator"])
            text, fetched = pages[c["locator"]]
            if _WS.sub(" ", c["quote"]) not in text:
                st[f"{kind}_quote_not_found"] += 1
                continue
            p = Provenance(MACTUTOR.id, c["locator"], EXTRACTOR, VERSION, MACTUTOR.license, fetched, quote=c["quote"])
            if kind == "copied_by":
                subj, obj = work_of.get(c["witness"]), persons.get(c["person"])
                rel = R.COPIED_BY
            else:
                subj, obj = f"wd:{c['subject']}", work_of.get(c["work"])
                rel = R.INTRODUCED_IN
                if taken_type(store, subj) is None:
                    subj = None
            if not subj or not obj:
                st[f"{kind}_endpoint_missing"] = st.get(f"{kind}_endpoint_missing", 0) + 1
                continue
            out.append(binary(stable_id(CLAIM_PREFIX, rel.value, subj, obj), rel, subj, obj, (p,),
                              validity_domain=c.get("validity_domain")))
            st[f"{kind}_emitted"] += 1
    return out


def edition_merges(store: Store, curated: dict[str, Any]) -> list[tuple[str, str, str]]:
    """(work, edition, evidence) pairs for Store.merge_many: an edition is merged into its work."""
    out = []
    for it in curated.get("items", ()):
        if it.get("role") != "edition":
            continue
        work = store.find_by_external_id("wikidata", it["edition_of"])
        ed = store.find_by_external_id("wikidata", it["qid"])
        work = [w for w in work if taken_type(store, w) == "publication"]
        ed = [e for e in ed if taken_type(store, e) == "publication"]
        if work and ed and work[0] != ed[0]:
            out.append((work[0], ed[0], json.dumps({"kind": "edition_of", "key": f"P629:{it['edition_of']}"})))
    return out


# ---------------------------------------------------------------------- CDLI


_RANGE = re.compile(r"(ca\.\s*)?(\d{1,4})\s*-\s*(\d{1,4})\s*BC")


def cdli_date(time_range: str | None) -> str | None:
    """'ca. 1900-1600 BC' -> '-1899~/-1599~' (BC years to astronomical: y BC = 1 - y)."""
    if not time_range:
        return None
    m = _RANGE.search(time_range)
    if not m:
        return None
    mark = "~" if m.group(1) else ""
    a, b = (1 - int(m.group(2)), 1 - int(m.group(3)))
    fmt = lambda y: f"-{-y:04d}" if y < 0 else f"{y:04d}"  # noqa: E731
    return f"{fmt(a)}{mark}/{fmt(b)}{mark}"


def fetch_cdli(genre: str = "Mathematical", limit: int = 100) -> dict[str, Any]:
    client, url, page, got = _client("cdli", 1.0), f"{CDLI_SEARCH}?genre={genre}&limit={limit}&page=1", 1, 0
    t0 = time.perf_counter()
    while url:
        path = cache_path("cdli", f"genre={genre};page={page:03d}")
        resp = client.get(url, headers={"Accept": "application/json"}, use_cache=False)
        rows = [{"id": a["id"], "designation": a.get("designation"), "museum_no": a.get("museum_no"),
                 "period": (a.get("period") or {}).get("period"),
                 "time_range": (a.get("period") or {}).get("time_range"),
                 "type": (a.get("artifact_type") or {}).get("artifact_type"), "retrieved_at": resp.fetched_at}
                for a in json.loads(resp.body)]
        write_cache(path, rows)
        got += len(rows)
        url, page = _next_link(client, url, page, len(rows), limit), page + 1
    return {"records": got, "pages": page - 1, "elapsed_s": round(time.perf_counter() - t0, 1)}


def _next_link(client: CachedClient, url: str, page: int, n: int, limit: int) -> str | None:
    # the Link header is not exposed by CachedClient; CDLI accepts plain page numbers too
    return None if n < limit else re.sub(r"page=\d+", f"page={page + 1}", url)


def ingest_cdli(store: Store, genre: str = "Mathematical") -> dict[str, Any]:
    register_sources(store, ["cdli"])
    out = {"batches": 0, "skipped_batches": 0, "witnesses": 0, "dates": 0, "no_date": 0, "merged_with_wikidata": 0}
    for path in sorted((WORKS_CACHE / "cdli").glob(f"genre={genre}_*.jsonl.gz")):
        batch = read_cache(path)
        ents, st = [], {"witnesses": 0, "dates": 0, "no_date": 0}
        for a in batch.rows:
            pid = f"P{int(a['id']):06d}"
            p = prov("cdli", f"https://cdli.earth/artifacts/{a['id']}", EXTRACTOR, VERSION, a["retrieved_at"])
            edtf = cdli_date(a["time_range"])
            dates = (HistoricalDate(DateKind.COPIED, edtf, p, raw=a["period"]),) if edtf else ()
            st["dates" if dates else "no_date"] += 1
            ids = [("cdli", pid)] + ([("museum", a["museum_no"])] if a["museum_no"] else [])
            ents.append(Entity(f"cdli:{pid}", EntityType.WITNESS, (LangString(a["designation"] or pid, "und"),),
                               external_ids=tuple(ids), provenance=(p,), dates=dates))
            st["witnesses"] += 1
        bid = run_batch(store, "cdli", RUN + ":cdli", path.name.removesuffix(".jsonl.gz"), batch, ents, [], st)
        if bid is None:  # already ingested: a date the parser learned since then replaces the entity (with history)
            out["skipped_batches"] += 1
            stored = {e.id: e for e in (store.get_entity(x.id, resolve=False) for x in ents) if e}
            redated = [e for e in ents if e.dates and e.id in stored and not stored[e.id].dates]
            out["redated"] = out.get("redated", 0) + store.add_entities(redated, actor=ACTOR, on_conflict="replace")
            continue
        out["batches"] += 1
        for k in st:
            out[k] += st[k]
    return out


def cdli_merges(store: Store) -> list[tuple[str, str, str]]:
    """(wikidata witness, cdli witness, evidence) for a CDLI id stated on a Wikidata witness (P2474)."""
    rows = store._conn.execute(  # noqa: SLF001
        "SELECT a.entity_id, b.entity_id, a.value FROM external_ids a JOIN external_ids b "
        "ON b.scheme='cdli' AND b.value=a.value AND b.entity_id GLOB 'cdli:*' "
        "JOIN entities ea ON ea.id=a.entity_id AND ea.type='witness' AND ea.merged_into IS NULL "
        "JOIN entities eb ON eb.id=b.entity_id AND eb.merged_into IS NULL "
        "WHERE a.scheme='cdli' AND a.entity_id NOT GLOB 'cdli:*'").fetchall()
    return [(w, c, f"cdli:{v}") for w, c, v in rows]


def load_curated() -> dict[str, Any]:
    return json.loads(CURATED.read_text(encoding="utf-8"))


__all__ = ["canon_qids", "scholarly_count", "fetch_items", "ingest_canon", "edition_merges", "fetch_cdli",
           "ingest_cdli", "cdli_merges", "cdli_date", "wd_dates", "load_curated", "registry_persons", "existing",
           "ValidationError", "claim", "prov"]


def fetch_person_bridges(store: Store, chunk: int = 400) -> Path:
    """zbMATH author id (P1556), OpenAlex id (P10283) and ORCID (P496) of the registry persons, from WDQS
    (identifiers only; no dates are read through this channel)."""
    path = cache_path("wikidata", "person-bridges")
    if path.exists():
        return path
    client, rows = _client("wikidata", 1.0), []
    qids = sorted(registry_persons(store))
    for i in range(0, len(qids), chunk):
        vals = " ".join(f"wd:{q}" for q in qids[i:i + chunk])
        for b in _sparql(client, WDQS, f"SELECT ?p ?prop ?v WHERE {{ VALUES ?p {{ {vals} }} "
                                       f"VALUES ?prop {{ wdt:P1556 wdt:P10283 wdt:P496 }} ?p ?prop ?v }}"):
            rows.append({"qid": b["p"]["value"].rsplit("/", 1)[-1], "prop": b["prop"]["value"].rsplit("/", 1)[-1],
                         "value": b["v"]["value"]})
    write_cache(path, rows)
    return path


def person_merges(store: Store) -> list[tuple[str, str, str]]:
    """(wd person, works-layer person, evidence) for exact identifier matches: P1556 = zbMATH author code,
    P10283 = OpenAlex author id, P496 = ORCID of an OpenAlex author."""
    persons = registry_persons(store)
    out, seen = [], set()
    for r in read_cache(cache_path("wikidata", "person-bridges")).rows:
        survivor = persons.get(r["qid"])
        cand = {"P1556": [f"zba:{r['value']}"], "P10283": [f"oaa:{r['value']}"]}.get(r["prop"])
        if r["prop"] == "P496":
            cand = [e for (e,) in store._conn.execute(  # noqa: SLF001
                "SELECT entity_id FROM external_ids WHERE scheme='orcid' AND value=? AND entity_id GLOB 'oaa:*'",
                (r["value"],))]
        for m in cand or ():
            row = store._conn.execute("SELECT merged_into FROM entities WHERE id=?", (m,)).fetchone()  # noqa: SLF001
            if survivor and row is not None and row[0] is None and m not in seen:
                seen.add(m)
                out.append((survivor, m, f"{r['prop']}:{r['value']}"))
    return out
