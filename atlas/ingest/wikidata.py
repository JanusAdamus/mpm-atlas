"""Wikidata adapter (CC0) over the public SPARQL endpoint.

Two phases, both cached and resumable:
1. discovery: one query per seed returns item ids only.  Seeds whose single query
   would run past WDQS's 60 s budget are partitioned by subclass subtree (one
   query per direct subclass of the root, recursing into a subtree that still
   times out).
2. details: VALUES batches of `BATCH` items fetch ES/EN labels, aliases,
   descriptions, Wikipedia sitelinks, external ids, defining formulas and every
   truthy item-valued statement.  A batch that times out is split in half.

Every property is mapped to a SPEC relation through `PROPERTY_MAP` (version
`MAPPING_VERSION`, which is the claims' extractor_version).  Unmapped properties,
statements whose endpoints do not fit the relation, and objects outside the
ingested set are dropped and counted, never guessed.
"""

from __future__ import annotations

import json
import logging
import re
import threading
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Iterable, Mapping

from ..model import (
    MATH,
    WORLD,
    Entity,
    EntityType,
    LangString,
    Provenance,
    RelationType,
    Source,
    ValidationError,
    binary,
    parse_edtf,
    stable_id,
)
from ..store import Store
from .common import IngestReport, Timer, Writer, batched, endpoint_ok, ensure_source
from .http import CachedClient, HttpError, OfflineMiss, Response

ENDPOINT = "https://query.wikidata.org/sparql"
SOURCE_ID = "src:wikidata"
LICENSE = "CC0-1.0"
EXTRACTOR = "atlas.ingest.wikidata"
MAPPING_VERSION = "wd-map/3"  # v3 (ADR-0005 §7b-c): P31 -> instance_of, P460 -> said_equivalent_to
BATCH = 200
WORKERS = 4  # WDQS allows 5 concurrent queries per client
LANGS = ("en", "es")
WIKIS = {"https://en.wikipedia.org/": "enwiki", "https://es.wikipedia.org/": "eswiki"}
SITELINK = ("?sl schema:about ?item ; schema:isPartOf ?wiki . "
            "VALUES ?wiki { <https://en.wikipedia.org/> <https://es.wikipedia.org/> }")
QID_RE = re.compile(r"^Q\d+$")
E = EntityType
R = RelationType
log = logging.getLogger("atlas.ingest.wikidata")


@dataclass(frozen=True)
class Seed:
    name: str
    etype: EntityType
    pattern: str = ""  # SPARQL group pattern binding ?item
    root: str | None = None  # partitioned seed: subclass tree under this QID
    via_instance: bool = False  # partitioned: instances (P31/P279*) instead of subclasses (P279*)
    broad: bool = False  # leaky class tree: an item stays only if `in_scope` finds a math signal


# Order = priority when `limit` truncates, and type precedence (first seed wins).
SEEDS: tuple[Seed, ...] = (
    Seed("conjecture", E.CONJECTURE, "?item wdt:P31/wdt:P279* wd:Q319141 ."),
    Seed("theorem", E.STATEMENT, "?item wdt:P31/wdt:P279* wd:Q65943 ."),
    Seed("lemma", E.STATEMENT, "?item wdt:P31/wdt:P279* wd:Q207505 ."),
    Seed("axiom", E.STATEMENT, "?item wdt:P31/wdt:P279* wd:Q17736 ."),
    # not "proposition" (Q108163): its subtree holds hypotheses, incl. ~30k "hypothetical protein" items (2026-10-16)
    Seed("identity", E.STATEMENT, "?item wdt:P31/wdt:P279* wd:Q609647 . " + SITELINK),
    Seed("inequality", E.STATEMENT, "?item wdt:P31/wdt:P279* wd:Q28113351 . " + SITELINK),
    Seed("algorithm", E.ALGORITHM, "?item wdt:P31/wdt:P279* wd:Q8366 . " + SITELINK),
    Seed("model", E.MODEL, "{ ?item wdt:P279* wd:Q486902 } UNION { ?item wdt:P31/wdt:P279* wd:Q486902 } " + SITELINK,
         broad=True),
    Seed("area", E.AREA, "{ ?item wdt:P31/wdt:P279* wd:Q1936384 } UNION { ?item wdt:P279+ wd:Q1936384 } "
                         "UNION { ?item wdt:P3285 [] }"),
    Seed("math-reference-ids", E.CONCEPT, "{ ?item wdt:P2812 [] } UNION { ?item wdt:P4215 [] } UNION "
                                          "{ ?item wdt:P6781 ?pw FILTER(!STRSTARTS(?pw, \"Mathematician:\")) } UNION { ?item wdt:P7554 [] } UNION "
                                          "{ ?item wdt:P7726 [] } UNION { ?item wdt:P12888 [] } UNION "
                                          "{ ?item wdt:P14534 [] }"),
    Seed("studied-by-area", E.CONCEPT, "?a wdt:P31/wdt:P279* wd:Q1936384 . ?item wdt:P2579 ?a ."),
    Seed("math-object-subclasses", E.CONCEPT, root="Q246672", broad=True),
    Seed("math-concept-subclasses", E.CONCEPT, root="Q24034552", broad=True),
    Seed("math-object-instances", E.CONCEPT, root="Q246672", via_instance=True, broad=True),
    Seed("math-concept-instances", E.CONCEPT, root="Q24034552", via_instance=True, broad=True),
)
# Classes that only say "this is a theorem / a concept": P31/P279 to them is typing, not a relation.
META_CLASSES = frozenset({"Q65943", "Q207505", "Q17736", "Q108163", "Q319141", "Q1936384", "Q24034552", "Q246672",
                          "Q8366", "Q486902", "Q11538", "Q5", "Q170790", "Q151885", "Q4167836", "Q13442814",
                          "Q17444909", "Q16889133", "Q7184903", "Q11862829", "Q1047113", "Q395"})
