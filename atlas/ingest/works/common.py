"""Shared plumbing of the works adapters (ADR-0005 §2, §8): sources, ids, provenance, the filtered
input cache and one `Store.ingest_batch` per batch key.

Metadata only: title, authors, date, venue, ids, classification and citations.  No abstract, review,
keyword or full text is ever written, cached or stored (`assert_metadata_only` enforces it on every
entity before a batch is written).
"""

from __future__ import annotations

import gzip
import hashlib
import json
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Iterator

from ...model import Claim, Entity, Provenance, RelationType, Source, ValidationError, binary, now_iso, stable_id
from ...store import Store
from ..common import DEFAULT_CACHE, ensure_source

CONTACT = os.environ.get("ATLAS_CONTACT", "").strip()
USER_AGENT = f"MPM-atlas-works/1 (personal open math atlas; contact: {CONTACT or 'set ATLAS_CONTACT'}) python-urllib"
WORKS_CACHE = DEFAULT_CACHE / "works"
ACTOR = "ingest:works/1"
LAYER = json.loads((Path(__file__).resolve().parents[3] / "shapes" / "works_layer.json").read_text(encoding="utf-8"))
CLAIM_PREFIX = LAYER["claim_id_prefix"].rstrip(":")  # "cw": the Rust layer filter keys on it
ENTITY_PREFIXES = tuple(LAYER["entity_id_prefixes"])

# Verified on each source's own page, 2026-10-19 (docs/plans/r5/piloto-obras.md §Licencias).
SOURCES: dict[str, Source] = {
    "zbmath": Source("src:zbmath", "zbMATH Open", "CC-BY-SA-4.0", uri="https://api.zbmath.org/v1/",
                     version="REST v1, 2026-10-19", kind="dataset",
                     terms="API terms (June 2021): all data CC BY-SA 4.0; reviews of third parties excluded. "
                           "Only metadata is stored: no reviews, no summaries, no keywords."),
    "openalex": Source("src:openalex", "OpenAlex", "CC0-1.0", uri="https://api.openalex.org/",
                       version="API, 2026-10-19", kind="dataset",
                       terms="'Our data is CC0' (openalex.org).  Abstracts (inverted index) are never requested."),
    "opencitations": Source("src:opencitations", "OpenCitations Index", "CC0-1.0",
                            uri="https://api.opencitations.net/index/v2/", version="REST v2, 2026-10-19",
                            kind="dataset",
                            terms="'The data held in any of the OpenCitations datasets are available under a "
                                  "Creative Commons public domain dedication (CC0)' (opencitations.net/about, "
                                  "read 2026-10-19)."),
    "crossref": Source("src:crossref", "Crossref REST API", "CC0-1.0", uri="https://api.crossref.org/",
                       version="REST, 2026-10-19", kind="dataset",
                       terms="'almost none of the metadata is subject to copyright, and you may use it for any "
                             "purpose' (crossref.org).  Only DOI references are read; abstracts are not."),
    "cdli": Source("src:cdli", "Cuneiform Digital Library Initiative (CDLI)", "https://cdli.earth/terms-of-use",
                   uri="https://cdli.earth/", version="cdli.earth search JSON, 2026-10-19", kind="dataset",
                   terms="Text 'may be freely copied, aggregated and re-used according to common and fair academic "
                         "practice', with mention of CDLI.  Only catalogue metadata (designation, museum number, "
                         "period) is stored: no ATF transliteration, no images."),
}
LICENSE = {k: s.license for k, s in SOURCES.items()}
# Fields that would carry text with its own rights; works entities must not have them.
FORBIDDEN_KEYS = ("descriptions", "statements", "formula")


def register_sources(store: Store, keys: Iterable[str]) -> None:
    for k in keys:
        ensure_source(store, SOURCES[k])


def prov(source: str, locator: str, extractor: str, version: str, retrieved_at: str) -> Provenance:
    return Provenance(SOURCES[source].id, locator, extractor, version, LICENSE[source], retrieved_at)


def claim(relation: RelationType, subject: str, obj: str, p: Provenance, *key: str) -> Claim:
    """A works-layer binary claim with a deterministic `cw:` id (idempotent re-runs)."""
    return binary(stable_id(CLAIM_PREFIX, relation.value, subject, obj, *key), relation, subject, obj, (p,))


def assert_metadata_only(entities: Iterable[Entity]) -> None:
    for e in entities:
        if e.descriptions or e.statements or e.formula:
            raise ValidationError(f"works entity {e.id} carries text beyond metadata (abstract/review?)")


