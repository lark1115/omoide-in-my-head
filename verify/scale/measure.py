"""Paired first-page record delivery on fixed nested snapshots; no agent or trial grades."""

import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT / "verify/trials"), str(ROOT / "tests/trials-large")]
from generate import build
from seed import populate


def load(p):
    return json.loads(p.read_text())


def save(p, data):
    p.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")


def prepare(root, binary):
    root.mkdir(exist_ok=True, parents=True)
    archive = build()
    originals = [r for _, kind, r in archive if kind == "original"]
    mapping = {r["id"]: k for k, kind, r in archive if kind == "original"}
    sorted_extras = sorted(
        (r for _, kind, r in archive if kind != "original"),
        key=lambda r: hashlib.sha256(("delivery-v1:" + r["id"]).encode()).digest(),
    )
    byid = {r["id"]: r for _, _, r in archive}
    for size in (48, 500, 3048):
        if (root / f"{size}.json").exists():
            continue
        ids = {r["id"] for r in originals}
        for r in sorted_extras:
            chain = set()
            while r["id"] not in ids:
                chain.add(r["id"])
                if not r.get("successor"):
                    break
                r = byid[r["successor"]]
            if len(ids | chain) <= size:
                ids.update(chain)
            if len(ids) == size:
                break
        rows = [r for _, _, r in archive if r["id"] in ids]
        with tempfile.TemporaryDirectory(dir=root) as tmp:
            db = Path(tmp) / "store.db"
            index = populate(binary, rows, db, expected_count=size)
            shutil.move(db, root / f"{size}.db")
            save(
                root / f"{size}.json", {k: mapping.get(v, v) for k, v in index.items()}
            )


def measure(a):
    if a.split == "test":
        if not a.freeze:
            raise ValueError("TEST requires a frozen implementation commit")
        subprocess.run(
            [
                "git",
                "-C",
                ROOT,
                "diff",
                "--exit-code",
                a.freeze,
                "--",
                ":(glob)crates/*/src/**",
                "Cargo.lock",
            ],
            check=True,
            stdout=subprocess.DEVNULL,
        )
    root = a.snapshots
    data = ROOT / "tests/trials"
    keys = {r["id"]: r["memory"] for r in load(data / "grading/key.json")}
    kinds = {r["id"]: r["kind"] for r in load(data / "memory.json")}
    inputs = load(Path(__file__).with_name("lookups.json"))
    with a.output.open("x") as out:
        for size in (48, 500, 3048):
            ids = load(root / f"{size}.json")
            for op in load(data / "opportunities.json"):
                oid = op["id"]
                if int(oid[1:]) % 2 != (a.split == "dev"):
                    continue
                for n, q in enumerate([{"scene": op["prompt"]}, *inputs.get(oid, [])]):
                    cmd = [a.binary, "--db", root / f"{size}.db", "recall"]
                    for k, v in q.items():
                        cmd += ["--" + k, v]
                    if a.judge:
                        cmd += [
                            "--judge",
                            a.judge,
                            "--threshold",
                            "0.6",
                            "--timeout-ms",
                            "6000",
                        ]
                    reply = json.loads(subprocess.check_output(cmd))

                    def get(field):
                        return {ids[r["version_id"]] for r in reply[field]}

                    confirmed = get("results")
                    short = get("candidates")
                    supporting = [
                        dict(
                            id=k,
                            kind=kinds[k],
                            tier=1 if k in confirmed else 2 if k in short else 0,
                        )
                        for k in keys[oid]
                    ]
                    print(
                        json.dumps(
                            dict(
                                size=size,
                                op=oid,
                                input=n,
                                support=supporting,
                                state=reply["state"],
                                judge=reply["judge_state"],
                                pool=reply["candidate_count"],
                            )
                        ),
                        file=out,
                        flush=True,
                    )


def summary(path):
    groups = defaultdict(list)
    for row in map(json.loads, path.read_text().splitlines()):
        group = (row["size"],)
        for k in row["support"]:
            groups[group + (k["kind"],)].append(k["tier"])
        if row["support"]:
            groups[group + ("all supporting records",)].append(
                (
                    0
                    if any(k["tier"] == 0 for k in row["support"])
                    else max(k["tier"] for k in row["support"])
                )
            )
    for k, v in sorted(groups.items()):
        print(*k, v.count(1), sum(bool(x) for x in v), len(v))


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("step", choices=["seed", "dev", "test", "summary"])
    p.add_argument("snapshots", type=Path)
    p.add_argument("--binary", type=Path)
    p.add_argument("--output", type=Path)
    p.add_argument("--judge", type=Path)
    p.add_argument("--freeze")
    a = p.parse_args()
    if a.step == "seed":
        prepare(a.snapshots, a.binary.resolve())
    elif a.step == "summary":
        summary(a.output)
    else:
        a.split = a.step
        measure(a)
