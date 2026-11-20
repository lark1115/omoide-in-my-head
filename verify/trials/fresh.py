"""Run the preregistered prospective workshop study on a pinned product build."""

import argparse
import hashlib
import itertools
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

from run import ROOT, SYSTEM, read, write, run_schedule
from large import HERE

ARMS = ("no_device", "hot_only", "device_core", "device_jev")
from large_server import LARGE_LEDGER
from jev_bridge import host
from seed import populate

FIXTURE = ROOT / "tests/trials-large-2"


def warm_store(binary, db):
    def cli(*args):
        return json.loads(subprocess.check_output([binary, "--db", db, *args]))

    cli("init")
    source = cli("say", "--text", "Wrap the practice tile.")["id"]
    cli(
        "rule",
        "add",
        "--source",
        source,
        "--text",
        "Wrap the practice tile.",
        "--scene",
        "practice packing",
        "--action",
        "practice",
    )


def warm_up(binary, study, sock, opportunity):
    path = study / "warmups" / (opportunity + ".json")
    with tempfile.TemporaryDirectory() as temp:
        db = Path(temp) / "practice.db"
        shutil.copyfile(study / "practice.db", db)
        cmd = [
            binary,
            "--db",
            db,
            "recall",
            "--action",
            "practice",
            "--judge",
            sys.executable,
            "--judge-arg",
            str(study / "jev_bridge.py"),
            "--judge-arg",
            sock,
            "--judge-arg",
            "warm-" + opportunity,
            "--timeout-ms",
            "6800",
            "--threshold",
            "0.60",
        ]
        attempts = []
        for _ in range(3):
            start = time.monotonic()
            try:
                p = subprocess.run(cmd, capture_output=True, timeout=8, check=True)
                reply = json.loads(p.stdout)
                state = reply["judge_state"]
                judged = sum(v["judged"] for v in reply["search"]["by_kind"].values())
            except (subprocess.SubprocessError, ValueError, KeyError):
                state = "failed"
                judged = 0
            attempts.append(
                dict(
                    state=state,
                    judged=judged,
                    ms=round((time.monotonic() - start) * 1000),
                )
            )
            write(path, attempts)
            if state == "used" and judged == 1:
                return
    raise RuntimeError("practice judgment unavailable before actor launch")


def run(study, binary, adapter, claude, jobs):
    if not (study / "practice.db").exists():
        warm_store(binary, study / "practice.db")
    for name in ("warmups", "attempts"):
        (study / name).mkdir(exist_ok=True)
    with host(adapter, study / "usage.jsonl") as sock:

        def before(op, arm):
            if arm == "device_jev":
                warm_up(binary, study, sock, op)
            # Refuse to repeat an attempt if a previous driver stopped before writing its transcript.
            with (study / "attempts" / (op + "-" + arm)).open("x") as f:
                f.write("started\n")

        run_schedule(
            study,
            binary,
            claude,
            120,
            jobs,
            data=FIXTURE,
            ledger_tools=LARGE_LEDGER,
            arm_env={"device_jev": {"TRIAL_JUDGE_SOCKET": sock}},
            before_trial=before,
        )


