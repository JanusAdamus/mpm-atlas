"""`python -m bench <command>`: AtlasBench (bench/PROTOCOL.md §9)."""

from __future__ import annotations

import argparse
import json
import socket
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterator

from atlas import NO_WINDOW, Store

from . import evaluate, leakage, report, silver, splits
from .backends import BackendError, ComponentMissing, CliBackend, HttpBackend, StoreBackend, find_binary
from .env import hardware
from .items import Item, iter_dir, read_jsonl, write_jsonl
from .metrics import cohen_kappa, krippendorff_alpha
from .registry import Registry
from .render import markdown

BENCH = Path(__file__).resolve().parent
ROOT = BENCH.parent
DEFAULT_DB = ROOT / "data" / "atlas.db"
HANDWRITTEN = BENCH / "items" / "handwritten"
SILVER = BENCH / "silver" / "current"
SERVE_START_S = 120
VERSION = "v3"  # item-set version (PROTOCOL §6.2 i, §6.3 n); v1 and v2 kept as history


def _out(obj: object) -> None:
    sys.stdout.reconfigure(encoding="utf-8")
    print(json.dumps(obj, ensure_ascii=False, indent=1, default=str))


def _backends(args: argparse.Namespace, reg: Registry) -> tuple[list, dict]:
    found: list = [StoreBackend(reg)]
    notes: dict = {"store": f"registry {reg.path}"}
    binary = Path(args.bin) if args.bin else find_binary()
    if getattr(args, "skip_cli", False):  # the CLI spawns one process per call (~1 s); with --http it is redundant
        notes["cli"] = "skipped by --skip-cli (the HTTP API serves the same engine)"
    elif binary is None or not binary.exists():
        notes["cli"] = "missing: no atlas binary (set --bin or ATLAS_BIN)"
    else:
        cli = CliBackend(binary, snapshot=Path(args.snapshot), index=Path(args.index),
                         composition=Path(args.composition), db=reg.path)
        try:
            if args.compile:
                out = Path(tempfile.gettempdir()) / "atlasbench" / "snapshots" / f"{reg.path.stem}-{reg.event_seq()}"
                notes["compile"] = cli.compile(reg.path, out)
            notes["cli"] = {"binary": str(binary), **cli.check(reg.event_seq())}
            found.append(cli)
            if notes["cli"]["stale"]:
                print(f"warning: snapshot is stale ({notes['cli']['source_event_seq']} != registry "
                      f"{notes['cli']['registry_event_seq']}); pass --compile", file=sys.stderr)
        except ComponentMissing as exc:
            notes["cli"] = f"missing: {exc}"
    if args.http:
        http = HttpBackend(args.http)
        found.append(http)
        try:  # which snapshot the server holds, so staleness (PROTOCOL §6.1 d) is checked for HTTP too
            snap = http.get("/api/health", {}).value.get("snapshot") or {}
            seq = snap.get("source_event_seq")
            notes["http"] = {"url": args.http, "snapshot": snap.get("path"), "content_hash": snap.get("content_hash"),
                             "source_event_seq": seq, "registry_event_seq": reg.event_seq(),
                             "stale": seq is not None and seq != reg.event_seq()}
        except Exception as exc:  # noqa: BLE001 - health is informative; the components report themselves
            notes["http"] = {"url": args.http, "health": f"{type(exc).__name__}: {exc}"}
    else:
        notes["http"] = "missing: no --http URL given"
    return found, notes


