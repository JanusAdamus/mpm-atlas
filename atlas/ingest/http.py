"""Polite, cached HTTP for ingest adapters (stdlib only).

Every response is stored gzip-compressed under `cache_dir/<k[:2]>/<k>.gz`, where
`k` is sha256(method, url, body).  A run that is interrupted resumes from the
cache; `offline=True` reads only the cache and raises `OfflineMiss` otherwise,
which is how the tests run against committed fixtures without touching the net.

Rate limiting is a per-client minimum interval between request starts (shared
by all threads); retries back off exponentially and honour `Retry-After`.
"""

from __future__ import annotations

import gzip
import hashlib
import json
import os
import random
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable, Mapping

# Wikimedia's UA policy asks for contact info; it is taken from ATLAS_CONTACT, never hard-coded.
CONTACT = os.environ.get("ATLAS_CONTACT", "").strip()
USER_AGENT = ("MPM-atlas-ingest/0.1 (personal open math atlas; contact: "
              + (CONTACT or "set ATLAS_CONTACT") + ") python-urllib")
RETRY_STATUS = frozenset({429, 500, 502, 503, 504})
MIN_429_WAIT = 5.0  # Wikimedia: without a usable Retry-After wait at least 5 s; "Retry-After: 1" is not enough


def has_contact(contact: str = CONTACT) -> bool:
    """A UA counts as identified for Wikimedia only with an e-mail address or a full URL."""
    return "@" in contact or contact.startswith(("http://", "https://"))


class OfflineMiss(LookupError):
    """Offline mode and the request is not in the cache."""


class HttpError(RuntimeError):
    def __init__(self, status: int, url: str, body: bytes = b"") -> None:
        super().__init__(f"HTTP {status} for {url[:200]}: {body[:300]!r}")
        self.status = status
        self.body = body


@dataclass(frozen=True)
class Response:
    body: bytes
    fetched_at: str  # ISO-8601 UTC of the original network fetch (from the cache when cached)
    from_cache: bool

    def json(self) -> object:
        return json.loads(self.body)