def materialize(study, binary, adapter, claude):
    """Create private host inputs from the merged fixture, once, before any actor starts."""
    if ROOT in study.parents or study == ROOT:
        raise ValueError("private study must be outside checkout")
    manifest = read(FIXTURE / "manifest.json")
    revision = subprocess.check_output(
        ["git", "-C", ROOT, "rev-parse", "origin/main"], text=True
    ).strip()
    design = dict(
        product_commit=revision,
        manifest_sha256=hashlib.sha256(
            (FIXTURE / "manifest.json").read_bytes()
        ).hexdigest(),
        model="claude-sonnet-5-5",
        cli=subprocess.check_output([claude, "--version"], text=True).strip(),
        grader="gpt-6.2-sol",
        grader_cli=subprocess.check_output(["codex", "--version"], text=True).strip(),
        helper_sha256=hashlib.sha256(adapter.read_bytes()).hexdigest(),
        judge_model="jev-1.14.0",
        ledger_budget=manifest["budget"],
        arm_order=manifest["arms"],
        grading="separate fresh tool-less process; neutralized work decision and opportunity-specific key",
        trials=192,
        effort="low",
        system=SYSTEM,
        binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        session_seconds=120,
        tool_seconds=8,
        port_ms=6800,
        bridge_reader_ms=6500,
        adapter_ms=5800,
        threshold=0.60,
        candidate_bound=0.30,
        warmup="one successful real practice judgment per Jev session",
    )
    study.mkdir(mode=0o700, parents=True, exist_ok=True)
    if (study / "design.json").exists():
        if read(study / "design.json") != design:
            raise ValueError("changed study design")
        return
    subprocess.run(
        [sys.executable, FIXTURE / "generate.py", study / "archive.jsonl"], check=True
    )
    subprocess.run(
        [
            sys.executable,
            FIXTURE / "tools/validate.py",
            "--archive",
            study / "archive.jsonl",
        ],
        check=True,
    )
    rows = [json.loads(l) for l in (study / "archive.jsonl").read_text().splitlines()]
    if (
        len(rows) != manifest["archive_rows"]
        or hashlib.sha256((study / "archive.jsonl").read_bytes()).hexdigest()
        != manifest["archive_sha256"]
    ):
        raise ValueError("archive mismatch")
    write(study / "ledger.json", rows)
    for src, dest in [
        (HERE / "large_server.py", "server.py"),
        (HERE / "workshop.py", "workshop.py"),
        (HERE / "jev_bridge.py", "jev_bridge.py"),
        (ROOT / "tests/trials-large/tools/ledger.py", "ledger.py"),
    ]:
        shutil.copyfile(src, study / dest)
    sys.path.insert(0, str(FIXTURE))
    from generate import build

    names = {
        r["id"]: k if family == "original" else r["id"] for k, family, r in build()
    }
    with tempfile.TemporaryDirectory(dir=study) as td:
        db = Path(td) / "seed.db"
        index = populate(
            binary,
            rows,
            db,
            expected_count=len(rows),
            default_fact_speaker="unspecified",
        )
        with sqlite3.connect(db) as c:
            counts = dict(c.execute("SELECT kind,count(*) FROM memories GROUP BY kind"))
            counts["rule"] = c.execute(
                "SELECT count(*) FROM rules WHERE state='current'"
            ).fetchone()[0]
            revisions = c.execute(
                "SELECT count(*) FROM memories WHERE json_extract(details,'$.decision')='revised'"
            ).fetchone()[0]
            if sum(counts.values()) != len(rows) or c.execute(
                "PRAGMA integrity_check"
            ).fetchone() != ("ok",):
                raise ValueError("snapshot census mismatch")
            c.execute("PRAGMA wal_checkpoint(TRUNCATE)")
        write(study / "id-map.json", {k: names[v] for k, v in index.items()})
        os.replace(db, study / "baseline.db")
    write(
        study / "seed.json",
        dict(
            rows=len(rows),
            kinds=counts,
            revised=revisions,
            sha256=hashlib.sha256((study / "archive.jsonl").read_bytes()).hexdigest(),
        ),
    )
    (study / "transcripts").mkdir(exist_ok=True)
    allocation = allocate(FIXTURE, manifest)
    write(study / "allocation.json", allocation)
    write(study / "design.json", design)


def allocate(data, manifest):
    assert tuple(manifest["arms"]) == ARMS
    orders = list(itertools.permutations(ARMS))
    return [
        dict(id=r["id"], arms=list(orders[i % len(orders)]))
        for i, r in enumerate(
            sorted(read(data / "opportunities.json"), key=lambda r: r["id"])
        )
    ]


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("study", type=Path)
    p.add_argument("binary", type=Path)
    p.add_argument("adapter", type=Path)
    p.add_argument("--claude", default="claude")
    p.add_argument("--jobs", type=int, default=6)
    a = p.parse_args()
    os.umask(0o077)
    folder = a.study.resolve()
    binary = a.binary.resolve()
    adapter = a.adapter.resolve()
    actor = shutil.which(a.claude)
    materialize(folder, binary, adapter, actor)
    run(folder, binary, adapter, actor, a.jobs)
