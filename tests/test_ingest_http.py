"""Cached polite HTTP client against a local server (127.0.0.1 only; no external network)."""

from __future__ import annotations

import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import pytest

from atlas.ingest.http import USER_AGENT, CachedClient, HttpError, OfflineMiss, has_contact


class _Handler(BaseHTTPRequestHandler):
    plan: list[tuple[int, bytes, dict]] = []
    seen: list[tuple[str, str, str]] = []

    def _serve(self) -> None:
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length).decode() if length else ""
        type(self).seen.append((self.command, self.path, self.headers.get("User-Agent", "")))
        status, payload, headers = type(self).plan.pop(0) if type(self).plan else (200, b"default", {})
        self.send_response(status)
        for k, v in headers.items():
            self.send_header(k, v)
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)
        _ = body

    do_GET = _serve
    do_POST = _serve

    def log_message(self, *args: object) -> None:  # keep test output quiet
        pass


@pytest.fixture()
def server():
    _Handler.plan, _Handler.seen = [], []
    srv = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
    th = threading.Thread(target=srv.serve_forever, daemon=True)
    th.start()
    yield f"http://127.0.0.1:{srv.server_address[1]}"
    srv.shutdown()


def test_cache_roundtrip_and_offline_reads(server: str, tmp_path: Path) -> None:
    _Handler.plan = [(200, b'{"ok": 1}', {})]
    c = CachedClient(tmp_path)
    r1 = c.get(server + "/x", {"b": "2", "a": "1"})
    r2 = c.get(server + "/x", {"a": "1", "b": "2"})  # same request, params order irrelevant
    assert r1.body == r2.body == b'{"ok": 1}' and not r1.from_cache and r2.from_cache
    assert r1.fetched_at == r2.fetched_at
    assert len(_Handler.seen) == 1 and _Handler.seen[0][2] == USER_AGENT
    off = CachedClient(tmp_path, offline=True)
    assert off.get(server + "/x", {"a": "1", "b": "2"}).json() == {"ok": 1}
    with pytest.raises(OfflineMiss):
        off.get(server + "/never")


def test_retry_with_backoff_and_retry_after(server: str, tmp_path: Path) -> None:
    _Handler.plan = [(503, b"busy", {"Retry-After": "0"}), (429, b"slow down", {"Retry-After": "0"}),
                     (200, b"done", {})]
    c = CachedClient(tmp_path, backoff_base=0.01, min_429_wait=0.0)
    assert c.post(server + "/sparql", {"query": "q"}).body == b"done"
    assert c.stats["retries"] == 2 and len(_Handler.seen) == 3


def test_429_waits_at_least_the_floor_even_with_short_retry_after(server: str, tmp_path: Path) -> None:
    # Wikimedia answers "Retry-After: 1" while its per-minute bucket is empty; retrying after 1 s just burns 429s
    _Handler.plan = [(429, b"slow down", {"Retry-After": "0"}), (200, b"ok", {})]
    c = CachedClient(tmp_path, backoff_base=0.01, min_429_wait=0.3)
    t0 = time.monotonic()
    assert c.get(server + "/api").body == b"ok"
    assert time.monotonic() - t0 >= 0.3


@pytest.mark.parametrize("contact, ok", [("", False), ("set ATLAS_CONTACT", False), ("me@example.org", True),
                                         ("https://example.org/atlas", True)])
def test_contact_must_be_email_or_url(contact: str, ok: bool) -> None:
    assert has_contact(contact) is ok


def test_non_retryable_status_raises_without_caching(server: str, tmp_path: Path) -> None:
    _Handler.plan = [(400, b"bad query", {}), (200, b"fixed", {})]
    c = CachedClient(tmp_path, backoff_base=0.01)
    with pytest.raises(HttpError) as err:
        c.get(server + "/q")
    assert err.value.status == 400
    assert c.get(server + "/q").body == b"fixed"  # the failure was not cached


@pytest.mark.parametrize("status, body", [(500, b"java.util.concurrent.TimeoutException"),
                                          (504, b"upstream request timeout")])  # WDQS gateway, seen 2026-10-16
def test_sparql_timeout_is_not_retried(server: str, tmp_path: Path, status: int, body: bytes) -> None:
    _Handler.plan = [(status, body, {})]
    c = CachedClient(tmp_path, backoff_base=0.01)
    with pytest.raises(HttpError):
        c.post(server + "/sparql", {"query": "slow"})
    assert len(_Handler.seen) == 1


def test_rejected_body_is_retried_and_never_cached(server: str, tmp_path: Path) -> None:
    _Handler.plan = [(200, b'{"error": {"code": "maxlag"}}', {"Retry-After": "0"}), (200, b'{"query": {}}', {})]
    c = CachedClient(tmp_path, backoff_base=0.01)
    r = c.get(server + "/api", accept=lambda b: b"maxlag" not in b)
    assert r.body == b'{"query": {}}' and c.stats["retries"] == 1
    assert CachedClient(tmp_path, offline=True).get(server + "/api").body == b'{"query": {}}'


def test_rate_limit_spaces_request_starts(server: str, tmp_path: Path) -> None:
    _Handler.plan = [(200, b"a", {}), (200, b"b", {}), (200, b"c", {})]
    c = CachedClient(tmp_path, min_interval=0.1)
    t0 = time.monotonic()
    for i in range(3):
        c.get(server + f"/r{i}")
    assert time.monotonic() - t0 >= 0.2
