"""Systems under test.  Each backend answers search / path / entity, or raises ComponentMissing.

- `store`: the Python registry.  Search = lexical FTS5 lane (the v1 retrieval baseline,
  SPEC §10).  Path = untyped bidirectional BFS that ignores relation semantics and claims
  the strongest single-hop conclusion on its path (a null model: it shows whether route
  items can be won without composition rules).  Entity = get_entity + claims_for.
- `cli`: the Rust binary, `atlas --json search|path ...` (one process per call, so its
  latency includes process start and snapshot open; reported, never gated).
- `http`: the API of `atlas serve` (`/api/search`, `/api/path`, `/api/entity/{id}`).
"""

from __future__ import annotations

import http.client
import json
import os
import sqlite3
import subprocess
import threading
import time
import urllib.parse
from collections import deque
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Sequence

from atlas import NO_WINDOW
from atlas.model import CURRENT_STATES

from . import compose
from .registry import Registry

ROOT = Path(__file__).resolve().parents[1]
CLI_TIMEOUT_S = 120
HTTP_TIMEOUT_S = 30
HTTP_ATTEMPTS = 3
HTTP_RETRY_SLEEP_S = 0.25


MMAP_BYTES = 1 << 32


def _follow(redirects: dict[str, str], eid: str) -> str:
    seen = 0
    while eid in redirects and seen < 64:  # merge chains are short; the bound guards against a corrupt cycle
        eid, seen = redirects[eid], seen + 1
    return eid


class ComponentMissing(RuntimeError):
    """The component does not exist yet, answers not_implemented, or is unreachable."""


class BackendError(RuntimeError):
    """The component exists but answered with an error for this request."""


@dataclass(frozen=True)
class Timed:
    value: Any
    ms: float


@dataclass(frozen=True)
class PathAnswer:
    abstained: bool
    reason: str | None
    paths: tuple[dict, ...] = field(default_factory=tuple)  # SPEC §15 FoundPath dicts


def _timed(fn, *args, **kw) -> Timed:
    t = time.perf_counter()
    v = fn(*args, **kw)
    return Timed(v, (time.perf_counter() - t) * 1e3)


def _check_envelope(obj: Any, what: str) -> Any:
    if isinstance(obj, dict) and "error" in obj:
        err = obj["error"]
        if err.get("code") == "not_implemented":
            raise ComponentMissing(f"{what}: {err.get('message')}")
        raise BackendError(f"{what}: {err.get('code')}: {err.get('message')}")
    return obj


@dataclass(frozen=True)
class Ranked:
    """A search answer: ranked entity ids and, per hit, the SPEC §15 `lanes` object."""
    ids: tuple[str, ...]
    lanes: tuple[dict, ...]

    def lanes_used(self) -> frozenset[str]:
        return frozenset(k for ln in self.lanes for k, v in ln.items() if v is not None)

    def by_lane(self) -> dict[str, list[str]]:
        """Per lane, the hits that lane contributed, in that lane's own rank order (diagnostic only:
        a lane's list is truncated to what survived fusion into the returned top-k)."""
        out: dict[str, list[tuple[int, str]]] = {}
        for i, ln in zip(self.ids, self.lanes):
            for k, v in ln.items():
                if v is not None:
                    out.setdefault(k, []).append((int(v.get("rank", 0)), i))
        return {k: [i for _, i in sorted(v)] for k, v in out.items()}


def _ranked(obj: dict) -> Ranked:
    hits = obj.get("hits") or []
    return Ranked(tuple(h["id"] for h in hits), tuple(h.get("lanes") or {} for h in hits))


def _path_answer(obj: dict) -> PathAnswer:
    return PathAnswer(bool(obj.get("abstained")), obj.get("reason"), tuple(obj.get("paths") or ()))


# ------------------------------------------------------------------ store (baseline / null)