def cmd_probe(args: argparse.Namespace) -> int:
    out: dict = {"registry": None, "components": {}}
    try:
        reg = Registry.open(args.db)
        out["registry"] = reg.fingerprint()
    except FileNotFoundError as exc:
        _out({"registry": str(exc), "components": "registry missing: every component is NOT_EVALUABLE"})
        return 0
    bks, notes = _backends(args, reg)
    out["backends"] = notes
    claim = next(iter(reg.store.iter_claims(batch=1)), None)  # a real edge: the path probe needs two endpoints
    eid, dst = (claim.subject, claim.targets[0]) if claim else ("x", "y")
    for b in bks:
        for comp, call in (("search", lambda: b.search("a", lang="en", limit=1)),
                           ("path", lambda: b.path(eid, dst, objective="best_supported", max_hops=2)),
                           ("entity", lambda: b.entity(eid))):
            try:
                call()
                out["components"][f"{b.name}.{comp}"] = "available"
            except ComponentMissing as exc:
                out["components"][f"{b.name}.{comp}"] = f"MISSING: {exc}"
            except Exception as exc:  # noqa: BLE001 - a probe reports, never crashes
                out["components"][f"{b.name}.{comp}"] = f"ERROR: {type(exc).__name__}: {exc}"
    _out(out)
    return 0


def cmd_build_silver(args: argparse.Namespace) -> int:
    t = time.perf_counter()
    with Store(args.db, read_only=True) as store:
        items, stats = silver.build(store, seed=args.seed, progress=lambda s: print(s, file=sys.stderr))
    out = Path(args.out)
    n = write_jsonl(out / "silver.jsonl", sorted(items, key=lambda i: i.id))
    stats.update(db=Path(args.db).as_posix(), registry=Registry.open(args.db).fingerprint(), written=n, seconds=round(time.perf_counter() - t, 2),
                 by_task={k: len(v) for k, v in evaluate.by_task(items).items()},
                 created=datetime.now(timezone.utc).isoformat())
    (out / "stats.json").write_text(json.dumps(stats, indent=1, default=str), encoding="utf-8")
    _out(stats)
    return 0


def _collect(inputs: list[str]) -> tuple[list[Item], list[str]]:
    items, used = [], []
    for inp in inputs:
        p = Path(inp)
        if p.is_dir():
            items += list(iter_dir(p))
        elif p.exists():
            items += read_jsonl(p)
        else:
            print(f"skipping missing input {p}", file=sys.stderr)
            continue
        used.append(p.relative_to(ROOT).as_posix() if p.is_relative_to(ROOT) else p.as_posix())
    return items, used


def _silver_stats() -> dict | None:
    p = SILVER / "stats.json"
    return json.loads(p.read_text(encoding="utf-8")) if p.exists() else None


def cmd_split(args: argparse.Namespace) -> int:
    items, used = _collect(args.inputs or [str(HANDWRITTEN), str(SILVER)])
    man = splits.write_splits(items, BENCH / "splits" / args.version, BENCH / "manifest" / f"{args.version}.json",
                              version=args.version, cutoff=args.cutoff, inputs=used, root=ROOT,
                              extra={"silver_stats": _silver_stats()})
    _out({k: {kk: vv for kk, vv in v.items() if kk != "items"} for k, v in man["splits"].items()}
         | {"components": man["components"]})
    return 0


def cmd_verify(args: argparse.Namespace) -> int:
    probs = splits.verify(BENCH / "manifest" / f"{args.version}.json", ROOT)
    _out({"version": args.version, "intact": not probs, "problems": probs[:50]})
    return 1 if probs else 0


def cmd_leakage(args: argparse.Namespace) -> int:
    items, _ = _collect(args.inputs or [str(HANDWRITTEN), str(SILVER)])
    reg = Registry.open(args.db)
    verdicts = leakage.annotate(items, reg)
    _out({"db": reg.fingerprint(), "resolved_items": sum(bool(v) for v in verdicts.values()),
          "retrieval_items": sum(i.task == "retrieval" for i in items), "by_generator": leakage.report(items, verdicts)})
    return 0


