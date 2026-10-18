"""Mathlib4 adapter: source tarball -> modules, imports, declarations (Apache-2.0).

Model (docs/SPEC.md):
- each Lean module is a `theory` (MMT reading of a module, R01); imports are
  `depends_on_logically` claims module -> module, with a `context` participant
  naming the layer of the imported module: `infrastructure` (tactics, meta code,
  Lean core and other packages) or `content` (R06);
- the Mathlib directory tree is a hierarchy of `area`s (`part_of_area`, transitive);
- each named declaration is a `statement` (theorem/lemma/axiom), a `definition`
  (def, structure, class, instance, ...) or, inside infrastructure modules, a
  `resource` (software, not mathematics).  Its claim `part_of_area <dir>` is
  `formally_linked` and names the artifact: FormalArtifact(mathlib4, commit, decl);
- statement-level references: a declaration `depends_on_logically` the
  conjunction of the definitions its signature mentions (resolved by name from
  the source text, without elaboration; marked as such in `assumptions`);
- `alias` declarations carry ("mathlib_alias_of", target) so resolve.py can merge them;
- curated Wikidata links (`@[wikidata Q…]` attributes, docs/1000.yaml) are kept
  for `link_wikidata`, which runs after the Wikidata adapter.
"""

from __future__ import annotations

import logging
import multiprocessing as mp
import os
import re
import tarfile
import time
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Iterator

from ..model import (
    EntityType,
    EvidenceState,
    FormalArtifact,
    LangString,
    Participant,
    Provenance,
    RelationType,
    Role,
    Source,
    Entity,
    binary,
    conjunction,
    stable_id,
)
from ..store import Store
from .common import Checkpoint, IngestReport, Timer, Writer, ensure_source
from .http import USER_AGENT
from .lean import KIND_STATEMENT, PARSER_VERSION, FileResult, parse_file

SOURCE_ID = "src:mathlib4"
LICENSE = "Apache-2.0"
TARBALL_URL = "https://codeload.github.com/leanprover-community/mathlib4/tar.gz/refs/heads/master"
EXTRACTOR = "atlas.ingest.mathlib"
ADAPTER_VERSION = f"mathlib/1+{PARSER_VERSION}"
INFRA_DIRS = frozenset({"Tactic", "Lean", "Util", "Testing", "Linter", "Mathport"})
INFRA_MODULES = frozenset({"Mathlib.Init", "Mathlib.Tactic"})
DEFINITION_KINDS = frozenset({"def", "abbrev", "structure", "class", "class inductive", "inductive", "opaque",
                              "irreducible_def"})
MAX_PREMISES = 24
AXIOMS_NOTE = "axioms not computed (source import, no Lean build)"
SIGDEP_NOTE = "statement-level reference resolved by name from source text, without elaboration"
LAYER_IDS = {"content": "mathlib-layer:content", "infrastructure": "mathlib-layer:infrastructure"}
log = logging.getLogger("atlas.ingest.mathlib")
ID_SAFE = re.compile(r"^[A-Za-z0-9_.]{1,200}$")
IDENT_RE = re.compile(r"(?<![\w.'])[^\W\d][\w'!?₀-₉.]*")


# ---------------------------------------------------------------------- ids


def decl_id(name: str) -> str:
    return "mathlib:" + name if ID_SAFE.match(name) else stable_id("mathlib", name)


def module_id(name: str) -> str:
    return ("mathlib-mod:" if name.startswith("Mathlib") else "lean-mod:") + name


def dir_id(path: str) -> str:
    return "mathlib-dir:" + path


def layer_of(module: str) -> str:
    if not (module == "Mathlib" or module.startswith("Mathlib.")):
        return "infrastructure"
    parts = module.split(".")
    if module in INFRA_MODULES or (len(parts) > 1 and parts[1] in INFRA_DIRS):
        return "infrastructure"
    return "content"


# ---------------------------------------------------------------------- tarball


@dataclass(frozen=True)
class Snapshot:
    path: Path
    commit: str
    toolchain: str
    retrieved_at: str