def norm_doi(doi: str | None) -> str | None:
    if not doi:
        return None
    d = doi.strip().lower()
    for pre in ("https://doi.org/", "http://doi.org/", "http://dx.doi.org/", "doi:"):
        if d.startswith(pre):
            d = d[len(pre):]
    return d or None


LANGS = {"English": "en", "French": "fr", "German": "de", "Russian": "ru", "Spanish": "es", "Italian": "it",
         "Portuguese": "pt", "Chinese": "zh", "Japanese": "ja", "Polish": "pl", "Dutch": "nl", "Latin": "la"}


# ---------------------------------------------------------------------- filtered input cache


@dataclass(frozen=True)
class CachedBatch:
    path: Path
    rows: list[dict[str, Any]]
    sha256: str


def cache_path(source: str, key: str) -> Path:
    safe = "".join(ch if ch.isalnum() or ch in "-_.=" else "_" for ch in key)
    return WORKS_CACHE / source / f"{safe}.jsonl.gz"


def write_cache(path: Path, rows: Iterable[dict[str, Any]]) -> None:
    """Atomic: a partial file never exists under the final name (an interrupted fetch re-runs)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(f".{os.getpid()}.tmp")
    with gzip.open(tmp, "wt", encoding="utf-8", compresslevel=6) as fh:
        for r in rows:
            fh.write(json.dumps(r, ensure_ascii=False, sort_keys=True) + "\n")
    tmp.replace(path)


def read_cache(path: Path) -> CachedBatch:
    raw = gzip.decompress(path.read_bytes())
    rows = [json.loads(line) for line in raw.decode("utf-8").splitlines() if line]
    return CachedBatch(path, rows, hashlib.sha256(raw).hexdigest())


def iter_cached(source: str) -> Iterator[Path]:
    yield from sorted((WORKS_CACHE / source).glob("*.jsonl.gz"))


def existing(store: Store, ids: Iterable[str]) -> set[str]:
    """Which of `ids` are entities in the registry (chunked IN queries)."""
    out: set[str] = set()
    ids = list(dict.fromkeys(ids))
    for i in range(0, len(ids), 900):
        part = ids[i:i + 900]
        out.update(r[0] for r in store._conn.execute(  # noqa: SLF001 - read-only lookup
            f"SELECT id FROM entities WHERE id IN ({','.join('?' * len(part))})", part))
    return out


def by_external(store: Store, scheme: str, values: Iterable[str], *, skip_prefix: str = "") -> dict[str, str]:
    """value -> canonical entity id, for one external-id scheme (chunked).  `skip_prefix` ignores the ids
    of one source in SQL, so a row of that source can never hide another source's record."""
    out: dict[str, str] = {}
    vals = list(dict.fromkeys(values))
    skip = " AND entity_id NOT GLOB ?" if skip_prefix else ""
    for i in range(0, len(vals), 900):
        part = vals[i:i + 900]
        for v, eid in store._conn.execute(  # noqa: SLF001
                f"SELECT value, entity_id FROM external_ids WHERE scheme=? AND value IN ({','.join('?' * len(part))})"
                f"{skip} ORDER BY entity_id", [scheme, *part, *([skip_prefix + "*"] if skip_prefix else [])]):
            out.setdefault(v, eid)
    return out


def run_batch(store: Store, source: str, run: str, key: str, batch: CachedBatch,
              entities: list[Entity], claims: list[Claim], stats: dict[str, Any]) -> int | None:
    assert_metadata_only(entities)
    done = store._conn.execute(  # noqa: SLF001 - read-only lookup
        "SELECT input_sha256 FROM ingest_batches WHERE source_id=? AND run=? AND batch_key=?",
        (SOURCES[source].id, run, key)).fetchone()
    if done and done[0] != batch.sha256:  # a refetched cache file is never skipped as "already ingested"
        raise ValidationError(f"batch {source}/{run}/{key}: input changed since it was ingested "
                              f"({done[0][:12]} -> {batch.sha256[:12]}); use a new run name")
    claims = list({c.id: c for c in claims}.values())  # one claim per id (a source may repeat a statement)
    return store.ingest_batch(source_id=SOURCES[source].id, run=run, batch_key=key, input_sha256=batch.sha256,
                              actor=ACTOR, entities=entities, claims=claims, stats=stats)


def today() -> str:
    return now_iso()
