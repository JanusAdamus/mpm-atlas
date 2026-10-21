"""Spanish labels for Wikidata items that have none (r5, plan point 2; ADR-0005 migration M2).

Sources, all keyed by the item's QID (no string matching, so no homonyms):
- Wikidata labels `es` and its regional variants, and `mul` (the language-independent label that
  Wikidata itself shows to Spanish readers);
- Wikidata lexemes in Spanish (Q1321) whose sense points to the item with P5137 "item for this sense";
- a single Spanish alias, when there is exactly one.
Wikidata's main, Property and Lexeme namespaces are CC0 (https://www.wikidata.org/wiki/Wikidata:Copyright,
read 2026-10-19).

The label is written with `Store.add_entities(on_conflict="replace")`: one `entity_replaced` event per item,
whose payload keeps the previous data.  The label carries its own provenance row (locator = item or lexeme
URL, quote = the label).  One SPARQL query per `BATCH` items through the cached, rate-limited client.
"""

from __future__ import annotations

import json
import re
from dataclasses import replace

from ..model import Entity, LangString, Provenance
from ..store import Store
from .common import IngestReport, batched
from .http import CachedClient
from .wikidata import LICENSE, SOURCE_ID, Sparql, _run_split

EXTRACTOR = "atlas.ingest.labels_es"
VERSION = "labels-es/2"  # 2: a `mul` label made of plain English words is not a Spanish name
BATCH = 200
SPANISH = "Q1321"
ES_VARIANTS = ("es", "es-419", "es-formal", "es-mx", "es-ar", "es-cl", "es-co", "es-es")
PREFERENCE = ("label", "variant", "lexeme", "alias", "mul")  # first available wins


def terms_query(qids: list[str]) -> str:
    values = " ".join(f"wd:{q}" for q in qids)
    langs = ", ".join(f'"{l}"' for l in ES_VARIANTS + ("mul",))
    return (f"SELECT ?item ?kind ?v ?ref WHERE {{ VALUES ?item {{ {values} }} "
            f"{{ ?item rdfs:label ?v . FILTER(LANG(?v) IN ({langs})) BIND(\"label\" AS ?kind) }} "
            "UNION { ?item skos:altLabel ?v . FILTER(LANG(?v) = \"es\") BIND(\"alias\" AS ?kind) } "
            f"UNION {{ ?sense wdt:P5137 ?item . ?ref ontolex:sense ?sense ; dct:language wd:{SPANISH} ; "
            "wikibase:lemma ?v . BIND(\"lexeme\" AS ?kind) } }")


def candidates(rows: list[dict]) -> dict[str, dict[str, list[tuple[str, str]]]]:
    """qid -> kind -> [(text, locator)], kinds of PREFERENCE."""
    out: dict[str, dict[str, list[tuple[str, str]]]] = {}
    for r in rows:
        qid = r["item"]["value"].rsplit("/", 1)[-1]
        v = r["v"]["value"].strip()
        lang = r["v"].get("xml:lang", "")
        kind = r["kind"]["value"]
        if not v:
            continue
        if kind == "label":
            kind = "label" if lang == "es" else "mul" if lang == "mul" else "variant"
            loc = f"https://www.wikidata.org/wiki/{qid}#label-{lang}"
        elif kind == "lexeme":
            loc = r["ref"]["value"].replace("http://www.wikidata.org/entity/", "https://www.wikidata.org/wiki/Lexeme:")
        else:
            loc = f"https://www.wikidata.org/wiki/{qid}#aliases-es"
        bucket = out.setdefault(qid, {}).setdefault(kind, [])
        if all(t != v for t, _ in bucket):
            bucket.append((v, loc))
    return out


_PLAIN_WORD = re.compile(r"^[a-z]{4,}$")


def mul_is_name(text: str) -> bool:
    """`mul` should hold language-independent names (symbols, acronyms, proper names); some items carry an
    English phrase there ("Alternating polynomial"), which would show English as if it were Spanish."""
    return not any(_PLAIN_WORD.match(w) for w in re.split(r"[\s\-/]+", text))