class StoreBackend:
    name = "store"

    def __init__(self, registry: Registry, *, bfs_max_nodes: int = 2_000_000) -> None:
        self.reg = registry
        self._graph: _Graph | None = None
        self.bfs_max_nodes = bfs_max_nodes

    def search(self, query: str, *, lang: str, limit: int, types: Sequence[str] = ()) -> Timed:
        t = _timed(self.reg.store.search, query, lang=lang, limit=limit, types=list(types) or None)
        return Timed(Ranked(tuple(h.entity_id for h in t.value),
                            tuple({"lexical": {"rank": i + 1, "score": h.score}} for i, h in enumerate(t.value))),
                     t.ms)

    def path(self, src: str, dst: str, *, objective: str, max_hops: int) -> Timed:
        if self._graph is None:
            self._graph = _Graph.build(self.reg)
        return _timed(self._graph.shortest, src, dst, max_hops)

    def entity(self, eid: str, *, limit: int | None = None) -> Timed:  # the registry never truncates
        def load() -> dict:
            e = self.reg.store.get_entity(eid)
            if e is None:
                raise BackendError(f"entity {eid} not found")
            return {"canonical_id": e.id, "claims": [c.to_json() for c in self.reg.store.claims_for(e.id)]}
        return _timed(load)


class _Graph:
    """Undirected view of every current edge, for the null path model."""

    def __init__(self) -> None:
        self.adj: dict[str, list[tuple[str, str, str, str]]] = {}  # node -> (nbr, claim, relation, direction)

    @staticmethod
    def build(reg: Registry) -> _Graph:
        """Same edges as Store.iter_edges(states=CURRENT_STATES), read as two plain scans in one read
        transaction: the three-way join with a window function took 123 s on the 515k-claim registry,
        the two scans 25 s (bench run, 2026-10-16)."""
        g = _Graph()
        if reg.path is None:
            raise BackendError("null path model needs a file registry")
        con = sqlite3.connect(f"file:{reg.path.as_posix()}?mode=ro", uri=True, isolation_level=None)
        current = {st.value for st in CURRENT_STATES}
        try:
            con.execute(f"PRAGMA mmap_size = {MMAP_BYTES}")
            con.execute("BEGIN")  # one consistent view while ingestion may be writing
            rel_of = {cid: rel for cid, rel, st in con.execute("SELECT id, relation, evidence_state FROM claims")
                      if st in current}
            red = dict(con.execute("SELECT id, merged_into FROM entities WHERE merged_into IS NOT NULL"))
            subj: dict[str, str] = {}
            tgts: dict[str, list[str]] = {}
            for cid, role, eid in con.execute("SELECT claim_id, role, entity_id FROM claim_participants "
                                              "WHERE role IN ('subject', 'object', 'premise')"):
                if cid in rel_of:
                    if role == "subject":
                        subj[cid] = _follow(red, eid)
                    else:
                        tgts.setdefault(cid, []).append(_follow(red, eid))
            con.execute("COMMIT")
        finally:
            con.close()
        for cid, targets in tgts.items():
            s, rel = subj[cid], rel_of[cid]
            for t in targets:
                g.adj.setdefault(s, []).append((t, cid, rel, "forward"))
                g.adj.setdefault(t, []).append((s, cid, rel, "inverse"))
        return g

    def shortest(self, src: str, dst: str, max_hops: int) -> PathAnswer:
        if src == dst:
            return PathAnswer(True, "source equals target")
        if src not in self.adj or dst not in self.adj:
            return PathAnswer(True, "endpoint has no edges")
        prev: dict[str, tuple[str, str, str, str] | None] = {src: None}
        q = deque([(src, 0)])
        while q:
            node, d = q.popleft()
            if node == dst:
                break
            if d == max_hops:
                continue
            for nbr, cid, rel, direction in self.adj[node]:
                if nbr not in prev:
                    prev[nbr] = (node, cid, rel, direction)
                    q.append((nbr, d + 1))
        if dst not in prev:
            return PathAnswer(True, f"no path within {max_hops} hops")
        steps, node = [], dst
        while prev[node] is not None:
            p, cid, rel, direction = prev[node]
            steps.append({"claim_id": cid, "relation": rel, "from": p, "to": node, "direction": direction,
                          "premises": [], "premises_missing": []})
            node = p
        steps.reverse()
        best = max((compose.table()["relations"][s["relation"]]["single_hop"] for s in steps), key=compose.strength)
        return PathAnswer(False, None, ({"score": float(len(steps)), "steps": steps,
                                         "conclusion": {"authorizes": best, "is_hypothesis": False,
                                                        "assumptions": [], "weakest_evidence": None}},))


