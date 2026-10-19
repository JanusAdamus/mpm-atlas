"""Concept projection: every Mathlib declaration -> concept(s), as reversible claims.

Plan: docs/plans/2026-10-17-capa-conceptual.md.  A content declaration gets up to `TOP_K` concepts,
each a `member_of_concept` claim (schema v2) with provenance (extractor `concept-projection`),
a confidence that says it is an uncalibrated score, and the evidence that produced it.

Signals, combined by max per (declaration, concept):

1. curated `formalized_as` links (Mathlib `@[wikidata]`, docs/1000.yaml, Wikidata P14534): `extracted`;
2. names and docstrings against Wikidata labels and aliases (ES/EN): bold phrases, first sentence,
   whole docstring and the declaration name (CamelCase split, also with the namespace read backwards:
   `Ideal.IsMaximal` -> "maximal ideal"); a label scores by where it matched and how specific it is
   (document frequency over all declarations); plus the item -> declaration candidates of
   resolve.py (`item_candidates`).  `model_proposed`;
3. the namespace head (`Nat.Prime.two_le` -> `Nat.Prime`) and the definitions the signature mentions
   (`depends_on_logically` premises), scaled down: `model_proposed`;
4. otherwise the Mathlib zone (module, or its directory when the file is `Basic`, `Defs`, ...): the
   Wikidata concept most of the zone's declarations point to (`model_proposed`, low confidence), else a
   *derived* concept for the zone (`concept:Mathlib/...`, labelled as derived), `extracted` from the
   module path.  Zones with fewer than `MIN_ZONE` such declarations fold into their parent directory.

Infrastructure declarations (`resource`: tactics, meta code) are left out on purpose: they stay
explicitly unassigned.  Nothing here overwrites source data: a run adds claims and derived entities;
claims of an older version that the new run does not produce are retracted, and `retract_all` undoes
the whole projection.  The concept layer of the snapshot (primary concept, members, aggregated
concept -> concept edges) is computed from these claims by `atlas compile` (core/atlas-core/src/concepts.rs).
"""

from __future__ import annotations

import hashlib
import json
import math
import re
from collections import Counter, defaultdict
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Iterable, Iterator, Mapping

from . import resolve as rv
from .ingest.lean import name_tokens
from .model import (
    Confidence,
    Entity,
    EntityType,
    EvidenceState,
    LangString,
    Provenance,
    RelationType,
    Role,
    Source,
    binary,
    now_iso,
    stable_id,
)
from .store import Store

EXTRACTOR = "concept-projection"
VERSION = "8"  # 2: homonym guard by domain, Wikidata areas as candidates; 3: Lean `Is` predicates;
# 4: retracted/superseded formalized_as links are not curated (v3 took the guard's own retractions as gold);
# 5: RESOLVE_FACTOR 0.8 -> 0.7 (dev half, v4 gold: top-1 150 -> 153 of 209, recall@3 0.806 -> 0.810);
# 6 (round 3, docs/plans/2026-10-18-ronda-3-correcciones.md): only mathematical items are candidates
# (resolve.non_math_reason); 1-2 character labels and name tokens give no signal; a one-token alias or core key
# read from the declaration name gives no signal; secondary concepts need a signal independent of the primary;
# 7: W_SOURCE name_rev 0.9 -> 0.8, first 0.7 -> 0.8 (dev half: top-1 154 -> 158 of 209, recall@3 167 -> 172 of 211;
# 14 configurations tried, neighbours 156-158); the zone fallback is labelled as such in the claim's explanation
# 8: resolve.py no longer matches 1-2 character labels ("M" -> mega, nautical mile, a.m.) and typographic
# characters are not concepts (the prime symbol took `IsLocalization.AtPrime`)
ACTOR = "atlas.concepts"
SOURCE_ID = "src:concept-projection"
DERIVED_PREFIX = "concept:"
TOP_K = 3
MAX_GRAM = 5
NAME_SOURCES = frozenset({"name_last", "name_rev", "name"})
# Tuned on the dev half of the curated links only (2026-10-17, trail in docs/STATUS.md); test is reported once.
W_SOURCE = {"name_last": 1.0, "name_rev": 0.8, "name": 0.6, "bold": 1.0, "first": 0.8, "doc": 0.4}
CORE_FACTOR = 0.85  # label matched without its generic words ("theorem", "set", ...)
ALIAS_FACTOR = 0.9  # the matched text is only an alias of the item, not one of its labels
SUBSUMED_FACTOR = 0.6  # a label found only as part of a longer label found in the same declaration
MIN_KEY_CHARS = 3  # labels/aliases (and their core keys) shorter than this are not matched ("N", "M set" -> "m")
MIN_NAME_TOKEN = 3  # name tokens shorter than this give no signal ("M", "X", "f"), unless rv.ABBREV expands them
NAME_ONE_TOKEN_ALIAS = False  # a one-token alias read from the name ("app", "desc", "prime") is a signal
NAME_ONE_TOKEN_CORE = False  # a one-token core key read from the name ("convex" of "convex function") is a signal
SECONDARY_INDEPENDENT = True  # rank 1-2 need a signal class the primary does not have
SECONDARY_PROPAGATED = False  # a namespace head's or signature premise's concept counts as an independent signal
UNTYPED_FACTOR = 0.75  # concept-like item on a statement, or theorem-like item on a definition
SPEC_POW = 0.5  # specificity (idf over declarations) exponent
RESOLVE_FACTOR = 0.7  # resolve.py item -> declaration score, same scale
RESOLVE_TOP_K = 10
NS_FACTOR = 0.4  # namespace head's concepts
DEP_FACTOR = 0.6  # signature premises' concepts (times the premise's specificity)
ASSIGN_FLOOR = 0.25  # below this a Wikidata candidate is not asserted; the zone decides
RELATIVE_FLOOR = 0.5  # 2nd/3rd concepts must reach this fraction of the first
DEDUP_LABELS = True  # skip a second item with the same English label
ZONE_SHARE = 0.3  # a zone maps to a Wikidata concept if it is top-1 of >= this share of its assigned decls
ZONE_MIN_VOTES = 3
ZONE_CONF = ASSIGN_FLOOR  # confidence of a zone vote = ZONE_CONF * share: always below the floor, visibly weak
MIN_ZONE = 5
ITEM_TYPES = rv.WD_TYPES + (EntityType.AREA,)  # concept candidates: Wikidata items and areas (plan: "teorías/áreas")
DEFINITIONAL = rv.DEFINITIONAL | {EntityType.AREA}
HEAD_NOUNS = frozenset({"set", "function", "number", "map", "element"})
GENERIC_FILES = frozenset({"Basic", "Defs", "Def", "Lemmas", "Instances", "Init", "Notation", "Prelude", "Results",
                           "Properties", "Operations", "Constructions", "Misc", "Aux", "API", "Facts", "Theorems",
                           "Main", "Core", "Order", "Lattice"})
