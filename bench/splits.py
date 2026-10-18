"""Grouped, time-aware splits with a sealed final split and a hash manifest (PROTOCOL.md §3)."""

from __future__ import annotations

import hashlib
import json
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Sequence

from .items import Item, item_hash, read_jsonl, write_jsonl

SPLITS = ("dev", "test", "sealed")
FRACTIONS = {"dev": 0.4, "test": 0.4, "sealed": 0.2}
SALT = "atlasbench-split-v1"
LEDGER = "sealed-ledger.jsonl"


class _UnionFind:
    def __init__(self) -> None:
        self.parent: dict[str, str] = {}

    def find(self, x: str) -> str:
        self.parent.setdefault(x, x)
        root = x
        while self.parent[root] != root:
            root = self.parent[root]
        while self.parent[x] != root:  # path compression
            self.parent[x], x = root, self.parent[x]
        return root

    def union(self, a: str, b: str) -> None:
        ra, rb = self.find(a), self.find(b)
        if ra != rb:
            self.parent[max(ra, rb)] = min(ra, rb)


def _unit(key: str) -> float:
    return int(hashlib.sha256(f"{SALT}|{key}".encode()).hexdigest()[:12], 16) / float(1 << 48)


def components(items: Sequence[Item]) -> dict[str, list[Item]]:
    """Items joined when they share a document or a family key (task-scoped keys)."""
    uf = _UnionFind()
    for it in items:
        doc, fam = f"doc:{it.split_keys['document']}", f"fam:{it.split_keys['family']}"
        uf.union(doc, fam)
    comps: dict[str, list[Item]] = defaultdict(list)
    for it in items:
        comps[uf.find(f"doc:{it.split_keys['document']}")].append(it)
    return comps


def assign(items: Sequence[Item], cutoff: str) -> dict[str, list[Item]]:
    """dev/test/sealed by stable hash of each component; components with items after `cutoff` skip dev."""
    out: dict[str, list[Item]] = {s: [] for s in SPLITS}
    for root, members in sorted(components(items).items()):
        u = _unit(root)
        split = "dev" if u < FRACTIONS["dev"] else "test" if u < FRACTIONS["dev"] + FRACTIONS["test"] else "sealed"
        if split == "dev" and any(m.split_keys["time"][:10] > cutoff for m in members):
            split = "test"
        out[split].extend(sorted(members, key=lambda m: m.id))
    return out


def leaks(splits: dict[str, Sequence[Item]]) -> list[str]:
    """Document or family keys present in more than one split (must be empty)."""
    where: dict[tuple[str, str], set[str]] = defaultdict(set)
    for name, items in splits.items():
        for it in items:
            for k in ("document", "family"):
                where[(k, it.split_keys[k])].add(name)
    return sorted(f"{k}={v} in {sorted(s)}" for (k, v), s in where.items() if len(s) > 1)


def write_splits(items: Sequence[Item], out_dir: Path, manifest_path: Path, *, version: str, cutoff: str,
                 inputs: Iterable[str] = (), root: Path | None = None, extra: dict | None = None) -> dict:
    """Write dev/test/sealed JSONL + manifest.  File paths are stored relative to `root` when given."""
    splits = assign(items, cutoff)
    bad = leaks(splits)
    if bad:
        raise RuntimeError(f"split leakage: {bad[:5]}")
    sizes = sorted((len(m) for m in components(items).values()), reverse=True)
    manifest = {"version": version, "format": "atlasbench/1", "created": datetime.now(timezone.utc).isoformat(),
                "cutoff": cutoff, "salt": SALT, "fractions": FRACTIONS, "inputs": sorted(inputs),
                "components": {"count": len(sizes), "largest": sizes[0] if sizes else 0,
                               "largest_share": (sizes[0] / len(items)) if items else 0.0},
                "splits": {}, **(extra or {})}
    for name, members in splits.items():
        write_jsonl(out_dir / f"{name}.jsonl", members)
        hashes = {m.id: item_hash(m) for m in members}
        manifest["splits"][name] = {
            "file": _rel(out_dir / f"{name}.jsonl", root), "count": len(members),
            "by_task": dict(Counter(m.task for m in members)),
            "by_origin": dict(Counter(f"{m.origin}/{m.label_status}" for m in members)),
            "set_sha256": _set_hash(hashes.values()), "items": hashes}
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=1, sort_keys=True), encoding="utf-8")
    return manifest


def _rel(path: Path, root: Path | None) -> str:
    return path.relative_to(root).as_posix() if root is not None and path.is_relative_to(root) else path.as_posix()


def _set_hash(hashes: Iterable[str]) -> str:
    return hashlib.sha256("\n".join(sorted(hashes)).encode()).hexdigest()


def verify(manifest_path: Path, root: Path) -> list[str]:
    """Problems found re-hashing every split file against the manifest (empty = intact)."""
    man = json.loads(manifest_path.read_text(encoding="utf-8"))
    problems: list[str] = []
    for name, info in man["splits"].items():
        path = Path(info["file"])
        path = path if path.is_absolute() else root / path
        if not path.exists():
            problems.append(f"{name}: missing file {path}")
            continue
        got = {it.id: item_hash(it) for it in read_jsonl(path)}
        for iid, h in info["items"].items():
            if iid not in got:
                problems.append(f"{name}: item {iid} removed")
            elif got[iid] != h:
                problems.append(f"{name}: item {iid} changed")
        problems += [f"{name}: item {iid} added" for iid in sorted(set(got) - set(info["items"]))]
        if not problems and _set_hash(got.values()) != info["set_sha256"]:
            problems.append(f"{name}: set hash mismatch")
    return problems


def log_sealed_opening(bench_root: Path, *, reason: str, manifest_version: str, system: str) -> int:
    """Append an opening of the sealed split to the ledger; returns how many openings exist now."""
    if not reason.strip():
        raise ValueError("opening the sealed split needs a reason")
    path = bench_root / LEDGER
    entry = {"ts": datetime.now(timezone.utc).isoformat(), "manifest": manifest_version, "system": system,
             "reason": reason}
    with open(path, "a", encoding="utf-8") as fh:
        fh.write(json.dumps(entry, ensure_ascii=False) + "\n")
    return sealed_openings(bench_root)


def sealed_openings(bench_root: Path) -> int:
    path = bench_root / LEDGER
    if not path.exists():
        return 0
    return sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line.strip())
