"""SQLite registry of the atlas: the source of truth (claims + events).

Indexes (FTS5), layouts and embeddings are derived and rebuildable.  One Store
object per thread; open extra read-only Stores for concurrent readers (WAL).

Over the 800-line soft ceiling on purpose: this file is the whole persistence
contract (DDL, writes, reads, merges, FTS, JSON-LD + its validator) and only the
architect edits it.  Split JSON-LD out first if it grows.
"""

from __future__ import annotations

import json
import re
import sqlite3
from collections import deque
from contextlib import contextmanager
from dataclasses import replace
from datetime import datetime
from pathlib import Path
from typing import Any, Iterable, Iterator, Mapping, Sequence
from urllib.parse import quote

from .model import (
    CURRENT_STATES,
    RELATIONS,
    SCHEMA_VERSION,
    TRANSITIONS,
    Claim,
    Edge,
    Entity,
    EntityType,
    Event,
    EventKind,
    EvidenceState,
    FormalArtifact,
    Merge,
    RelationType,
    Role,
    SearchHit,
    Source,
    ValidationError,
    now_iso,
)

CONTEXT_PATH = Path(__file__).resolve().parent.parent / "shapes" / "context.jsonld"
SHACL_PATH = Path(__file__).resolve().parent.parent / "shapes" / "atlas.shacl.ttl"
FTS_SLOTS = 1024  # max FTS rows per entity; fts rowid = entity_rowid * FTS_SLOTS + k
FIELD_WEIGHT = {"label": 3.0, "alias": 2.0, "external_id": 2.0, "description": 1.0, "statement": 1.0}
CACHE_KIB = 256 * 1024  # page cache per connection; index inserts thrash with the 2 MiB default
_CHUNK = 900  # stay under SQLite's host-parameter limit

_DDL_V1 = """
CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE sources(id TEXT PRIMARY KEY, data TEXT NOT NULL, created_at TEXT NOT NULL);
CREATE TABLE entities(
  id TEXT PRIMARY KEY, type TEXT NOT NULL, data TEXT NOT NULL,
  merged_into TEXT REFERENCES entities(id), created_at TEXT NOT NULL);
CREATE INDEX entities_type ON entities(type);
CREATE INDEX entities_merged ON entities(merged_into) WHERE merged_into IS NOT NULL;
CREATE TABLE external_ids(scheme TEXT NOT NULL, value TEXT NOT NULL,
  entity_id TEXT NOT NULL REFERENCES entities(id), PRIMARY KEY(scheme, value, entity_id)) WITHOUT ROWID;
CREATE TABLE claims(
  id TEXT PRIMARY KEY, relation TEXT NOT NULL, evidence_state TEXT NOT NULL,
  alternative_group TEXT, supersedes TEXT REFERENCES claims(id), superseded_by TEXT REFERENCES claims(id),
  formal TEXT, data TEXT NOT NULL, created_at TEXT NOT NULL);
CREATE INDEX claims_relation ON claims(relation, evidence_state);
CREATE INDEX claims_alt ON claims(alternative_group) WHERE alternative_group IS NOT NULL;
CREATE TABLE claim_participants(
  claim_id TEXT NOT NULL REFERENCES claims(id), role TEXT NOT NULL,
  entity_id TEXT NOT NULL REFERENCES entities(id), position INTEGER NOT NULL,
  PRIMARY KEY(claim_id, position)) WITHOUT ROWID;
CREATE INDEX cp_entity ON claim_participants(entity_id, role);
CREATE TABLE provenance(
  claim_id TEXT REFERENCES claims(id), entity_id TEXT REFERENCES entities(id),
  source_id TEXT NOT NULL REFERENCES sources(id), locator TEXT NOT NULL, extractor TEXT NOT NULL,
  extractor_version TEXT NOT NULL, license TEXT NOT NULL, retrieved_at TEXT NOT NULL, quote TEXT,
  CHECK ((claim_id IS NULL) != (entity_id IS NULL)));
CREATE INDEX prov_claim ON provenance(claim_id) WHERE claim_id IS NOT NULL;
CREATE INDEX prov_entity ON provenance(entity_id) WHERE entity_id IS NOT NULL;
CREATE INDEX prov_source ON provenance(source_id);
CREATE TABLE events(
  seq INTEGER PRIMARY KEY AUTOINCREMENT, ts TEXT NOT NULL, actor TEXT NOT NULL,
  kind TEXT NOT NULL, target_id TEXT NOT NULL, payload TEXT NOT NULL);
CREATE INDEX events_target ON events(target_id);
CREATE TRIGGER events_no_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;
CREATE TRIGGER events_no_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;
CREATE TABLE merges(
  id INTEGER PRIMARY KEY, survivor_id TEXT NOT NULL REFERENCES entities(id),
  merged_id TEXT NOT NULL REFERENCES entities(id), actor TEXT NOT NULL, reason TEXT NOT NULL,
  evidence TEXT, created_at TEXT NOT NULL, undone_at TEXT);
CREATE TABLE relation_endpoints(relation TEXT NOT NULL, side TEXT NOT NULL, entity_type TEXT NOT NULL,
  PRIMARY KEY(relation, side, entity_type)) WITHOUT ROWID;
CREATE VIRTUAL TABLE fts USING fts5(
  target_id UNINDEXED, field UNINDEXED, lang UNINDEXED, text,
  tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3');
"""

# version -> DDL applied when upgrading from version-1.  Append, never edit.
MIGRATIONS: dict[int, str] = {1: _DDL_V1}


def _dumps(obj: Any) -> str:
    return json.dumps(obj, ensure_ascii=False, separators=(",", ":"))


def _compact(d: Mapping[str, Any]) -> dict[str, Any]:
    """Drop None/empty top-level fields before storing (from_json fills defaults)."""
    return {k: v for k, v in d.items() if v not in (None, [], {}, ())}