EXTERNAL_IDS = {"P2812": "mathworld", "P4215": "nlab", "P6781": "proofwiki", "P7554": "eom", "P7726": "planetmath",
                "P12888": "metamath", "P3285": "msc", "P829": "oeis", "P1563": "mactutor"}
FORMULA_PID = "P2534"
MATHLIB_PID = "P14534"
PERSON_PIDS = frozenset({"P138", "P61", "P1318", "P737"})
APPLICATION_PIDS = frozenset({"P366", "P1535"})
# Wikimedia-internal pages (categories, lists, templates...) leak into class trees; never math entities.
WIKIMEDIA_META = frozenset({"Q4167836", "Q4167410", "Q11266439", "Q13406463", "Q17633526", "Q14204246",
                            "Q15184295", "Q11753321", "Q19887878"})
HUMAN = "Q5"
# An instance partition this large is ontology leakage, not mathematics: measured 2026-10-16, "topological
# space", "subset", "subclass" and "structure (model theory)" each hold 250k-330k sitelinked instances
# (places, organisms, ...).  Genuine named mathematical instances come in far smaller subtrees.
MAX_INSTANCE_PARTITION = 5000
# Math signal for items of broad seeds.  Measured 2026-10-16: a 20-item sample of "subclass of mathematical
# object" held diseases, calendar days and rail services, and following one P31/P279 hop to an item with a
# math id still let in coins, obelisks and engine parts ("gudgeon pin" P279 "cylinder").  So a broad item must
# carry the signal itself: a math reference id (ProofWiki "Mathematician:" listing pages excluded) or a math
# term in its own en/es description.
# ponytail: keyword heuristic; replace with a classifier trained on reviewed items once AtlasBench has labels.
MATH_ID_PIDS = frozenset({"P2812", "P4215", "P6781", "P7554", "P7726", "P12888", "P3285", "P829", "P14534"})
MATH_TERMS = re.compile(
    r"mathemat|matem[aá]tic|theorem|teorema|lemma|conjecture|conjetura|algebra|álgebra|geometr|topolog|polynomi"
    r"|polinomi|matri(x|ces|z)|equation|ecuaci[oó]n|integral|derivative|calculus|c[aá]lculo|manifold|variedad"
    r"|polyhedr|poliedr|polygon|pol[ií]gono|polytope|statistic|estad[ií]stic|probabilit|combinatori|graph theory"
    r"|teor[ií]a de grafos|number theory|teor[ií]a de n[uú]meros|natural number|n[uú]mero natural|integer"
    r"|function|funci[oó]n|vector|tensor|set theory|category theory|logic|l[oó]gica|curve|curva|group theory"
    r"|ring theory|field theory|lie group|differential|diferencial|numerical|num[eé]ric|algorithm|algoritmo"
    r"|optimization|optimizaci[oó]n|prime|primo|fraction|fracci[oó]n|symmetr|simetr|invariant|knot|nudo|axiom"
    r"|tiling|tesel|fractal|sequence|sucesi[oó]n|dimension|operator|operador|space of|espacio de",
    re.I)


@dataclass(frozen=True)
class PropRule:
    pid: str
    label: str
    relation: RelationType
    inverse: bool  # True: the Wikidata object is the claim's subject
    object_types: frozenset[EntityType] | None = None  # extra condition on the claim's object type
    note: str | None = None  # recorded as a claim assumption


