"""Silver items derived automatically from the registry (PROTOCOL.md §8).

Silver = conformance and regression evidence, never expert gold.  Every generator
records in `notes` which assumption it shares with the system it will evaluate.
One streaming pass over entities and one over claims, with seeded per-stratum
reservoirs, then targeted lookups for route chains and corroboration.
"""

from __future__ import annotations

import random
import re
from collections import defaultdict
from dataclasses import dataclass, field
from typing import Any, Callable, Iterable

from atlas import Store
from atlas.model import CURRENT_STATES, RELATIONS, Claim, Entity, RelationType, Role

from . import compose
from .items import Item
from .leakage import tokens

AUTHOR = "bench.silver/1"
# reservoir size per stratum (label_exact/docstring: type x Mathlib top folder; relation: family x relation x source;
# sourced: source x relation; er: hardness; route: relation; formula: type; informal_to_formal: one pool)
DEFAULT_CAPS = {"label_exact": 12, "docstring": 12, "informal_to_formal": 500, "formula": 300,
                "er": 400, "relation": 200, "route": 150, "sourced": 1000}
MAX_DOC_TOKENS = 12
ALIAS_SCHEME = "mathlib_alias_of"
IDENTITY_SCHEMES_EXCLUDED = frozenset({"mathlib", ALIAS_SCHEME, "lean_module"})  # unique per entity by construction


class Reservoir:
    def __init__(self, k: int, rng: random.Random) -> None:
        self.k, self.rng, self.seen, self.items = k, rng, 0, []

    def add(self, x: Any) -> None:
        self.seen += 1
        if len(self.items) < self.k:
            self.items.append(x)
        else:
            j = self.rng.randrange(self.seen)
            if j < self.k:
                self.items[j] = x


@dataclass
class _Pools:
    rng: random.Random
    caps: dict[str, int]
    res: dict[str, Reservoir] = field(default_factory=dict)

    def add(self, key: str, cap: str, x: Any) -> None:
        if key not in self.res:
            self.res[key] = Reservoir(self.caps[cap], self.rng)
        self.res[key].add(x)

    def get(self, prefix: str) -> list[Any]:
        return [x for k, r in sorted(self.res.items()) if k.split("|")[0] == prefix for x in r.items]


def _date(e: Entity | Claim) -> str:
    return e.provenance[0].retrieved_at[:10] if e.provenance else "1970-01-01"


def document_of(e: Entity | Claim) -> str:
    if not e.provenance:
        return f"noprov:{e.id}"
    p = e.provenance[0]
    return f"{p.source_id}:{re.sub(r':\d+.*$', '', p.locator)}"


def area_of(e: Entity | Claim) -> str:
    if not e.provenance:
        return "unknown"
    p = e.provenance[0]
    if p.locator.startswith("Mathlib/"):
        return "/".join(p.locator.split("/")[:2])
    return p.source_id


def _lang_label(e: Entity) -> tuple[str, str] | None:
    for lang in ("es", "en"):
        t = next((l.text for l in e.labels if l.lang == lang), None)
        if t:
            return t, lang
    return (e.labels[0].text, "en") if e.labels else None


def _item(iid: str, task: str, payload: dict, *, strata: dict, doc: str, fam: str, time: str, notes: str,
          status: str = "silver_auto") -> Item:
    return Item(iid, task, "silver", status, AUTHOR, time, payload, strata,
                {"document": doc, "family": fam, "time": time}, notes)


# ------------------------------------------------------------------ entity pass

def _entity_pass(store: Store, pools: _Pools) -> dict[str, Any]:
    ext_index: dict[tuple[str, str], list[str]] = defaultdict(list)
    short_index: dict[tuple[str, str], list[str]] = defaultdict(list)
    alias_pairs: list[tuple[str, str]] = []
    types: dict[str, str] = {}
    n = 0
    for e in store.iter_entities(include_merged=True):
        n += 1
        types[e.id] = e.type.value
        for scheme, value in e.external_ids:
            if scheme == ALIAS_SCHEME:
                alias_pairs.append((e.id, value))
            elif scheme not in IDENTITY_SCHEMES_EXCLUDED:
                ext_index[(scheme, value)].append(e.id)
        if any(s == "mathlib" for s, _ in e.external_ids) and e.aliases:
            short_index[(e.type.value, e.aliases[0].text)].append(e.id)
        lab = _lang_label(e)
        if lab is None:
            continue
        stratum = f"{e.type.value}|{area_of(e)}"
        pools.add(f"label_exact|{stratum}", "label_exact", e)
        if e.descriptions and any(s == "mathlib" for s, _ in e.external_ids):
            pools.add(f"docstring|{stratum}", "docstring", e)
        if e.formula:
            pools.add(f"formula|{e.type.value}", "formula", e)
    return {"entities": n, "ext_index": ext_index, "short_index": short_index, "alias_pairs": alias_pairs,
            "types": types}


