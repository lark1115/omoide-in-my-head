"""Compile deliberate field/literal swaps against CLI and adapter tests in a scratch checkout."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
# Each target names a test that checks the value or change in observable behavior.
CASES = [
    (
        "omoide",
        "coverage",
        'r["kind"] == kind',
        'r["state"] == kind',
        "--bin",
        "omoide",
        "compare_tallies",
    ),
    (
        "omoide",
        "coverage",
        'r["version_id"]',
        'r["id"]',
        "--bin",
        "omoide",
        "compare_tallies",
    ),
    (
        "omoide",
        "coverage",
        "'$.period'",
        "'$.claim'",
        "--test",
        "evidence_activity",
        "eligible_counts_use_structured_scope",
    ),
    (
        "omoide",
        "coverage",
        "'$.scope'",
        "'$.period'",
        "--test",
        "evidence_activity",
        "eligible_counts_use_structured_scope",
    ),
    (
        "omoide",
        "coverage",
        "'$.claim'",
        "'$.period'",
        "--test",
        "evidence_activity",
        "eligible_counts_use_structured_scope",
    ),
    (
        "omoide",
        "search",
        'let evidence = record["evidence"]',
        'let evidence = record["source_hint"]',
        "--test",
        "evidence_activity",
        "eligible_counts_use_structured_scope",
    ),
    (
        "omoide",
        "search",
        'record["period"].as_str().is_some_and',
        'record["purpose"].as_str().is_some_and',
        "--test",
        "evidence_activity",
        "temporal_scope_affects",
    ),
    (
        "omoide",
        "search",
        'json!("incomplete_search")',
        'json!("none")',
        "--test",
        "roundtrip",
        "coverage_distinguishes",
    ),
    (
        "omoide",
        "search",
        'Some("judge timeout")',
        'Some("judge output limit")',
        "--test",
        "evidence_activity",
        "fallback_keeps_only",
    ),
    (
        "omoide",
        "search",
        "ordered(&scores.confirmed)",
        "ordered(&scores.candidates)",
        "--test",
        "evidence_activity",
        "semantic_order_survives",
    ),
    (
        "omoide",
        "judging",
        '"kind":r["kind"]',
        '"kind":r["scene"]',
        "--bin",
        "omoide",
        "bounded_batches_keep",
    ),
    (
        "omoide-judge-jev",
        "protocol",
        'Some("decision")',
        'Some("rule")',
        "--bin",
        "omoide-judge-jev",
        "choose_question_by_kind",
    ),
    (
        "omoide-judge-jev",
        "protocol",
        'Some("experience")',
        'Some("skill")',
        "--bin",
        "omoide-judge-jev",
        "choose_question_by_kind",
    ),
    (
        "omoide-judge-jev",
        "protocol",
        'Some("fact")',
        'Some("skill")',
        "--bin",
        "omoide-judge-jev",
        "choose_question_by_kind",
    ),
]


def main():
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
            shutil.copyfile(ROOT / name, root / name)
        shutil.copytree(ROOT / "crates", root / "crates")
        for crate, file, old, new, flag, target, test in CASES:
            path = root / f"crates/{crate}/src/{file}.rs"
            source = path.read_text()
            # Rustfmt may wrap chained methods over multiple lines.
            import re

            pattern = (
                re.escape(old)
                .replace(r"\.as_str\(\)", r"\s*\.as_str\(\)")
                .replace(r"\.is_some_and", r"\s*\.is_some_and")
            )
            changed, count = re.subn(pattern, lambda _: new, source)
            if count != 1:
                raise ValueError(f"{crate}/{file}: wrong count for {old}: {count}")
            path.write_text(changed)
            cmd = ["cargo", "test", "--locked", "-p", crate, flag, target, test]
            run = subprocess.run(
                cmd,
                cwd=root,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                env=dict(os.environ),
            )
            path.write_text(source)
            if not run.returncode or b"test result: FAILED" not in run.stdout:
                raise ValueError(f"expected failing test for {old}")
            print(f"{crate}/{file} | {old} | {new} | {test} | caught", flush=True)


if __name__ == "__main__":
    main()
