"""Require compiled, observably broken registration implementations to fail a targeted test."""

from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
CASES = [
    (
        "aliases",
        'record["aliases"].clone()',
        'record["actions"].clone()',
        "lexical_fields",
    ),
    (
        "actions",
        'record["actions"].clone()',
        'record["aliases"].clone()',
        "lexical_fields",
    ),
    (
        "target",
        'record["target"].as_str(),',
        'record["purpose"].as_str(),',
        "lexical_fields",
    ),
    (
        "scene",
        'record["scene"].as_str().unwrap(),',
        'record["text"].as_str().unwrap(),',
        "lexical_fields",
    ),
    (
        "body",
        'record["text"].as_str().unwrap(),',
        'record["scene"].as_str().unwrap(),',
        "lexical_fields",
    ),
    ("score field", '"near_score":score', '"points":score', "lexical_fields"),
    (
        "omitted field",
        '"near_remaining":omitted',
        '"remaining":omitted',
        "hidden_neighbor",
    ),
    (
        "hidden digest",
        "matches.iter().map(|(_, r, _)|",
        "matches.iter().take(9).map(|(_, r, _)|",
        "hidden_neighbor",
    ),
    ("score order", "b.0.cmp(&a.0)", "a.0.cmp(&b.0)", "hidden_neighbor"),
    ("cap", "take(MAX_NEIGHBORS)", "take(MAX_NEIGHBORS+1)", "hidden_neighbor"),
    (
        "version tie break",
        'a.1["version_id"].as_str().cmp(&b.1["version_id"].as_str())',
        'b.1["version_id"].as_str().cmp(&a.1["version_id"].as_str())',
        "hidden_neighbor",
    ),
]


def main():
    with tempfile.TemporaryDirectory() as work:
        d = Path(work)
        for f in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
            shutil.copyfile(ROOT / f, d / f)
        shutil.copytree(ROOT / "crates", d / "crates")
        p = d / "crates/omoide/src/relations.rs"
        original = p.read_text()
        for name, old, new, test in CASES:
            assert original.count(old) == 1, (name, original.count(old))
            p.write_text(original.replace(old, new))
            result = subprocess.run(
                [
                    "cargo",
                    "test",
                    "--locked",
                    "-p",
                    "omoide",
                    "--test",
                    "relations",
                    test,
                ],
                cwd=d,
                capture_output=True,
            )
            assert result.returncode and b"test result: FAILED" in result.stdout, name
            print(name + " | " + test + " | caught", flush=True)


if __name__ == "__main__":
    main()
