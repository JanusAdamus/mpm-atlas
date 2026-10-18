"""AtlasBench item format `atlasbench/1` (bench/PROTOCOL.md §2): validation, refs, JSONL I/O, hashing."""

from __future__ import annotations

import hashlib
import json
from collections import Counter
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Any, Iterable, Iterator, Mapping

FORMAT = "atlasbench/1"
TASKS = ("er_pair", "relation", "retrieval", "route", "sourced_answer")
ORIGINS = ("silver", "handwritten", "expert")
LABEL_STATUSES = ("silver_auto", "authored_by_agent_pending_expert_review", "expert_single", "expert_adjudicated",
                  "unlabeled")
GOLD_STATUSES = frozenset({"expert_adjudicated"})
SPLIT_KEYS = ("document", "family", "time")
MODALITIES = ("text", "formula", "lean_name", "informal_to_formal")
ER_LABELS = ("same", "different", None)
REL_LABELS = ("correct", "incorrect", None)
STEP_LABELS = ("valid", "invalid")
DIRECTIONS = ("forward", "inverse")


class ItemError(ValueError):
    """An item breaks the atlasbench/1 format."""


def _need(cond: bool, msg: str) -> None:
    if not cond:
        raise ItemError(msg)


def check_ref(ref: Any, where: str) -> None:
    _need(isinstance(ref, Mapping), f"{where}: ref must be an object")
    if "id" in ref:
        _need(isinstance(ref["id"], str) and ref["id"], f"{where}: empty id")
    else:
        ext = ref.get("ext")
        _need(isinstance(ext, list) and len(ext) == 2 and all(isinstance(x, str) and x for x in ext),
              f"{where}: ref needs 'id' or 'ext': [scheme, value]")


def ref_key(ref: Mapping[str, Any]) -> str:
    """Stable text key of a ref (used for split families and dedup)."""
    return ref["id"] if "id" in ref else f"{ref['ext'][0]}={ref['ext'][1]}"


def _check_er(p: Mapping[str, Any]) -> None:
    check_ref(p.get("a"), "a")
    check_ref(p.get("b"), "b")
    _need(p.get("label") in ER_LABELS, f"er label {p.get('label')!r}")
    _need(isinstance(p.get("hardness"), str), "er hardness missing")


def _check_relation(p: Mapping[str, Any]) -> None:
    _need(isinstance(p.get("claim_id"), str), "relation claim_id missing")
    _need(isinstance(p.get("relation"), str) and isinstance(p.get("family"), str), "relation/family missing")
    _need(p.get("label") in REL_LABELS, f"relation label {p.get('label')!r}")


def _check_retrieval(p: Mapping[str, Any]) -> None:
    _need(isinstance(p.get("query"), str) and p["query"].strip(), "empty query")
    _need(p.get("lang") in ("es", "en"), f"lang {p.get('lang')!r}")
    _need(p.get("modality") in MODALITIES, f"modality {p.get('modality')!r}")
    qrels = p.get("qrels")
    _need(isinstance(qrels, list) and qrels, "qrels must be a non-empty list")
    for i, q in enumerate(qrels):
        check_ref(q.get("ref"), f"qrels[{i}]")
        _need(q.get("grade") in (0, 1, 2, 3), f"qrels[{i}] grade {q.get('grade')!r}")
    _need(any(q["grade"] >= 2 for q in qrels), "a retrieval item needs one qrel with grade >= 2")


# route_pair: no route between the endpoints may conclude these; reference_path: only the item's own steps
FORBIDDEN_SCOPES = ("route_pair", "reference_path")


def _check_route(p: Mapping[str, Any]) -> None:
    check_ref(p.get("from"), "from")
    check_ref(p.get("to"), "to")
    _need(isinstance(p.get("answerable"), bool), "route answerable must be bool")
    for k in ("allowed", "forbidden"):
        _need(isinstance(p.get(k), list) and all(isinstance(x, str) for x in p[k]), f"route {k} must be a list")
    _need(not set(p["allowed"]) & set(p["forbidden"]), "allowed and forbidden overlap")
    _need(p["answerable"] == bool(p["allowed"]), "answerable routes need allowed conclusions, others none")
    _need(p.get("forbidden_scope", "route_pair") in FORBIDDEN_SCOPES, f"route forbidden_scope in {FORBIDDEN_SCOPES}")
    for i, s in enumerate(p.get("steps", [])):
        check_ref(s.get("from"), f"steps[{i}].from")
        check_ref(s.get("to"), f"steps[{i}].to")
        _need(isinstance(s.get("relation"), str), f"steps[{i}].relation")
        _need(s.get("direction", "forward") in DIRECTIONS, f"steps[{i}].direction")
        _need(s.get("label") in STEP_LABELS, f"steps[{i}].label")


