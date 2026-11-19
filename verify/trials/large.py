"""Run the four-arm large-archive study under the frozen paging/budget contract."""

import argparse
import hashlib
import os
import shutil
import subprocess
import sys
from pathlib import Path
from run import DATA, ROOT, SYSTEM, read, write, run_schedule, plan
from large_server import LARGE_LEDGER
from jev_bridge import host

FIXTURE = ROOT / "tests/trials-large"
HERE = Path(__file__).resolve().parent
ARMS = ("no_device", "hot_only", "device", "device_jev")


def prepare(study, claude):
    if ROOT in study.parents:
        raise ValueError("study must be private outside checkout")
    study.mkdir(mode=0o700, parents=True, exist_ok=True)
    design = dict(
        manifest=read(FIXTURE / "manifest.json"),
        system=SYSTEM,
        model="claude-sonnet-5-5",
        effort="low",
        cli=subprocess.check_output([claude, "--version"], text=True).strip(),
        deadline_seconds=120,
        tool_timeout_seconds=8,
        arms=ARMS,
        blocks=[["no_device", "hot_only"], ["device", "device_jev"]],
        grading_model="gpt-6.2-sol",
    )
    if (study / "design.json").exists():
        if read(study / "design.json") != {**design, "arms": list(ARMS)}:
            raise ValueError("changed design")
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
    import json

    rows = [json.loads(l) for l in (study / "archive.jsonl").read_text().splitlines()]
    write(study / "ledger.json", rows)
    # Block order was externally specified before outcomes. Each within-block order is balanced.
    allocation = plan(
        read(DATA / "opportunities.json"), read(DATA / "grading/key.json")
    )
    for i, p in enumerate(allocation):
        p["arms"] = [a for a in p["arms"] if a != "device"] + list(
            ("device", "device_jev") if i % 2 == 0 else ("device_jev", "device")
        )
    write(study / "allocation.json", allocation)
    for source, dest in [
        (HERE / "large_server.py", "server.py"),
        (HERE / "workshop.py", "workshop.py"),
        (FIXTURE / "tools/ledger.py", "ledger.py"),
        (HERE / "jev_bridge.py", "jev_bridge.py"),
    ]:
        shutil.copyfile(source, study / dest)
    (study / "transcripts").mkdir(exist_ok=True)
    write(study / "design.json", design)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("phase", choices=["controls", "device"])
    p.add_argument("study", type=Path)
    p.add_argument("binary", type=Path)
    p.add_argument("--adapter", type=Path)
    p.add_argument("--claude", default="claude")
    p.add_argument("--jobs", type=int, default=6)
    a = p.parse_args()
    os.umask(0o077)
    study = a.study.resolve()
    binary = a.binary.resolve()
    claude = shutil.which(a.claude)
    prepare(study, claude)
    if a.phase == "controls":
        run_schedule(
            study,
            binary,
            claude,
            120,
            a.jobs,
            ledger_tools=LARGE_LEDGER,
            variants=ARMS[:2],
        )
    else:
        from large_seed import seed_large

        # Controls have finished; install the calibrated Jev arguments for device sessions.
        shutil.copyfile(HERE / "workshop.py", study / "workshop.py")
        write(
            study / "device-design.json",
            {
                "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                "adapter_sha256": hashlib.sha256(a.adapter.read_bytes()).hexdigest(),
                "model": "jev-1.14.0",
                "threshold": 0.60,
                "port_ms": 6800,
                "adapter_ms": 5800,
            },
        )
        seed_large(study, binary)
        with host(a.adapter.resolve(), study / "usage.jsonl") as sock:
            run_schedule(
                study,
                binary,
                claude,
                120,
                a.jobs,
                ledger_tools=LARGE_LEDGER,
                variants=ARMS[2:],
                arm_env={"device_jev": {"TRIAL_JUDGE_SOCKET": sock}},
            )


if __name__ == "__main__":
    main()
