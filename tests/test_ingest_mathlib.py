"""Mathlib adapter and Lean source parser, offline against tests/fixtures/ingest/mathlib/."""

from __future__ import annotations

import io
import tarfile
from pathlib import Path

import pytest

from atlas import EntityType, EvidenceState, RelationType, Role, Store
from atlas.ingest import mathlib
from atlas.ingest.lean import name_tokens, parse_file, strip_comments

FIXTURE = Path(__file__).parent / "fixtures" / "ingest" / "mathlib"
COMMIT = "0123456789abcdef0123456789abcdef01234567"


def build_tarball(cache_root: Path, commit: str = COMMIT) -> Path:
    """Pack the fixture tree like a GitHub codeload tarball (pax comment = commit)."""
    folder = cache_root / "mathlib"
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / f"mathlib4-{commit}.tar.gz"
    with tarfile.open(path, "w:gz", format=tarfile.PAX_FORMAT, pax_headers={"comment": commit}) as tf:
        for f in sorted(FIXTURE.rglob("*")):
            if f.is_file():
                data = f.read_bytes()
                info = tarfile.TarInfo("mathlib4-master/" + f.relative_to(FIXTURE).as_posix())
                info.size = len(data)
                tf.addfile(info, io.BytesIO(data))
    return path


def parse_fixture(rel: str):
    return parse_file(rel, (FIXTURE / rel).read_text(encoding="utf-8"))


# ---------------------------------------------------------------------- parser


def test_comment_stripping_keeps_offsets_and_collects_docs() -> None:
    text = 'a /- x /- nested -/ y -/ b -- tail\n/-- doc -/ c "s -- t" d'
    code, docs = strip_comments(text)
    assert len(code) == len(text) and code.count("\n") == 1
    assert "nested" not in code and "tail" not in code and "s -- t" not in code
    assert code.split() == ["a", "b", "c", '"', '"', "d"]
    assert [(k, b) for _, _, k, b in docs] == [("/--", "doc")]


def test_parser_reads_names_namespaces_docs_and_attributes() -> None:
    r = parse_fixture("Mathlib/Data/Nat/Prime/Defs.lean")
    assert r.error is None
    assert r.module == "Mathlib.Data.Nat.Prime.Defs"
    assert r.imports == ("Mathlib.Tactic.Basic", "Mathlib.Order.Basic", "Batteries.Data.Nat")
    assert r.title == "Prime numbers (fixture)"
    by = {d.name: d for d in r.decls}
    assert set(by) == {"Nat.Prime", "Nat.Prime.two_le", "Nat.Prime.one_lt", "Nat.Prime.fact_one_lt", "Nat.Primes",
                       "Nat.prime_two_le", "prime_two", "exists_infinite_primes", "message"}
    prime = by["Nat.Prime"]
    assert (prime.kind, prime.line, prime.wikidata) == ("def", 24, ("Q49008",))
    assert prime.signature == "(p : ℕ)" and prime.doc.startswith("`Nat.Prime p` means")
    assert by["Nat.Prime.two_le"].signature == ": ∀ {p : ℕ}, Prime p → 2 ≤ p"  # stops at the `|` equations
    assert by["Nat.Primes"].signature == "(α : Type u)"  # universes stripped, stops at `where`
    assert by["Nat.Prime.fact_one_lt"].signature.startswith("(p : ℕ) [Fact p.Prime]")  # priority dropped
    alias = by["Nat.prime_two_le"]
    assert alias.alias_candidates == ("Nat.Prime.two_le", "Prime.two_le") and alias.deprecated
    assert r.skipped == {"private": 1, "anonymous_instance": 1, "alias_iff_pair": 1}
    assert "Nat" in r.opens and by["exists_infinite_primes"].namespace == ()


def test_parse_failure_is_recorded_not_raised(monkeypatch: pytest.MonkeyPatch) -> None:
    import atlas.ingest.lean as lean

    def boom(path: str, text: str):
        raise RuntimeError("bad file")

    monkeypatch.setattr(lean, "_parse", boom)
    r = lean.parse_file("Mathlib/X.lean", "")
    assert r.error == "RuntimeError: bad file" and r.decls == ()


def test_name_tokens_split_camel_and_snake() -> None:
    assert name_tokens("Nat.Prime.two_le") == ["nat", "prime", "two", "le"]
    assert name_tokens("CauchySeq.isBounded") == ["cauchy", "seq", "is", "bounded"]
    assert name_tokens("HasFDerivAt") == ["has", "f", "deriv", "at"]


def test_links_yaml_reader() -> None:
    entries = mathlib.parse_links_yaml((FIXTURE / "docs" / "1000.yaml").read_text(encoding="utf-8"))
    by = {e["key"]: e for e in entries}
    assert by["Q11518"]["decl"].startswith("EuclideanGeometry.")
    assert by["Q154210"]["decls"] == ["Nat.Prime.two_le", "Nat.Prime.one_lt"]
    assert by["Q132469"]["statement"] == "FermatLastTheorem"
    assert "Q180345X" in by


# ---------------------------------------------------------------------- adapter


@pytest.fixture()
def ingested(tmp_path: Path):
    build_tarball(tmp_path)
    store = Store()
    rep = mathlib.ingest(store, cache_root=tmp_path, offline=True, workers=1)
    yield store, rep, tmp_path
    store.close()


