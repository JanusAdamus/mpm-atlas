"""`python -m atlas <command>`: orchestrates ingestion (Python) and the Rust engine (`atlas` binary).

    ingest [...]        sources -> SQLite registry (same flags as `python -m atlas.ingest`)
    build               ingest (offline: reuses data/atlas.db and data/cache) -> resolve ->
                        `cargo build --release` -> `atlas compile --all` (snapshot + search indexes + layout + tiles)
    serve               API + web on http://127.0.0.1:8765 (Ctrl+C stops it)
    search QUERY        hybrid search, same response as GET /api/search
    path FROM TO        typed paths, same response as GET /api/path
    stats               snapshot counts (+ --verify) and registry coverage
    fichas [...]        reader's cards: import editorial markdown, render, review (atlas/fichas.py)

Every Rust call runs with cwd = project root so the binary's `data/...` defaults resolve.
`--json` prints one JSON value (what scripts should read); without it the output is indented.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence

from . import NO_WINDOW

ROOT = Path(__file__).resolve().parent.parent
CORE = ROOT / "core"
DATA = ROOT / "data"
DEFAULT_DB = DATA / "atlas.db"
REPORTS = DATA / "cache" / "reports"
EXE = "atlas.exe" if os.name == "nt" else "atlas"


class CliError(RuntimeError):
    """A step failed; the message is meant for the person running the command."""


# ------------------------------------------------------------------ Rust binary

def target_dir() -> Path:
    """CARGO_TARGET_DIR, else core/.cargo/config.toml `target-dir`, else core/target."""
    env = os.environ.get("CARGO_TARGET_DIR")
    if env:
        return Path(env)
    cfg = CORE / ".cargo" / "config.toml"
    if cfg.exists():
        m = re.search(r'^\s*target-dir\s*=\s*"([^"]+)"', cfg.read_text(encoding="utf-8"), re.M)
        if m:
            return Path(m.group(1))
    return CORE / "target"


def find_binary() -> Path:
    env = os.environ.get("ATLAS_BIN")
    path = Path(env) if env else target_dir() / "release" / EXE
    if not path.exists():
        raise CliError(f"no atlas binary at {path}; run `python -m atlas build` or set ATLAS_BIN")
    return path


def cargo() -> str | None:
    found = shutil.which("cargo")
    if found:
        return found
    home = Path.home() / ".cargo" / "bin" / ("cargo.exe" if os.name == "nt" else "cargo")
    return str(home) if home.exists() else None


def cargo_build() -> dict[str, Any]:
    exe = cargo()
    if exe is None:
        return {"skipped": "cargo not found; using the existing binary"}
    t = time.perf_counter()
    env = {**os.environ, "CARGO_TARGET_DIR": str(target_dir())}
    proc = subprocess.run([exe, "build", "--release", "-p", "atlas-server"], cwd=CORE, env=env,
                          capture_output=True, text=True, encoding="utf-8", errors="replace", **NO_WINDOW)
    if proc.returncode != 0:
        raise CliError("cargo build failed:\n" + proc.stderr[-4000:])
    return {"elapsed_s": round(time.perf_counter() - t, 2), "binary": str(target_dir() / "release" / EXE)}


def run_atlas(args: Sequence[str]) -> Any:
    """Run `atlas <args> --json` and return its JSON value; raise CliError on its error envelope."""
    proc = subprocess.run([str(find_binary()), *args, "--json"], cwd=ROOT, capture_output=True,
                          text=True, encoding="utf-8", errors="replace", **NO_WINDOW)
    out = proc.stdout.strip().splitlines()
    try:
        value = json.loads(out[-1]) if out else None
    except json.JSONDecodeError:
        value = None
    if proc.returncode != 0 or value is None:
        err = value.get("error") if isinstance(value, dict) else None
        detail = f"{err['code']}: {err['message']}" if err else (proc.stderr.strip()[-2000:] or "no output")
        raise CliError(f"atlas {args[0]} failed ({detail})")
    return value


# ------------------------------------------------------------------ commands

def cmd_ingest(argv: list[str]) -> int:
    from .ingest.__main__ import main as ingest_main

    return ingest_main(argv)


def cmd_build(a: argparse.Namespace) -> dict[str, Any]:
    import logging

    from .ingest import SOURCES

    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(name)s %(message)s", stream=sys.stderr)
    db = Path(a.db)
    report: dict[str, Any] = {"started_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
                              "db": str(db), "offline": not a.online}
    t0 = time.perf_counter()
    if a.skip_ingest:
        report["ingest"] = {"skipped": True}
    else:
        if not db.exists() and not a.online:
            print(f"note: {db} does not exist; an offline build only reads data/cache", file=sys.stderr)
        sources = [s for s in (a.sources or ",".join(SOURCES)).split(",") if s]
        _step(f"ingest {','.join(sources)} ({'online' if a.online else 'offline, cache only'})")
        report["ingest"] = _ingest_each(db, sources, online=a.online,
                                        budget_s=None if a.budget_min is None else a.budget_min * 60)
    if a.no_cargo:
        report["cargo"] = {"skipped": "--no-cargo"}
    else:
        _step("cargo build --release -p atlas-server")
        report["cargo"] = cargo_build()
    _step("atlas compile --all (snapshot + search indexes + layout + tiles)")
    t = time.perf_counter()
    report["compile"] = run_atlas(["compile", "--all", "--db", str(db), "--out", a.snapshot,
                                   "--index", a.index, "--layout", a.layout])
    report["compile_wall_s"] = round(time.perf_counter() - t, 2)
    report["stats"] = run_atlas(["stats", "--snapshot", a.snapshot, "--verify"])
    report["elapsed_s"] = round(time.perf_counter() - t0, 2)
    REPORTS.mkdir(parents=True, exist_ok=True)
    path = REPORTS / f"build-{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}.json"
    path.write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf-8")
    report["report_path"] = str(path)
    return report


def _ingest_each(db: Path, sources: list[str], *, online: bool, budget_s: float | None) -> dict[str, Any]:
    """One source at a time, in order.  Offline, a source whose cache cannot replay its last import
    (e.g. Wikidata answers fetched across runs with different seed results) is skipped and reported:
    its earlier import is already in the registry, which is the source of truth."""
    from .ingest import run_ingest
    from .ingest.http import OfflineMiss
    from .store import Store

    out: dict[str, Any] = {}
    with Store(db) as store:
        for src in sources:
            _step(f"ingest {src}")
            t = time.perf_counter()
            try:
                out[src] = run_ingest(store, [src], offline=not online, budget_s=budget_s)
            except OfflineMiss as exc:
                if online:
                    raise
                out[src] = {"skipped": "offline replay needs a request missing from data/cache; "
                                       "the registry keeps this source's earlier import",
                            "first_miss": str(exc), "elapsed_s": round(time.perf_counter() - t, 2)}
                print(f"[atlas build] {src}: {out[src]['skipped']}", file=sys.stderr, flush=True)
    return out


def cmd_serve(a: argparse.Namespace) -> int:
    args = [str(find_binary()), "serve", "--addr", a.addr, "--db", a.db, "--snapshot", a.snapshot,
            "--index", a.index, "--layout", a.layout]
    print(f"atlas serve on http://{a.addr} (Ctrl+C to stop)", file=sys.stderr)
    try:
        return subprocess.run(args, cwd=ROOT).returncode
    except KeyboardInterrupt:
        return 0


def cmd_search(a: argparse.Namespace) -> Any:
    args = ["search", a.query, "--lang", a.lang, "--limit", str(a.limit)]
    if a.types:
        args += ["--types", a.types]
    return run_atlas(args)


def cmd_path(a: argparse.Namespace) -> Any:
    return run_atlas(["path", a.source, a.target, "--objective", a.objective, "--max-hops", str(a.max_hops)])


def cmd_stats(a: argparse.Namespace) -> dict[str, Any]:
    from .store import Store

    out: dict[str, Any] = {"snapshot": run_atlas(["stats", "--snapshot", a.snapshot] + (["--verify"] if a.verify else []))}
    if Path(a.db).exists():
        with Store(a.db, read_only=True) as store:
            out["registry"] = store.coverage()
    return out


# ------------------------------------------------------------------ entry point

def _step(msg: str) -> None:
    print(f"[atlas build] {msg}", file=sys.stderr, flush=True)


def _parser() -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(prog="python -m atlas", description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("ingest", add_help=False, help="sources -> registry (flags of python -m atlas.ingest)")

    def common(p: argparse.ArgumentParser) -> argparse.ArgumentParser:
        p.add_argument("--json", action="store_true", help="print one compact JSON value")
        return p

    def paths(p: argparse.ArgumentParser) -> None:
        p.add_argument("--db", default=str(DEFAULT_DB))
        p.add_argument("--snapshot", default="data/snapshot")
        p.add_argument("--index", default="data/index")
        p.add_argument("--layout", default="data/layout")

    b = common(sub.add_parser("build", help="ingest -> resolve -> compile snapshot, indexes, layout"))
    paths(b)
    b.add_argument("--sources", default=None, help="comma-separated (default: all, resolve last)")
    b.add_argument("--online", action="store_true", help="allow network fetches (default: cache only)")
    b.add_argument("--budget-min", type=float, default=None, help="cap the Wikipedia stage (online)")
    b.add_argument("--skip-ingest", action="store_true", help="compile the registry as it is")
    b.add_argument("--no-cargo", action="store_true", help="do not rebuild the Rust binary")

    s = sub.add_parser("serve", help="API + web")
    paths(s)
    s.add_argument("--addr", default="127.0.0.1:8765")

    q = common(sub.add_parser("search", help="hybrid search"))
    q.add_argument("query")
    q.add_argument("--lang", default="es", choices=("es", "en"))
    q.add_argument("--limit", type=int, default=10)
    q.add_argument("--types", default=None, help="comma-separated entity types")

    p = common(sub.add_parser("path", help="typed paths between two entity ids"))
    p.add_argument("source")
    p.add_argument("target")
    p.add_argument("--objective", default="best_supported")
    p.add_argument("--max-hops", type=int, default=6)

    st = common(sub.add_parser("stats", help="snapshot and registry counts"))
    st.add_argument("--db", default=str(DEFAULT_DB))
    st.add_argument("--snapshot", default="data/snapshot")
    st.add_argument("--verify", action="store_true")
    return ap


COMMANDS = {"build": cmd_build, "search": cmd_search, "path": cmd_path, "stats": cmd_stats}


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv[:1] == ["ingest"]:
        return cmd_ingest(argv[1:])
    if argv[:1] == ["fichas"]:
        from .fichas import main as fichas_main

        return fichas_main(argv[1:])
    a = _parser().parse_args(argv)
    sys.stdout.reconfigure(encoding="utf-8")
    try:
        if a.cmd == "serve":
            return cmd_serve(a)
        value = COMMANDS[a.cmd](a)
    except CliError as exc:
        print(f"atlas: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(value, ensure_ascii=False) if a.json else json.dumps(value, ensure_ascii=False, indent=1))
    return 0