def locate_tarball(cache_root: Path, *, offline: bool, checkpoint: Checkpoint) -> Snapshot:
    """Download the master tarball once; later runs reuse the cached file named by its commit."""
    folder = cache_root / "mathlib"
    known = checkpoint.get("tarball")
    if known and (folder / known["file"]).exists():
        return Snapshot(folder / known["file"], known["commit"], known["toolchain"], known["retrieved_at"])
    cached = sorted(folder.glob("mathlib4-*.tar.gz"), key=lambda p: p.stat().st_mtime)
    if cached:
        path = cached[-1]
        retrieved = datetime.fromtimestamp(path.stat().st_mtime, timezone.utc).isoformat(timespec="seconds")
    elif offline:
        raise FileNotFoundError(f"offline and no mathlib4-*.tar.gz in {folder}")
    else:
        folder.mkdir(parents=True, exist_ok=True)
        part = folder / "download.tar.gz.part"
        _download(TARBALL_URL, part)
        commit = _tar_commit(part)
        path = folder / f"mathlib4-{commit}.tar.gz"
        part.replace(path)
        retrieved = datetime.now(timezone.utc).isoformat(timespec="seconds")
    commit = _tar_commit(path)
    toolchain = _tar_member(path, "lean-toolchain").strip() or "unknown"
    snap = Snapshot(path, commit, toolchain, retrieved)
    checkpoint.mark("tarball", {"file": path.name, "commit": commit, "toolchain": toolchain,
                                "retrieved_at": retrieved})
    return snap


def _download(url: str, dest: Path, retries: int = 4) -> None:
    for attempt in range(retries + 1):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
            with urllib.request.urlopen(req, timeout=300) as resp, open(dest, "wb") as fh:
                while chunk := resp.read(1 << 20):
                    fh.write(chunk)
            return
        except OSError:
            if attempt == retries:
                raise
            time.sleep(2 ** attempt)


def _tar_commit(path: Path) -> str:
    with tarfile.open(path, "r:gz") as tf:
        tf.next()
        return tf.pax_headers.get("comment") or "unknown-commit"


def _tar_member(path: Path, suffix: str) -> str:
    with tarfile.open(path, "r:gz") as tf:
        for m in tf:
            if m.isfile() and m.name.split("/", 1)[-1] == suffix:
                return tf.extractfile(m).read().decode("utf-8")
    return ""


def iter_sources(path: Path) -> Iterator[tuple[str, str]]:
    """(relative path, text) for Mathlib/**/*.lean and the docs/*.yaml link files."""
    with tarfile.open(path, "r:gz") as tf:
        for m in tf:
            if not m.isfile():
                continue
            rel = m.name.split("/", 1)[-1]
            if (rel.startswith("Mathlib/") and rel.endswith(".lean")) or rel in ("docs/1000.yaml", "docs/100.yaml"):
                yield rel, tf.extractfile(m).read().decode("utf-8", errors="replace")


def _parse_pair(pair: tuple[str, str]) -> FileResult:
    return parse_file(*pair)


def parse_all(files: list[tuple[str, str]], workers: int) -> list[FileResult]:
    if workers <= 1 or len(files) < 64:
        return [parse_file(p, t) for p, t in files]
    ctx = mp.get_context("spawn")
    with ctx.Pool(workers) as pool:
        return list(pool.imap_unordered(_parse_pair, files, chunksize=16))


# ---------------------------------------------------------------------- docs/1000.yaml


def parse_links_yaml(text: str) -> list[dict]:
    """Minimal reader for docs/1000.yaml: top-level keys, `field: value`, and `- item` lists."""
    entries: list[dict] = []
    cur: dict | None = None
    last_field = None
    for raw in text.splitlines():
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        if not raw[0].isspace():
            key = raw.split(":", 1)[0].strip()
            cur = {"key": key}
            entries.append(cur)
            last_field = None
            continue
        if cur is None:
            continue
        s = raw.strip()
        if s.startswith("- ") and last_field:
            cur.setdefault(last_field, [])
            if isinstance(cur[last_field], list):
                cur[last_field].append(_unquote(s[2:]))
            continue
        if ":" in s and raw.startswith("  ") and not raw.startswith("    "):
            k, v = s.split(":", 1)
            last_field = k.strip()
            v = _unquote(v.strip())
            cur[last_field] = v if v else []
    return entries


def _unquote(v: str) -> str:
    v = v.strip()
    return v[1:-1] if len(v) >= 2 and v[0] == v[-1] and v[0] in "\"'" else v


# ---------------------------------------------------------------------- ingest


