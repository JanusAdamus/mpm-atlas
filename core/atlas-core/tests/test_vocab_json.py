"""shapes/vocab.json must match atlas.model (Python side of the Rust/Python enum check)."""

import subprocess
import sys
from pathlib import Path

GEN = Path(__file__).resolve().parents[1] / "vocab_gen.py"


def test_vocab_json_is_current() -> None:
    result = subprocess.run([sys.executable, str(GEN), "--check"], capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