AREA_ONLY = frozenset({E.AREA})
PERSON_ONLY = frozenset({E.PERSON})
MATH_NOT_AREA = MATH
PROPERTY_MAP: tuple[PropRule, ...] = (
    PropRule("P279", "subclass of", R.PART_OF_AREA, False, AREA_ONLY),
    PropRule("P279", "subclass of", R.SPECIAL_CASE_OF, False, MATH_NOT_AREA),
    PropRule("P31", "instance of", R.PART_OF_AREA, False, AREA_ONLY),
    PropRule("P31", "instance of", R.INSTANCE_OF, False, MATH_NOT_AREA),  # an example, not a subclass
    PropRule("P361", "part of", R.PART_OF_AREA, False, AREA_ONLY),
    PropRule("P1269", "facet of", R.PART_OF_AREA, False, AREA_ONLY),
    PropRule("P2579", "studied by", R.PART_OF_AREA, False, AREA_ONLY),
    PropRule("P2578", "is the study of", R.PART_OF_AREA, True, AREA_ONLY),
    PropRule("P138", "named after", R.NAMED_AFTER, False, PERSON_ONLY),
    PropRule("P61", "discoverer or inventor", R.AUTHORED_BY, False, PERSON_ONLY),
    PropRule("P1318", "proved by", R.AUTHORED_BY, False, PERSON_ONLY),
    PropRule("P7719", "generalization of", R.GENERALIZES, False),
    PropRule("P2283", "uses", R.PREREQUISITE_FOR, True, None,
             "Wikidata P2283 'uses' read as a study-order suggestion (used item first), not a logical dependency"),
    PropRule("P366", "has use", R.APPLIED_IN, False, WORLD),
    PropRule("P1535", "used by", R.APPLIED_IN, False, WORLD),
    PropRule("P460", "said to be the same as", R.SAID_EQUIVALENT_TO, False, None,
             "Wikidata P460 'said to be the same as': sameness is asserted, possibly uncertain or disputed"),
    PropRule("P737", "influenced by", R.INFLUENCED, True),
)
RULES_BY_PID: dict[str, tuple[PropRule, ...]] = {}
for _r in PROPERTY_MAP:
    RULES_BY_PID[_r.pid] = RULES_BY_PID.get(_r.pid, ()) + (_r,)


def entity_id(qid: str) -> str:
    return "wd:" + qid


# Relations whose claims got a new id in wd-map/3: the wd-map/2 claim with the old id (same statement,
# other relation) is superseded by atlas.ingest.wikidata_m2, and re-ingest must not collide with it.
V3_RELATIONS = frozenset({R.SAID_EQUIVALENT_TO, R.INSTANCE_OF})


def claim_id(qid: str, pid: str, obj: str, relation: RelationType) -> str:
    """Deterministic id of the claim for statement (qid, pid, obj) mapped to `relation`."""
    parts = ("c", "wikidata", qid, pid, obj) + ((relation.value,) if relation in V3_RELATIONS else ())
    return stable_id(*parts)


def _qid(uri: str) -> str:
    return uri.rsplit("/", 1)[-1]


# ---------------------------------------------------------------------- SPARQL client


class Sparql:
    """SPARQL SELECT with a process-wide cap of WORKERS queries in flight (WDQS etiquette)."""

    def __init__(self, client: CachedClient, max_in_flight: int = WORKERS) -> None:
        self.client = client
        self._slots = threading.BoundedSemaphore(max_in_flight)

    def select(self, query: str) -> tuple[list[dict], str]:
        with self._slots:
            resp: Response = self.client.post(ENDPOINT, {"query": query, "format": "json"},
                                              headers={"Accept": "application/sparql-results+json"},
                                              accept=_complete_json)
        data = json.loads(resp.body, strict=False)  # labels may hold raw control characters
        rows = [{k: v for k, v in b.items()} for b in data["results"]["bindings"]]  # type: ignore[index]
        return rows, resp.fetched_at


def _complete_json(body: bytes) -> bool:
    """WDQS can stream a 200 that is cut off when the query dies mid-way: never cache that."""
    try:
        json.loads(body, strict=False)
        return True
    except ValueError:
        return False


def _is_timeout(exc: Exception) -> bool:
    """Out of query budget: a 5xx TimeoutException, or a 200 whose JSON was cut off (rejected, status 200).
    Offline, a query missing from the cache counts as one: every completed answer is cached, so the
    online run that filled the cache timed out there and took the same fallback (split or skip)."""
    if isinstance(exc, OfflineMiss):
        return True
    return isinstance(exc, HttpError) and (b"TimeoutException" in exc.body or exc.status in (200, 500, 502, 503, 504))


# ---------------------------------------------------------------------- discovery


