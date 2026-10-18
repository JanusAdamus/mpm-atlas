"""Cross-source entity resolution: candidates, declared thresholds, reversible decisions.

Three candidate generators, all with blocking so they scale to 10^5-10^6 entities:

1. Mathlib aliases (`alias b := a`): the same proof term under two names.  Score 1.0
   (explicit evidence).  Same type, so above MERGE_THRESHOLD it becomes a
   `Store.merge` with the evidence recorded; `undo_resolver_merges` reverts it.
2. Wikidata item <-> Mathlib declaration.  These are not the same object (an
   informal concept vs a formal artifact, SPEC §2), so the decision is never a
   merge: above LINK_THRESHOLD a `formalized_as` claim in `model_proposed` with a
   Confidence that says what it estimates and that it is not calibrated; between
   PROPOSAL_FLOOR and LINK_THRESHOLD the pair goes to the review queue file.
   Blocking: an exact index on the last name component, token postings capped by
   document frequency, and phrase lookup of the label in docstrings.
3. Mathlib directory area <-> Wikidata area (`part_of_area`, same rules as 2).

`evaluate_against_curated` measures generator 2 against the curated links
(Mathlib `@[wikidata]` attributes, docs/1000.yaml, Wikidata P14534), which the
resolver does not see while scoring.
"""

from __future__ import annotations

import json
import math
import re
import unicodedata
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Iterator