# ------------------------------------------------------------------ Rust CLI

def find_binary() -> Path | None:
    env = os.environ.get("ATLAS_BIN")
    if env:
        return Path(env) if Path(env).exists() else None
    home = Path.home()
    cands = list((home / ".cargo-target").glob("*/release/atlas.exe")) + list(
        (home / ".cargo-target").glob("*/release/atlas"))
    cands += [p for p in (ROOT / "core/target/release/atlas.exe", ROOT / "core/target/release/atlas") if p.exists()]
    return max(cands, key=lambda p: p.stat().st_mtime) if cands else None


class CliBackend:
    name = "cli"

    def __init__(self, binary: Path, *, snapshot: Path, index: Path, composition: Path, db: Path | None = None) -> None:
        self.binary, self.snapshot, self.index, self.composition, self.db = binary, snapshot, index, composition, db

    def check(self, registry_event_seq: int | None = None) -> dict:
        """`atlas stats` on the snapshot: raises ComponentMissing if it does not open.  Reports staleness
        (snapshot compiled from an older registry state: ids may not match, results are still recorded)."""
        try:
            stats = self._run(["stats", "--snapshot", str(self.snapshot)], "atlas stats").value
        except BackendError as exc:
            raise ComponentMissing(f"snapshot {self.snapshot} does not open: {exc}") from exc
        seq = stats.get("source_event_seq")
        return {"snapshot": stats.get("path"), "content_hash": stats.get("content_hash"), "nodes": stats.get("nodes"),
                "claims": stats.get("claims"), "source_event_seq": seq, "registry_event_seq": registry_event_seq,
                "stale": registry_event_seq is not None and seq != registry_event_seq}

    def compile(self, db: Path, out: Path) -> dict:
        """`atlas compile --db db --out out`; points this backend at the new snapshot."""
        rep = self._run(["compile", "--db", str(db), "--out", str(out)], "atlas compile")
        self.snapshot = out
        return {**(rep.value if isinstance(rep.value, dict) else {"report": rep.value}), "wall_ms": rep.ms}

    def _run(self, args: list[str], what: str) -> Timed:
        def call() -> Any:
            try:
                out = subprocess.run([str(self.binary), "--json", *args], capture_output=True, text=True,
                                     encoding="utf-8", timeout=CLI_TIMEOUT_S, cwd=ROOT, **NO_WINDOW)
            except (OSError, subprocess.TimeoutExpired) as exc:
                raise ComponentMissing(f"{what}: {exc}") from exc
            try:
                obj = json.loads(out.stdout)
            except json.JSONDecodeError as exc:
                raise BackendError(f"{what}: non-JSON output (exit {out.returncode}): {out.stdout[:200]!r}") from exc
            return _check_envelope(obj, what)
        return _timed(call)

    def search(self, query: str, *, lang: str, limit: int, types: Sequence[str] = ()) -> Timed:
        args = ["search", query, "--lang", lang, "--limit", str(limit), "--snapshot", str(self.snapshot),
                "--index", str(self.index)]
        if self.db is not None:
            args += ["--db", str(self.db)]
        if types:
            args += ["--types", ",".join(types)]
        t = self._run(args, "atlas search")
        return Timed(_ranked(t.value), t.ms)

    def path(self, src: str, dst: str, *, objective: str, max_hops: int) -> Timed:
        t = self._run(["path", src, dst, "--objective", objective, "--max-hops", str(max_hops), "--snapshot",
                       str(self.snapshot), "--composition", str(self.composition)], "atlas path")
        return Timed(_path_answer(t.value), t.ms)

    def entity(self, eid: str, *, limit: int | None = None) -> Timed:
        raise ComponentMissing("atlas CLI has no entity subcommand; use the HTTP backend for sourced answers")