def choose(found: dict[str, list[tuple[str, str]]]) -> tuple[str, str, str] | None:
    """(text, locator, kind) of the preferred Spanish label, or None.  Several lexemes or several aliases
    with different lemmas are ambiguous: not chosen (they still become aliases)."""
    for kind in PREFERENCE:
        opts = sorted(found.get(kind, []))
        if kind in ("lexeme", "alias") and len({t.casefold() for t, _ in opts}) > 1:
            continue
        if kind == "mul":
            opts = [o for o in opts if mul_is_name(o[0])]
        if opts:
            return opts[0][0], opts[0][1], kind
    return None


def with_spanish(e: Entity, found: dict[str, list[tuple[str, str]]], fetched_at: str) -> Entity | None:
    pick = choose(found)
    if pick is None or any(l.lang == "es" for l in e.labels):
        return None
    text, loc, kind = pick
    have = {(a.lang, a.text) for a in e.aliases}
    extra = [LangString(t, "es") for k in ("lexeme", "alias") for t, _ in found.get(k, [])
             if t != text and ("es", t) not in have]
    prov = Provenance(SOURCE_ID, loc, EXTRACTOR, VERSION, LICENSE, fetched_at, quote=f"{kind}: {text}")
    return replace(e, labels=e.labels + (LangString(text, "es"),), aliases=e.aliases + tuple(dict.fromkeys(extra)),
                   provenance=e.provenance + (prov,))


def revert_v1_english_mul(store: Store) -> int:
    """labels-es/1 accepted every `mul` label; put back the previous data of those that v2 rejects (a new
    `entity_replaced`, so the v1 version stays in the history)."""
    back = []
    for (eid,) in store._conn.execute(
            "SELECT DISTINCT target_id FROM events WHERE kind='entity_replaced' AND actor='atlas.ingest.labels_es/labels-es/1'"):
        e = store.get_entity(eid, resolve=False)
        prov = [p for p in e.provenance if p.extractor == EXTRACTOR and p.extractor_version == "labels-es/1"]
        if not prov or not prov[0].quote.startswith("mul: ") or mul_is_name(prov[0].quote[5:]):
            continue
        text = prov[0].quote[5:]
        back.append(replace(e, labels=tuple(l for l in e.labels if not (l.lang == "es" and l.text == text)),
                            provenance=tuple(p for p in e.provenance if p not in prov)))
    return store.add_entities(back, actor=f"{EXTRACTOR}/{VERSION}", on_conflict="replace") if back else 0


def missing_es(store: Store) -> list[str]:
    return [r[0] for r in store._conn.execute(
        "SELECT id FROM entities WHERE id LIKE 'wd:Q%' AND merged_into IS NULL AND type != 'person' "
        "AND json_extract(data,'$.labels.es') IS NULL ORDER BY id")]


def ingest(store: Store, client: CachedClient, *, ids: list[str] | None = None,
           report: IngestReport | None = None) -> IngestReport:
    report = report or IngestReport("labels-es")
    sparql = Sparql(client)
    ids = missing_es(store) if ids is None else ids
    report.counters["items_without_es"] = len(ids)
    for part in batched(ids, BATCH):
        qids = [i[3:] for i in part]
        rows, fetched_at = _run_split(lambda b: sparql.select(terms_query(b)), qids, report)
        found = candidates(rows)
        new = []
        for eid in part:
            e = store.get_entity(eid, resolve=False)
            got = with_spanish(e, found.get(eid[3:], {}), fetched_at) if e else None
            if got is not None:
                new.append(got)
                report.counters[f"label_from:{choose(found[eid[3:]])[2]}"] += 1
        report.counters["labels_written"] += store.add_entities(new, actor=f"{EXTRACTOR}/{VERSION}",
                                                                on_conflict="replace")
    return report


def main() -> int:
    import sys

    from .common import DEFAULT_CACHE

    with Store(DEFAULT_CACHE.parent / "atlas.db") as store:
        store._conn.execute("PRAGMA busy_timeout = 60000")
        client = CachedClient(DEFAULT_CACHE / "wikidata", offline="--offline" in sys.argv, min_interval=1.0,
                              max_reject_retries=0)
        reverted = revert_v1_english_mul(store)
        rep = ingest(store, client)
        rep.counters["reverted_v1_english_mul"] = reverted
    rep.counters.update({f"http:{k}": v for k, v in client.stats.items()})
    sys.stdout.reconfigure(encoding="utf-8")
    print(json.dumps(rep.to_json(), ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

