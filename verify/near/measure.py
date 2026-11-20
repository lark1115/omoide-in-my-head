"""Exercise actual registration with each rule backed by a snapshot of the other 71."""

import argparse
import itertools
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "tests/discrimination"


def read_lines(name):
    return [json.loads(line) for line in (DATA / name).read_text().splitlines()]


def labels(rows):
    links = set()
    for query in read_lines("queries.jsonl"):
        links.update(
            itertools.combinations(sorted(set(query["must"] + query["must_not"])), 2)
        )
    result = {}
    for a, b in itertools.combinations(rows, 2):
        pair = tuple(sorted((a["id"], b["id"])))
        shared = bool(
            a["target"] and a["target"] == b["target"] and a["action"] == b["action"]
        )
        if shared or (a["domain"] == b["domain"] and pair in links):
            result[pair] = "positive"
        elif a["domain"] != b["domain"] and pair not in links:
            result[pair] = "negative"
    return result


def measure(binary):
    rows = read_lines("rules.jsonl")
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        base = root / "base.db"
        index = {}
        sources = {}

        def cli(db, args):
            p = subprocess.run(
                [binary, "--db", db, *args], capture_output=True, check=True
            )
            return json.loads(p.stdout)

        def command(r):
            return [
                "rule",
                "add",
                "--scene",
                r["scene"],
                "--text",
                r["text"],
                "--action",
                r["action"],
                "--source",
                sources[r["id"]],
                *(["--target", r["target"]] if r["target"] else []),
            ]

        cli(base, ["init"])
        for r in rows:
            sources[r["id"]] = cli(base, ["say", "--text", r["text"]])["id"]
            args = command(r)
            v = cli(base, args)
            if v["state"] == "near_rules":
                v = cli(
                    base,
                    args
                    + [
                        "--relation",
                        "unrelated",
                        "--near-digest",
                        v["near_digest"],
                        "--against",
                        v["near_rules"][0]["record"]["version_id"],
                    ],
                )
            index[v["version_id"]] = r["id"]
        by_id = {k: v for v, k in index.items()}
        records = {}
        for r in rows:
            db = root / (r["id"] + ".db")
            with sqlite3.connect(base) as src, sqlite3.connect(db) as dst:
                src.backup(dst)
                dst.execute(
                    "UPDATE rules SET state='superseded' WHERE version=?",
                    (by_id[r["id"]],),
                )
                assert (
                    dst.execute(
                        "SELECT count(*) FROM rules WHERE state='current'"
                    ).fetchone()[0]
                    == 71
                )
            v = cli(db, command(r))
            found = [index[n["record"]["version_id"]] for n in v.get("near_rules", [])]
            assert r["id"] not in found
            records[r["id"]] = {
                "returned": found,
                "omitted": v.get("near_remaining", 0),
            }
        return records


def summary(records):
    rows = read_lines("rules.jsonl")
    key = labels(rows)
    result = {}
    for split in ("dev", "test", "all"):
        counts = {"positive": [0, 0], "negative": [0, 0]}
        for (a, b), kind in key.items():
            if split != "all" and (int(a[1:]) % 2 == 1) != (split == "dev"):
                continue
            counts[kind][0] += int(b in records[a]["returned"]) + int(
                a in records[b]["returned"]
            )
            counts[kind][1] += 2
        n = [
            len(v["returned"])
            for k, v in records.items()
            if split == "all" or (int(k[1:]) % 2 == 1) == (split == "dev")
        ]
        result[split] = {**counts, "mean": sum(n) / len(n), "maximum": max(n)}
    return result


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("binary", type=Path)
    p.add_argument("output", type=Path)
    a = p.parse_args()
    found = measure(a.binary.resolve())
    a.output.write_text(json.dumps(found, indent=2) + "\n")
    print(json.dumps(summary(found), indent=2))
