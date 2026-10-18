"""`python -m atlas`: error mapping (always) and the fixture end to end through the real binary (when built)."""

import json
import sys
from pathlib import Path

import pytest

from atlas import Store, cli, load_bundle

FIXTURE = Path(__file__).parent / "fixtures" / "mini_atlas.json"


def test_missing_binary_is_a_clear_error(monkeypatch, capsys):
    monkeypatch.setenv("ATLAS_BIN", str(Path(__file__).parent / "no-such-atlas.exe"))
    assert cli.main(["stats", "--db", "nowhere.db"]) == 1
    assert "no atlas binary" in capsys.readouterr().err


def test_error_envelope_becomes_cli_error(monkeypatch):
    monkeypatch.setenv("ATLAS_BIN", sys.executable)  # stands in for the binary: python -c <code> --json
    code = "import sys; print('{\"error\": {\"code\": \"not_found\", \"message\": \"no such entity\"}}'); sys.exit(1)"
    with pytest.raises(cli.CliError, match="not_found: no such entity"):
        cli.run_atlas(["-c", code])


def test_target_dir_honours_env(monkeypatch, tmp_path):
    monkeypatch.setenv("CARGO_TARGET_DIR", str(tmp_path))
    monkeypatch.delenv("ATLAS_BIN", raising=False)
    assert cli.target_dir() == tmp_path
    with pytest.raises(cli.CliError):
        cli.find_binary()


def _binary_or_skip() -> None:
    try:
        cli.find_binary()
    except cli.CliError:
        pytest.skip("atlas binary not built")


def test_fixture_compile_search_path_end_to_end(tmp_path, capsys):
    _binary_or_skip()
    db = tmp_path / "mini.db"
    with Store(db) as st:
        load_bundle(st, json.loads(FIXTURE.read_text(encoding="utf-8")))
    snap = str(tmp_path / "snap")
    built = cli.run_atlas(["compile", "--db", str(db), "--out", snap])
    assert built["nodes"] == 17

    # no search index for this snapshot: the binary falls back to the registry's FTS5 lane
    hits = cli.run_atlas(["search", "pitagoras", "--snapshot", snap, "--index", str(tmp_path / "none"), "--db", str(db)])
    assert hits["hits"][0]["id"] == "s:pythagoras"

    # converse depends on Pythagoras: a typed, justified route must exist and cite the claim
    route = cli.run_atlas(["path", "s:pythagoras-converse", "s:pythagoras", "--snapshot", snap])
    assert not route["abstained"] and route["paths"], route
    step = route["paths"][0]["steps"][0]
    assert step["claim_id"] == "c:converse-uses" and step["sources"][0]["source_id"] == "src:fixture-hand"