def discover(sparql: Sparql, seeds: Iterable[Seed], report: IngestReport,
             extra: Iterable[str] = ()) -> dict[str, tuple[EntityType, str]]:
    """qid -> (type, seed name); the first seed (in SEEDS order) that finds an item types it."""
    found: dict[str, tuple[EntityType, str]] = {}
    seeds = list(seeds)
    with ThreadPoolExecutor(max_workers=len(seeds) or 1) as pool:  # concurrency is capped inside Sparql
        results = list(pool.map(lambda sd: _seed_ids(sparql, sd, report), seeds))
    for seed, qids in zip(seeds, results):
        log.info("seed %s: %d items", seed.name, len(qids))
        new = 0
        for q in sorted(qids, key=_qnum):
            if q not in found and q not in META_CLASSES:
                found[q] = (seed.etype, seed.name)
                new += 1
        report.counters[f"seed:{seed.name}:found"] = len(qids)
        report.counters[f"seed:{seed.name}:new"] = new
    for q in extra:
        if QID_RE.match(q) and q not in found:
            found[q] = (E.CONCEPT, "mathlib-curated")
            report.counters["seed:mathlib-curated:new"] += 1
    return found


def _seed_ids(sparql: Sparql, seed: Seed, report: IngestReport) -> set[str]:
    try:
        return _partitioned(sparql, seed, report) if seed.root else _ids(sparql, seed.pattern)
    except (HttpError, OfflineMiss) as exc:
        if not _is_timeout(exc):
            raise
        report.note(f"seed {seed.name} skipped: query over the WDQS budget ({getattr(exc, "status", "not cached, offline")})")
        report.counters[f"seed:{seed.name}:failed_timeout"] = 1
        return set()


def _ids(sparql: Sparql, pattern: str) -> set[str]:
    rows, _ = sparql.select(f"SELECT DISTINCT ?item WHERE {{ {pattern} }}")
    return {_qid(r["item"]["value"]) for r in rows if QID_RE.match(_qid(r["item"]["value"]))}


def _partitioned(sparql: Sparql, seed: Seed, report: IngestReport, max_depth: int = 4) -> set[str]:
    """Items under `seed.root`, one query per direct subclass, in parallel; a subtree whose
    query still times out is replaced by its own direct subclasses (up to `max_depth`)."""
    path = "wdt:P31/wdt:P279*" if seed.via_instance else "wdt:P279*"
    direct = "wdt:P31" if seed.via_instance else "wdt:P279"
    out = _ids(sparql, f"?item {direct} wd:{seed.root} . {SITELINK}")
    frontier, depth, seen = [seed.root], 0, {seed.root}

    def subtree(child: str) -> tuple[str, set[str] | None]:
        try:
            return child, _ids(sparql, f"?item {path} wd:{child} . {SITELINK}")
        except (HttpError, OfflineMiss) as exc:
            if not _is_timeout(exc):
                raise
            log.info("partition %s/%s timed out; splitting", seed.name, child)
            return child, None

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        while frontier and depth < max_depth:
            children = sorted({c for node in frontier for c in _ids(sparql, f"?item wdt:P279 wd:{node} .")} - seen,
                              key=_qnum)
            seen |= set(children)
            nxt: list[str] = []
            for child, ids in pool.map(subtree, children):
                if ids is not None and seed.via_instance and len(ids) > MAX_INSTANCE_PARTITION:
                    report.counters[f"seed:{seed.name}:partitions_dropped_too_large"] += 1
                    report.counters[f"seed:{seed.name}:items_dropped_too_large"] += len(ids)
                    continue
                if ids is not None:
                    out |= ids
                    report.counters[f"seed:{seed.name}:partitions"] += 1
                    continue
                report.counters[f"seed:{seed.name}:split_on_timeout"] += 1
                direct_ids = _ids(sparql, f"?item {direct} wd:{child} . {SITELINK}")
                if seed.via_instance and len(direct_ids) > MAX_INSTANCE_PARTITION:
                    report.counters[f"seed:{seed.name}:items_dropped_too_large"] += len(direct_ids)
                else:
                    out |= direct_ids
                nxt.append(child)
            frontier, depth = nxt, depth + 1
    return out


def _qnum(q: str) -> int:
    return int(q[1:]) if QID_RE.match(q) else 0


# ---------------------------------------------------------------------- details


@dataclass
class ItemData:
    qid: str
    labels: dict[str, str]
    aliases: list[tuple[str, str]]
    descriptions: dict[str, str]
    sitelinks: dict[str, str]
    literals: list[tuple[str, str]]  # (pid, value)
    statements: list[tuple[str, str]]  # (pid, object qid)
    fetched_at: str


def _values(qids: Iterable[str]) -> str:
    return " ".join(f"wd:{q}" for q in qids)