def _retrieval_from_entities(pools: _Pools) -> list[Item]:
    out: list[Item] = []
    for e in pools.get("label_exact"):
        text, lang = _lang_label(e)
        out.append(_retrieval(f"silver:ret:label:{e.id}", text, lang, "lean_name" if lang == "en" and "." in text
                              else "text", [(e.id, 3)], e, "label_exact",
                              "query = the entity's own label; trivial by construction (regression lane only)"))
    for e in pools.get("docstring"):
        q = " ".join(tokens(_first_sentence(e.descriptions[0].text))[:MAX_DOC_TOKENS])
        if len(q.split()) >= 3:
            out.append(_retrieval(f"silver:ret:doc:{e.id}", q, "en", "text", [(e.id, 3)], e, "docstring",
                                  "query = first sentence of the Mathlib docstring, which the lexical index also "
                                  "holds as the description field: expected trivial"))
    for e in pools.get("formula"):
        lab = _lang_label(e)
        out.append(_retrieval(f"silver:ret:formula:{e.id}", e.formula, lab[1] if lab else "en", "formula",
                              [(e.id, 3)], e, "formula",
                              "query = the entity's LaTeX formula; the lexical index does not hold the formula "
                              "field (SPEC §10), only statements that may repeat it"))
    return out


def _first_sentence(text: str) -> str:
    text = re.sub(r"`[^`]*`|\$[^$]*\$|\[[^\]]*\]\([^)]*\)", " ", text)
    return re.split(r"(?<=[.!?])\s|\n\n", text.strip(), maxsplit=1)[0]


def _retrieval(iid: str, query: str, lang: str, modality: str, qrels: list[tuple[str, int]], e: Entity,
               generator: str, notes: str) -> Item:
    return _item(iid, "retrieval", {"query": query, "lang": lang, "modality": modality,
                                    "qrels": [{"ref": {"id": i}, "grade": g} for i, g in qrels]},
                 strata={"generator": generator, "lang": lang, "modality": modality, "type": e.type.value,
                         "area": area_of(e)},
                 doc=document_of(e), fam=qrels[0][0], time=_date(e), notes=notes)


# ------------------------------------------------------------------ entity resolution pairs

def _er_items(store: Store, info: dict[str, Any], pools: _Pools) -> list[Item]:
    types = info["types"]
    rng = pools.rng
    for (scheme, value), ids in info["ext_index"].items():
        same_type = [i for i in ids if types.get(i)]
        for a, b in zip(same_type, same_type[1:]):
            if types[a] == types[b]:
                pools.add("er_shared|x", "er", (a, b, f"both carry external id {scheme}={value}"))
    for alias, target_name in info["alias_pairs"]:
        tids = store.find_by_external_id("mathlib", target_name)
        if tids:
            pools.add("er_alias|x", "er", (alias, tids[0], f"Mathlib `alias {alias} := {target_name}`"))
    for (etype, short), ids in info["short_index"].items():
        if len(ids) >= 2:
            a, b = rng.sample(ids, 2)
            pools.add("er_collision|x", "er", (a, b, f"same short name {short!r}, different declarations"))
    for m in store.merges(active_only=True):
        pools.add("er_system|x", "er", (m.survivor_id, m.merged_id, f"merge #{m.id} by {m.actor}: {m.reason}"))
    by_type: dict[str, list[str]] = defaultdict(list)
    for eid, t in types.items():
        by_type[t].append(eid)
    for _ in range(pools.caps["er"]):
        t = rng.choice(sorted(by_type))
        if len(by_type[t]) >= 2:
            a, b = rng.sample(by_type[t], 2)
            pools.add("er_random|x", "er", (a, b, "two random entities of the same type"))
    spec = {"er_system": (None, "system_merge", None,
                          "a merge the system made, sampled uniformly from active merges: the only pool that "
                          "estimates merge precision; label left for experts"),
            "er_shared": ("same", "shared_external_id", None,
                          "label = same because both records carry the same external identifier; noisy if the "
                          "source itself has duplicates or reuses ids for facets"),
            "er_alias": ("same", "alias", "resolve/1 alias generator",
                         "label = same because Mathlib declares an alias; this is the very evidence resolve/1 "
                         "merges on, so it is circular and excluded from the precision gate"),
            "er_collision": ("different", "label_collision", None,
                             "label = different: distinct Mathlib declarations sharing a short name (hard negative)"),
            "er_related": ("different", "related_not_same", None,
                           "label = different: a current generalizes/special_case_of claim links them"),
            "er_random": ("different", "random", None, "label = different: random same-type pair (easy negative)")}
    out: list[Item] = []
    for key, (label, hardness, circular, notes) in spec.items():
        for a, b, evidence in pools.get(key):
            ea, eb = store.get_entity(a, resolve=False), store.get_entity(b, resolve=False)
            if ea is None or eb is None or ea.type != eb.type or a == b:
                continue
            fam = min(a, b)
            out.append(_item(f"silver:er:{hardness}:{a}|{b}", "er_pair",
                             {"a": {"id": a}, "b": {"id": b}, "label": label, "hardness": hardness,
                              "evidence": evidence, "circular_with": circular},
                             strata={"type": ea.type.value, "hardness": hardness, "area": area_of(ea)},
                             doc=document_of(ea), fam=fam, time=max(_date(ea), _date(eb)), notes=notes,
                             status="silver_auto" if label else "unlabeled"))
    return out