from .ingest.lean import name_tokens
from .model import (
    Confidence,
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

RESOLVER_VERSION = "resolve/2"
ACTOR = "atlas.resolve"
SOURCE_ID = "src:atlas-resolve"
MERGE_THRESHOLD = 0.98  # identity merges (same type only)
# formalized_as / part_of_area proposals written as model_proposed claims; only an item's top-ranked candidate
# can pass.  0.90 from the curated-link check of resolve/2 (2026-10-16): top-1 band >= 0.9 matched the curated
# declaration 88/117, band 0.8-0.9 only 24/55 (lower bounds: a second valid formalization counts as wrong)
LINK_THRESHOLD = 0.90
PROPOSAL_FLOOR = 0.45  # below this a candidate is discarded; between floor and threshold: review queue
TOP_K = 3
MAX_DF = 400  # token postings larger than this are not used for blocking
MAX_DOC_HITS = 25
NS_FLOOR = 0.6  # name matches are scaled by NS_FLOOR + (1 - NS_FLOOR) * overlap(label, full declaration name)
INSTANCE_FACTOR = 0.8  # an `instance` is a use of a concept on some type, rarely its formalization
NAME_STOP = frozenset({"is", "has", "mk", "to", "of"})
CALIBRATION = (f"not calibrated: heuristic score of {RESOLVER_VERSION}; precision of this score band against "
               "curated Mathlib/Wikidata links is reported by evaluate_against_curated")
ESTIMATES = "P(the Mathlib declaration formalizes the Wikidata item)"

STOP = frozenset("the of a an and or in on for to with by at from is its de la el los las del y en".split())
GENERIC = frozenset("theorem theorems lemma conjecture formula law principle rule property identity "
                    "equation inequality problem".split())
ABBREV = {"seq": "sequence", "fn": "function", "deriv": "derivative", "poly": "polynomial", "mul": "multiplication",
          "div": "division", "sq": "square", "sqrt": "root", "inv": "inverse", "mat": "matrix", "vec": "vector",
          "finset": "set", "nat": "natural", "int": "integer", "rat": "rational", "irrat": "irrational",
          "dist": "distance", "comm": "commutative", "assoc": "associative", "hom": "homomorphism",
          "iso": "isomorphism", "ae": "almost", "cont": "continuous", "diff": "differentiable", "integral": "integral",
          "det": "determinant", "tsum": "sum", "prod": "product", "lim": "limit", "gcd": "gcd", "lcm": "lcm",
          "eq": "equal", "ne": "unequal", "lt": "less", "le": "less", "gt": "greater", "ge": "greater",
          "pos": "positive", "neg": "negative", "zero": "zero", "card": "cardinality", "char": "characteristic",
          "topological": "topological", "metric": "metric", "normed": "normed", "fintype": "finite"}
WD_TYPES = (EntityType.CONCEPT, EntityType.STATEMENT, EntityType.CONJECTURE, EntityType.ALGORITHM,
            EntityType.MODEL, EntityType.DEFINITION, EntityType.PROBLEM, EntityType.THEORY)
DEFINITIONAL = frozenset({EntityType.CONCEPT, EntityType.DEFINITION, EntityType.MODEL, EntityType.ALGORITHM,
                          EntityType.THEORY})


@dataclass(frozen=True)
class Candidate:
    left: str  # survivor (identity) or claim subject
    right: str  # merged entity (identity) or claim object
    score: float
    kind: str  # "identity" | "formalization" | "area"
    features: tuple[tuple[str, object], ...]

    def to_json(self) -> dict:
        return {"left": self.left, "right": self.right, "score": round(self.score, 4), "kind": self.kind,
                "features": dict(self.features)}


# ---------------------------------------------------------------------- text normalisation


def normalize(text: str) -> str:
    t = unicodedata.normalize("NFKD", text)
    t = "".join(ch for ch in t if not unicodedata.combining(ch)).lower()
    t = re.sub(r"['’]s\b", "", t)
    return " ".join(re.findall(r"[a-z0-9]+", t))


def _stem(tok: str) -> str:
    tok = ABBREV.get(tok, tok)
    if len(tok) > 4 and tok.endswith("ies"):
        return tok[:-3] + "y"
    if len(tok) > 3 and tok.endswith("s") and not tok.endswith("ss"):
        return tok[:-1]
    return tok


def content_tokens(text: str) -> tuple[str, ...]:
    return tuple(_stem(t) for t in normalize(text).split() if t not in STOP)


def decl_last_tokens(name: str) -> tuple[str, ...]:
    return tuple(_stem(t) for t in name_tokens(name.rsplit(".", 1)[-1]) if t not in STOP)


# ---------------------------------------------------------------------- 1. aliases -> merges


def alias_candidates(store: Store) -> Iterator[Candidate]:
    for etype in (EntityType.STATEMENT, EntityType.DEFINITION, EntityType.RESOURCE):
        for e in store.iter_entities(etype):
            targets = [v for s, v in e.external_ids if s == "mathlib_alias_of"]
            if not targets:
                continue
            found = store.find_by_external_id("mathlib", targets[0])
            survivors = [f for f in found if f != e.id]
            if not survivors:
                continue
            loc = e.provenance[0].locator if e.provenance else ""
            yield Candidate(survivors[0], e.id, 1.0, "identity",
                            (("explicit_alias", 1.0), ("alias_of", targets[0]), ("locator", loc)))


def apply_merges(store: Store, candidates: Iterable[Candidate], report: Counter) -> None:
    merged = {m.merged_id for m in store.merges(active_only=True)}
    for c in candidates:
        if c.score < MERGE_THRESHOLD:
            report["identity_below_threshold"] += 1
            continue
        if c.right in merged or c.left in merged:
            report["identity_already_merged"] += 1
            continue
        a, b = store.get_entity(c.left, resolve=False), store.get_entity(c.right, resolve=False)
        if a is None or b is None or a.type != b.type:
            report["identity_type_mismatch"] += 1
            continue
        store.merge(c.left, c.right, actor=ACTOR, reason="Mathlib alias: same declaration under another name",
                    evidence=json.dumps({"resolver": RESOLVER_VERSION, **c.to_json()}, ensure_ascii=False))
        merged.add(c.right)
        report["merges_applied"] += 1


def undo_resolver_merges(store: Store, *, actor: str, reason: str) -> int:
    n = 0
    for m in store.merges(active_only=True):
        if m.actor == ACTOR:
            store.undo_merge(m.id, actor=actor, reason=reason)
            n += 1
    return n


# ---------------------------------------------------------------------- 2. Wikidata <-> Mathlib


@dataclass(frozen=True)
class DeclRow:
    id: str
    name: str
    type: EntityType
    last: tuple[str, ...]
    doc_norm: str
    doc_bold: tuple[str, ...]
    kind: str = ""  # Lean declaration keyword (theorem, def, instance, ...)


class MathlibIndex:
    def __init__(self, rows: list[DeclRow]) -> None:
        self.rows = rows
        self.by_last: dict[tuple[str, ...], list[int]] = defaultdict(list)
        self.post: dict[str, list[int]] = defaultdict(list)
        self.doc_post: dict[str, list[int]] = defaultdict(list)
        for i, r in enumerate(rows):
            self.by_last[_key(r.last)].append(i)
            for t in set(r.last):
                self.post[t].append(i)
            for t in set(content_tokens(r.doc_norm)):
                self.doc_post[t].append(i)
        self.n = max(1, len(rows))
        self.full = [frozenset(_stem(t) for t in name_tokens(r.name) if t not in NAME_STOP) - GENERIC for r in rows]
        # sets once, so per-label intersections do not rebuild them (postings are read-only from here on)
        self.post_sets = {t: frozenset(v) for t, v in self.post.items()}
        self.doc_sets = {t: frozenset(v) for t, v in self.doc_post.items()}

    def idf(self, tok: str) -> float:
        return math.log(1 + self.n / (1 + len(self.post.get(tok, ()))))

    @staticmethod
    def from_store(store: Store) -> "MathlibIndex":
        rows = []
        for etype in (EntityType.STATEMENT, EntityType.DEFINITION):
            for e in store.iter_entities(etype):
                name = next((v for s, v in e.external_ids if s == "mathlib"), None)
                if name is None or any(s == "mathlib_alias_of" for s, _ in e.external_ids):
                    continue
                doc = next((d.text for d in e.descriptions if d.lang == "en"), "")
                bold = tuple(normalize(b) for b in re.findall(r"\*\*([^*]{3,120})\*\*", doc))
                stmt = e.statements[0].text if e.statements else ""
                kind = stmt.split(" " + name, 1)[0] if " " + name in stmt else ""
                rows.append(DeclRow(e.id, name, e.type, decl_last_tokens(name), normalize(doc), bold, kind))
        return MathlibIndex(rows)


def _key(tokens: Iterable[str]) -> tuple[str, ...]:
    return tuple(sorted(set(tokens) - GENERIC))


def _name_score(idx: MathlibIndex, toks: tuple[str, ...], row: DeclRow) -> float:
    a, b = set(toks), set(row.last)
    if not a or not b:
        return 0.0
    inter = sum(idx.idf(t) for t in a & b)
    union = sum(idx.idf(t) for t in a | b)
    return inter / union if union else 0.0


def _full_overlap(idx: MathlibIndex, core: tuple[str, ...], i: int) -> float:
    """idf-weighted Jaccard of the label and the *whole* name, namespace included: `UniformSpace`
    beats `PiLp.uniformSpace` for "uniform space" although their last components tie."""
    a, b = set(core), idx.full[i]
    union = sum(idx.idf(t) for t in a | b)
    return sum(idx.idf(t) for t in a & b) / union if union else 0.0


def item_candidates(idx: MathlibIndex, item_id: str, item_type: EntityType,
                    phrases: list[tuple[str, float]]) -> list[Candidate]:
    """Best Mathlib declarations for one item.  `phrases` = [(label or alias, weight)]."""
    best: dict[int, tuple[float, dict]] = {}

    def offer(i: int, score: float, feats: dict, core: tuple[str, ...] | None = None) -> None:
        row = idx.rows[i]
        typed = (row.type is EntityType.DEFINITION) == (item_type in DEFINITIONAL)
        score *= 1.0 if typed else 0.75
        if core is not None:  # name-based evidence: discount names that carry much more than the label
            full = _full_overlap(idx, core, i)
            score *= NS_FLOOR + (1 - NS_FLOOR) * full
            feats = {**feats, "full_name_overlap": round(full, 3)}
        if row.kind == "instance":
            score *= INSTANCE_FACTOR
            feats = {**feats, "instance": True}
        if i not in best or score > best[i][0]:
            best[i] = (score, {**feats, "type_compatible": typed})

    for text, weight in phrases:
        toks = content_tokens(text)
        core = tuple(t for t in toks if t not in GENERIC)
        if not core:
            continue
        for i in idx.by_last.get(_key(core), ()):  # exact last-component match
            offer(i, weight * 1.0, {"exact_name": text}, core)
        rare = sorted((t for t in set(core) if 0 < len(idx.post.get(t, ())) <= MAX_DF),
                      key=lambda t: len(idx.post[t]))
        if rare:  # intersect the postings of the informative tokens only
            pool = idx.post_sets[rare[0]]
            for t in rare[1:]:
                pool = pool & idx.post_sets[t]
            for i in pool:
                s = _name_score(idx, core, idx.rows[i])
                if s >= PROPOSAL_FLOOR:
                    offer(i, weight * s, {"name_overlap": round(s, 3), "label": text}, core)
        phrase = normalize(text)
        if len(toks) >= 2 or (len(core) == 1 and len(idx.post.get(core[0], ())) <= 50):
            pool = None
            for t in sorted(set(toks), key=lambda t: len(idx.doc_post.get(t, ()))):
                ids = idx.doc_sets.get(t)
                if not ids:
                    pool = frozenset()
                    break
                pool = ids if pool is None else pool & ids
                if len(pool) <= MAX_DOC_HITS:
                    break
            hits = [i for i in (pool or ()) if f" {phrase} " in f" {idx.rows[i].doc_norm} "]
            if 0 < len(hits) <= MAX_DOC_HITS:
                for i in hits:
                    bold = phrase in idx.rows[i].doc_bold
                    spec = 1.0 if len(hits) <= 3 else 3 / len(hits)
                    offer(i, weight * (0.95 if bold else 0.8) * spec,
                          {"doc_phrase": text, "bold": bold, "doc_hits": len(hits)})
    ranked = sorted(best.items(), key=lambda kv: (-kv[1][0], len(idx.rows[kv[0]].name)))[:TOP_K]
    return [Candidate(item_id, idx.rows[i].id, s, "formalization", tuple(sorted(f.items(), key=lambda x: x[0])))
            for i, (s, f) in ranked if s >= PROPOSAL_FLOOR]


def wikidata_items(store: Store) -> Iterator[tuple[str, EntityType, list[tuple[str, float]]]]:
    for etype in WD_TYPES:
        for e in store.iter_entities(etype):
            if not e.id.startswith("wd:"):
                continue
            phrases = [(l.text, 1.0) for l in e.labels if l.lang == "en"]
            phrases += [(a.text, 0.9) for a in e.aliases if a.lang == "en"][:10]
            if phrases:
                yield e.id, e.type, phrases


def formalization_candidates(store: Store, idx: MathlibIndex | None = None) -> Iterator[Candidate]:
    idx = idx or MathlibIndex.from_store(store)
    for item_id, etype, phrases in wikidata_items(store):
        yield from item_candidates(idx, item_id, etype, phrases)


# ---------------------------------------------------------------------- 3. Mathlib dirs <-> Wikidata areas


def area_candidates(store: Store) -> Iterator[Candidate]:
    by_tokens: dict[tuple[str, ...], list[str]] = defaultdict(list)
    for e in store.iter_entities(EntityType.AREA):
        if e.id.startswith("wd:"):
            for l in list(e.labels) + [a for a in e.aliases if a.lang == "en"]:
                if l.lang == "en":
                    by_tokens[tuple(sorted(set(content_tokens(l.text))))].append(e.id)
    for e in store.iter_entities(EntityType.AREA):
        if not e.id.startswith("mathlib-dir:"):
            continue
        last = e.id.rsplit("/", 1)[-1]
        key = tuple(sorted(set(_stem(t) for t in name_tokens(last) if t not in STOP)))
        targets = sorted(set(by_tokens.get(key, ())))
        for t in targets[:TOP_K]:
            yield Candidate(e.id, t, 1.0 / len(targets), "area", (("exact_tokens", last),))


# ---------------------------------------------------------------------- decisions


def _register_source(store: Store) -> None:
    if store.get_source(SOURCE_ID) is None:
        store.add_source(Source(SOURCE_ID, "MPM atlas entity resolver (heuristic candidates)", "CC0-1.0",
                                version=RESOLVER_VERSION, retrieved_at=now_iso(),
                                terms="Derived proposals; scores are heuristic and uncalibrated"), actor=ACTOR)


def apply_links(store: Store, candidates: Iterable[Candidate], report: Counter, queue: list[dict],
                curated: set[tuple[str, str]] = frozenset()) -> None:
    _register_source(store)
    ts = now_iso()
    claims = []
    linked: set[str] = set()  # subjects that already got their one proposal (candidates arrive best first)
    for c in candidates:
        if (c.left, c.right) in curated:
            report[f"{c.kind}_already_curated"] += 1
            linked.add(c.left)
            continue
        if c.score < LINK_THRESHOLD or c.left in linked:
            queue.append({**c.to_json(), "reason": "below_threshold" if c.score < LINK_THRESHOLD else "not_top_ranked"})
            report[f"{c.kind}_queued_for_review"] += 1
            continue
        linked.add(c.left)
        rel = RelationType.FORMALIZED_AS if c.kind == "formalization" else RelationType.PART_OF_AREA
        cid = stable_id("c", "resolve", c.kind, c.left, c.right)
        if store.get_claim(cid) is not None:
            report[f"{c.kind}_already_present"] += 1
            continue
        prov = (Provenance(SOURCE_ID, f"candidate {c.left} -> {c.right}", ACTOR, RESOLVER_VERSION, "CC0-1.0", ts,
                           quote=json.dumps(dict(c.features), ensure_ascii=False)[:500]),)
        claims.append(binary(cid, rel, c.left, c.right, prov, evidence_state=EvidenceState.MODEL_PROPOSED,
                             confidence=Confidence(round(min(c.score, 1.0), 4), ESTIMATES if c.kind == "formalization"
                                                   else "P(the Mathlib directory belongs to the Wikidata area)",
                                                   CALIBRATION),
                             explanation=(LangString("Candidate proposed by name/docstring matching; needs review.",
                                                     "en"),
                                          LangString("Candidato propuesto por coincidencia de nombre o docstring; "
                                                     "requiere revisión.", "es"))))
        report[f"{c.kind}_claims_proposed"] += 1
    store.add_claims(claims, actor=ACTOR)


def curated_pairs(store: Store) -> set[tuple[str, str]]:
    out = set()
    for c in store.iter_claims(RelationType.FORMALIZED_AS):
        if c.evidence_state is not EvidenceState.MODEL_PROPOSED:
            out.add((c.subject, c.ids(Role.OBJECT)[0]))
    return out


def evaluate_against_curated(candidates: list[Candidate], curated: set[tuple[str, str]]) -> dict:
    """Precision/recall of formalization candidates on items that have a curated link."""
    gold: dict[str, set[str]] = defaultdict(set)
    for item, decl in curated:
        gold[item].add(decl)
    by_item: dict[str, list[Candidate]] = defaultdict(list)
    for c in candidates:
        by_item[c.left].append(c)
    top1 = sum(1 for i in gold if by_item.get(i) and by_item[i][0].right in gold[i])
    topk = sum(1 for i in gold if any(c.right in gold[i] for c in by_item.get(i, ())))
    above = [cs[0] for i, cs in by_item.items() if i in gold and cs[0].score >= LINK_THRESHOLD]  # top-1 rule
    correct_above = sum(1 for c in above if c.right in gold[c.left])
    return {"gold_items": len(gold), "items_with_candidates": sum(1 for i in gold if by_item.get(i)),
            "recall_at_1": round(top1 / len(gold), 4) if gold else None,
            f"recall_at_{TOP_K}": round(topk / len(gold), 4) if gold else None,
            "above_threshold_on_gold_items": len(above),
            "precision_above_threshold": round(correct_above / len(above), 4) if above else None,
            "note": "gold = curated links; a candidate counted wrong may still be a valid formalization"}


def run(store: Store, *, queue_path: Path | None = None) -> dict:
    report: Counter = Counter()
    apply_merges(store, alias_candidates(store), report)
    curated = curated_pairs(store)
    cands = list(formalization_candidates(store))
    report["formalization_candidates"] = len(cands)
    evaluation = evaluate_against_curated(cands, curated)
    queue: list[dict] = []
    apply_links(store, cands, report, queue, curated)
    apply_links(store, area_candidates(store), report, queue)
    if queue_path is not None:
        queue_path.parent.mkdir(parents=True, exist_ok=True)
        with open(queue_path, "w", encoding="utf-8") as fh:
            for row in queue:
                fh.write(json.dumps(row, ensure_ascii=False) + "\n")
    return {"version": RESOLVER_VERSION, "thresholds": {"merge": MERGE_THRESHOLD, "link": LINK_THRESHOLD,
                                                         "floor": PROPOSAL_FLOOR},
            "counters": dict(sorted(report.items())), "evaluation_vs_curated": evaluation,
            "review_queue": str(queue_path) if queue_path else None, "review_queue_size": len(queue)}
