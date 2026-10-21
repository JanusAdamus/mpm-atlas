"""Ronda 5: compuerta de solo lectura sobre el registro y la copia del piloto.

    python docs/plans/r5/consultas/r5_gate.py

Imprime una línea por comprobación y sale con 1 si alguna falla.  No escribe nada.
"""
from __future__ import annotations

import sqlite3
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
DB = ROOT / "data" / "atlas.db"
PILOT = ROOT / "data" / "pilot" / "atlas-pilot.db"


def ro(path: Path) -> sqlite3.Connection:
    return sqlite3.connect(f"file:{path.as_posix()}?mode=ro", uri=True)


def one(c: sqlite3.Connection, sql: str) -> int:
    return c.execute(sql).fetchone()[0]


def main() -> int:
    c = ro(DB)
    checks = [
        ("schema v4", one(c, "PRAGMA user_version") == 4),
        ("part_of_area formally_linked = 0",
         one(c, "SELECT count(*) FROM claims WHERE relation='part_of_area' AND evidence_state='formally_linked'") == 0),
    ]
    by_rule = c.execute("SELECT kind, rule, count(*) FROM curation WHERE undone_at IS NULL GROUP BY 1, 2").fetchall()
    print("curadurías activas:", by_rule or "ninguna")
    checks.append(("hay curaduría activa", bool(by_rule)))
    print("disputed:", one(c, "SELECT count(*) FROM claims WHERE evidence_state='disputed'"))
    if PILOT.exists():
        p = ro(PILOT)
        batches = one(p, "SELECT count(*) FROM ingest_batches")
        orphans = one(p, "SELECT count(*) FROM claims WHERE ingest_batch IS NOT NULL "
                         "AND ingest_batch NOT IN (SELECT id FROM ingest_batches)")
        print("lotes del piloto:", batches)
        checks.append(("piloto sin filas de lotes inexistentes", orphans == 0))
    ok = True
    for name, passed in checks:
        print(("PASA " if passed else "FALLA"), name)
        ok &= passed
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