class CachedClient:
    def __init__(self, cache_dir: Path | str, *, offline: bool = False, min_interval: float = 0.0,
                 max_retries: int = 6, timeout: float = 90.0, user_agent: str = USER_AGENT,
                 backoff_base: float = 2.0, max_reject_retries: int = 2, min_429_wait: float = MIN_429_WAIT) -> None:
        self.cache_dir = Path(cache_dir)
        self.offline = offline
        self.min_interval = min_interval
        self.max_retries = max_retries
        self.timeout = timeout
        self.user_agent = user_agent
        self.backoff_base = backoff_base
        self.max_reject_retries = max_reject_retries
        self.min_429_wait = min_429_wait
        self._lock = threading.Lock()
        self._next_slot = 0.0
        self.stats = {"network": 0, "cache_hits": 0, "retries": 0, "bytes": 0}

    # ------------------------------------------------------------------ public
    def get(self, url: str, params: Mapping[str, str] | None = None, *, headers: Mapping[str, str] | None = None,
            use_cache: bool = True, accept: Callable[[bytes], bool] | None = None) -> Response:
        """`accept(body)` False = transient failure (e.g. MediaWiki maxlag): retried, never cached."""
        full = url + ("?" + urllib.parse.urlencode(sorted(params.items())) if params else "")
        return self._request("GET", full, None, headers or {}, use_cache, accept)

    def post(self, url: str, data: Mapping[str, str], *, headers: Mapping[str, str] | None = None,
             use_cache: bool = True, accept: Callable[[bytes], bool] | None = None) -> Response:
        body = urllib.parse.urlencode(sorted(data.items())).encode("utf-8")
        hdrs = {"Content-Type": "application/x-www-form-urlencoded", **(headers or {})}
        return self._request("POST", url, body, hdrs, use_cache, accept)

    def cache_key(self, method: str, url: str, body: bytes | None) -> str:
        h = hashlib.sha256()
        for part in (method.encode(), url.encode("utf-8"), body or b""):
            h.update(part)
            h.update(b"\x00")
        return h.hexdigest()

    def cache_path(self, key: str) -> Path:
        return self.cache_dir / key[:2] / f"{key}.gz"

    # ------------------------------------------------------------------ internals
    def _request(self, method: str, url: str, body: bytes | None, headers: Mapping[str, str],
                 use_cache: bool, accept: Callable[[bytes], bool] | None) -> Response:
        key = self.cache_key(method, url, body)
        path = self.cache_path(key)
        if use_cache and path.exists():
            cached = _read_cached(path)
            if accept is None or accept(cached.body):
                with self._lock:
                    self.stats["cache_hits"] += 1
                return cached
            path.unlink(missing_ok=True)  # a bad entry from an older client version: fetch again
        if self.offline:
            raise OfflineMiss(f"not cached (offline): {method} {url[:160]}")
        data = self._fetch(method, url, body, headers, accept)
        fetched_at = datetime.now(timezone.utc).isoformat(timespec="seconds")
        if use_cache:
            _write_cached(path, data, fetched_at, method, url, body)
        return Response(data, fetched_at, False)

    def _wait_slot(self) -> None:
        if self.min_interval <= 0:
            return
        with self._lock:
            now = time.monotonic()
            slot = max(now, self._next_slot)
            self._next_slot = slot + self.min_interval
        if slot > now:
            time.sleep(slot - now)

    def _fetch(self, method: str, url: str, body: bytes | None, headers: Mapping[str, str],
               accept: Callable[[bytes], bool] | None = None) -> bytes:
        hdrs = {"User-Agent": self.user_agent, "Accept-Encoding": "gzip", **headers}
        last: Exception | None = None
        rejected = 0
        for attempt in range(self.max_retries + 1):
            self._wait_slot()
            req = urllib.request.Request(url, data=body, headers=hdrs, method=method)
            try:
                with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                    raw = resp.read()
                    if resp.headers.get("Content-Encoding") == "gzip":
                        raw = gzip.decompress(raw)
                with self._lock:
                    self.stats["network"] += 1
                    self.stats["bytes"] += len(raw)
                if accept is None or accept(raw):
                    return raw
                last = HttpError(200, url, b"rejected body: " + raw[:300])
                delay = _retry_after(resp.headers.get("Retry-After"))
                rejected += 1
                if rejected > self.max_reject_retries:
                    break
            except urllib.error.HTTPError as exc:
                payload = exc.read() or b""
                if exc.code not in RETRY_STATUS or _is_query_timeout(payload):
                    raise HttpError(exc.code, url, payload) from exc
                last = HttpError(exc.code, url, payload)
                delay = _retry_after(exc.headers.get("Retry-After"))
                if exc.code == 429:
                    delay = max(delay or 0.0, self.min_429_wait)
            except (urllib.error.URLError, TimeoutError, ConnectionError, OSError) as exc:
                last = exc
                delay = None
            if attempt == self.max_retries:
                break
            with self._lock:
                self.stats["retries"] += 1
            time.sleep(delay if delay is not None else self.backoff_base ** attempt + random.random())
        raise last if last else RuntimeError("request failed")


def _is_query_timeout(payload: bytes) -> bool:
    # WDQS answers a query that ran out of its 60 s budget with a 500 + TimeoutException;
    # retrying the same query does not help, the caller must split it.
    head = payload[:20000]
    return b"TimeoutException" in head or b"upstream request timeout" in head  # 504 from the WDQS gateway


def _retry_after(value: str | None) -> float | None:
    if not value:
        return None
    try:
        return min(float(value), 120.0)
    except ValueError:
        return 30.0


def _read_cached(path: Path) -> Response:
    with gzip.open(path, "rb") as fh:
        header = json.loads(fh.readline())
        return Response(fh.read(), header["fetched_at"], True)


def _write_cached(path: Path, data: bytes, fetched_at: str, method: str, url: str, body: bytes | None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(f".{threading.get_ident()}.tmp")
    meta = {"fetched_at": fetched_at, "method": method, "url": url[:2000]}
    if body:
        meta["body"] = urllib.parse.unquote_plus(body.decode("utf-8", "replace"))[:2000]
    header = json.dumps(meta, ensure_ascii=False).encode("utf-8") + b"\n"
    with gzip.open(tmp, "wb", compresslevel=6) as fh:
        fh.write(header)
        fh.write(data)
    tmp.replace(path)