def _terms_query(qids: list[str]) -> str:
    return (f"SELECT ?item ?kind ?v WHERE {{ VALUES ?item {{ {_values(qids)} }} "
            "{ ?item rdfs:label ?v . BIND(\"label\" AS ?kind) } UNION { ?item skos:altLabel ?v . BIND(\"alias\" AS ?kind) } "
            "UNION { ?item schema:description ?v . BIND(\"description\" AS ?kind) } "
            "FILTER(LANG(?v) IN (\"en\", \"es\")) }")


def _sitelinks_query(qids: list[str]) -> str:
    # kept out of the terms UNION: inside it WDQS plans the sitelink branch first and times out (measured 2026-10-16)
    return (f"SELECT ?item ?kind ?v WHERE {{ VALUES ?item {{ {_values(qids)} }} "
            "?sl schema:about ?item ; schema:isPartOf ?w ; schema:name ?v . "
            "FILTER(?w IN (<https://en.wikipedia.org/>, <https://es.wikipedia.org/>)) BIND(STR(?w) AS ?kind) }")


def _statements_query(qids: list[str]) -> str:
    return (f"SELECT ?item ?p ?o WHERE {{ VALUES ?item {{ {_values(qids)} }} ?item ?p ?o . "
            "FILTER(STRSTARTS(STR(?p), \"http://www.wikidata.org/prop/direct/P\")) "
            "FILTER(isIRI(?o) && STRSTARTS(STR(?o), \"http://www.wikidata.org/entity/Q\")) }")


def _literals_query(qids: list[str]) -> str:
    pids = " ".join(f"wdt:{p}" for p in [*EXTERNAL_IDS, FORMULA_PID, MATHLIB_PID])
    return f"SELECT ?item ?p ?v WHERE {{ VALUES ?item {{ {_values(qids)} }} VALUES ?p {{ {pids} }} ?item ?p ?v . }}"


def _run_split(fn: Callable[[list[str]], tuple[list[dict], str]], qids: list[str],
               report: IngestReport) -> tuple[list[dict], str]:
    try:
        return fn(qids)
    except (HttpError, OfflineMiss) as exc:
        if not _is_timeout(exc) or len(qids) <= 1:
            raise
        report.counters["details_split_on_timeout"] += 1
        mid = len(qids) // 2
        a, ta = _run_split(fn, qids[:mid], report)
        b, tb = _run_split(fn, qids[mid:], report)
        return a + b, max(ta, tb)


def fetch_details(sparql: Sparql, qids: list[str], report: IngestReport, *, with_statements: bool = True,
                  workers: int = WORKERS) -> dict[str, ItemData]:
    def one(batch: list[str]) -> list[ItemData]:
        terms, t1 = _run_split(lambda b: sparql.select(_terms_query(b)), batch, report)
        links, _ = _run_split(lambda b: sparql.select(_sitelinks_query(b)), batch, report)
        terms = terms + links
        lits, _ = _run_split(lambda b: sparql.select(_literals_query(b)), batch, report)
        stmts, t3 = (_run_split(lambda b: sparql.select(_statements_query(b)), batch, report)
                     if with_statements else ([], t1))
        items = {q: ItemData(q, {}, [], {}, {}, [], [], max(t1, t3)) for q in batch}
        for r in terms:
            it = items.get(_qid(r["item"]["value"]))
            if it is None:
                continue
            kind, v, lang = r["kind"]["value"], r["v"]["value"], r["v"].get("xml:lang", "")
            if kind == "label":
                it.labels[lang] = v
            elif kind == "alias":
                it.aliases.append((lang, v))
            elif kind == "description":
                it.descriptions[lang] = v
            elif kind in WIKIS:
                it.sitelinks[WIKIS[kind]] = v
        for r in lits:
            it = items.get(_qid(r["item"]["value"]))
            if it is not None:
                it.literals.append((_qid(r["p"]["value"]), r["v"]["value"]))
        for r in stmts:
            it = items.get(_qid(r["item"]["value"]))
            if it is not None:
                it.statements.append((_qid(r["p"]["value"]), _qid(r["o"]["value"])))
        return list(items.values())

    out: dict[str, ItemData] = {}
    batches = list(batched(qids, BATCH))
    with ThreadPoolExecutor(max_workers=workers) as pool:
        for k, items in enumerate(pool.map(one, batches), 1):
            out.update({i.qid: i for i in items})
            if k % 25 == 0 or k == len(batches):
                log.info("details: %d/%d batches", k, len(batches))
    return out


# ---------------------------------------------------------------------- build entities and claims