def _load_split(args: argparse.Namespace) -> tuple[list[Item], dict]:
    if args.items:
        items, used = _collect(args.items)
        return items, {"items": used, "split": "adhoc", "manifest": None}
    man_path = BENCH / "manifest" / f"{args.version}.json"
    if not man_path.exists():
        raise SystemExit(f"no manifest {man_path}; run `python -m bench split` first or pass --items")
    probs = splits.verify(man_path, ROOT)
    if probs:
        raise SystemExit(f"manifest {args.version} does not match the split files: {probs[:3]}")
    if args.split == "sealed":
        if not args.open_sealed:
            raise SystemExit("the sealed split needs --open-sealed '<reason>' (PROTOCOL.md §3)")
        splits.log_sealed_opening(BENCH, reason=args.open_sealed, manifest_version=args.version,
                                  system=args.bin or "auto")
    items = read_jsonl(BENCH / "splits" / args.version / f"{args.split}.jsonl")
    return items, {"split": args.split, "manifest": man_path.relative_to(ROOT).as_posix()}


def _load_test(call, args: list, n: int, conc: int) -> tuple[list[float], int]:
    """n requests cycling over `args`, `conc` at a time; per-request wall ms.  One warm-up pass first
    (PROTOCOL §6: hot cache).  Returns (timings of successful requests, number of error answers)."""
    def one(a: object) -> float | None:  # an error answer (e.g. HTTP 400) is counted, not timed
        try:
            return call(a).ms
        except BackendError:
            return None
    for a in args[:min(len(args), 20)]:
        one(a)
    with ThreadPoolExecutor(conc) as ex:
        got = list(ex.map(lambda i: one(args[i % len(args)]), range(n)))
    errors = sum(g is None for g in got)
    if errors == len(got) and got:
        raise BackendError(f"all {errors} requests failed")
    return [g for g in got if g is not None], errors


def _interaction_load(http: HttpBackend, ids: list[str], n: int, lat: dict, notes: dict) -> None:
    """PROTOCOL §6 'latencia, interacción local': 500 requests per server endpoint, concurrency 1."""
    tiles = [(z, x, y) for z in range(4) for x in range(2 ** z) for y in range(2 ** z)]
    calls = {"entity": (lambda e: http.entity(e), ids),
             "neighborhood": (lambda e: http.get("/api/neighborhood", {"id": e, "depth": 1, "limit": 200}), ids),
             "tiles": (lambda t: http.get("/api/map/tiles", {"layer": "structure", "z": t[0], "x": t[1], "y": t[2]}),
                       tiles)}
    for name, (call, args) in calls.items():
        try:
            lat[f"http.{name}.load_c1"], notes[f"load_errors.{name}"] = _load_test(call, args, n, 1)
        except (ComponentMissing, Exception) as exc:  # noqa: BLE001 - a missing endpoint is reported, not fatal
            notes[f"load_{name}"] = f"{type(exc).__name__}: {exc}"


@contextmanager
def _served(args: argparse.Namespace, db: Path, log: Path) -> Iterator[None]:
    """`--serve`: run `atlas serve` on a free loopback port for the duration of the run, on the same registry
    (and the snapshot given or compiled), and point --http at it.  Stops it afterwards."""
    if not args.serve:
        yield
        return
    binary = Path(args.bin) if args.bin else find_binary()
    if binary is None or not binary.exists():
        raise SystemExit("--serve needs the atlas binary (--bin or ATLAS_BIN)")
    with socket.socket() as sk:
        sk.bind(("127.0.0.1", 0))
        port = sk.getsockname()[1]
    log.parent.mkdir(parents=True, exist_ok=True)
    with open(log, "w", encoding="utf-8") as fh:
        proc = subprocess.Popen([str(binary), "serve", "--snapshot", args.snapshot, "--db", str(db), "--addr",
                                 f"127.0.0.1:{port}", "--index", args.index, "--layout", args.layout,
                                 "--composition", args.composition], stdout=fh, stderr=subprocess.STDOUT, cwd=ROOT,
                                **NO_WINDOW)
        args.http = f"http://127.0.0.1:{port}"
        try:
            deadline = time.monotonic() + SERVE_START_S
            while True:
                try:
                    HttpBackend(args.http).get("/api/health", {})
                    break
                except Exception:  # noqa: BLE001 - not up yet
                    if proc.poll() is not None or time.monotonic() > deadline:
                        raise SystemExit(f"atlas serve did not start (exit {proc.poll()}); see {log}")
                    time.sleep(0.2)
            yield
        finally:
            args.server_exit = proc.poll()  # None = still alive at the end of the run (it should be)
            proc.terminate()
            proc.wait(timeout=30)