@dataclass
class _Ctx:
    snap: Snapshot
    writer: Writer
    report: IngestReport

    def prov(self, locator: str) -> tuple[Provenance, ...]:
        return (Provenance(SOURCE_ID, locator, EXTRACTOR, ADAPTER_VERSION, LICENSE, self.snap.retrieved_at),)

    def formal(self, name: str) -> FormalArtifact:
        return FormalArtifact("mathlib4", self.snap.commit, name)


def ingest(store: Store, *, cache_root: Path, offline: bool = False, limit: int | None = None,
           workers: int | None = None, report: IngestReport | None = None) -> IngestReport:
    report = report or IngestReport("mathlib")
    ck = Checkpoint(cache_root, "mathlib")
    with Timer(report, "locate_tarball"):
        snap = locate_tarball(cache_root, offline=offline, checkpoint=ck)
    report.counters["commit:" + snap.commit[:12]] = 1
    with Timer(report, "read_tarball"):
        sources = list(iter_sources(snap.path))
    lean_files = sorted((p, t) for p, t in sources if p.endswith(".lean"))
    if limit is not None:
        lean_files = lean_files[:limit]
    docs = {p: t for p, t in sources if p.endswith(".yaml")}
    report.counters["files_lean"] = len(lean_files)
    with Timer(report, "parse"):
        results = sorted(parse_all(lean_files, workers or max(1, (os.cpu_count() or 2) - 1)), key=lambda r: r.path)
    ensure_source(store, Source(SOURCE_ID, "Mathlib4, the Lean 4 mathematical library", LICENSE,
                                uri="https://github.com/leanprover-community/mathlib4", version=snap.commit,
                                retrieved_at=snap.retrieved_at,
                                terms=f"Apache License 2.0; toolchain {snap.toolchain}; parsed from source by "
                                      f"{ADAPTER_VERSION} without a Lean build ({AXIOMS_NOTE})"))
    log.info("parsed %d files, %d declarations", len(results), sum(len(r.decls) for r in results))
    ctx = _Ctx(snap, Writer(store, report, actor=EXTRACTOR), report)
    with Timer(report, "build_and_write"):
        names = _write_structure(ctx, results)
        _write_decls(ctx, results, names)
        ctx.writer.flush()
    links = _curated_links(results, docs, names, report)
    ck.mark("wikidata_links", {"commit": snap.commit, "links": links})
    report.counters["curated_wikidata_links"] = len(links)
    return report


def curated_links(cache_root: Path) -> list[dict]:
    """Wikidata<->Mathlib links found in the last ingest (attributes + docs/1000.yaml)."""
    saved = Checkpoint(cache_root, "mathlib").get("wikidata_links") or {}
    return list(saved.get("links", ()))