def test_mathlib_entities_and_types(ingested) -> None:
    store, rep, _ = ingested
    assert rep.counters["files_parsed"] == 3 and rep.counters.get("files_failed", 0) == 0
    prime = store.get_entity("mathlib:Nat.Prime")
    assert prime.type is EntityType.DEFINITION and prime.label("es") == "Nat.Prime"
    assert ("mathlib", "Nat.Prime") in prime.external_ids
    assert store.get_entity("mathlib:Nat.Prime.two_le").type is EntityType.STATEMENT
    assert store.get_entity("mathlib:Mathlib.Tactic.elabHelper").type is EntityType.RESOURCE  # infrastructure
    mod = store.get_entity("mathlib-mod:Mathlib.Data.Nat.Prime.Defs")
    assert mod.type is EntityType.THEORY and mod.descriptions[0].text == "Prime numbers (fixture)"
    assert store.get_entity("lean-mod:Batteries.Data.Nat").type is EntityType.THEORY
    assert store.get_entity("mathlib-dir:Mathlib/Data/Nat/Prime").type is EntityType.AREA
    assert store.get_source("src:mathlib4").version == COMMIT
    assert store.coverage()["unknown_by_type"] == {}  # every entity has at least one claim


def test_declaration_claims_are_formally_linked_to_the_commit(ingested) -> None:
    store, _, _ = ingested
    (c,) = store.claims_for("mathlib:Nat.Prime", roles=[Role.SUBJECT], relations=[RelationType.PART_OF_AREA])
    assert c.evidence_state is EvidenceState.FORMALLY_LINKED
    assert (c.formal.library, c.formal.version, c.formal.declaration) == ("mathlib4", COMMIT, "Nat.Prime")
    assert c.targets == ("mathlib-dir:Mathlib/Data/Nat/Prime",)
    assert c.ids(Role.CONTEXT) == ("mathlib-mod:Mathlib.Data.Nat.Prime.Defs",)
    p = c.provenance[0]
    assert (p.locator, p.license) == ("Mathlib/Data/Nat/Prime/Defs.lean:24", "Apache-2.0")


def test_imports_are_marked_infrastructure_or_content(ingested) -> None:
    store, rep, _ = ingested
    deps = store.claims_for("mathlib-mod:Mathlib.Data.Nat.Prime.Defs", roles=[Role.SUBJECT],
                            relations=[RelationType.DEPENDS_ON_LOGICALLY])
    layer = {c.targets[0]: c.ids(Role.CONTEXT)[0] for c in deps}
    assert layer == {"mathlib-mod:Mathlib.Tactic.Basic": "mathlib-layer:infrastructure",
                     "mathlib-mod:Mathlib.Order.Basic": "mathlib-layer:content",
                     "lean-mod:Batteries.Data.Nat": "mathlib-layer:infrastructure"}
    assert all(c.evidence_state is EvidenceState.EXTRACTED for c in deps)


def test_signature_references_become_one_conjunctive_claim(ingested) -> None:
    store, _, _ = ingested
    (c,) = store.claims_for("mathlib:EuclideanGeometry.rightAngle_of_prime", roles=[Role.SUBJECT],
                            relations=[RelationType.DEPENDS_ON_LOGICALLY])
    assert c.is_conjunction and set(c.targets) == {"mathlib:Nat.Prime", "mathlib:EuclideanGeometry.RightAngle"}
    assert "without elaboration" in c.assumptions[0]


def test_alias_carries_its_target_and_curated_links_are_saved(ingested) -> None:
    store, rep, root = ingested
    alias = store.get_entity("mathlib:Nat.prime_two_le")
    assert ("mathlib_alias_of", "Nat.Prime.two_le") in alias.external_ids
    assert alias.type is EntityType.STATEMENT
    links = mathlib.curated_links(root)
    pairs = {(l["qid"], l["decl"], l["origin"]) for l in links}
    assert ("Q49008", "Nat.Prime", "attribute") in pairs
    assert ("Q11518", "EuclideanGeometry.dist_sq_eq_dist_sq_add_dist_sq_iff_angle_eq_pi_div_two",
            "docs/1000.yaml") in pairs
    assert ("Q132469", "FermatLastTheorem", "docs/1000.yaml") in pairs
    assert rep.counters["yaml_bad_key"] == 1


def test_rerun_is_idempotent(ingested) -> None:
    store, rep, root = ingested
    before = store.coverage()
    rep2 = mathlib.ingest(store, cache_root=root, offline=True, workers=1)
    assert store.coverage() == before
    assert rep2.counters.get("claims_written", 0) == 0 and rep2.counters["claims_already_present"] > 0


def test_parallel_parse_matches_serial(tmp_path: Path) -> None:
    files = [(p.relative_to(FIXTURE).as_posix(), p.read_text(encoding="utf-8"))
             for p in FIXTURE.rglob("*.lean")] * 30  # >64 files to take the pool path
    serial = sorted(mathlib.parse_all(files, 1), key=lambda r: r.path)
    parallel = sorted(mathlib.parse_all(files, 2), key=lambda r: r.path)
    assert serial == parallel


def test_offline_without_tarball_fails_clearly(tmp_path: Path) -> None:
    with pytest.raises(FileNotFoundError):
        mathlib.ingest(Store(), cache_root=tmp_path, offline=True)