def _check_sourced(p: Mapping[str, Any]) -> None:
    check_ref(p.get("entity"), "entity")
    _need(isinstance(p.get("claim_id"), str), "sourced claim_id")
    exp = p.get("expected")
    _need(isinstance(exp, list) and exp and all("source_id" in e and "locator" in e for e in exp),
          "sourced expected must list {source_id, locator}")


CHECKS = {"er_pair": _check_er, "relation": _check_relation, "retrieval": _check_retrieval, "route": _check_route,
          "sourced_answer": _check_sourced}


@dataclass(frozen=True)
class Item:
    id: str
    task: str
    origin: str
    label_status: str
    author: str
    created: str
    payload: Mapping[str, Any]
    strata: Mapping[str, Any] = field(default_factory=dict)
    split_keys: Mapping[str, str] = field(default_factory=dict)
    notes: str = ""

    def __post_init__(self) -> None:
        _need(bool(self.id), "item id is empty")
        _need(self.task in TASKS, f"{self.id}: task {self.task!r}")
        _need(self.origin in ORIGINS, f"{self.id}: origin {self.origin!r}")
        _need(self.label_status in LABEL_STATUSES, f"{self.id}: label_status {self.label_status!r}")
        _need(self.origin != "silver" or self.label_status in ("silver_auto", "unlabeled"),
              f"{self.id}: silver items cannot claim expert labels")
        _need(self.origin != "handwritten" or self.label_status != "silver_auto", f"{self.id}: handwritten != silver")
        _need(len(self.created) >= 10, f"{self.id}: created must be an ISO date")
        for k in SPLIT_KEYS:
            _need(isinstance(self.split_keys.get(k), str) and self.split_keys[k], f"{self.id}: split_keys.{k} missing")
        try:
            CHECKS[self.task](self.payload)
        except ItemError as exc:
            raise ItemError(f"{self.id}: {exc}") from None

    @property
    def is_gold(self) -> bool:
        return self.label_status in GOLD_STATUSES

    def to_json(self) -> dict[str, Any]:
        return {"format": FORMAT, "id": self.id, "task": self.task, "origin": self.origin,
                "label_status": self.label_status, "author": self.author, "created": self.created,
                "strata": dict(self.strata), "split_keys": dict(self.split_keys), "payload": _plain(self.payload),
                "notes": self.notes}

    @staticmethod
    def from_json(d: Mapping[str, Any]) -> Item:
        _need(d.get("format") == FORMAT, f"{d.get('id')}: format {d.get('format')!r}, expected {FORMAT}")
        return Item(d["id"], d["task"], d["origin"], d["label_status"], d.get("author", ""), d.get("created", ""),
                    d["payload"], d.get("strata", {}), d.get("split_keys", {}), d.get("notes", ""))

    def with_label_status(self, status: str) -> Item:
        return replace(self, label_status=status)


def _plain(obj: Any) -> Any:
    if isinstance(obj, Mapping):
        return {k: _plain(v) for k, v in obj.items()}
    if isinstance(obj, (list, tuple)):
        return [_plain(v) for v in obj]
    return obj


def canonical(item: Item) -> str:
    return json.dumps(item.to_json(), ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def item_hash(item: Item) -> str:
    return hashlib.sha256(canonical(item).encode("utf-8")).hexdigest()


def read_jsonl(path: Path | str) -> list[Item]:
    items: list[Item] = []
    with open(path, encoding="utf-8") as fh:
        for n, line in enumerate(fh, 1):
            if not line.strip():
                continue
            try:
                items.append(Item.from_json(json.loads(line)))
            except (ItemError, KeyError, json.JSONDecodeError) as exc:
                raise ItemError(f"{path}:{n}: {exc}") from None
    dup = [k for k, n in Counter(i.id for i in items).items() if n > 1]
    _need(not dup, f"{path}: duplicate ids {sorted(dup)[:5]}")
    return items


def write_jsonl(path: Path | str, items: Iterable[Item]) -> int:
    p = Path(path)
    p.parent.mkdir(parents=True, exist_ok=True)
    n = 0
    with open(p, "w", encoding="utf-8", newline="\n") as fh:
        for it in items:
            fh.write(canonical(it) + "\n")
            n += 1
    return n


def iter_dir(root: Path | str) -> Iterator[Item]:
    """All items of every *.jsonl under root, in path order."""
    for path in sorted(Path(root).rglob("*.jsonl")):
        yield from read_jsonl(path)