def _entity(it: ItemData, etype: EntityType) -> Entity | None:
    labels = tuple(LangString(it.labels[l].strip(), l) for l in LANGS if it.labels.get(l, "").strip())
    if not labels:
        return None
    seen: set[tuple[str, str]] = set()
    aliases = []
    for lang, text in sorted(it.aliases):
        key = (lang, text.strip().lower())
        if text.strip() and key not in seen and text.strip() != it.labels.get(lang):
            seen.add(key)
            aliases.append(LangString(text.strip(), lang))
    descr = tuple(LangString(it.descriptions[l].strip(), l) for l in LANGS if it.descriptions.get(l, "").strip())
    ext = [("wikidata", it.qid)] + [(w, t) for w, t in sorted(it.sitelinks.items())]
    ext += sorted({(EXTERNAL_IDS[p], v) for p, v in it.literals if p in EXTERNAL_IDS})
    formula = next((v for p, v in it.literals if p == FORMULA_PID), None)
    prov = (Provenance(SOURCE_ID, f"https://www.wikidata.org/wiki/{it.qid}", EXTRACTOR, MAPPING_VERSION, LICENSE,
                       it.fetched_at),)
    return Entity(entity_id(it.qid), etype, labels, tuple(aliases[:30]), descr, (), formula, tuple(ext), prov)


def math_signal(it: ItemData) -> str | None:
    """Why an item counts as mathematics on its own data ("id:<pid>" or "description"), or None."""
    for pid, value in it.literals:
        if pid in MATH_ID_PIDS and not (pid == "P6781" and value.startswith("Mathematician:")):
            return f"id:{pid}"
    if any(MATH_TERMS.search(d) for d in it.descriptions.values()):
        return "description"
    return None


def in_scope(items: dict[str, ItemData], seed_of: dict[str, str], broad: frozenset[str],
             report: IngestReport) -> dict[str, ItemData]:
    """Items of precise seeds pass; items of broad seeds need their own `math_signal` (see MATH_TERMS)."""
    keep: dict[str, ItemData] = {}
    for q, it in items.items():
        if seed_of[q] not in broad:
            keep[q] = it
            continue
        why = math_signal(it)
        if why is None:
            report.counters[f"seed:{seed_of[q]}:out_of_scope"] += 1
            continue
        report.counters[f"scope_signal:{why}"] += 1
        keep[q] = it
    report.counters["items_in_scope"] = len(keep)
    return keep


def scope_type(it: ItemData, seed_type: EntityType) -> EntityType | None:
    """Final type of a discovered item: None for Wikimedia-internal pages, PERSON for humans
    (reference-id seeds also find mathematicians), the seed's type otherwise."""
    classes = {o for p, o in it.statements if p == "P31"}
    if classes & WIKIMEDIA_META:
        return None
    if HUMAN in classes:
        return E.PERSON
    return seed_type


def map_statement(pid: str, subj_type: EntityType, obj_type: EntityType) -> tuple[PropRule | None, str]:
    """The first rule for `pid` whose endpoint types fit; (None, reason) otherwise."""
    rules = RULES_BY_PID.get(pid)
    if not rules:
        return None, "unmapped"
    for rule in rules:
        s, o = (obj_type, subj_type) if rule.inverse else (subj_type, obj_type)
        if rule.object_types is not None and o not in rule.object_types:
            continue
        if endpoint_ok(rule.relation, s, o):
            return rule, "mapped"
    return None, "endpoint_types"


def ingest(store: Store, *, client: CachedClient, limit: int | None = None, extra_qids: Iterable[str] = (),
           report: IngestReport | None = None, seeds: Iterable[Seed] | None = None) -> IngestReport:
    report = report or IngestReport("wikidata")
    seeds = tuple(SEEDS if seeds is None else seeds)
    sparql = Sparql(client)
    with Timer(report, "discovery"):
        found = discover(sparql, seeds, report, extra_qids)
    qids = list(found)
    if limit is not None:
        qids = qids[:limit]
    report.counters["items_selected"] = len(qids)
    with Timer(report, "details"):
        items = fetch_details(sparql, qids, report)
    items = in_scope(items, {q: found[q][1] for q in items}, frozenset(s.name for s in seeds if s.broad), report)
    with Timer(report, "closure"):
        closure, closure_types = _closure(sparql, items, report)
    ensure_source(store, Source(SOURCE_ID, "Wikidata", LICENSE, uri=ENDPOINT, version=f"WDQS live, {MAPPING_VERSION}",
                                retrieved_at=min((i.fetched_at for i in items.values()), default=None),
                                terms="Structured data under CC0 1.0; property mapping in atlas/ingest/wikidata.py"))
    writer = Writer(store, report, actor=EXTRACTOR)
    with Timer(report, "write"):
        types: dict[str, EntityType] = {}
        for q, it in list(items.items()) + list(closure.items()):
            etype = closure_types.get(q) if q in closure_types else scope_type(it, found[q][0])
            if q in items and etype is not found[q][0]:
                report.counters[f"items_retyped:{etype.value if etype else 'out_of_scope'}"] += 1
            e = _entity(it, etype) if etype else None
            if e is None:
                report.counters["items_skipped:no_en_es_label" if etype else "items_skipped:untyped"] += 1
                continue
            types[q] = etype
            writer.entity(e)
            report.counters[f"entities:{etype.value}"] += 1
        writer.flush_entities()
        _write_claims(store, writer, items, types, report)
        writer.flush()
    return report


