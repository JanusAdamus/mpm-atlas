"""Read-only view of the registry for evaluation: ref resolution, entities, claims, fingerprint.

`Registry.open` takes a SQLite registry or a JSON bundle ({'sources','entities','claims'},
e.g. tests/fixtures/mini_atlas.json); a bundle is loaded into a temporary SQLite file so the
Rust binary can compile a snapshot from it too.
"""

from __future__ import annotations

import hashlib
import json
import os
import sqlite3
import tempfile
from functools import lru_cache
from pathlib import Path
from typing import Mapping

from atlas import Store, load_bundle
from atlas.model import CURRENT_STATES, Claim, Entity


class Registry:
    """Wraps a read-only Store.  Every id it returns is canonical (merges resolved)."""

    def __init__(self, store: Store, path: Path | None = None, origin: Path | None = None) -> None:
        self.store = store
        self.path = path  # the SQLite file actually read
        self.origin = origin or path  # what the user passed (a .db or a .json bundle)
        self._entity = lru_cache(maxsize=200_000)(self._load_entity)
        self._claim = lru_cache(maxsize=200_000)(store.get_claim)

    @staticmethod
    def open(path: Path | str) -> Registry:
        p = Path(path)
        if not p.exists():
            raise FileNotFoundError(f"registry not found: {p}")
        if p.suffix.lower() == ".json":
            return Registry(Store(_bundle_db(p), read_only=True), _bundle_db(p), p)
        return Registry(Store(p, read_only=True), p)

    def close(self) -> None:
        self.store.close()

    def _load_entity(self, eid: str) -> Entity | None:
        return self.store.get_entity(eid)

    def ids(self, ref: Mapping) -> list[str]:
        """Canonical ids a ref points to ([] = not in the registry)."""
        if "id" in ref:
            e = self._entity(ref["id"])
            return [e.id] if e is not None else []
        scheme, value = ref["ext"]
        return sorted({self.store.resolve(i) for i in self.store.find_by_external_id(scheme, value)})

    def entity(self, eid: str) -> Entity | None:
        return self._entity(eid)

    def claim(self, cid: str) -> Claim | None:
        return self._claim(cid)

    def is_current(self, cid: str) -> bool:
        c = self._claim(cid)
        return c is not None and c.evidence_state in CURRENT_STATES

    def canonical(self, eid: str) -> str:
        return self.store.resolve(eid)

    def event_seq(self) -> int:
        """MAX(events.seq): what a snapshot compiled from this registry records as source_event_seq."""
        if self.path is None:
            return 0
        con = sqlite3.connect(f"file:{self.path.as_posix()}?mode=ro", uri=True)
        try:
            return int(con.execute("SELECT COALESCE(MAX(seq), 0) FROM events").fetchone()[0])
        finally:
            con.close()

    def fingerprint(self) -> dict:
        if self.path is None:
            return {"path": None}
        st = os.stat(self.origin)
        return {"path": self.origin.as_posix(), "sqlite": self.path.as_posix(), "bytes": st.st_size,
                "mtime_ns": st.st_mtime_ns, "schema_version": self.store.schema_version,
                "event_seq": self.event_seq()}


def _bundle_db(bundle: Path) -> Path:
    """Temporary SQLite registry built from a JSON bundle, cached by the bundle's content hash."""
    raw = bundle.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()[:16]
    db = Path(tempfile.gettempdir()) / "atlasbench" / f"{bundle.stem}-{digest}.db"
    if db.exists():
        return db
    db.parent.mkdir(parents=True, exist_ok=True)
    tmp = db.with_suffix(f".{os.getpid()}.tmp")
    with Store(tmp) as store:
        load_bundle(store, json.loads(raw.decode("utf-8")), actor="bench")
    con = sqlite3.connect(tmp)  # checkpoint the WAL into the main file so it can be renamed alone
    try:
        con.execute("PRAGMA journal_mode = DELETE")
    finally:
        con.close()
    os.replace(tmp, db)
    return db