BOLD_RE = re.compile(r"\*\*([^*]{2,120})\*\*|(?<![*\w])\*([^*\s][^*]{1,118})\*(?![*\w])")
FIRST_RE = re.compile(r"(?<=[.:;])\s")
ESTIMATES = "P(the declaration is about the concept)"
CALIBRATION = (f"not calibrated: heuristic score of {EXTRACTOR}/{VERSION}; top-1 precision and recall@3 against the "
               "curated test half are in docs/STATUS.md")
SAMPLE_SIZE = 200
SAMPLE_MIN_PER_STRATUM = 2

# Homonym guard (plan P0 E01/E06: "integral" must not take `IsIntegral`).  A one-word label match (or a
# resolve.py candidate of an item with a one-word label) is dropped when the declaration's Mathlib top-level
# directory is outside the item's domain: the directories AREA_DIRS gives its part_of_area ancestors.
# OPEN_DIRS are never guarded; items with no mapped area are not guarded.
# ponytail: hand table over ~60 Wikidata areas; a learned area -> directory model if the table misses homonyms.
_ALG = ("Algebra", "GroupTheory", "RingTheory", "FieldTheory", "LinearAlgebra", "RepresentationTheory")
_ARITH = _ALG + ("NumberTheory", "AlgebraicGeometry")
_CAT = ("CategoryTheory", "AlgebraicTopology", "Condensed", "AlgebraicGeometry")
_TOP = ("Topology", "AlgebraicTopology", "Analysis", "Geometry", "MeasureTheory", "Dynamics")
_AN = ("Analysis", "MeasureTheory", "Dynamics", "Topology", "Geometry", "Probability")  # no NumberTheory: IsIntegral
_GEO = ("Geometry", "LinearAlgebra", "Analysis", "Topology", "MeasureTheory", "AlgebraicGeometry")
_PROB = ("Probability", "MeasureTheory", "InformationTheory", "Analysis")
_LOGIC = ("ModelTheory", "SetTheory", "Computability")
_AREA_GROUPS: dict[tuple[str, ...], tuple[int, ...]] = {
    _ALG + ("CategoryTheory",): (159943, 1636734, 205464, 114705886, 874429, 82571, 2256206, 44337, 114722746,
                                 579978, 18694550, 1057968, 13220368, 211294, 173853, 173183),
    _ARITH: (3968, 1208658, 727659, 1455652, 43260, 190109, 12479, 49008, 613048, 11205, 180969, 114715748,
             114682712),
    _ALG + _GEO: (357300, 622679),
    _CAT + ("Topology",): (217413, 5758318, 9357062, 77827144),
    _TOP: (42989, 621550, 170058, 849798, 212803, 1046291, 7825032, 2721559, 1224402),
    _AN + ("InformationTheory",): (15614122,),
    _AN: (149972, 150008, 149999, 29175, 200802, 1339058, 190549, 1198874, 193756, 114728138, 867707, 774123,
          27058, 271977, 18205066, 28575007, 5164492, 7834072, 902618, 11216, 3174497, 1318181, 8084),
    _GEO + ("Combinatorics",): (8087, 162886, 2088417, 326905, 233858, 177409, 1783542, 193657, 906377, 747980,
                                188444, 10380344),
    _PROB: (12483, 5862903, 176737, 1308570, 745328, 177571),
    ("Combinatorics", "Computability"): (76592, 131476),
    _LOGIC: (12482, 1166618, 467606),
    ("Computability", "InformationTheory", "Combinatorics", "ModelTheory"): (21198,),
}
AREA_DIRS: dict[str, frozenset[str]] = {f"wd:Q{q}": frozenset(dirs) for dirs, qs in _AREA_GROUPS.items() for q in qs}
OPEN_DIRS = frozenset({"Data", "Order", "Logic", "Init", "Tactic", "Util", "Lean", "Control", "Deprecated", "Basic",
                       "Testing"})  # every domain's basics live here: never guarded
DOMAIN_DEPTH = 6


# ---------------------------------------------------------------------- data


@dataclass(frozen=True)
class Decl:
    id: str
    name: str
    type: EntityType
    doc: str
    module: str  # "Mathlib/Data/Nat/Prime/Defs"
    infra: bool


@dataclass(frozen=True)
class Item:
    id: str
    type: EntityType
    texts: tuple[str, ...]  # labels and aliases, ES and EN
    label_en: str
    n_labels: int = 0  # texts[:n_labels] are labels, the rest aliases


@dataclass(frozen=True)
class Assignment:
    decl: str
    concept: str
    score: float  # stored confidence (4 decimals)
    rank: int  # 0 = primary concept
    signal: str  # curated | name_doc | resolve | namespace | signature | zone | zone_derived
    evidence: str
    state: EvidenceState


@dataclass(frozen=True)
class Inputs:
    decls: tuple[Decl, ...]
    items: Mapping[str, Item]
    premises: Mapping[str, tuple[str, ...]]  # decl -> definitions its signature mentions
    curated: frozenset[tuple[str, str]]  # (item, decl), decl resolved through merges
    resolve_cands: Mapping[str, Mapping[str, float]]  # decl -> item -> resolve score
    domains: Mapping[str, frozenset[str]] | None = None  # item -> Mathlib top-level dirs it may take (homonym guard)
    excluded: Mapping[str, str] = field(default_factory=dict)  # Wikidata item -> why it is not a candidate concept


def top_dir(module: str) -> str:
    parts = module.split("/")
    return parts[1] if len(parts) > 1 and parts[0] == "Mathlib" else ""


def item_domains(up: Mapping[str, Iterable[str]], items: Iterable[str]) -> dict[str, frozenset[str]]:
    """item -> union of AREA_DIRS over its part_of_area ancestors; items with no mapped area are left out (not
    guarded).  special_case_of parents are not followed: on the dev half they pulled in wrong domains."""
    def ancestors(x: str) -> set[str]:
        seen, frontier = {x}, [x]
        for _ in range(DOMAIN_DEPTH):
            frontier = [o for f in frontier for o in up.get(f, ()) if o not in seen]
            seen.update(frontier)
        return seen

    out = {}
    for it in items:
        anc = ancestors(it)
        dirs = frozenset().union(*(AREA_DIRS[a] for a in anc if a in AREA_DIRS))
        if dirs:
            out[it] = dirs
    return out


def domain_ok(domains: Mapping[str, frozenset[str]] | None, item: str, module: str) -> bool:
    d = top_dir(module)
    return not domains or not d or d in OPEN_DIRS or item not in domains or d in domains[item]