def _chunks(seq: Sequence[Any], n: int = _CHUNK) -> Iterator[Sequence[Any]]:
    for i in range(0, len(seq), n):
        yield seq[i : i + n]


class Store:
    """The atlas registry.  See docs/SPEC.md §API for the contract."""

    def __init__(self, path: str | Path = ":memory:", *, read_only: bool = False) -> None:
        self.path = str(path)
        if read_only:
            uri = Path(path).resolve().as_uri() + "?mode=ro"
            self._conn = sqlite3.connect(uri, uri=True, autocommit=True)
        else:
            if self.path != ":memory:":
                Path(self.path).parent.mkdir(parents=True, exist_ok=True)
            self._conn = sqlite3.connect(self.path, autocommit=True)
        self._conn.execute("PRAGMA foreign_keys = ON")
        self._conn.execute("PRAGMA busy_timeout = 5000")
        if not read_only:
            self._conn.execute("PRAGMA journal_mode = WAL")
            self._conn.execute("PRAGMA synchronous = NORMAL")
            self._conn.execute(f"PRAGMA cache_size = {-CACHE_KIB}")
            self._conn.execute("PRAGMA temp_store = MEMORY")
        self._depth = 0
        self._redirect_cache: dict[str, str] | None = None
        self._redirect_version = -1
        if not read_only:
            self.migrate()

    # ------------------------------------------------------------------ lifecycle
    def close(self) -> None:
        self._conn.close()

    def __enter__(self) -> Store:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    @property
    def schema_version(self) -> int:
        return self._conn.execute("PRAGMA user_version").fetchone()[0]

    def migrate(self) -> int:
        """Apply pending MIGRATIONS in order; each one logs a MIGRATION event."""
        current = self.schema_version
        for version in sorted(v for v in MIGRATIONS if v > current):
            self._conn.execute("BEGIN IMMEDIATE")
            try:
                for stmt in _split_sql(MIGRATIONS[version]):
                    self._conn.execute(stmt)
                if version == 1:
                    self._fill_relation_endpoints()
                self._conn.execute(f"PRAGMA user_version = {int(version)}")
                self._log([(EventKind.MIGRATION, f"schema:{version}", {"from": current, "to": version})], "system")
                self._conn.execute("COMMIT")
            except BaseException:
                self._conn.execute("ROLLBACK")
                raise
            current = version
        return current

    def _fill_relation_endpoints(self) -> None:
        rows = [
            (spec.name.value, side, t.value)
            for spec in RELATIONS.values()
            for side, types in (("subject", spec.subject_types), ("object", spec.object_types))
            if types is not None
            for t in types
        ]
        self._conn.executemany("INSERT INTO relation_endpoints VALUES (?,?,?)", rows)

    @contextmanager
    def transaction(self) -> Iterator[None]:
        """Atomic unit; nests (inner blocks join the outer transaction)."""
        if self._depth:
            self._depth += 1
            try:
                yield
            finally:
                self._depth -= 1
            return
        self._conn.execute("BEGIN IMMEDIATE")
        self._depth = 1
        try:
            yield
        except BaseException:
            self._conn.execute("ROLLBACK")
            self._redirect_cache = None
            raise
        else:
            self._conn.execute("COMMIT")
        finally:
            self._depth = 0

    def _log(self, items: Iterable[tuple[EventKind, str, Mapping[str, Any]]], actor: str) -> None:
        ts = now_iso()
        self._conn.executemany(
            "INSERT INTO events(ts, actor, kind, target_id, payload) VALUES (?,?,?,?,?)",
            ((ts, actor, k.value, t, _dumps(p)) for k, t, p in items),
        )

    # ------------------------------------------------------------------ sources
    def add_source(self, source: Source, *, actor: str = "system") -> None:
        with self.transaction():
            try:
                self._conn.execute("INSERT INTO sources VALUES (?,?,?)",
                                   (source.id, _dumps(source.__dict__), now_iso()))
            except sqlite3.IntegrityError as exc:
                raise ValidationError(f"source {source.id} already exists") from exc
            self._log([(EventKind.SOURCE_ADDED, source.id, {"license": source.license})], actor)

    def get_source(self, source_id: str) -> Source | None:
        row = self._conn.execute("SELECT data FROM sources WHERE id=?", (source_id,)).fetchone()
        return Source(**json.loads(row[0])) if row else None

    def iter_sources(self) -> Iterator[Source]:
        for (data,) in self._conn.execute("SELECT data FROM sources ORDER BY id"):
            yield Source(**json.loads(data))

    # ------------------------------------------------------------------ entities
    def add_entity(self, entity: Entity, *, actor: str = "system", on_conflict: str = "error") -> None:
        self.add_entities([entity], actor=actor, on_conflict=on_conflict)

    def add_entities(self, entities: Iterable[Entity], *, actor: str = "system",
                     on_conflict: str = "error", batch_size: int = 20_000) -> int:
        """Bulk insert.  on_conflict: 'error' | 'ignore' (keep existing) | 'replace' (same type only).
        Returns the number of entities inserted or replaced."""
        if on_conflict not in ("error", "ignore", "replace"):
            raise ValueError("on_conflict must be 'error', 'ignore' or 'replace'")
        total = 0
        batch: list[Entity] = []
        for e in entities:
            batch.append(e)
            if len(batch) >= batch_size:
                total += self._insert_entities(batch, actor, on_conflict)
                batch = []
        if batch:
            total += self._insert_entities(batch, actor, on_conflict)
        return total

    def _insert_entities(self, batch: list[Entity], actor: str, on_conflict: str) -> int:
        ts = now_iso()
        with self.transaction():
            existing: dict[str, tuple[int, str]] = {}
            if on_conflict != "error":
                for part in _chunks([e.id for e in batch]):
                    q = f"SELECT id, rowid, type FROM entities WHERE id IN ({','.join('?' * len(part))})"
                    existing.update({i: (r, t) for i, r, t in self._conn.execute(q, part)})
            new = [e for e in batch if e.id not in existing]
            repl = [e for e in batch if e.id in existing] if on_conflict == "replace" else []
            for e in repl:
                if existing[e.id][1] != e.type.value:
                    raise ValidationError(f"entity {e.id}: replace cannot change type {existing[e.id][1]} -> {e.type}")
            lo = self._conn.execute("SELECT COALESCE(MAX(rowid), 0) FROM entities").fetchone()[0]
            try:
                self._conn.executemany(
                    "INSERT INTO entities(id, type, data, created_at) VALUES (?,?,?,?)",
                    ((e.id, e.type.value, _dumps(_compact(e.to_json())), ts) for e in new))
            except sqlite3.IntegrityError as exc:
                raise ValidationError(f"entity insert failed: {exc}") from exc
            self._conn.executemany("UPDATE entities SET data=? WHERE id=?",
                                   ((_dumps(_compact(e.to_json())), e.id) for e in repl))
            rowids = dict(self._conn.execute("SELECT id, rowid FROM entities WHERE rowid > ?", (lo,)))
            rowids.update({e.id: existing[e.id][0] for e in repl})
            if repl:
                ids = [e.id for e in repl]
                self._conn.executemany("DELETE FROM fts WHERE rowid BETWEEN ? AND ?",
                                       ((rowids[i] * FTS_SLOTS, rowids[i] * FTS_SLOTS + FTS_SLOTS - 1) for i in ids))
                self._conn.executemany("DELETE FROM external_ids WHERE entity_id=?", ((i,) for i in ids))
                self._conn.executemany("DELETE FROM provenance WHERE entity_id=?", ((i,) for i in ids))
            touched = new + repl
            self._conn.executemany("INSERT INTO fts(rowid, target_id, field, lang, text) VALUES (?,?,?,?,?)",
                                   (row for e in touched for row in _fts_rows(e, rowids[e.id])))
            self._conn.executemany("INSERT OR IGNORE INTO external_ids VALUES (?,?,?)",
                                   ((s, v, e.id) for e in touched for s, v in e.external_ids))
            self._insert_provenance((("entity", e.id, p) for e in touched for p in e.provenance))
            self._log([(EventKind.ENTITY_ADDED, e.id, {"type": e.type.value}) for e in new]
                      + [(EventKind.ENTITY_REPLACED, e.id, {}) for e in repl], actor)
        return len(touched)

    def _insert_provenance(self, rows: Iterable[tuple[str, str, Any]]) -> None:
        try:
            self._conn.executemany(
                "INSERT INTO provenance VALUES (?,?,?,?,?,?,?,?,?)",
                ((tid if kind == "claim" else None, tid if kind == "entity" else None, p.source_id, p.locator,
                  p.extractor, p.extractor_version, p.license, p.retrieved_at, p.quote) for kind, tid, p in rows))
        except sqlite3.IntegrityError as exc:
            raise ValidationError(f"provenance refers to an unregistered source ({exc})") from exc

    def get_entity(self, entity_id: str, *, resolve: bool = True) -> Entity | None:
        eid = self.resolve(entity_id) if resolve else entity_id
        row = self._conn.execute("SELECT data FROM entities WHERE id=?", (eid,)).fetchone()
        return Entity.from_json(json.loads(row[0])) if row else None

    def iter_entities(self, type: EntityType | str | None = None, *, include_merged: bool = False,
                      batch: int = 5000) -> Iterator[Entity]:
        sql = "SELECT data FROM entities WHERE 1=1"
        args: list[Any] = []
        if type is not None:
            sql += " AND type=?"
            args.append(EntityType(type).value)
        if not include_merged:
            sql += " AND merged_into IS NULL"
        cur = self._conn.execute(sql + " ORDER BY rowid", args)
        while rows := cur.fetchmany(batch):
            for (data,) in rows:
                yield Entity.from_json(json.loads(data))

    def find_by_external_id(self, scheme: str, value: str) -> list[str]:
        rows = self._conn.execute("SELECT entity_id FROM external_ids WHERE scheme=? AND value=?", (scheme, value))
        return sorted({self.resolve(r[0]) for r in rows})

    # ------------------------------------------------------------------ claims
    def add_claim(self, claim: Claim, *, actor: str = "system") -> None:
        self.add_claims([claim], actor=actor)

    def add_claims(self, claims: Iterable[Claim], *, actor: str = "system", batch_size: int = 20_000) -> int:
        """Bulk insert in transactions of `batch_size`.  Each batch is atomic and checked for
        endpoint types and alternative-group consistency.  Returns the number inserted."""
        total = 0
        batch: list[Claim] = []
        for c in claims:
            batch.append(c)
            if len(batch) >= batch_size:
                total += self._insert_claims(batch, actor)
                batch = []
        if batch:
            total += self._insert_claims(batch, actor)
        return total

    def _insert_claims(self, batch: list[Claim], actor: str, *, allow_supersedes: bool = False) -> int:
        for c in batch:
            if c.supersedes is not None and not allow_supersedes:
                raise ValidationError(f"claim {c.id}: use Store.supersede() to create a new version")
            if c.evidence_state in (EvidenceState.SUPERSEDED, EvidenceState.RETRACTED):
                raise ValidationError(f"claim {c.id}: cannot be inserted as {c.evidence_state}")
        ts = now_iso()
        with self.transaction():
            lo = self._conn.execute("SELECT COALESCE(MAX(rowid), 0) FROM claims").fetchone()[0]
            try:
                self._conn.executemany(
                    "INSERT INTO claims VALUES (?,?,?,?,?,NULL,?,?,?)",
                    ((c.id, c.relation.value, c.evidence_state.value, c.alternative_group, c.supersedes,
                      None if c.formal is None else _dumps(_formal_json(c.formal)), _dumps(_compact(c.to_json())), ts)
                     for c in batch))
                self._conn.executemany(
                    "INSERT INTO claim_participants VALUES (?,?,?,?)",
                    ((c.id, p.role.value, p.entity_id, i) for c in batch for i, p in enumerate(c.participants)))
            except sqlite3.IntegrityError as exc:
                raise ValidationError(f"claim insert failed (duplicate id or unknown entity): {exc}") from exc
            self._insert_provenance((("claim", c.id, p) for c in batch for p in c.provenance))
            self._check_batch(lo)
            self._log(((EventKind.CLAIM_ADDED, c.id, {}) for c in batch), actor)
        return len(batch)

    def _check_batch(self, lo: int) -> None:
        bad = self._conn.execute(
            # CROSS JOIN pins the loop order: start from the new rows only (else O(table) per batch)
            """SELECT c.id, c.relation, p.role, e.type FROM claims c
               CROSS JOIN claim_participants p ON p.claim_id = c.id
               CROSS JOIN entities e ON e.id = p.entity_id
               WHERE c.rowid > ? AND p.role IN ('subject','object','premise')
               AND EXISTS (SELECT 1 FROM relation_endpoints r WHERE r.relation = c.relation
                           AND r.side = CASE p.role WHEN 'subject' THEN 'subject' ELSE 'object' END)
               AND NOT EXISTS (SELECT 1 FROM relation_endpoints r WHERE r.relation = c.relation
                           AND r.side = CASE p.role WHEN 'subject' THEN 'subject' ELSE 'object' END
                           AND r.entity_type = e.type)
               LIMIT 5""", (lo,)).fetchall()
        if bad:
            raise ValidationError("endpoint type not allowed: " + "; ".join(
                f"{cid} {rel} {role}={etype}" for cid, rel, role, etype in bad))
        mixed = self._conn.execute(
            """SELECT c.alternative_group FROM claims c
               JOIN claim_participants s ON s.claim_id = c.id AND s.role = 'subject'
               WHERE c.alternative_group IN (SELECT alternative_group FROM claims
                                            WHERE rowid > ? AND alternative_group IS NOT NULL)
               GROUP BY c.alternative_group
               HAVING COUNT(DISTINCT c.relation) > 1 OR COUNT(DISTINCT s.entity_id) > 1 LIMIT 5""",
            (lo,)).fetchall()
        if mixed:
            raise ValidationError("alternative_group must share subject and relation: "
                                  + ", ".join(r[0] for r in mixed))

    def existing_claim_ids(self, ids: Iterable[str]) -> set[str]:
        """The subset of `ids` already in the registry (any state), in chunked IN queries."""
        found: set[str] = set()
        for chunk in _chunks(list(dict.fromkeys(ids))):
            found.update(r[0] for r in self._conn.execute(
                f"SELECT id FROM claims WHERE id IN ({','.join('?' * len(chunk))})", tuple(chunk)))
        return found

    def get_claim(self, claim_id: str) -> Claim | None:
        row = self._conn.execute(
            "SELECT data, evidence_state, superseded_by, formal FROM claims WHERE id=?", (claim_id,)).fetchone()
        return _claim_from_row(row) if row else None

    def iter_claims(self, relation: RelationType | str | None = None, *,
                    states: Iterable[EvidenceState] | None = None, batch: int = 5000) -> Iterator[Claim]:
        """All claims (any state unless `states` is given), in insertion order."""
        sql, args = "SELECT data, evidence_state, superseded_by, formal FROM claims WHERE 1=1", []
        if relation is not None:
            sql += " AND relation=?"
            args.append(RelationType(relation).value)
        if states is not None:
            st = [EvidenceState(s).value for s in states]
            sql += f" AND evidence_state IN ({','.join('?' * len(st))})"
            args += st
        cur = self._conn.execute(sql + " ORDER BY rowid", args)
        while rows := cur.fetchmany(batch):
            yield from (_claim_from_row(r) for r in rows)

    def claims_for(self, entity_id: str, *, roles: Iterable[Role] | None = None,
                   relations: Iterable[RelationType] | None = None, current_only: bool = True) -> list[Claim]:
        """Claims where the entity (or anything merged into it) participates."""
        ids = self.merged_ids(entity_id)
        sql = (f"SELECT DISTINCT c.data, c.evidence_state, c.superseded_by, c.formal, c.rowid FROM claims c "
               f"JOIN claim_participants p ON p.claim_id = c.id WHERE p.entity_id IN ({','.join('?' * len(ids))})")
        args: list[Any] = list(ids)
        if roles is not None:
            rl = [Role(r).value for r in roles]
            sql += f" AND p.role IN ({','.join('?' * len(rl))})"
            args += rl
        if relations is not None:
            rel = [RelationType(r).value for r in relations]
            sql += f" AND c.relation IN ({','.join('?' * len(rel))})"
            args += rel
        if current_only:
            cs = [s.value for s in CURRENT_STATES]
            sql += f" AND c.evidence_state IN ({','.join('?' * len(cs))})"
            args += cs
        return [_claim_from_row(r[:4]) for r in self._conn.execute(sql + " ORDER BY c.rowid", args)]

    def alternatives(self, group: str) -> list[Claim]:
        rows = self._conn.execute("SELECT data, evidence_state, superseded_by, formal FROM claims "
                                  "WHERE alternative_group=? ORDER BY rowid", (group,))
        return [_claim_from_row(r) for r in rows]

    def iter_edges(self, *, relations: Iterable[RelationType] | None = None,
                   states: Iterable[EvidenceState] | None = CURRENT_STATES, resolve: bool = True) -> Iterator[Edge]:
        """Fast binary projection for graph algorithms; no Claim objects are built."""
        sql = ("SELECT c.id, c.relation, c.evidence_state, c.alternative_group, s.entity_id, t.entity_id, "
               "COUNT(*) OVER (PARTITION BY c.id) FROM claims c "
               "JOIN claim_participants s ON s.claim_id = c.id AND s.role = 'subject' "
               "JOIN claim_participants t ON t.claim_id = c.id AND t.role IN ('object','premise') WHERE 1=1")
        args: list[Any] = []
        if relations is not None:
            rel = [RelationType(r).value for r in relations]
            sql += f" AND c.relation IN ({','.join('?' * len(rel))})"
            args += rel
        if states is not None:
            st = [EvidenceState(s).value for s in states]
            sql += f" AND c.evidence_state IN ({','.join('?' * len(st))})"
            args += st
        red = self._redirects() if resolve else {}
        cur = self._conn.execute(sql, args)
        while rows := cur.fetchmany(10_000):
            for cid, rel, st, alt, s, t, n in rows:
                yield Edge(cid, RelationType(rel), _follow(red, s), _follow(red, t), EvidenceState(st), n, alt)

    # ------------------------------------------------------------------ evidence & versions
    def set_evidence_state(self, claim_id: str, state: EvidenceState | str, *, actor: str, reason: str,
                           formal: FormalArtifact | None = None) -> Claim:
        state = EvidenceState(state)
        if not reason.strip():
            raise ValidationError("a reason is required to change evidence state")
        if state is EvidenceState.SUPERSEDED:
            raise ValidationError("use Store.supersede() to supersede a claim")
        if state is EvidenceState.EDITORIALLY_REVIEWED and actor in ("", "system"):
            raise ValidationError("editorial review needs an identified reviewer as actor")
        with self.transaction():
            row = self._conn.execute("SELECT evidence_state, formal FROM claims WHERE id=?", (claim_id,)).fetchone()
            if row is None:
                raise ValidationError(f"unknown claim {claim_id}")
            current = EvidenceState(row[0])
            if state not in TRANSITIONS[current]:
                raise ValidationError(f"transition {current} -> {state} not allowed")
            if state is EvidenceState.FORMALLY_LINKED and formal is None:
                raise ValidationError("formally_linked requires a FormalArtifact")
            formal_json = _dumps(_formal_json(formal)) if formal else row[1]
            self._conn.execute("UPDATE claims SET evidence_state=?, formal=? WHERE id=?",
                               (state.value, formal_json, claim_id))
            self._log([(EventKind.EVIDENCE_CHANGED, claim_id, {
                "from": current.value, "to": state.value, "reason": reason,
                "formal": _formal_json(formal) if formal else None})], actor)
        return self.get_claim(claim_id)  # type: ignore[return-value]

    def supersede(self, old_id: str, new_claim: Claim, *, actor: str, reason: str) -> Claim:
        """Insert `new_claim` as the next version of `old_id`; the old one becomes SUPERSEDED."""
        if not reason.strip():
            raise ValidationError("a reason is required to supersede")
        if new_claim.supersedes not in (None, old_id):
            raise ValidationError("new_claim.supersedes points to another claim")
        new_claim = replace(new_claim, supersedes=old_id)
        with self.transaction():
            row = self._conn.execute("SELECT evidence_state FROM claims WHERE id=?", (old_id,)).fetchone()
            if row is None:
                raise ValidationError(f"unknown claim {old_id}")
            if EvidenceState(row[0]) not in CURRENT_STATES:
                raise ValidationError(f"claim {old_id} is {row[0]}; only current claims can be superseded")
            self._insert_claims([new_claim], actor, allow_supersedes=True)
            self._conn.execute("UPDATE claims SET evidence_state='superseded', superseded_by=? WHERE id=?",
                               (new_claim.id, old_id))
            self._log([(EventKind.CLAIM_SUPERSEDED, old_id, {"by": new_claim.id, "reason": reason,
                                                              "from": row[0]})], actor)
        return self.get_claim(new_claim.id)  # type: ignore[return-value]

    def history(self, claim_id: str) -> list[Claim]:
        """All versions of a claim, oldest first."""
        first = claim_id
        while (row := self._conn.execute("SELECT supersedes FROM claims WHERE id=?", (first,)).fetchone()) and row[0]:
            first = row[0]
        out, cur = [], first
        while cur and (c := self.get_claim(cur)):
            out.append(c)
            cur = c.superseded_by
        return out

    # ------------------------------------------------------------------ merges
    def merge(self, survivor_id: str, merged_id: str, *, actor: str, reason: str,
              evidence: str | None = None) -> Merge:
        """Redirect `merged_id` to `survivor_id`.  Participants keep their original ids; reads resolve."""
        if survivor_id == merged_id:
            raise ValidationError("cannot merge an entity with itself")
        if not reason.strip() or actor in ("", "system"):
            raise ValidationError("a merge needs an identified actor and a reason")
        with self.transaction():
            rows = {i: (t, m) for i, t, m in self._conn.execute(
                "SELECT id, type, merged_into FROM entities WHERE id IN (?,?)", (survivor_id, merged_id))}
            for i in (survivor_id, merged_id):
                if i not in rows:
                    raise ValidationError(f"unknown entity {i}")
                if rows[i][1] is not None:
                    raise ValidationError(f"entity {i} is already merged into {rows[i][1]}")
            if rows[survivor_id][0] != rows[merged_id][0]:
                raise ValidationError(f"type mismatch: {rows[survivor_id][0]} vs {rows[merged_id][0]}")
            ts = now_iso()
            self._conn.execute("UPDATE entities SET merged_into=? WHERE id=?", (survivor_id, merged_id))
            mid = self._conn.execute(
                "INSERT INTO merges(survivor_id, merged_id, actor, reason, evidence, created_at) "
                "VALUES (?,?,?,?,?,?)", (survivor_id, merged_id, actor, reason, evidence, ts)).lastrowid
            self._log([(EventKind.MERGE, merged_id, {"merge_id": mid, "into": survivor_id, "reason": reason,
                                                     "evidence": evidence})], actor)
            self._redirect_cache = None
        return self.get_merge(mid)  # type: ignore[arg-type,return-value]

    def undo_merge(self, merge_id: int, *, actor: str, reason: str) -> Merge:
        if not reason.strip():
            raise ValidationError("a reason is required to undo a merge")
        with self.transaction():
            m = self.get_merge(merge_id)
            if m is None:
                raise ValidationError(f"unknown merge {merge_id}")
            if m.undone_at is not None:
                raise ValidationError(f"merge {merge_id} was already undone")
            ts = now_iso()
            self._conn.execute("UPDATE entities SET merged_into=NULL WHERE id=?", (m.merged_id,))
            self._conn.execute("UPDATE merges SET undone_at=? WHERE id=?", (ts, merge_id))
            self._log([(EventKind.MERGE_UNDONE, m.merged_id, {"merge_id": merge_id, "from": m.survivor_id,
                                                               "reason": reason})], actor)
            self._redirect_cache = None
        return self.get_merge(merge_id)  # type: ignore[return-value]

    def get_merge(self, merge_id: int) -> Merge | None:
        row = self._conn.execute("SELECT * FROM merges WHERE id=?", (merge_id,)).fetchone()
        return Merge(*row) if row else None

    def merges(self, *, active_only: bool = False) -> list[Merge]:
        sql = "SELECT * FROM merges" + (" WHERE undone_at IS NULL" if active_only else "") + " ORDER BY id"
        return [Merge(*r) for r in self._conn.execute(sql)]

    def _redirects(self) -> dict[str, str]:
        # data_version changes when another connection commits, so readers see merges made elsewhere
        version = self._conn.execute("PRAGMA data_version").fetchone()[0]
        if self._redirect_cache is None or version != self._redirect_version:
            self._redirect_cache = dict(self._conn.execute(
                "SELECT id, merged_into FROM entities WHERE merged_into IS NOT NULL"))
            self._redirect_version = version
        return self._redirect_cache

    def resolve(self, entity_id: str) -> str:
        """Canonical id after following merges."""
        return _follow(self._redirects(), entity_id)

    def merged_ids(self, entity_id: str) -> list[str]:
        """The canonical id plus every id that resolves to it."""
        red = self._redirects()
        canon = _follow(red, entity_id)
        return [canon] + sorted(i for i in red if _follow(red, i) == canon)

    # ------------------------------------------------------------------ search
    def search(self, query: str, *, lang: str | None = None, limit: int = 20,
               types: Iterable[EntityType] | None = None) -> list[SearchHit]:
        """Lexical FTS5 lookup over labels, aliases, descriptions, statements and external ids.
        Accent- and case-insensitive; last token is a prefix.  Results are canonical entities."""
        match = _fts_query(query)
        if match is None:
            return []
        sql, args = "SELECT target_id, field, lang, text, bm25(fts) FROM fts WHERE fts MATCH ?", [match]
        if lang:
            sql += " AND lang IN (?, 'und')"
            args.append(lang)
        rows = self._conn.execute(sql + " ORDER BY bm25(fts) LIMIT ?", args + [limit * 10]).fetchall()
        wanted = None if types is None else {EntityType(t).value for t in types}
        best: dict[str, SearchHit] = {}
        for tid, fld, lg, text, bm in rows:
            eid = self.resolve(tid)
            hit = SearchHit(eid, fld, lg, text, -bm * FIELD_WEIGHT.get(fld, 1.0))
            if eid not in best or hit.score > best[eid].score:
                best[eid] = hit
        hits = sorted(best.values(), key=lambda h: -h.score)
        if wanted is not None:
            types_of = self._types([h.entity_id for h in hits])
            hits = [h for h in hits if types_of.get(h.entity_id) in wanted]
        return hits[:limit]

    def _types(self, ids: Sequence[str]) -> dict[str, str]:
        out: dict[str, str] = {}
        for part in _chunks(list(ids)):
            out.update(self._conn.execute(
                f"SELECT id, type FROM entities WHERE id IN ({','.join('?' * len(part))})", part))
        return out

    def rebuild_fts(self) -> int:
        """Drop and rebuild the lexical index from entities (it is derived data)."""
        with self.transaction():
            self._conn.execute("DELETE FROM fts")
            n = 0
            for rowid, data in self._conn.execute("SELECT rowid, data FROM entities").fetchall():
                rows = list(_fts_rows(Entity.from_json(json.loads(data)), rowid))
                self._conn.executemany("INSERT INTO fts(rowid, target_id, field, lang, text) VALUES (?,?,?,?,?)",
                                       rows)
                n += len(rows)
        return n

    # ------------------------------------------------------------------ events & coverage
    def events(self, target_id: str | None = None, *, kinds: Iterable[EventKind] | None = None,
               since_seq: int = 0) -> Iterator[Event]:
        sql, args = "SELECT seq, ts, actor, kind, target_id, payload FROM events WHERE seq > ?", [since_seq]
        if target_id is not None:
            sql += " AND target_id=?"
            args.append(target_id)
        if kinds is not None:
            ks = [EventKind(k).value for k in kinds]
            sql += f" AND kind IN ({','.join('?' * len(ks))})"
            args += ks
        cur = self._conn.execute(sql + " ORDER BY seq", args)
        while rows := cur.fetchmany(5000):
            for seq, ts, actor, kind, tid, payload in rows:
                yield Event(seq, ts, actor, EventKind(kind), tid, json.loads(payload))

    def coverage(self) -> dict[str, Any]:
        """Observed coverage.  Entities with no claims are 'unknown', never 'isolated'."""
        q = self._conn.execute
        return {
            "schema_version": self.schema_version,
            "sources": q("SELECT COUNT(*) FROM sources").fetchone()[0],
            "entities_by_type": dict(q("SELECT type, COUNT(*) FROM entities WHERE merged_into IS NULL "
                                       "GROUP BY type ORDER BY type")),
            "claims_by_relation": dict(q("SELECT relation, COUNT(*) FROM claims GROUP BY relation ORDER BY relation")),
            "claims_by_state": dict(q("SELECT evidence_state, COUNT(*) FROM claims GROUP BY evidence_state "
                                      "ORDER BY evidence_state")),
            "unknown_by_type": dict(q("SELECT type, COUNT(*) FROM entities e WHERE merged_into IS NULL AND NOT EXISTS "
                                      "(SELECT 1 FROM claim_participants p WHERE p.entity_id = e.id) "
                                      "GROUP BY type ORDER BY type")),
            "claims_by_source": dict(q("SELECT source_id, COUNT(DISTINCT claim_id) FROM provenance "
                                       "WHERE claim_id IS NOT NULL GROUP BY source_id ORDER BY source_id")),
            "active_merges": q("SELECT COUNT(*) FROM merges WHERE undone_at IS NULL").fetchone()[0],
        }

    # ------------------------------------------------------------------ JSON-LD
    def export_jsonld(self, entity_ids: Iterable[str], *, depth: int = 1, current_only: bool = True) -> dict[str, Any]:
        """Subgraph around `entity_ids` (BFS over claims, `depth` hops) as JSON-LD with an embedded context."""
        seen_e: dict[str, None] = {}
        claims: dict[str, Claim] = {}
        frontier = deque((self.resolve(e), 0) for e in entity_ids)
        while frontier:
            eid, d = frontier.popleft()
            if eid in seen_e:
                continue
            seen_e[eid] = None
            if d >= depth:
                continue
            for c in self.claims_for(eid, current_only=current_only):
                claims.setdefault(c.id, c)
                for p in c.participants:
                    frontier.append((self.resolve(p.entity_id), d + 1))
        for c in claims.values():  # every referenced entity must be in the graph
            for p in c.participants:
                seen_e.setdefault(self.resolve(p.entity_id), None)
        graph: list[dict[str, Any]] = []
        sources: set[str] = set()
        for eid in seen_e:
            ent = self.get_entity(eid)
            if ent is not None:
                graph.append(_entity_ld(ent, self.merged_ids(eid)[1:]))
                sources.update(p.source_id for p in ent.provenance)
        for c in claims.values():
            graph.append(_claim_ld(c, self.resolve))
            sources.update(p.source_id for p in c.provenance)
        for sid in sorted(sources):
            s = self.get_source(sid)
            if s is not None:
                graph.append({"@id": _iri(s.id), "@type": "Source", "title": s.title, "license": s.license,
                              **({"uri": s.uri} if s.uri else {}), **({"version": s.version} if s.version else {})})
        context = json.loads(CONTEXT_PATH.read_text(encoding="utf-8"))["@context"]
        return {"@context": context, "@graph": graph}