# ------------------------------------------------------------------ HTTP API

class HttpBackend:
    name = "http"

    def __init__(self, base: str, *, attempts: int = HTTP_ATTEMPTS) -> None:
        self.base = base.rstrip("/")
        u = urllib.parse.urlsplit(self.base)
        if u.scheme != "http" or not u.hostname:
            raise ValueError(f"--http must be an http:// base URL, got {base!r}")
        self.host, self.port, self.prefix = u.hostname, u.port or 80, u.path.rstrip("/")
        self.attempts = attempts
        self.retries = 0  # connection-level retries (resets/refusals); reported, and included in the timings
        self._local = threading.local()  # one keep-alive connection per thread, like a browser tab

    def _conn(self) -> http.client.HTTPConnection:
        conn = getattr(self._local, "conn", None)
        if conn is None:
            conn = self._local.conn = http.client.HTTPConnection(self.host, self.port, timeout=HTTP_TIMEOUT_S)
        return conn

    def _drop(self) -> None:
        conn = getattr(self._local, "conn", None)
        if conn is not None:
            conn.close()
            self._local.conn = None

    def _get(self, route: str, params: dict[str, Any], what: str) -> Timed:
        query = urllib.parse.urlencode({k: v for k, v in params.items() if v not in (None, "")})
        target = f"{self.prefix}{route}?{query}"

        def call() -> Any:
            try:
                conn = self._conn()
                conn.request("GET", target, headers={"Connection": "keep-alive"})
                resp = conn.getresponse()
                status, body = resp.status, resp.read().decode("utf-8", "replace")
                if resp.will_close:
                    self._drop()
            except (http.client.HTTPException, OSError) as exc:
                self._drop()
                raise ComponentMissing(f"{what}: server not reachable at {self.base} ({exc})") from exc
            try:
                obj = json.loads(body)
            except json.JSONDecodeError:
                obj = {"error": {"code": "internal", "message": f"HTTP {status}: {body[:200]}"}}
            if status == 501:
                raise ComponentMissing(f"{what}: HTTP 501 {obj.get('error', {}).get('message')}")
            return _check_envelope(obj, f"{what} HTTP {status}") if status >= 400 else obj

        def with_retry() -> Any:  # a reset on a reused connection is not a missing component
            for attempt in range(self.attempts):
                try:
                    return call()
                except ComponentMissing as exc:
                    if "not reachable" not in str(exc) or attempt == self.attempts - 1:
                        raise
                    self.retries += 1
                    time.sleep(HTTP_RETRY_SLEEP_S * attempt)  # first retry is immediate (stale keep-alive)
        t = _timed(with_retry)
        return Timed(_check_envelope(t.value, what), t.ms)

    def search(self, query: str, *, lang: str, limit: int, types: Sequence[str] = ()) -> Timed:
        t = self._get("/api/search", {"q": query, "lang": lang, "limit": limit, "types": ",".join(types)},
                      "GET /api/search")
        return Timed(_ranked(t.value), t.ms)

    def path(self, src: str, dst: str, *, objective: str, max_hops: int) -> Timed:
        t = self._get("/api/path", {"from": src, "to": dst, "objective": objective, "max_hops": max_hops},
                      "GET /api/path")
        return Timed(_path_answer(t.value), t.ms)

    def entity(self, eid: str, *, limit: int | None = None) -> Timed:
        return self._get(f"/api/entity/{urllib.parse.quote(eid, safe='')}", {"limit": limit}, "GET /api/entity")

    def get(self, route: str, params: dict[str, Any]) -> Timed:
        return self._get(route, params, f"GET {route}")