def load(store: Store, *, with_resolve: bool = True) -> Inputs:
    decls = []
    for etype in (EntityType.STATEMENT, EntityType.DEFINITION, EntityType.RESOURCE):
        for e in store.iter_entities(etype):
            name = next((v for s, v in e.external_ids if s == "mathlib"), None)
            if name is None:
                continue
            loc = e.provenance[0].locator if e.provenance else ""
            module = loc.split(".lean", 1)[0] if ".lean" in loc else ""
            doc = next((d.text for d in e.descriptions if d.lang == "en"), "")
            decls.append(Decl(e.id, name, etype, doc, module, etype is EntityType.RESOURCE))
    curated_items = {item for item, _ in rv.curated_pairs(store)}
    items, excluded = {}, {}
    for etype in ITEM_TYPES:
        for e in store.iter_entities(etype):
            if e.id.startswith("wd:"):
                why = rv.entity_non_math_reason(e)
                if why is not None and e.id not in curated_items:  # a curated Mathlib link makes it mathematical
                    excluded[e.id] = why
                    continue
                labels = tuple(dict.fromkeys(l.text for l in e.labels))
                texts = labels + tuple(t for t in dict.fromkeys(a.text for a in e.aliases) if t not in labels)
                items[e.id] = Item(e.id, e.type, texts, e.label("en"), len(labels))
    ids = {d.id for d in decls}
    premises: dict[str, list[str]] = defaultdict(list)
    for edge in store.iter_edges(relations=[RelationType.DEPENDS_ON_LOGICALLY]):
        if edge.source in ids and edge.target in ids:
            premises[edge.source].append(edge.target)
    curated = frozenset((item, store.resolve(decl)) for item, decl in rv.curated_pairs(store) if item in items)
    up: dict[str, list[str]] = defaultdict(list)
    for edge in store.iter_edges(relations=[RelationType.PART_OF_AREA]):
        if edge.source.startswith("wd:") and edge.target.startswith("wd:"):
            up[edge.source].append(edge.target)
    cands: dict[str, dict[str, float]] = defaultdict(dict)
    if with_resolve:
        idx = rv.MathlibIndex.from_store(store)
        for item_id, etype, phrases_ in rv.wikidata_items(store):
            for c in rv.item_candidates(idx, item_id, etype, phrases_, top_k=RESOLVE_TOP_K):
                d = store.resolve(c.right)
                if item_id in items:
                    cands[d][item_id] = max(c.score, cands[d].get(item_id, 0.0))
    return Inputs(tuple(decls), items, {k: tuple(dict.fromkeys(v)) for k, v in premises.items()}, curated,
                  dict(cands), item_domains(up, items), excluded)


# ---------------------------------------------------------------------- names and docstrings


def _q(item_id: str) -> int:
    """Wikidata Q-number of `wd:Q<n>`; anything else sorts after every item."""
    tail = item_id[4:] if item_id.startswith("wd:Q") else ""
    return int(tail) if tail.isdigit() and tail.isascii() else 1 << 40


class LabelIndex:
    """Stemmed label tokens -> items.  `core` keys drop generic words and trailing head nouns.  `labelled`
    holds the (key, item) pairs that come from a label rather than only from an alias."""

    def __init__(self, items: Iterable[Item]) -> None:
        exact: dict[tuple[str, ...], set[str]] = defaultdict(set)
        core: dict[tuple[str, ...], set[str]] = defaultdict(set)
        labelled: set[tuple[tuple[str, ...], str]] = set()
        whole: set[tuple[tuple[str, ...], str]] = set()
        for it in items:
            for n, text in enumerate(it.texts):
                key = rv.content_tokens(text)
                if not key or len(key) > MAX_GRAM or len(rv.normalize(text).replace(" ", "")) < MIN_KEY_CHARS:
                    continue  # length of the label as written: "NE" is not "unequal" after rv.ABBREV
                exact[key].add(it.id)
                keys = {k for k in _core_keys(key) if len("".join(k)) >= MIN_KEY_CHARS}
                for c in keys:
                    core[c].add(it.id)
                if n < it.n_labels:
                    labelled.update((k, it.id) for k in keys | {key})
                    whole.add((key, it.id))
        self.exact = {k: tuple(sorted(v, key=_q)) for k, v in exact.items()}
        self.core = {k: tuple(sorted(v, key=_q)) for k, v in core.items() if k not in exact}
        self.labelled = frozenset(labelled)
        self.whole_labels = frozenset(whole)  # (key, item) of whole labels, without the core keys

    def lookup(self, gram: tuple[str, ...]) -> Iterator[tuple[str, tuple[str, ...]]]:
        if gram in self.exact:
            yield "exact", self.exact[gram]
        if gram in self.core:
            yield "core", self.core[gram]


def _core_keys(key: tuple[str, ...]) -> set[tuple[str, ...]]:
    out = set()
    c = tuple(t for t in key if t not in rv.GENERIC)
    if c and c != key:
        out.add(c)
    if len(c) > 1 and c[-1] in HEAD_NOUNS:
        out.add(c[:-1])
    return out


def _name_toks(text: str) -> tuple[str, ...]:
    return tuple(rv._stem(t) for t in name_tokens(text)
                 if t not in rv.STOP and t not in rv.NAME_STOP and (len(t) >= MIN_NAME_TOKEN or t in rv.ABBREV))


def phrases(d: Decl) -> Iterator[tuple[str, tuple[str, ...]]]:
    parts = d.name.split(".")
    yield "name_last", _name_toks(parts[-1])
    if len(parts) > 1:
        yield "name_rev", _name_toks(parts[-1]) + _name_toks(parts[-2])
    yield "name", _name_toks(d.name)
    if d.doc:
        for m in BOLD_RE.finditer(d.doc):
            yield "bold", rv.content_tokens(m.group(1) or m.group(2))
        yield "first", rv.content_tokens(FIRST_RE.split(d.doc, maxsplit=1)[0])
        yield "doc", rv.content_tokens(d.doc)


def predicate_adjectives(name: str) -> frozenset[str]:
    """Stems right after `is` in a Lean name (`IsIntegral`, `isOpen`): adjectives, so a one-word noun label read
    from the name ("integral") is a homonym; core keys ("open" of "open set") still match."""
    toks = name_tokens(name)
    return frozenset(rv._stem(b) for a, b in zip(toks, toks[1:]) if a == "is")


def _grams(toks: tuple[str, ...]) -> set[tuple[str, ...]]:
    return {toks[i:i + n] for n in range(1, min(MAX_GRAM, len(toks)) + 1) for i in range(len(toks) - n + 1)}


Found = dict[tuple[str, tuple[str, ...]], dict[str, float]]  # (kind, gram) -> source -> coverage of that phrase