def load_bundle(store: Store, bundle: Mapping[str, Any], *, actor: str = "system") -> dict[str, int]:
    """Load {"sources": [...], "entities": [...], "claims": [...]} (to_json shapes) in one transaction."""
    with store.transaction():
        for s in bundle.get("sources", ()):
            store.add_source(Source(**s), actor=actor)
        n_e = store.add_entities((Entity.from_json(e) for e in bundle.get("entities", ())), actor=actor)
        n_c = store.add_claims((Claim.from_json(c) for c in bundle.get("claims", ())), actor=actor)
    return {"sources": len(bundle.get("sources", ())), "entities": n_e, "claims": n_c}


# ---------------------------------------------------------------------- module helpers


def _split_sql(script: str) -> list[str]:
    """Split DDL on ';' at line end, keeping trigger bodies (… END;) intact."""
    out, buf = [], []
    for line in script.strip().splitlines():
        buf.append(line)
        joined = "\n".join(buf).strip()
        if joined.endswith(";") and sqlite3.complete_statement(joined):
            out.append(joined)
            buf = []
    if "\n".join(buf).strip():
        out.append("\n".join(buf))
    return out


def _follow(red: Mapping[str, str], eid: str) -> str:
    hops = 0
    while eid in red:
        eid = red[eid]
        hops += 1
        if hops > 64:
            raise ValidationError("merge redirect cycle")
    return eid