def _closure(sparql: Sparql, items: dict[str, ItemData],
             report: IngestReport) -> tuple[dict[str, ItemData], dict[str, EntityType | None]]:
    """Fetch people (named after, discoverer, proved by, influenced by) and uses (has use, used by)
    that are objects of mapped statements but not in the seed set."""
    want_person: set[str] = set()
    want_app: set[str] = set()
    for it in items.values():
        for pid, o in it.statements:
            if o in items or o in META_CLASSES:
                continue
            if pid in PERSON_PIDS:
                want_person.add(o)
            elif pid in APPLICATION_PIDS:
                want_app.add(o)
    todo = sorted(want_person | want_app, key=_qnum)
    report.counters["closure_candidates"] = len(todo)
    if not todo:
        return {}, {}
    extra = fetch_details(sparql, todo, report)
    types: dict[str, EntityType | None] = {}
    for q, it in extra.items():
        if any(p == "P31" and o == "Q5" for p, o in it.statements):
            types[q] = E.PERSON
        elif q in want_app:
            types[q] = E.APPLICATION
        else:
            types[q] = None  # e.g. named after a place: out of scope
        report.counters[f"closure:{types[q] or 'dropped'}"] += 1
    return extra, types


def _write_claims(store: Store, writer: Writer, items: dict[str, ItemData], types: dict[str, EntityType],
                  report: IngestReport) -> None:
    for it in items.values():
        s_type = types.get(it.qid)
        if s_type is None:
            continue
        for pid, o in it.statements:
            if o in META_CLASSES and pid in ("P31", "P279"):
                report.counters["statements:typing"] += 1
                continue
            o_type = types.get(o)
            if pid not in RULES_BY_PID:
                report.counters[f"unmapped:{pid}"] += 1
                continue
            if o_type is None:
                report.counters[f"dropped_out_of_scope:{pid}"] += 1
                continue
            rule, why = map_statement(pid, s_type, o_type)
            if rule is None:
                report.counters[f"dropped_{why}:{pid}"] += 1
                continue
            subj, obj = (o, it.qid) if rule.inverse else (it.qid, o)
            if subj == obj:
                continue
            prov = (Provenance(SOURCE_ID, f"https://www.wikidata.org/wiki/{it.qid}#{pid}", EXTRACTOR,
                               MAPPING_VERSION, LICENSE, it.fetched_at),)
            ok = writer.claim(binary(claim_id(it.qid, pid, o, rule.relation), rule.relation, entity_id(subj),
                                     entity_id(obj), prov, assumptions=(rule.note,) if rule.note else ()))
            if ok:
                report.counters[f"mapped:{pid}->{rule.relation.value}"] += 1
        _mathlib_links(store, writer, it, report)


def _mathlib_links(store: Store, writer: Writer, it: ItemData, report: IngestReport) -> None:
    for pid, name in it.literals:
        if pid != MATHLIB_PID:
            continue
        targets = store.find_by_external_id("mathlib", name)
        if not targets:
            report.counters["mathlib_id:decl_not_in_store"] += 1
            continue
        decl = store.get_entity(targets[0])
        writer.know(decl.id, decl.type)
        prov = (Provenance(SOURCE_ID, f"https://www.wikidata.org/wiki/{it.qid}#{MATHLIB_PID}", EXTRACTOR,
                           MAPPING_VERSION, LICENSE, it.fetched_at),)
        if writer.claim(binary(stable_id("c", "wikidata", it.qid, MATHLIB_PID, name), R.FORMALIZED_AS,
                               entity_id(it.qid), decl.id, prov)):
            report.counters["mapped:P14534->formalized_as"] += 1


def mapping_table() -> list[dict]:
    """The declared property -> relation table (for docs and reports)."""
    return [{"pid": r.pid, "label": r.label, "relation": r.relation.value, "inverse": r.inverse,
             "object_types": sorted(t.value for t in r.object_types) if r.object_types else None,
             "note": r.note, "version": MAPPING_VERSION} for r in PROPERTY_MAP]