def match_labels(d: Decl, idx: LabelIndex) -> Found:
    """Every label found in the name or docstring, with where it matched and how much of that phrase it covers."""
    found: Found = {}
    for src, toks in phrases(d):
        if not toks:
            continue
        for g in _grams(toks):
            for kind, _ in idx.lookup(g):
                cov = len(g) / len(toks)
                slot = found.setdefault((kind, g), {})
                if cov > slot.get(src, 0.0):
                    slot[src] = cov
    return found


def _hit_weight(kind: str, srcs: Mapping[str, float]) -> tuple[float, str]:
    """(positional weight, best source) of one matched label."""
    kf = 1.0 if kind == "exact" else CORE_FACTOR
    return max((W_SOURCE[s] * kf * (0.5 + 0.5 * cov), s) for s, cov in srcs.items())


# ---------------------------------------------------------------------- assignment


Scores = dict[str, tuple[float, str, str]]  # item -> (score, signal, evidence)
Support = dict[str, set[tuple]]  # item -> evidence units: ("phrase", tokens) or (signal,) for the other signals
Chosen = list[tuple[str, tuple[float, str, str]]]


def _offer(sc: Scores, item: str, score: float, signal: str, evidence: str, sup: Support | None = None) -> None:
    if sup is not None:
        sup.setdefault(item, set()).add((signal, item) if signal == "resolve" else (signal,))
    if score > sc.get(item, (0.0,))[0]:
        sc[item] = (score, signal, evidence)


def ranked(sc: Scores) -> Chosen:
    return sorted(sc.items(), key=lambda kv: (-kv[1][0], _q(kv[0]), kv[0]))


def namespace_heads(decls: Iterable[Decl]) -> dict[str, str]:
    by_name = {d.name: d.id for d in decls}
    heads = {}
    for name, did in by_name.items():
        parts = name.split(".")
        for k in range(len(parts) - 1, 0, -1):
            head = by_name.get(".".join(parts[:k]))
            if head is not None:
                heads[did] = head
                break
    return heads


def independent(sec: set[tuple], prim: set[tuple]) -> bool:
    """Is a secondary concept backed by evidence the primary does not share?  A matched phrase is independent
    unless it is one of the primary's phrases or part of one ("abel" behind both Abel's identity and Abel's
    theorem is one piece of evidence; "ideal" inside "prime ideal" too); a longer phrase is new evidence ("unit
    circle" next to "circle").  A curated link or a resolve.py candidate of the item itself is independent; the
    namespace head's and the signature's concepts are inherited, not evidence about this declaration (unless
    SECONDARY_PROPAGATED)."""
    prim_grams = {g for u in prim if u[0] == "phrase" for g in _grams(u[1])}
    for u in sec - prim:
        if u[0] == "phrase":
            if u[1] not in prim_grams:
                return True
        elif u[0] in ("curated", "resolve") or (SECONDARY_PROPAGATED and u[0] in ("namespace", "signature")):
            return True
    return False


class Scorer:
    """Signals 1-3 per declaration, memoised.  `hits` (the expensive label matching) can be passed in, so
    re-scoring (dev tuning, the masked evaluation run) does not re-match."""

    def __init__(self, inp: Inputs, *, use_curated: bool, hits: Mapping[str, Found] | None = None) -> None:
        self.inp = inp
        self.idx = LabelIndex(inp.items.values())
        self.content = {d.id: d for d in inp.decls if not d.infra}
        self.hits = hits if hits is not None else {i: match_labels(d, self.idx) for i, d in self.content.items()}
        self.df = Counter(k for f in self.hits.values() for k in f)
        self.log_n = math.log(max(2, len(self.content)))
        self.heads = namespace_heads(self.content.values())
        self.indeg = Counter(p for ps in inp.premises.values() for p in ps)
        self.curated: dict[str, set[str]] = defaultdict(set)
        if use_curated:
            for item, decl in inp.curated:
                self.curated[decl].add(item)
        self.label_key = {i: rv.normalize(it.label_en) for i, it in inp.items.items()}
        self.one_word = frozenset(i for k, its in self.idx.exact.items() if len(k) == 1 for i in its)
        self._direct: dict[str, tuple[Scores, Support]] = {}
        self.dropped: set[tuple[str, str]] = set()

    def spec(self, df: int) -> float:
        return math.log(len(self.content) / max(1, df)) / self.log_n

    def _name_ok(self, kind: str, g: tuple[str, ...], item: str, adjectives: frozenset[str]) -> bool:
        """A one-token match read from the declaration name counts only through a label of the item: a one-token
        alias ("app" of mobile app, "prime" of prime number) is a homonym more often than not, and so is a
        one-token core key ("convex" of convex function, "prime" of prime number in `AtPrime`) unless the name
        uses it as a predicate (`IsOpen` -> open set)."""
        if len(g) > 1:
            return True
        if kind == "core":
            return NAME_ONE_TOKEN_CORE or g[0] in adjectives
        return NAME_ONE_TOKEN_ALIAS or (g, item) in self.idx.whole_labels

    def direct(self, did: str) -> Scores:
        return self.direct_sup(did)[0]

    def direct_sup(self, did: str) -> tuple[Scores, Support]:
        """Signals 1 and 2, with the signal classes behind every item."""
        if did in self._direct:
            return self._direct[did]
        sc: Scores = {}
        sup: Support = {}
        d = self.content.get(did)
        if d is not None:
            definitional = d.type is EntityType.DEFINITION
            hits = self.hits.get(did, {})
            inside: set[tuple[str, ...]] = set()  # proper sub-grams of longer labels found here (longest match wins)
            for _, g in hits:
                if len(g) > 1:
                    inside.update(_grams(g) - {g})
            adjectives = predicate_adjectives(d.name)
            for (kind, g), srcs in hits.items():
                if kind == "exact" and len(g) == 1 and g[0] in adjectives:
                    srcs = {s: v for s, v in srcs.items() if s not in NAME_SOURCES}
                    if not srcs:
                        continue
                spec = max(0.0, self.spec(self.df[(kind, g)])) ** SPEC_POW
                for item in (self.idx.exact if kind == "exact" else self.idx.core)[g]:
                    if len(g) == 1 and not domain_ok(self.inp.domains, item, d.module):
                        continue  # one word out of the item's domain: a homonym ("integral" in RingTheory)
                    use = {s: v for s, v in srcs.items() if s not in NAME_SOURCES or self._name_ok(kind, g, item, adjectives)}
                    if not use:
                        continue
                    w, src = _hit_weight(kind, use)
                    if g in inside:
                        w *= SUBSUMED_FACTOR
                    typed = definitional == (self.inp.items[item].type in DEFINITIONAL)
                    via = 1.0 if (g, item) in self.idx.labelled else ALIAS_FACTOR
                    sup.setdefault(item, set()).add(("phrase", g))
                    _offer(sc, item, w * spec * via * (1.0 if typed else UNTYPED_FACTOR), "name_doc",
                           f"{src}:{kind}{'' if via == 1.0 else ':alias'} '{' '.join(g)}'")
            for item, s in self.inp.resolve_cands.get(did, {}).items():
                if item in self.one_word and not domain_ok(self.inp.domains, item, d.module):
                    continue
                _offer(sc, item, RESOLVE_FACTOR * s, "resolve", f"{rv.RESOLVER_VERSION} item_candidates", sup)
            for item in self.curated.get(did, ()):
                sc[item] = (1.0, "curated", "formalized_as (curated link)")
                sup.setdefault(item, set()).add(("curated", item))
        self._direct[did] = (sc, sup)
        return sc, sup

    def scores(self, did: str) -> Scores:
        return self.scores_sup(did)[0]

    def scores_sup(self, did: str) -> tuple[Scores, Support]:
        """Signal 3 on top: the namespace head's and the signature premises' concepts, scaled down."""
        direct, dsup = self.direct_sup(did)
        sc = dict(direct)
        sup = {k: set(v) for k, v in dsup.items()}
        head = self.heads.get(did)
        if head is not None:
            for item, (s, _, _) in ranked(self.direct(head))[:TOP_K]:
                _offer(sc, item, NS_FACTOR * s, "namespace", f"namespace head {head}", sup)
        for p in self.inp.premises.get(did, ()):
            top = ranked(self.direct(p))[:1]
            if top:
                item, (s, _, _) = top[0]
                _offer(sc, item, DEP_FACTOR * s * self.spec(self.indeg[p]), "signature", f"signature mentions {p}",
                       sup)
        return sc, sup

    def select(self, sc: Scores, sup: Support | None = None) -> Chosen:
        return self.select_dropped(sc, sup)[0]

    def select_dropped(self, sc: Scores, sup: Support | None = None) -> tuple[Chosen, list[str]]:
        """Up to TOP_K concepts above the absolute and relative floors, plus the secondaries left out.  With `sup`
        a secondary concept (rank 1-2) stays only if a signal class supports it that does not support the
        primary (plan ronda 3: a secondary needs a signal independent of the primary)."""
        out: Chosen = []
        dropped: list[str] = []
        labels: set[str] = set()
        for item, v in ranked(sc):
            if v[0] < ASSIGN_FLOOR or (out and v[0] < RELATIVE_FLOOR * out[0][1][0]) or len(out) == TOP_K:
                break
            key = self.label_key.get(item, item)
            if DEDUP_LABELS and key in labels:
                continue
            if out and sup is not None and SECONDARY_INDEPENDENT and not independent(sup.get(item, set()), sup[out[0][0]]):
                dropped.append(item)
                continue
            labels.add(key)
            out.append((item, v))
        return out, dropped

    def choose(self, did: str) -> Chosen:
        """Signals 1-3 and the selection rule for one declaration; the secondaries left out for lack of an
        independent signal are recorded in `self.dropped` as (declaration, concept)."""
        chosen, dropped = self.select_dropped(*self.scores_sup(did))
        self.dropped.update((did, item) for item in dropped)
        return chosen