def _fts_rows(e: Entity, rowid: int) -> Iterator[tuple[int, str, str, str, str]]:
    fields = ([("label", l.lang, l.text) for l in e.labels] + [("alias", a.lang, a.text) for a in e.aliases]
              + [("description", d.lang, d.text) for d in e.descriptions]
              + [("statement", s.lang, s.text) for s in e.statements]
              + [("external_id", "und", v) for _, v in e.external_ids])
    if len(fields) > FTS_SLOTS:
        raise ValidationError(f"entity {e.id} has more than {FTS_SLOTS} searchable texts")
    for k, (fld, lang, text) in enumerate(fields):
        yield rowid * FTS_SLOTS + k, e.id, fld, lang, text


def _fts_query(q: str) -> str | None:
    toks = re.findall(r"\w+", q)
    if not toks:
        return None
    return " ".join([f'"{t}"' for t in toks[:-1]] + [f'"{toks[-1]}"*'])


def _formal_json(f: FormalArtifact) -> dict[str, Any]:
    return {"library": f.library, "version": f.version, "declaration": f.declaration, "axioms": list(f.axioms)}


def _claim_from_row(row: Sequence[Any]) -> Claim:
    data, state, sup_by, formal = row
    d = json.loads(data)
    d.update(evidence_state=state, superseded_by=sup_by, formal=json.loads(formal) if formal else d.get("formal"))
    return Claim.from_json(d)