def _write_structure(ctx: _Ctx, results: list[FileResult]) -> dict[str, tuple[str, EntityType, bool]]:
    w, rep = ctx.writer, ctx.report
    for layer, (es, en) in {"content": ("Mathlib · contenido matemático", "Mathlib · mathematical content"),
                            "infrastructure": ("Mathlib · infraestructura (tácticas, metaprogramación, núcleo de Lean)",
                                               "Mathlib · infrastructure (tactics, metaprogramming, Lean core)")}.items():
        w.entity(Entity(LAYER_IDS[layer], EntityType.AREA, (LangString(es, "es"), LangString(en, "en")),
                        provenance=ctx.prov(f"layer:{layer}")))
    dirs: set[str] = set()
    modules = {r.module: r for r in results}
    for r in results:
        if r.error:
            rep.note(f"parse error {r.path}: {r.error}")
            rep.counters["files_failed"] += 1
            continue
        rep.counters["files_parsed"] += 1
        for k, v in r.skipped.items():
            rep.counters[f"decls_skipped:{k}"] += v
        parts = r.path[: -len(".lean")].split("/")
        for depth in range(2, len(parts)):
            dirs.add("/".join(parts[:depth]))
    for d in sorted(dirs):
        top = d.split("/")[1]
        layer = "infrastructure" if top in INFRA_DIRS else "content"
        w.entity(Entity(dir_id(d), EntityType.AREA, (LangString(d, "und"),), provenance=ctx.prov(d + "/")))
        parent = d.rsplit("/", 1)[0]
        target = dir_id(parent) if parent in dirs else LAYER_IDS[layer]
        w.claim(binary(stable_id("c", "mathlib", "dir", d), RelationType.PART_OF_AREA, dir_id(d), target,
                       ctx.prov(d + "/")))
    imported = {i for r in results for i in r.imports}
    for name in sorted(set(modules) | imported):
        r = modules.get(name)
        descr = (LangString(r.title, "en"),) if r is not None and r.title else ()
        loc = r.path if r is not None else f"import {name}"
        w.entity(Entity(module_id(name), EntityType.THEORY, (LangString(name, "und"),), descriptions=descr,
                        external_ids=(("lean_module", name),), provenance=ctx.prov(loc)))
    for r in results:
        if r.error:
            continue
        parent = r.path.rsplit("/", 1)[0]
        area = dir_id(parent) if parent in dirs else LAYER_IDS[layer_of(r.module)]
        w.claim(binary(stable_id("c", "mathlib", "modarea", r.module), RelationType.PART_OF_AREA,
                       module_id(r.module), area, ctx.prov(r.path)))
        for imp in r.imports:
            if imp == r.module:
                continue
            rep.counters[f"imports:{layer_of(imp)}"] += 1
            w.claim(binary(stable_id("c", "mathlib", "import", r.module, imp), RelationType.DEPENDS_ON_LOGICALLY,
                           module_id(r.module), module_id(imp), ctx.prov(f"{r.path}: import {imp}"),
                           extra=(Participant(Role.CONTEXT, LAYER_IDS[layer_of(imp)]),)))
    names: dict[str, tuple[str, EntityType, bool]] = {}
    for r in results:
        infra = layer_of(r.module) == "infrastructure"
        for d in r.decls:
            if d.kind == "alias":
                continue
            et = EntityType.RESOURCE if infra else (
                EntityType.STATEMENT if d.kind in KIND_STATEMENT else EntityType.DEFINITION)
            names.setdefault(d.name, (d.kind, et, infra))
    return names


def _write_decls(ctx: _Ctx, results: list[FileResult], names: dict[str, tuple[str, EntityType, bool]]) -> None:
    w, rep = ctx.writer, ctx.report
    seen: set[str] = set()
    pending_sigdeps: list[tuple[FileResult, object, str]] = []  # after all entities: refs point forward too
    for r in results:
        if r.error:
            continue
        parent = r.path.rsplit("/", 1)[0]
        area = dir_id(parent) if parent.count("/") >= 1 else LAYER_IDS[layer_of(r.module)]
        for d in r.decls:
            if d.name in seen:
                rep.counters["decls_duplicate_name"] += 1
                continue
            ext: tuple[tuple[str, str], ...] = (("mathlib", d.name),)
            if d.kind == "alias":
                target = next((c for c in d.alias_candidates if c in names), None)
                if target is None:
                    rep.counters["alias_target_unresolved"] += 1
                    continue
                et = names[target][1]
                ext += (("mathlib_alias_of", target),)
            else:
                et = names[d.name][1]
            seen.add(d.name)
            loc = f"{r.path}:{d.line}"
            short = d.name.rsplit(".", 1)[-1]
            w.entity(Entity(
                decl_id(d.name), et, (LangString(d.name, "und"),),
                aliases=(LangString(short, "und"),) if short != d.name else (),
                descriptions=(LangString(d.doc, "en"),) if d.doc else (),
                statements=(LangString(f"{d.kind} {d.name} {d.signature}".strip(), "und"),),
                external_ids=ext, provenance=ctx.prov(loc)))
            rep.counters[f"decls:{d.kind}"] += 1
            rep.counters[f"entities:{et.value}"] += 1
            w.claim(binary(stable_id("c", "mathlib", "decl", d.name), RelationType.PART_OF_AREA, decl_id(d.name),
                           area, ctx.prov(loc), evidence_state=EvidenceState.FORMALLY_LINKED,
                           formal=ctx.formal(d.name), assumptions=(AXIOMS_NOTE,),
                           extra=(Participant(Role.CONTEXT, module_id(r.module)),)))
            if et is not EntityType.RESOURCE and d.kind != "alias":
                pending_sigdeps.append((r, d, loc))
    for r, d, loc in pending_sigdeps:
        _sigdeps(ctx, r, d, names, loc)