def zone_of(module: str) -> str:
    parts = module.split("/")
    if len(parts) > 2 and parts[-1] in GENERIC_FILES:
        parts = parts[:-1]
    return "/".join(parts)


def _ordered(chosen: Chosen) -> list[tuple[str, float, str, str]]:
    """Final ranks: stored confidence (4 decimals) descending, then the lower Wikidata Q-number (older, more
    general items first; derived concepts last), then concept id.  `atlas compile` picks the primary concept
    by the same rule (core/atlas-core/src/concepts.rs), so Python and Rust agree."""
    rows = [(item, round(min(s, 1.0), 4), signal, ev) for item, (s, signal, ev) in chosen]
    return sorted(rows, key=lambda r: (-r[1], _q(r[0]), r[0]))


def _state(signal: str) -> EvidenceState:
    return EvidenceState.EXTRACTED if signal in ("curated", "zone_derived") else EvidenceState.MODEL_PROPOSED


def assign(inp: Inputs, *, use_curated: bool = True, scorer: Scorer | None = None,
           only: Iterable[str] | None = None) -> tuple[list[Assignment], dict[str, str]]:
    """All assignments, plus derived concept id -> zone path.  `only` scores just those declarations with
    signals 1-3 (dev tuning): no zone fallback, no derived concepts."""
    sc = scorer or Scorer(inp, use_curated=use_curated)
    out: list[Assignment] = []
    if only is not None:
        for did in only:
            for rank, (item, s, signal, ev) in enumerate(_ordered(sc.choose(did))):
                out.append(Assignment(did, item, s, rank, signal, ev, _state(signal)))
        return out, {}
    zone_votes: dict[str, Counter] = defaultdict(Counter)
    fallback: dict[str, list[Decl]] = defaultdict(list)
    for d in inp.decls:
        if d.infra:
            continue
        chosen = _ordered(sc.choose(d.id))
        zone = zone_of(d.module) if d.module else "Mathlib"
        if not chosen:
            fallback[zone].append(d)
            continue
        zone_votes[zone][chosen[0][0]] += 1
        for rank, (item, s, signal, ev) in enumerate(chosen):
            out.append(Assignment(d.id, item, s, rank, signal, ev, _state(signal)))
    derived: dict[str, str] = {}
    for zone, members in sorted(fold_zones(fallback).items()):
        votes = zone_votes.get(zone, Counter())
        total = sum(votes.values())
        best = min(votes.items(), key=lambda kv: (-kv[1], _q(kv[0]), kv[0])) if votes else None
        if best and best[1] >= ZONE_MIN_VOTES and best[1] >= ZONE_SHARE * total:
            ev = f"zone {zone}: {best[1]}/{total} assigned declarations have it first"
            out += [Assignment(d.id, best[0], round(ZONE_CONF * best[1] / total, 4), 0, "zone", ev,
                               EvidenceState.MODEL_PROPOSED) for d in members]
            continue
        cid = DERIVED_PREFIX + zone
        derived[cid] = zone
        out += [Assignment(d.id, cid, 1.0, 0, "zone_derived", f"Mathlib zone {zone} (module {d.module})",
                           EvidenceState.EXTRACTED) for d in members]
    return out, derived


def fold_zones(fallback: Mapping[str, list[Decl]]) -> dict[str, list[Decl]]:
    """Zones with fewer than MIN_ZONE declarations move into their parent directory (deepest first)."""
    zones = {z: list(ms) for z, ms in fallback.items()}
    for z in sorted(zones, key=lambda z: (-z.count("/"), z)):
        parent = z.rsplit("/", 1)[0]
        if len(zones[z]) < MIN_ZONE and z.count("/") > 1:
            zones.setdefault(parent, []).extend(zones.pop(z))
    return zones


# ---------------------------------------------------------------------- evaluation (curated links)