# ---------------------------------------------------------------------- dates (ADR-0005 §1)

CIRCA_QID, PRESUMABLY_QID = "Q5727902", "Q18122778"  # P1480 sourcing circumstances
JULIAN_QID, GREGORIAN_QID = "Q1985786", "Q1985727"
_WD_TIME = re.compile(r"^([+-])(\d{1,16})-(\d{2})-(\d{2})T")


@dataclass(frozen=True)
class WikidataDate:
    edtf: str  # ADR-0005 §1 subset, astronomical numbering
    calendar: str  # gregorian | julian | unknown; never converted
    raw: str  # the source value, for HistoricalDate.raw


def _astro_text(hist: int) -> str:
    """Historical year (no year 0) -> EDTF year in astronomical numbering."""
    y = hist + 1 if hist < 0 else hist
    if abs(y) > 9999:
        raise ValidationError(f"year {hist} is outside the 4-digit EDTF subset")
    return f"{y:04d}" if y >= 0 else f"-{-y:04d}"


def _wd_span(time: str, precision: int, channel: str) -> tuple[str, str]:
    """Earliest and latest EDTF endpoint (no qualifier) of one Wikidata time value."""
    m = _WD_TIME.match(time)
    if not m:
        raise ValidationError(f"not a Wikidata time value: {time!r}")
    sign, ys, mo, dd = m.groups()
    year = -int(ys) if sign == "-" else int(ys)
    if channel == "json":  # historical numbering: -300 is 300 BC, there is no year 0
        if year == 0:
            raise ValidationError(f"Wikidata JSON has no year 0: {time!r}")
        hist = year
    elif channel == "wdqs":  # NV: assumed astronomical (0 = 1 BC); the pilot checks it with Elements (Q172891)
        hist = year - 1 if year <= 0 else year
    else:
        raise ValidationError(f"channel must be 'json' or 'wdqs', got {channel!r}")
    if precision >= 9:
        text = _astro_text(hist)
        if precision >= 10 and mo != "00":
            text += f"-{mo}"
            if precision >= 11 and dd != "00":
                text += f"-{dd}"
        return text, text
    if precision == 8:  # decade: 1950s = 1950..1959; 300s BC = 309..300 BC
        d = abs(hist) // 10 * 10
        lo, hi = (max(d, 1), d + 9) if hist > 0 else (-(d + 9), -max(d, 1))
    else:  # century (7), millennium (6), ... : 3rd century BC = 300..201 BC; 19th century = 1801..1900
        span = 10 ** (9 - precision)
        k = max(1, -(-abs(hist) // span))
        lo, hi = ((k - 1) * span + 1, k * span) if hist > 0 else (-(k * span), -((k - 1) * span + 1))
    return _astro_text(lo), _astro_text(hi)


def edtf_from_wikidata(time: str, precision: int, calendar_qid: str | None = None,
                       qualifiers: Mapping[str, Any] | None = None, channel: str = "json") -> WikidataDate:
    """The one conversion of a Wikidata time value to the EDTF subset of ADR-0005 §1.

    `qualifiers`: optional {"P1480": qid or [qids], "P1319": (time, precision), "P1326": (time, precision)};
    circa -> "~", presumably -> "?", both -> "%"; P1319/P1326 are the interval ends.  Coarser than
    a year becomes an explicit interval (never X digits in negative years)."""
    q = dict(qualifiers or {})
    circ = q.get("P1480") or ()
    circ = {circ} if isinstance(circ, str) else set(circ)
    mark = {(True, False): "~", (False, True): "?", (True, True): "%"}.get(
        (CIRCA_QID in circ, PRESUMABLY_QID in circ), "")
    lo, hi = _wd_span(time, precision, channel)
    if "P1319" in q:
        lo = _wd_span(*q["P1319"], channel)[0]
    if "P1326" in q:
        hi = _wd_span(*q["P1326"], channel)[1]
    edtf = f"{lo}{mark}" if lo == hi else f"{lo}{mark}/{hi}{mark}"
    calendar = {JULIAN_QID: "julian", GREGORIAN_QID: "gregorian", None: "gregorian"}.get(calendar_qid, "unknown")
    raw = f"{time}/{precision}" + (f" {calendar_qid}" if calendar_qid else "")
    if q:
        raw += " " + json.dumps(q, sort_keys=True, default=list)
    parse_edtf(edtf, calendar)
    return WikidataDate(edtf, calendar, raw)


__all__ = ["ingest", "discover", "fetch_details", "map_statement", "mapping_table", "PROPERTY_MAP", "SEEDS",
           "MAPPING_VERSION", "entity_id", "claim_id", "V3_RELATIONS", "edtf_from_wikidata", "WikidataDate"]