def _sigdeps(ctx: _Ctx, r: FileResult, d, names, loc: str) -> None:
    refs: list[str] = []
    for tok in IDENT_RE.findall(d.signature):
        tok = tok.rstrip(".")
        for cand in _candidates(tok, d.namespace, r.opens):
            hit = names.get(cand)
            if hit is not None:
                if hit[0] in DEFINITION_KINDS and not hit[2] and cand != d.name and cand not in refs:
                    refs.append(cand)
                break
    if not refs:
        return
    if len(refs) > MAX_PREMISES:
        ctx.report.counters["sigdeps_truncated"] += 1
        refs = refs[:MAX_PREMISES]
    ctx.report.counters["sigdeps_premises"] += len(refs)
    ctx.writer.claim(conjunction(stable_id("c", "mathlib", "sigdep", d.name), RelationType.DEPENDS_ON_LOGICALLY,
                                 decl_id(d.name), [decl_id(x) for x in refs], ctx.prov(loc + " (signature)"),
                                 assumptions=(SIGDEP_NOTE,)))


def _candidates(tok: str, ns: tuple[str, ...], opens: tuple[str, ...]) -> Iterable[str]:
    if tok.startswith("_root_."):
        yield tok[len("_root_."):]
        return
    for k in range(len(ns), 0, -1):
        yield ".".join(ns[:k]) + "." + tok
    yield tok
    for o in opens:
        yield o + "." + tok


def _curated_links(results: list[FileResult], docs: dict[str, str], names, report: IngestReport) -> list[dict]:
    links: list[dict] = []
    for r in results:
        for d in r.decls:
            for q in d.wikidata:
                links.append({"qid": q, "decl": d.name, "origin": "attribute", "locator": f"{r.path}:{d.line}",
                              "statement_only": False})
    for e in parse_links_yaml(docs.get("docs/1000.yaml", "")):
        q = e["key"]
        if not re.fullmatch(r"Q\d+", q):
            report.counters["yaml_bad_key"] += 1
            continue
        decls = ([e["decl"]] if isinstance(e.get("decl"), str) and e["decl"] else []) + \
                [x for x in e.get("decls", []) if isinstance(e.get("decls"), list)]
        stmt = [e["statement"]] if isinstance(e.get("statement"), str) and e["statement"] else []
        for name, only in [(x, False) for x in decls] + [(x, True) for x in stmt]:
            links.append({"qid": q, "decl": name, "origin": "docs/1000.yaml", "locator": f"docs/1000.yaml#{q}",
                          "statement_only": only})
    report.counters["curated_links_decl_unknown"] = sum(1 for x in links if x["decl"] not in names)
    return links


def link_wikidata(store: Store, *, cache_root: Path, report: IngestReport | None = None) -> IngestReport:
    """`wd:Q… formalized_as mathlib:decl` for curated links whose two ends are in the store."""
    report = report or IngestReport("mathlib-links")
    ck = Checkpoint(cache_root, "mathlib")
    snap_info = ck.get("tarball")
    links = curated_links(cache_root)
    if not snap_info or not links:
        return report
    snap = Snapshot(cache_root / "mathlib" / snap_info["file"], snap_info["commit"], snap_info["toolchain"],
                    snap_info["retrieved_at"])
    ctx = _Ctx(snap, Writer(store, report, actor=EXTRACTOR), report)
    for link in links:
        wd, decl = store.get_entity("wd:" + link["qid"]), store.get_entity(decl_id(link["decl"]))
        if wd is None or decl is None:
            report.counters["links_missing_endpoint:" + ("wikidata" if wd is None else "mathlib")] += 1
            continue
        ctx.writer.know(wd.id, wd.type)
        ctx.writer.know(decl.id, decl.type)
        note = ("Mathlib states this result without a proof (docs/1000.yaml `statement`)",) if link["statement_only"] else ()
        if ctx.writer.claim(binary(
                stable_id("c", "mathlib", "wikidata", link["qid"], link["decl"]), RelationType.FORMALIZED_AS,
                wd.id, decl.id, ctx.prov(link["locator"]), formal=ctx.formal(link["decl"]), assumptions=note,
                explanation=(LangString(f"Curated link in Mathlib ({link['origin']}); fidelity not assessed", "en"),
                             LangString(f"Vínculo curado en Mathlib ({link['origin']}); fidelidad no evaluada",
                                        "es")))):
            report.counters[f"links_written:{link['origin']}"] += 1
    ctx.writer.flush()
    return report