def cmd_run(args: argparse.Namespace) -> int:
    if args.serve and args.compile:
        raise SystemExit("--serve with --compile: compile first (`probe --compile`), then pass --snapshot")
    out = Path(args.out) if args.out else BENCH / "runs" / datetime.now().strftime("%Y%m%d-%H%M%S")
    with _served(args, Path(args.db), out / "server.log"):
        return _run(args, out)


def _run(args: argparse.Namespace, out: Path) -> int:
    items, where = _load_split(args)
    reg = Registry.open(args.db)
    reg_start = reg.fingerprint()
    bks, notes = _backends(args, reg)
    tasks = evaluate.by_task(items)
    t0 = time.perf_counter()
    verd = leakage.annotate(tasks.get("retrieval", []), reg)
    ret = evaluate.retrieval(tasks.get("retrieval", []), reg, bks, leakage.trivial_ids(verd))
    rts = evaluate.routes(tasks.get("route", []), reg, bks, max_hops=args.max_hops)
    er = evaluate.er_pairs(tasks.get("er_pair", []), reg, evaluate.load_queue(Path(args.er_queue) if args.er_queue
                                                                              else None))
    rel = evaluate.relations(tasks.get("relation", []), reg)
    src = evaluate.sourced(tasks.get("sourced_answer", []), bks, reg)
    system = "http" if args.http else "cli"
    if system not in {b.name for b in bks}:  # never constructed (no binary / bad snapshot): MISSING, not unlabeled
        for res in (ret, rts, src):
            res["missing"].setdefault(system, str(notes.get(system)))
    lat: dict[str, list[float]] = {}
    for comp, res in (("search", ret), ("path", rts), ("entity", src)):
        for r in res["records"]:
            for b, v in r["by_backend"].items():
                if "ms" in v:
                    lat.setdefault(f"{b}.{comp}", []).append(v["ms"])
    http = next((b for b in bks if b.name == "http"), None)
    if http is not None and "http" not in ret["missing"] and tasks.get("retrieval"):
        # load corpus = the queries the server answered in the item pass (a rejected query is scored there)
        qs = [(r["query"], r["lang"]) for r in ret["records"] if "failed" not in r["by_backend"].get("http", {})]
        for conc in (1, 8):
            try:
                lat[f"http.search.load_c{conc}"], notes[f"load_errors.search_c{conc}"] = _load_test(
                    lambda a: http.search(a[0], lang=a[1], limit=10), qs, args.load_n, conc)
            except (ComponentMissing, BackendError) as exc:
                notes[f"load_c{conc}"] = f"{type(exc).__name__}: {exc}"
    if http is not None:
        ids = list(dict.fromkeys(i.payload["entity"].get("id") or i.payload["entity"]["ext"][1]
                                 for i in tasks.get("sourced_answer", [])))
        if ids:
            _interaction_load(http, ids, args.load_n, lat, notes)
    rep = {
        "date": datetime.now(timezone.utc).isoformat(), "protocol": "bench/PROTOCOL.md v1.3 (thresholds frozen 2026-10-16; clarifications 6.1-6.3)",
        "data": where, "registry": reg_start, "registry_at_end": reg.fingerprint(), "hardware": hardware(), "backends": notes,
        "system_under_test": system, "baseline": report.BASELINE, "sealed_openings": splits.sealed_openings(BENCH),
        "items": {k: len(v) for k, v in tasks.items()},
        "evidence": {k: sorted({f"{i.origin}/{i.label_status}" for i in v}) for k, v in tasks.items()},
        "summary": {"retrieval": report.retrieval_summary(ret), "routes": report.routes_summary(rts),
                    "er": report.er_summary(er), "relations": report.relations_summary(rel),
                    "sourced": report.sourced_summary(src), "latency_ms": report.latency_summary(lat)},
        "verdicts": {"retrieval": report.retrieval_verdict(ret, system), **{
            f"routes.{k}": v for k, v in report.routes_verdicts(rts, system).items()},
            "routes.null_model_adversarial": report.routes_verdicts(rts, "store")["adversarial"],
            "er": report.er_verdict(er), "relations": report.relations_verdict(rel),
            "provenance.registry": report.sourced_verdict(src, "store"),
            "provenance.served": report.sourced_verdict(src, "http") if args.http else {
                "verdict": "MISSING", "rule": "served provenance", "reason": "no --http server"},
            **{f"latency.search_hot.c{c}": report.latency_verdict(
                lat.get(f"http.search.load_c{c}", []), report.SEARCH_P95_MS, f"HTTP search c={c}") for c in (1, 8)},
            **{f"latency.interaction_server.{e}": report.latency_verdict(
                lat.get(f"http.{e}.load_c1", []), report.INTERACTION_P95_MS, f"HTTP {e}")
               for e in ("entity", "neighborhood", "tiles")}},
        "seconds": round(time.perf_counter() - t0, 2)}
    for k, v in rep["verdicts"].items():  # a load that could not run is a missing component, not missing labels
        if k.startswith("latency.") and v["verdict"] == "NOT_EVALUABLE":
            why = notes.get(f"load_{k.rsplit('.', 1)[1]}") if "interaction" in k else notes.get(f"load_{k[-2:]}")
            if http is None or why:
                rep["verdicts"][k] = {"verdict": "MISSING", "rule": v["rule"], "reason": why or "no --http server"}
    rep["registry_changed_during_run"] = rep["registry_at_end"]["event_seq"] != reg_start["event_seq"]
    if rep["registry_changed_during_run"]:
        print("warning: the registry changed during the run (concurrent ingestion?)", file=sys.stderr)
    sut_info = notes.get(system)
    if isinstance(sut_info, dict) and sut_info.get("stale"):  # PROTOCOL §6.1 (d)
        for k in [k for k in rep["verdicts"] if k == "retrieval" or k.startswith("routes.") and "null" not in k]:
            rep["verdicts"][k] = {"verdict": "NOT_EVALUABLE", "rule": rep["verdicts"][k]["rule"],
                                  "reason": "snapshot stale vs registry (PROTOCOL §6.1 d); rerun with --compile"}
    for b in bks:
        if isinstance(b, HttpBackend):
            rep["http_connection_retries"] = b.retries
    if args.http:  # a server that died mid-run turns every later item MISSING: say so instead of hiding it
        try:
            rep["server_alive_at_end"] = bool(HttpBackend(args.http).get("/api/health", {}).value)
        except ComponentMissing:
            rep["server_alive_at_end"] = False
            print("warning: the HTTP server died during the run; later items are MISSING", file=sys.stderr)
    out.mkdir(parents=True, exist_ok=True)
    (out / "report.json").write_text(json.dumps(rep, ensure_ascii=False, indent=1, default=str), encoding="utf-8")
    (out / "report.md").write_text(markdown(rep), encoding="utf-8")
    with open(out / "route_outputs.jsonl", "w", encoding="utf-8") as fh:  # annotation queue for experts
        for r in rts["records"]:
            fh.write(json.dumps(r, ensure_ascii=False, default=str) + "\n")
    _out({"out": out.as_posix(), "verdicts": {k: v["verdict"] for k, v in rep["verdicts"].items()},
          "seconds": rep["seconds"]})
    return 0