def split_of(decl_id: str) -> str:
    """Deterministic half split of the curated declarations: tune on dev, report test."""
    return "dev" if int(hashlib.sha1(decl_id.encode("utf-8")).hexdigest()[:8], 16) % 2 == 0 else "test"


def gold_by_decl(curated: Iterable[tuple[str, str]], split: str) -> dict[str, set[str]]:
    gold: dict[str, set[str]] = defaultdict(set)
    for item, decl in curated:
        if split_of(decl) == split:
            gold[decl].add(item)
    return dict(gold)


def evaluate(assignments: Iterable[Assignment], curated: Iterable[tuple[str, str]], *, split: str) -> dict:
    """recall@3 over curated (item, declaration) pairs; top-1 precision over curated declarations that got
    any assignment (a derived or zone concept in first place counts as wrong)."""
    gold = gold_by_decl(curated, split)
    by_decl: dict[str, list[Assignment]] = defaultdict(list)
    for a in assignments:
        if a.decl in gold:
            by_decl[a.decl].append(a)
    for v in by_decl.values():
        v.sort(key=lambda a: a.rank)
    pairs = [(d, i) for d, items in gold.items() for i in items]
    hit3 = sum(1 for d, i in pairs if i in {a.concept for a in by_decl.get(d, [])[:TOP_K]})
    assigned = [d for d in gold if by_decl.get(d)]
    hit1 = sum(1 for d in assigned if by_decl[d][0].concept in gold[d])
    wd_first = [d for d in assigned if by_decl[d][0].concept.startswith("wd:")]
    return {"split": split, "gold_items": len({i for _, i in pairs}), "gold_declarations": len(gold),
            "gold_pairs": len(pairs), "recall_at_3": _rate(hit3, len(pairs)),
            "top1_precision": _rate(hit1, len(assigned)),
            "top1_precision_wikidata_first_only": _rate(sum(1 for d in wd_first if by_decl[d][0].concept in gold[d]),
                                                        len(wd_first)),
            "first_by_signal": dict(sorted(Counter(by_decl[d][0].signal for d in assigned).items())),
            "note": "curated links are masked from the signals; a concept counted wrong may still be valid"}


def _rate(k: int, n: int) -> dict:
    return {"k": k, "n": n, "value": round(k / n, 4) if n else None, "wilson95": list(wilson(k, n))}


def wilson(k: int, n: int, z: float = 1.959964) -> tuple[float, float]:
    if n == 0:
        return (0.0, 1.0)
    p = k / n
    den = 1 + z * z / n
    mid = (p + z * z / (2 * n)) / den
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / den
    return (round(max(0.0, mid - half), 4), round(min(1.0, mid + half), 4))


def coverage(inp: Inputs, assignments: Iterable[Assignment]) -> dict:
    content = [d for d in inp.decls if not d.infra]
    assignments = list(assignments)
    first = {a.decl: a for a in assignments if a.rank == 0}
    kinds = Counter("wikidata" if a.concept.startswith("wd:") else "derived" for a in first.values())
    signals = Counter(a.signal for a in first.values())
    strong = sum(1 for a in first.values() if a.signal not in ("zone", "zone_derived"))
    n = len(content)
    return {"content_declarations": n, "infrastructure_declarations": len(inp.decls) - n,
            "assigned": len(first), "assigned_share": round(len(first) / n, 4) if n else None,
            "assigned_by_own_signals": strong, "assigned_by_own_signals_share": round(strong / n, 4) if n else None,
            "primary_wikidata": kinds["wikidata"], "primary_derived": kinds["derived"],
            "unassigned": n - len(first), "primary_by_signal": dict(sorted(signals.items())),
            "assignments": len(assignments),
            "concepts_with_primary_members": len({a.concept for a in first.values()})}


# ---------------------------------------------------------------------- registry writes (reversible)


def claim_id(decl: str, concept: str) -> str:
    return stable_id("c", EXTRACTOR, VERSION, decl, concept)


def _register_source(store: Store) -> None:
    if store.get_source(SOURCE_ID) is None:
        store.add_source(Source(SOURCE_ID, "MPM atlas concept projection (declaration -> concept)", "CC0-1.0",
                                version=f"{EXTRACTOR}/{VERSION}", retrieved_at=now_iso(),
                                terms="Derived assignments; heuristic, uncalibrated scores; reversible"), actor=ACTOR)


def _humanize(zone: str) -> str:
    parts = zone.split("/")[1:] or ["Mathlib"]
    return " › ".join(" ".join(name_tokens(p)) or p for p in parts[-2:])


def derived_entity(cid: str, zone: str, ts: str) -> Entity:
    human = _humanize(zone)
    prov = (Provenance(SOURCE_ID, zone, EXTRACTOR, VERSION, "CC0-1.0", ts),)
    return Entity(cid, EntityType.CONCEPT,
                  (LangString(f"{human} (Mathlib, derived)", "en"), LangString(f"{human} (Mathlib, derivado)", "es")),
                  descriptions=(LangString(f"Derived concept: the Mathlib zone {zone}, which has no Wikidata concept "
                                           "of its own.", "en"),
                                LangString(f"Concepto derivado: la zona {zone} de Mathlib, sin concepto propio en "
                                           "Wikidata.", "es")),
                  external_ids=(("mathlib_zone", zone),), provenance=prov)


def assignment_claim(a: Assignment, ts: str, note: str = ""):  # -> Claim
    quote = json.dumps({"signal": a.signal, "evidence": a.evidence, "rank": a.rank}, ensure_ascii=False)
    prov = (Provenance(SOURCE_ID, f"{a.decl} -> {a.concept}", EXTRACTOR, VERSION, "CC0-1.0", ts, quote=quote[:500]),)
    if a.signal == "zone_derived":
        conf = Confidence(1.0, "P(the declaration lies in the Mathlib zone that defines the derived concept)",
                          "exact: read from the module path")
    else:
        conf = Confidence(a.score, ESTIMATES, CALIBRATION)
    return binary(claim_id(a.decl, a.concept), RelationType.MEMBER_OF_CONCEPT, a.decl, a.concept, prov,
                  evidence_state=a.state, confidence=conf, explanation=explanation(a, note))


ZONE_LABEL = {  # the zone fallback is not a signal of the declaration (plan ronda 3): said in so many words
    "zone": ("Zone fallback, not a signal of its own: the concept most declarations of this Mathlib zone have first.",
             "Respaldo por zona, sin señal propia: el concepto que tiene primero la mayoría de las declaraciones de "
             "esta zona de Mathlib."),
    "zone_derived": ("Zone fallback, not a signal of its own: the derived concept of this Mathlib zone.",
                     "Respaldo por zona, sin señal propia: el concepto derivado de esta zona de Mathlib.")}


