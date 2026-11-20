"""Compile field and SQL swaps and require the relevant operational assertion to fail."""

from pathlib import Path
import shutil, subprocess, tempfile

ROOT = Path(__file__).resolve().parents[3]
CASES = [
    (
        "omoide-store",
        "near_counts",
        "r.state='current'",
        "r.state='pending'",
        "--lib",
        None,
        "draft_and_persisted",
    ),
    (
        "omoide-store",
        "near_counts",
        "r.kind='rule'",
        "r.kind='skill'",
        "--lib",
        None,
        "draft_and_persisted",
    ),
    (
        "omoide-store",
        "near_counts",
        "SELECT 'words',j.value",
        "SELECT 'singles',j.value",
        "--lib",
        None,
        "draft_and_persisted",
    ),
    (
        "omoide-store",
        "near_counts",
        "SELECT 'total','',count(*)",
        "SELECT 'population','',count(*)",
        "--lib",
        None,
        "draft_and_persisted",
    ),
    (
        "omoide-store",
        "near_counts",
        "SELECT i.words,i.singles",
        "SELECT i.singles,i.words",
        "--lib",
        None,
        "draft_and_persisted",
    ),
    (
        "omoide",
        "relations",
        '"words" =>',
        '"terms" =>',
        "--test",
        "relations",
        "current_corpus_changes",
    ),
    (
        "omoide",
        "relations",
        '"total" =>',
        '"population" =>',
        "--test",
        "relations",
        "current_corpus_changes",
    ),
    (
        "omoide",
        "relations",
        '(&r["version_id"], strength)',
        '(&r["version_id"], 0)',
        "--test",
        "relations",
        "current_corpus_changes",
    ),
    (
        "omoide-core",
        "near",
        "self.weight(word, false)",
        "self.weight(word, true)",
        "--lib",
        None,
        "shared_boilerplate",
    ),
    (
        "omoide-core",
        "near",
        "self.weight(w, !w.is_ascii())",
        "self.weight(w, w.is_ascii())",
        "--lib",
        None,
        "sibling_uses",
    ),
]


def main():
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        for f in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
            shutil.copyfile(ROOT / f, root / f)
        shutil.copytree(ROOT / "crates", root / "crates")
        for crate, source, old, new, flag, target, test in CASES:
            path = root / f"crates/{crate}/src/{source}.rs"
            s = path.read_text()
            assert old in s, old
            path.write_text(s.replace(old, new))
            p = subprocess.run(
                [
                    "cargo",
                    "test",
                    "--locked",
                    "-p",
                    crate,
                    flag,
                    *([target] if target else []),
                    test,
                ],
                cwd=root,
                capture_output=True,
            )
            assert p.returncode and b"test result: FAILED" in p.stdout, (
                old,
                p.stderr[-300:],
            )
            path.write_text(s)
            print(f"{source} | {old} | {new} | {test} | caught", flush=True)


if __name__ == "__main__":
    main()