def cmd_agreement(args: argparse.Namespace) -> int:
    """Each file: JSONL of {item_id, label}; one file per annotator."""
    raters = [{json.loads(l)["item_id"]: json.loads(l)["label"] for l in Path(f).read_text(encoding="utf-8").splitlines()
               if l.strip()} for f in args.files]
    units = {u: {r: lab.get(u) for r, lab in enumerate(raters)} for u in set().union(*raters)}
    out = {"raters": len(raters), "units": len(units),
           "alpha": krippendorff_alpha(units, "interval" if args.interval else "nominal")}
    if len(raters) == 2:
        common = sorted(set(raters[0]) & set(raters[1]))
        out["cohen_kappa"] = cohen_kappa([raters[0][u] for u in common], [raters[1][u] for u in common])
        out["common_units"] = len(common)
    _out(out)
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="python -m bench", description="AtlasBench (bench/PROTOCOL.md)")
    sub = ap.add_subparsers(dest="cmd", required=True)

    def with_backends(p: argparse.ArgumentParser) -> None:
        p.add_argument("--db", default=str(DEFAULT_DB))
        p.add_argument("--bin", default=None, help="atlas binary (default: ATLAS_BIN or newest release build)")
        p.add_argument("--http", default=None, help="base URL of `atlas serve`, e.g. http://127.0.0.1:8765")
        p.add_argument("--snapshot", default=str(ROOT / "data" / "snapshot"))
        p.add_argument("--index", default=str(ROOT / "data" / "index"))
        p.add_argument("--composition", default=str(ROOT / "shapes" / "composition.json"))
        p.add_argument("--skip-cli", action="store_true", help="do not evaluate the per-call CLI backend")
        p.add_argument("--layout", default=str(ROOT / "data" / "layout"))
        p.add_argument("--compile", action="store_true",
                       help="compile a fresh snapshot of --db with the binary (temp dir outside the repo)")

    p = sub.add_parser("probe", help="which components exist")
    with_backends(p)
    p = sub.add_parser("build-silver", help="derive silver items from the registry")
    p.add_argument("--db", default=str(DEFAULT_DB))
    p.add_argument("--out", default=str(SILVER))
    p.add_argument("--seed", type=int, default=20261016)
    p = sub.add_parser("split", help="grouped/time-aware splits + manifest")
    p.add_argument("inputs", nargs="*")
    p.add_argument("--version", default=VERSION)
    p.add_argument("--cutoff", default="2026-10-16", help="items after this date never enter dev")
    p = sub.add_parser("verify", help="re-hash split files against the manifest")
    p.add_argument("--version", default=VERSION)
    p = sub.add_parser("leakage", help="lexical leakage of retrieval qrels")
    p.add_argument("inputs", nargs="*")
    p.add_argument("--db", default=str(DEFAULT_DB))
    p = sub.add_parser("run", help="evaluate a split against the available backends")
    with_backends(p)
    p.add_argument("--version", default=VERSION)
    p.add_argument("--split", default="test", choices=splits.SPLITS)
    p.add_argument("--items", nargs="*", help="evaluate these files/dirs instead of a split")
    p.add_argument("--open-sealed", default=None, metavar="REASON")
    p.add_argument("--er-queue", default=None, help="resolver review-queue JSONL (identity abstentions)")
    p.add_argument("--max-hops", type=int, default=6)
    p.add_argument("--load-n", type=int, default=500)
    p.add_argument("--out", default=None)
    p.add_argument("--serve", action="store_true", help="start `atlas serve` on --db/--snapshot for this run")
    p = sub.add_parser("agreement", help="Cohen's kappa / Krippendorff's alpha between annotators")
    p.add_argument("files", nargs="+")
    p.add_argument("--interval", action="store_true", help="interval metric (relevance grades 0-3)")
    args = ap.parse_args(argv)
    return {"probe": cmd_probe, "build-silver": cmd_build_silver, "split": cmd_split, "verify": cmd_verify,
            "leakage": cmd_leakage, "run": cmd_run, "agreement": cmd_agreement}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