# ------------------------------------------------------------------ claim pass

def _claim_pass(store: Store, pools: _Pools) -> int:
    n = 0
    for c in store.iter_claims(states=CURRENT_STATES):
        n += 1
        spec = RELATIONS[c.relation]
        pools.add(f"relation|{spec.projection.value}|{c.relation.value}|{c.provenance[0].source_id}", "relation", c)
        pools.add(f"sourced|{c.provenance[0].source_id}|{c.relation.value}", "sourced", c)
        if c.relation is RelationType.FORMALIZED_AS:
            pools.add("informal_to_formal|x", "informal_to_formal", c)
        if c.relation in (RelationType.GENERALIZES, RelationType.SPECIAL_CASE_OF):
            pools.add("er_related|x", "er", (c.subject, c.targets[0], f"claim {c.id} ({c.relation.value})"))
        if spec.transitive or c.relation in (RelationType.DEPENDS_ON_LOGICALLY, RelationType.NAMED_AFTER):
            pools.add(f"route_seed|{c.relation.value}", "route", c)
    return n


def _relation_items(store: Store, pools: _Pools) -> list[Item]:
    out = []
    for c in pools.get("relation"):
        p = c.provenance[0]
        corr = _corroboration(store, c)
        fam = RELATIONS[c.relation].projection.value
        out.append(_item(f"silver:rel:{c.id}", "relation",
                         {"claim_id": c.id, "relation": c.relation.value, "family": fam,
                          "subject": {"id": c.subject}, "targets": [{"id": t} for t in c.targets],
                          "source_id": p.source_id, "locator": p.locator, "label": None, "error_type": None,
                          "corroborated_by": corr},
                         strata={"family": fam, "relation": c.relation.value, "source": p.source_id,
                                 "area": area_of(c)},
                         doc=document_of(c), fam=c.subject, time=_date(c), status="unlabeled",
                         notes="stratified sample to annotate; corroborated_by lists other sources asserting the "
                               "same (subject, relation, targets): a silver proxy, never a precision estimate"))
    return out


def _corroboration(store: Store, c: Claim) -> list[str]:
    key = frozenset(store.resolve(t) for t in c.targets)
    mine = {p.source_id for p in c.provenance}
    others: set[str] = set()
    for o in store.claims_for(c.subject, roles=[Role.SUBJECT], relations=[c.relation]):
        if o.id != c.id and frozenset(store.resolve(t) for t in o.targets) == key:
            others |= {p.source_id for p in o.provenance}
    return sorted(others - mine)


def _formal_items(store: Store, pools: _Pools) -> list[Item]:
    out = []
    for c in pools.get("informal_to_formal"):
        subj = store.get_entity(c.subject)
        if subj is None or not c.targets:
            continue
        for lab in subj.labels:
            if lab.lang not in ("es", "en"):
                continue
            out.append(_item(f"silver:ret:i2f:{c.id}:{lab.lang}", "retrieval",
                             {"query": lab.text, "lang": lab.lang, "modality": "informal_to_formal",
                              "qrels": [{"ref": {"id": c.targets[0]}, "grade": 3},
                                        {"ref": {"id": subj.id}, "grade": 1}]},
                             strata={"generator": "informal_to_formal", "lang": lab.lang,
                                     "modality": "informal_to_formal", "area": area_of(c),
                                     "evidence_state": c.evidence_state.value},
                             doc=document_of(c), fam=store.resolve(c.targets[0]), time=_date(c),
                             notes="query = informal label; relevant = the formal declaration linked by a current "
                                   f"formalized_as claim ({c.evidence_state.value}); model_proposed links are "
                                   "the resolver's own output, so those items share its errors"))
    return out


