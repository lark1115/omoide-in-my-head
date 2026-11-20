"""Check the frozen fixture and, optionally, both directions through the real CLI."""

import context_check
import argparse
import csv
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent


def data():
    expected = json.loads((HERE / "manifest.json").read_text())
    for name, digest in expected.items():
        if hashlib.sha256((HERE / name).read_bytes()).hexdigest() != digest:
            raise ValueError("fixture hash mismatch: " + name)

    def read(name):
        with (HERE / name).open(encoding="utf-8") as f:
            return list(csv.DictReader(f, delimiter="\t"))

    base = read("unrelated.tsv")
    positive = read("related.tsv")
    assert len(base) == len({(r["scene"], r["text"]) for r in base}) == 30
    assert len(positive) == len({int(r["base"]) for r in positive}) == 10
    assert all(1 <= int(r["base"]) <= len(base) for r in positive)
    assert set(r["relation"] for r in positive) == {
        "restated",
        "exception",
        "contradiction",
    }
    return base, positive


def check(binary):
    base, positives = data()

    def call(db, args, success=True):
        proc = subprocess.run([binary, "--db", db, *args], capture_output=True)
        assert (proc.returncode == 0) == success
        return json.loads(proc.stdout)

    def draft(db, r, action):
        source = call(db, ["say", "--text", r["text"]])["id"]
        return [
            "rule",
            "add",
            "--source",
            source,
            "--action",
            action,
            "--scene",
            r["scene"],
            "--text",
            r["text"],
            "--target",
            "共同作業",
        ]

    with tempfile.TemporaryDirectory() as td:
        for reverse in (False, True):
            db = Path(td) / f"unrelated-{reverse}.db"
            call(db, ["init"])
            ids = {}
            for i in reversed(range(len(base))) if reverse else range(len(base)):
                result = call(db, draft(db, base[i], "report"))
                assert result["state"] == "activated", (i + 1, result["state"])
                ids[i + 1] = result["version_id"]
            for r in positives:
                args = draft(db, r, "review")
                result = call(db, args)
                assert result["state"] == "near_rules" and ids[int(r["base"])] in {
                    n["record"]["version_id"] for n in result["near_rules"]
                }, r
                call(db, args + ["--near-digest", result["near_digest"]], False)
        for r in positives:
            db = Path(td) / f"reverse-{r['base']}.db"
            call(db, ["init"])
            first = call(db, draft(db, r, "review"))
            assert first["state"] == "activated"
            back = call(db, draft(db, base[int(r["base"]) - 1], "report"))
            assert first["version_id"] in {
                n["record"]["version_id"] for n in back["near_rules"]
            }
    print(
        "435 unrelated pairs in both directions; 10 required comparisons with distinct actions; relation-less finish refused"
    )


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--binary", type=Path)
    p.add_argument(
        "--context-result",
        type=Path,
        help="validate the saved independent confirmation instead of repeating it",
    )
    a = p.parse_args()
    context_check.data()
    if a.binary:
        check(a.binary.resolve())
        result = (
            json.loads(a.context_result.read_text())
            if a.context_result
            else context_check.run(a.binary.resolve())
        )
        assert result["required"] == 5 and result["positive_extras"] == 0
        assert (
            max(
                result["manual"]["maximum"],
                result["common"]["maximum"],
                result["sequential_max"],
            )
            <= 2
        )
        print("追加ファイルの手引き・頻出語の誤提示と必須5組を検査済み")
    else:
        data()
        print("30 unrelated rules and 10 related pairs verified")
