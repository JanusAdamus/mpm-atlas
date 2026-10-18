"""Shared plumbing for ingest adapters: idempotent batched writes, endpoint checks,
counters and stage checkpoints.  Adapters only talk to the store through its API."""

from __future__ import annotations

import json
import time
from collections import Counter
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable, Iterator

from ..model import RELATIONS, Claim, Entity, EntityType, Role, Source
from ..store import Store

DEFAULT_CACHE = Path(__file__).resolve().parents[2] / "data" / "cache"
FLUSH = 20_000


@dataclass
class IngestReport:
    source: str
    counters: Counter = field(default_factory=Counter)
    timings: dict[str, float] = field(default_factory=dict)
    failures: list[str] = field(default_factory=list)  # first N problems, for the log

    def note(self, problem: str, limit: int = 200) -> None:
        self.counters["problems"] += 1
        if len(self.failures) < limit:
            self.failures.append(problem)

    def to_json(self) -> dict:
        return {"source": self.source, "counters": dict(sorted(self.counters.items())),
                "timings_s": {k: round(v, 2) for k, v in self.timings.items()}, "failures": self.failures[:50]}


class Timer:
    def __init__(self, report: IngestReport, name: str) -> None:
        self.report, self.name = report, name

    def __enter__(self) -> "Timer":
        self.t0 = time.perf_counter()
        return self

    def __exit__(self, *exc: object) -> None:
        self.report.timings[self.name] = self.report.timings.get(self.name, 0.0) + time.perf_counter() - self.t0


def ensure_source(store: Store, source: Source) -> None:
    """Register the source once; later runs keep the first registration (sources are immutable)."""
    if store.get_source(source.id) is None:
        store.add_source(source, actor="atlas.ingest")


def endpoint_ok(relation, subject_type: EntityType, object_type: EntityType) -> bool:
    spec = RELATIONS[relation]
    return ((spec.subject_types is None or subject_type in spec.subject_types)
            and (spec.object_types is None or object_type in spec.object_types))


class Writer:
    """Buffers entities and claims and writes them idempotently in large batches.

    Entities: `on_conflict='ignore'` (first writer wins, re-runs are no-ops).
    Claims: deterministic ids; ids already in the store are skipped, so a resumed
    run never fails on duplicates.  Claims are validated for endpoint types here
    (using RELATIONS) so one bad row cannot roll back a 20k batch.
    """

    def __init__(self, store: Store, report: IngestReport, *, actor: str = "atlas.ingest") -> None:
        self.store, self.report, self.actor = store, report, actor
        self._entities: dict[str, Entity] = {}
        self._claims: dict[str, Claim] = {}
        self.types: dict[str, EntityType] = {}  # type of every entity this writer knows about

    def entity(self, e: Entity) -> None:
        if e.id in self.types:
            return
        self.types[e.id] = e.type
        self._entities[e.id] = e
        if len(self._entities) >= FLUSH:
            self.flush_entities()

    def know(self, entity_id: str, etype: EntityType) -> None:
        """Declare an entity that already exists in the store (from an earlier stage or run)."""
        self.types.setdefault(entity_id, etype)

    def claim(self, c: Claim) -> bool:
        for p in c.participants:
            if p.entity_id not in self.types:
                self.report.counters[f"claims_dropped:missing_endpoint:{c.relation.value}"] += 1
                return False
        subj = self.types[c.subject]
        for t in c.targets:
            if not endpoint_ok(c.relation, subj, self.types[t]):
                self.report.counters[f"claims_dropped:endpoint_type:{c.relation.value}"] += 1
                return False
        if c.id in self._claims:
            return False
        self._claims[c.id] = c
        if len(self._claims) >= FLUSH:
            self.flush()
        return True

    def flush_entities(self) -> None:
        if self._entities:
            n = self.store.add_entities(list(self._entities.values()), actor=self.actor, on_conflict="ignore")
            self.report.counters["entities_written"] += n
            self._entities = {}

    def flush(self) -> None:
        self.flush_entities()
        if self._claims:
            present = self.store.existing_claim_ids(self._claims)
            fresh = [c for c in self._claims.values() if c.id not in present]
            self.report.counters["claims_already_present"] += len(self._claims) - len(fresh)
            n = self.store.add_claims(fresh, actor=self.actor)
            self.report.counters["claims_written"] += n
            self._claims = {}


class Checkpoint:
    """Per-source stage markers in `data/cache/checkpoints/<source>.json`.  A finished stage
    is skipped on the next run; an unfinished one re-runs from the HTTP cache."""

    def __init__(self, cache_root: Path, source: str) -> None:
        self.path = cache_root / "checkpoints" / f"{source}.json"
        self.state: dict = json.loads(self.path.read_text(encoding="utf-8")) if self.path.exists() else {}

    def done(self, stage: str) -> bool:
        return stage in self.state

    def get(self, stage: str):
        return self.state.get(stage)

    def mark(self, stage: str, value: object = True) -> None:
        self.state = {**self.state, stage: value}
        self.path.parent.mkdir(parents=True, exist_ok=True)
        tmp = self.path.with_suffix(".tmp")
        tmp.write_text(json.dumps(self.state, ensure_ascii=False, indent=1), encoding="utf-8")
        tmp.replace(self.path)


def batched(items: Iterable, n: int) -> Iterator[list]:
    buf: list = []
    for x in items:
        buf.append(x)
        if len(buf) >= n:
            yield buf
            buf = []
    if buf:
        yield buf


def subject_of(c: Claim) -> str:
    return next(p.entity_id for p in c.participants if p.role is Role.SUBJECT)