def _sourced_items(pools: _Pools) -> list[Item]:
    return [_item(f"silver:src:{c.id}", "sourced_answer",
                  {"entity": {"id": c.subject}, "claim_id": c.id,
                   "expected": [{"source_id": p.source_id, "locator": p.locator} for p in c.provenance]},
                  strata={"source": c.provenance[0].source_id, "relation": c.relation.value, "area": area_of(c)},
                  doc=document_of(c), fam=c.subject, time=_date(c),
                  notes="expected provenance read from the registry: checks serving fidelity, not truth")
            for c in pools.get("sourced")]


# ------------------------------------------------------------------ routes

def _route_items(store: Store, pools: _Pools) -> list[Item]:
    out: list[Item] = []
    rng = pools.rng
    for c1 in pools.get("route_seed"):
        rel = c1.relation
        mid = c1.targets[rng.randrange(len(c1.targets))]
        a = c1.subject
        if RELATIONS[rel].transitive:
            nxt = [c for c in store.claims_for(mid, roles=[Role.SUBJECT], relations=[rel]) if c.subject == mid]
            if nxt:
                c2 = rng.choice(nxt)
                z = c2.targets[rng.randrange(len(c2.targets))]
                if z != a:
                    out.append(_route(f"silver:route:chain:{c1.id}>{c2.id}", a, z, [(c1, a, mid, "forward"),
                                      (c2, mid, z, "forward")], "transitive_chain", c1))
        sibs = [c for c in store.claims_for(mid, roles=[Role.OBJECT, Role.PREMISE], relations=[rel])
                if c.id != c1.id and c.subject != a]
        if sibs:
            c2 = rng.choice(sibs)
            out.append(_route(f"silver:route:shared:{c1.id}<{c2.id}", a, c2.subject,
                              [(c1, a, mid, "forward"), (c2, mid, c2.subject, "inverse")], "shared_target", c1))
        if rel is RelationType.DEPENDS_ON_LOGICALLY:
            out.append(_route(f"silver:route:reverse:{c1.id}", mid, a, [(c1, mid, a, "inverse")],
                              "reverse_dependency", c1))
    return out


def _route(iid: str, src: str, dst: str, hops: list[tuple[Claim, str, str, str]], attack: str, c1: Claim) -> Item:
    chain = [compose.Hop(c.relation.value, d) for c, _, _, d in hops]
    got = compose.conclude(chain)
    allowed = [] if got == "none" else [got]
    ceiling = compose.strength(got) if allowed else -1
    forbidden = sorted(k for k in compose.table()["conclusions"]
                       if k not in allowed and k != "none" and (compose.is_mathematical(k) or
                                                                 compose.strength(k) > ceiling))
    steps = [{"from": {"id": f}, "relation": c.relation.value, "to": {"id": t}, "direction": d,
              "label": "valid" if compose.step(compose.Hop(c.relation.value, d))[0] != "none" else "invalid",
              "why": f"claim {c.id} ({c.evidence_state.value})", "claim_id": c.id} for c, f, t, d in hops]
    return _item(iid, "route", {"from": {"id": src}, "to": {"id": dst}, "objective": "best_supported",
                                "answerable": bool(allowed), "allowed": allowed, "forbidden": forbidden,
                                "attack": None if allowed else attack, "steps": steps,
                                "forbidden_scope": "reference_path"},
                 strata={"attack": attack, "relation": c1.relation.value, "adversarial": not allowed,
                         "area": area_of(c1)},
                 doc=document_of(c1), fam=src, time=_date(c1),
                 notes="labels derived from shapes/composition.json by bench.compose: the same table the path "
                       "engine loads, so this item tests SPEC conformance, not mathematical validity")


# ------------------------------------------------------------------ entry point

def build(store: Store, *, seed: int = 20261016, caps: dict[str, int] | None = None,
          progress: Callable[[str], None] = lambda s: None) -> tuple[list[Item], dict]:
    pools = _Pools(random.Random(seed), {**DEFAULT_CAPS, **(caps or {})})
    info = _entity_pass(store, pools)
    progress(f"entities scanned: {info['entities']}")
    n_claims = _claim_pass(store, pools)
    progress(f"current claims scanned: {n_claims}")
    items = _retrieval_from_entities(pools) + _formal_items(store, pools)
    items += _er_items(store, info, pools)
    items += _relation_items(store, pools)
    items += _sourced_items(pools)
    items += _route_items(store, pools)
    items = _dedup(items)
    stats = {"entities_scanned": info["entities"], "current_claims_scanned": n_claims, "seed": seed,
             "caps": pools.caps, "items": len(items)}
    return items, stats


def _dedup(items: Iterable[Item]) -> list[Item]:
    seen: dict[str, Item] = {}
    for it in items:
        seen.setdefault(it.id, it)
    return list(seen.values())