def _iri(atlas_id: str) -> str:
    return "aid:" + quote(atlas_id, safe="")


def _lang_map(items: Iterable[Any]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for x in items:
        out.setdefault(x.lang, []).append(x.text)
    return {k: v[0] if len(v) == 1 else v for k, v in out.items()}


def _entity_ld(e: Entity, merged_from: list[str]) -> dict[str, Any]:
    node: dict[str, Any] = {"@id": _iri(e.id), "@type": "".join(w.capitalize() for w in e.type.value.split("_")),
                            "atlasId": e.id, "label": _lang_map(e.labels)}
    if e.aliases:
        node["altLabel"] = _lang_map(e.aliases)
    if e.descriptions:
        node["description"] = _lang_map(e.descriptions)
    if e.statements:
        node["statementText"] = _lang_map(e.statements)
    if e.formula:
        node["formula"] = e.formula
    if e.external_ids:
        node["externalId"] = [f"{s}:{v}" for s, v in e.external_ids]
    if merged_from:
        node["mergedFrom"] = [_iri(m) for m in merged_from]
    if e.provenance:
        node["provenance"] = [_prov_ld(p) for p in e.provenance]
    return node


def _prov_ld(p: Any) -> dict[str, Any]:
    node = {"source": _iri(p.source_id), "locator": p.locator, "extractor": p.extractor,
            "extractorVersion": p.extractor_version, "license": p.license, "retrievedAt": p.retrieved_at}
    if p.quote:
        node["quote"] = p.quote
    return node


def _claim_ld(c: Claim, resolve: Any) -> dict[str, Any]:
    node: dict[str, Any] = {
        "@id": _iri(c.id), "@type": "Claim", "atlasId": c.id, "relation": c.relation.value,
        "evidenceState": c.evidence_state.value,
        "participant": [{"role": p.role.value, "entity": _iri(resolve(p.entity_id))} for p in c.participants],
        "provenance": [_prov_ld(p) for p in c.provenance],
    }
    optional = {
        "assumption": list(c.assumptions) or None, "validityDomain": c.validity_domain,
        "alternativeGroup": c.alternative_group, "supersedes": c.supersedes and _iri(c.supersedes),
        "supersededBy": c.superseded_by and _iri(c.superseded_by),
        "formalArtifact": c.formal and {"library": c.formal.library, "version": c.formal.version,
                                        "declaration": c.formal.declaration, "axiom": list(c.formal.axioms)},
        "confidence": c.confidence and {"value": c.confidence.value, "estimates": c.confidence.estimates,
                                        "calibration": c.confidence.calibration},
        "historicalDate": c.historical_date,
        "explanation": _lang_map(c.explanation) or None,
        "disagreement": list(c.disagreements) or None,
    }
    node.update({k: v for k, v in optional.items() if v})
    return node


# ---------------------------------------------------------------------- JSON-LD validator (mirrors SHACL)

_PROV_FIELDS = ("source", "locator", "extractor", "extractorVersion", "license", "retrievedAt")


def validate_jsonld(doc: Mapping[str, Any]) -> list[str]:
    """Pure-Python check of an exported graph against the rules in shapes/atlas.shacl.ttl.
    Returns a list of violations (empty = conforms)."""
    errors: list[str] = []
    nodes = doc.get("@graph", [])
    ids = {n.get("@id") for n in nodes}
    rel_values = {r.value for r in RelationType}
    state_values = {s.value for s in EvidenceState}
    for n in nodes:
        if n.get("@type") != "Claim":
            if n.get("@type") not in ("Source",) and not n.get("label"):
                errors.append(f"{n.get('@id')}: entity without label")
            continue
        cid = n.get("@id")
        rel, state = n.get("relation"), n.get("evidenceState")
        if rel not in rel_values:
            errors.append(f"{cid}: relation {rel!r} not in vocabulary")
        if state not in state_values:
            errors.append(f"{cid}: evidenceState {state!r} not in vocabulary")
        parts = n.get("participant", [])
        roles = [p.get("role") for p in parts]
        if roles.count("subject") != 1:
            errors.append(f"{cid}: needs exactly one subject participant")
        n_obj, n_prem = roles.count("object"), roles.count("premise")
        if n_prem and (n_obj or rel not in rel_values or not RELATIONS[RelationType(rel)].allows_premises):
            errors.append(f"{cid}: premises only without object and on relations that allow them")
        if not n_prem and n_obj != 1:
            errors.append(f"{cid}: needs exactly one object or a premise set")
        for p in parts:
            if p.get("role") not in {r.value for r in Role}:
                errors.append(f"{cid}: unknown role {p.get('role')!r}")
            if not isinstance(p.get("entity"), str) or p["entity"] not in ids:
                errors.append(f"{cid}: participant entity {p.get('entity')!r} missing from graph")
        provs = n.get("provenance", [])
        if not provs:
            errors.append(f"{cid}: no provenance")
        for p in provs:
            for f in _PROV_FIELDS:
                if not isinstance(p.get(f), str) or not p[f].strip():
                    errors.append(f"{cid}: provenance.{f} missing")
            try:
                datetime.fromisoformat(p.get("retrievedAt", ""))
            except (TypeError, ValueError):
                errors.append(f"{cid}: provenance.retrievedAt is not a dateTime")
        if state == "formally_linked":
            fa = n.get("formalArtifact") or {}
            if not all(fa.get(k) for k in ("library", "version", "declaration")):
                errors.append(f"{cid}: formally_linked requires formalArtifact(library, version, declaration)")
        if state == "superseded" and not n.get("supersededBy"):
            errors.append(f"{cid}: superseded requires supersededBy")
        conf = n.get("confidence")
        if conf is not None and not (isinstance(conf.get("value"), (int, float)) and 0 <= conf["value"] <= 1
                                     and conf.get("estimates") and conf.get("calibration")):
            errors.append(f"{cid}: confidence needs value in [0,1], estimates and calibration")
    return errors