def explanation(a: Assignment, note: str = "") -> tuple[LangString, LangString]:
    zen, zes = ZONE_LABEL.get(a.signal, ("", ""))
    return (LangString(f"Concept projection ({a.signal}, rank {a.rank}); grouping only, not a dependency."
                       f"{'  ' + zen if zen else ''}{'  Retired: ' + note if note else ''}", "en"),
            LangString(f"Proyección conceptual ({a.signal}, rango {a.rank}); solo agrupa, no es dependencia."
                       f"{'  ' + zes if zes else ''}"
                       f"{'  Retirada: secundaria sin señal independiente de la primaria.' if note else ''}", "es"))


def _projection_claims(store: Store) -> Iterator:
    for c in store.iter_claims(RelationType.MEMBER_OF_CONCEPT):
        if any(p.extractor == EXTRACTOR for p in c.provenance):
            yield c


def _unchanged(c, a: Assignment) -> bool:
    q = json.loads(c.provenance[0].quote or "{}") if c.provenance else {}
    return (c.confidence is not None and c.confidence.value == a.score and c.evidence_state is a.state
            and (q.get("signal"), q.get("evidence"), q.get("rank")) == (a.signal, a.evidence, a.rank)
            and tuple(c.explanation) == explanation(a))


def write(store: Store, assignments: Iterable[Assignment], derived: Mapping[str, str],
          retired: Iterable[tuple[str, str]] = ()) -> dict:
    """Add derived concepts and assignment claims.  A (declaration, concept) pair the projection already asserts
    is kept when this run reproduces it unchanged (its provenance keeps the version that first produced it),
    superseded by a claim of this version when its score, rank, signal or state changed, and retracted when this
    run no longer produces it.  A pair in `retired` (a secondary concept without an independent signal) is not
    simply retracted: it is superseded by a version of this run that says why, and that version is retracted, so
    `Store.history` shows the retirement.  Nothing is deleted: every earlier version stays in the registry."""
    _register_source(store)
    ts = now_iso()
    added_entities = store.add_entities((derived_entity(cid, z, ts) for cid, z in sorted(derived.items())),
                                        actor=ACTOR, on_conflict="ignore")
    claims = {c.id: c for c in _projection_claims(store)}
    current = {pair: claims[cid] for pair, cid in current_claim_ids(store).items()}
    new: list[Assignment] = []
    changed: list[tuple[str, Assignment]] = []
    kept = 0
    for a in assignments:
        old = current.pop((a.decl, a.concept), None)
        if old is None:
            new.append(a)
        elif _unchanged(old, a):
            kept += 1
        else:
            changed.append((old.id, a))
    retired = set(retired)
    retire = [c for pair, c in current.items() if pair in retired]
    taken = store.existing_claim_ids([claim_id(a.decl, a.concept) for a in new] +
                                     [claim_id(a.decl, a.concept) for _, a in changed] +
                                     [claim_id(c.subject, c.ids(Role.OBJECT)[0]) for c in retire])


    def claim_for(a: Assignment, note: str = ""):
        c = assignment_claim(a, ts, note)
        n, used = 0, c.id in taken
        while used:  # this version already wrote the pair (then retracted or re-scored it): next free id
            n += 1
            c = replace(c, id=stable_id("c", EXTRACTOR, VERSION, a.decl, a.concept, str(n)))
            used = bool(store.existing_claim_ids([c.id]))
        return c

    added = store.add_claims((claim_for(a) for a in new), actor=ACTOR)
    for old_id, a in changed:
        store.supersede(old_id, claim_for(a), actor=ACTOR, reason=f"re-scored by {EXTRACTOR}/{VERSION}")
    why = f"secondary concept without a signal independent of the primary ({EXTRACTOR}/{VERSION})"
    retirements = []
    for c in retire:
        q = json.loads(c.provenance[0].quote or "{}") if c.provenance else {}
        a = Assignment(c.subject, c.ids(Role.OBJECT)[0], c.confidence.value if c.confidence else 0.0,
                       int(q.get("rank", 1)), str(q.get("signal", "")), str(q.get("evidence", "")), c.evidence_state)
        retirements.append(store.supersede(c.id, claim_for(a, why), actor=ACTOR, reason=why).id)
    retract_ids(store, retirements, reason=why)
    stale = [c.id for pair, c in current.items() if pair not in retired]
    retract_ids(store, stale, reason=f"not produced by {EXTRACTOR}/{VERSION}")
    return {"derived_entities_added": added_entities, "claims_added": added, "claims_already_present": kept,
            "claims_superseded": len(changed), "claims_retired_secondary": len(retirements),
            "claims_retracted_stale": len(stale)}


def homonym_formalizations(store: Store, inp: Inputs, one_word: frozenset[str]) -> list[str]:
    """Current `model_proposed` formalized_as candidates (resolve.py) that the homonym guard rejects: an item
    that is not a mathematical concept (inp.excluded: a block cipher "Square" -> `Square`), or a one-word item
    whose domain excludes the declaration's directory (integral -> `IsIntegral`)."""
    decls = {d.id: d for d in inp.decls}
    out = []
    for c in store.iter_claims(RelationType.FORMALIZED_AS):
        if c.evidence_state is not EvidenceState.MODEL_PROPOSED:
            continue
        if c.subject in inp.excluded:
            out.append(c.id)
            continue
        if c.subject not in one_word:
            continue
        d = decls.get(store.resolve(c.ids(Role.OBJECT)[0]))
        if d is not None and not domain_ok(inp.domains, c.subject, d.module):
            out.append(c.id)
    return out


def current_claim_ids(store: Store) -> dict[tuple[str, str], str]:
    """(declaration, concept) -> id of the projection's current claim for that pair."""
    return {(c.subject, c.ids(Role.OBJECT)[0]): c.id for c in _projection_claims(store)
            if c.evidence_state not in (EvidenceState.RETRACTED, EvidenceState.SUPERSEDED)}


def retract_ids(store: Store, ids: Iterable[str], *, reason: str) -> int:
    ids = list(ids)
    with store.transaction():
        for cid in ids:
            store.set_evidence_state(cid, EvidenceState.RETRACTED, actor=ACTOR, reason=reason)
    return len(ids)


def retract_all(store: Store, *, reason: str) -> int:
    """Undo the projection: retract every current claim it wrote (derived entities stay, with no claims)."""
    return retract_ids(store, (c.id for c in _projection_claims(store)
                               if c.evidence_state not in (EvidenceState.RETRACTED, EvidenceState.SUPERSEDED)),
                       reason=reason)


# ---------------------------------------------------------------------- audit sample


def stratum(module: str) -> str:
    parts = module.split("/")
    return "/".join(parts[:2]) if len(parts) > 1 else (module or "(no module)")


def audit_sample(inp: Inputs, assignments: Iterable[Assignment], *, size: int = SAMPLE_SIZE, seed: str = "",
                 registry_ids: Mapping[tuple[str, str], str] | None = None) -> list[dict]:
    """`size` assignments stratified by Mathlib top-level area (largest remainder, at least
    SAMPLE_MIN_PER_STRATUM each), chosen by a hash of the claim id: deterministic, no RNG state.
    `registry_ids` maps (declaration, concept) to the current claim in the registry (an unchanged pair keeps
    the id of the version that first wrote it), so every row traces to a claim that exists."""
    registry_ids = registry_ids or {}
    decls = {d.id: d for d in inp.decls}
    by_stratum: dict[str, list[Assignment]] = defaultdict(list)
    for a in assignments:
        by_stratum[stratum(decls[a.decl].module)].append(a)
    total = sum(len(v) for v in by_stratum.values())
    quota = {s: min(len(v), SAMPLE_MIN_PER_STRATUM) for s, v in by_stratum.items()}
    rest = size - sum(quota.values())
    exact = {s: rest * len(v) / total for s, v in by_stratum.items()}
    for s in exact:
        quota[s] = min(len(by_stratum[s]), quota[s] + int(exact[s]))
    for s in sorted(exact, key=lambda s: (-(exact[s] - int(exact[s])), s)):
        if sum(quota.values()) >= size:
            break
        if quota[s] < len(by_stratum[s]):
            quota[s] += 1
    rows = []
    for s in sorted(by_stratum):
        pick = sorted(by_stratum[s],
                      key=lambda a: hashlib.sha1((seed + claim_id(a.decl, a.concept)).encode()).hexdigest())
        for a in pick[:quota[s]]:
            d = decls[a.decl]
            it = inp.items.get(a.concept)
            rows.append({"claim_id": registry_ids.get((a.decl, a.concept), claim_id(a.decl, a.concept)), "stratum": s, "declaration": a.decl,
                         "lean_name": d.name, "declaration_type": d.type.value, "module": d.module,
                         "docstring": d.doc[:400], "concept": a.concept,
                         "concept_label_en": it.label_en if it else _humanize(a.concept[len(DERIVED_PREFIX):]),
                         "concept_aliases": list(it.texts[:8]) if it else [],
                         "derived": a.concept.startswith(DERIVED_PREFIX), "rank": a.rank, "confidence": a.score,
                         "signal": a.signal, "evidence": a.evidence, "evidence_state": a.state.value,
                         "verdict": None, "auditor": None, "note": None})
    return rows


# ---------------------------------------------------------------------- run


def run(store: Store, *, sample_path: Path | None = None, write_claims: bool = True, seed: str = "") -> dict:
    """Load, evaluate (masked curated links, dev and test halves), assign, write, sample."""
    inp = load(store)
    masked = Scorer(inp, use_curated=False)
    eval_asg, _ = assign(inp, scorer=masked)
    evaluation = {s: evaluate(eval_asg, inp.curated, split=s) for s in ("dev", "test")}
    sc = Scorer(inp, use_curated=True, hits=masked.hits)
    asg, derived = assign(inp, scorer=sc)
    report = {"extractor": f"{EXTRACTOR}/{VERSION}", "evaluation_vs_curated": evaluation,
              "coverage": coverage(inp, asg), "derived_concepts": len(derived),
              "parameters": {"TOP_K": TOP_K, "ASSIGN_FLOOR": ASSIGN_FLOOR, "RELATIVE_FLOOR": RELATIVE_FLOOR,
                             "NS_FACTOR": NS_FACTOR, "DEP_FACTOR": DEP_FACTOR, "SPEC_POW": SPEC_POW,
                             "ALIAS_FACTOR": ALIAS_FACTOR, "SUBSUMED_FACTOR": SUBSUMED_FACTOR,
                             "MIN_KEY_CHARS": MIN_KEY_CHARS, "RESOLVE_FACTOR": RESOLVE_FACTOR,
                             "DEDUP_LABELS": DEDUP_LABELS, "AREA_DIRS": len(AREA_DIRS),
                             "W_SOURCE": W_SOURCE, "NAME_ONE_TOKEN_ALIAS": NAME_ONE_TOKEN_ALIAS,
                             "NAME_ONE_TOKEN_CORE": NAME_ONE_TOKEN_CORE, "MIN_NAME_TOKEN": MIN_NAME_TOKEN,
                             "SECONDARY_INDEPENDENT": SECONDARY_INDEPENDENT,
                             "SECONDARY_PROPAGATED": SECONDARY_PROPAGATED}}
    report["excluded_items"] = dict(sorted(Counter(inp.excluded.values()).items()))
    report["secondaries_without_independent_signal"] = len(sc.dropped)
    homonyms = homonym_formalizations(store, inp, sc.one_word)
    report["homonym_formalizations"] = len(homonyms)
    if write_claims:
        report["write"] = write(store, asg, derived, retired=sc.dropped)
        report["write"]["homonym_formalizations_retracted"] = retract_ids(
            store, homonyms, reason=f"homonym guard of {EXTRACTOR}/{VERSION}: one-word label outside the item's domain")
    if sample_path is not None:
        rows = audit_sample(inp, asg, registry_ids=current_claim_ids(store), seed=seed)
        sample_path.parent.mkdir(parents=True, exist_ok=True)
        with open(sample_path, "w", encoding="utf-8", newline="\n") as fh:
            for r in rows:
                fh.write(json.dumps(r, ensure_ascii=False) + "\n")
        report["audit_sample"] = {"path": str(sample_path), "rows": len(rows), "seed": seed,
                                  "strata": len({r["stratum"] for r in rows})}
    return report


def main(argv: list[str] | None = None) -> int:
    import argparse

    ap = argparse.ArgumentParser(prog="python -m atlas.concepts", description="Concept projection: Mathlib declarations -> concepts")
    ap.add_argument("--db", default="data/atlas.db")
    ap.add_argument("--sample", default="bench/concepts/audit-sample-200.jsonl", help="stratified audit sample")
    ap.add_argument("--seed", default="", help="prefix hashed with each claim id to pick the sample")
    ap.add_argument("--report", default=None, help="JSON report path (default data/cache/reports/concepts-<ts>.json)")
    ap.add_argument("--no-write", action="store_true", help="evaluate and sample only; registry untouched")
    ap.add_argument("--retract", metavar="REASON", help="undo: retract every claim of the projection")
    a = ap.parse_args(argv)
    with Store(a.db, read_only=a.no_write) as store:
        if a.retract:
            print(json.dumps({"retracted": retract_all(store, reason=a.retract)}))
            return 0
        report = run(store, sample_path=Path(a.sample) if a.sample else None, write_claims=not a.no_write,
                     seed=a.seed)
    out = Path(a.report or f"data/cache/reports/concepts-{now_iso()[:19].replace(':', '')}.json")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
