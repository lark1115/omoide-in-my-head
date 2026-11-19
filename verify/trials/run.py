"""Run fresh Claude sessions over a frozen archive. Private study directory is outside the repository."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import itertools
import json
import os
from pathlib import Path
import shutil
import signal
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time

from seed import populate
from workshop import LEDGER, DEVICE, compact

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "tests/trials"
ARMS = ("no_device", "hot_only", "device")
SYSTEM = {
    "ja": "次の工房の依頼に答えて。何をするか、そのまま担当に渡せる形で決めて。まだしていない作業を済んだとは言わないで。",
    "en": "Answer the workshop request with a decision or instructions the next shift can use. Do not claim to have performed work you have not done.",
}


def write(path, value):
    tmp = path.with_suffix(".writing")
    tmp.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
    tmp.replace(path)


def read(path):
    return json.loads(path.read_text())


def plan(rows, keys):
    orders = list(itertools.permutations(ARMS))
    first = set()
    # Precommit half-sample independently of any answers: 18 primary, four controls, two outside.
    types = sorted({k["type"] for k in keys})
    by_id = {r["id"]: r for r in rows}
    for i, typ in enumerate(types):
        for lang in ("ja", "en"):
            ids = sorted(
                k["id"]
                for k in keys
                if k["type"] == typ and by_id[k["id"]]["language"] == lang
            )
            take = len(ids) // 2
            if len(ids) % 2 and (i % 2 == (lang == "en")):
                take += 1
            first.update(ids[:take])
    return [
        {
            "id": r["id"],
            "arms": list(orders[i % 6]),
            "half": 1 if r["id"] in first else 2,
        }
        for i, r in enumerate(sorted(rows, key=lambda x: x["id"]))
    ]


def prepare(study, binary, claude, timeout):
    subprocess.run(
        [sys.executable, DATA / "tools/validate.py"],
        check=True,
        stdout=subprocess.DEVNULL,
    )
    manifest = read(DATA / "manifest.json")
    meta = {
        "data_commit": subprocess.check_output(
            ["git", "-C", ROOT, "log", "-1", "--format=%H", "--", "tests/trials"],
            text=True,
        ).strip(),
        "fixture_hashes": manifest["sha256"],
        "model": "claude-sonnet-5-5",
        "effort": "low",
        "cli": subprocess.check_output([claude, "--version"], text=True).strip(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "deadline_seconds": timeout,
        "sampling": "provider default; seed not exposed",
        "tool_timeout_seconds": 8,
        "judge": "not configured",
        "system": SYSTEM,
    }
    meta_file = study / "design.json"
    if meta_file.exists():
        if read(meta_file) != meta:
            raise ValueError("run parameters or fixture changed")
        return
    study.mkdir(mode=0o700, parents=True, exist_ok=True)
    # The acting model sees neither this host-side allocation nor the grading key.
    write(
        study / "allocation.json",
        plan(read(DATA / "opportunities.json"), read(DATA / "grading/key.json")),
    )
    shutil.copyfile(DATA / "memory.json", study / "ledger.json")
    shutil.copyfile(Path(__file__).with_name("workshop.py"), study / "server.py")
    db = study / "baseline.db"
    if db.exists():
        raise ValueError("partial initialization: choose a new study directory")
    write(study / "id-map.json", populate(binary, read(study / "ledger.json"), db))
    write(meta_file, meta)
    (study / "transcripts").mkdir(exist_ok=True)


def clean_env():
    keys = (
        "HOME",
        "USER",
        "LOGNAME",
        "LANG",
        "PATH",
        "TMPDIR",
        "CLAUDE_CONFIG_DIR",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "ANTHROPIC_API_KEY",
    )
    env = {k: os.environ[k] for k in keys if k in os.environ}
    env.update(
        CLAUDE_CODE_DISABLE_CLAUDE_MDS="1",
        CLAUDE_CODE_DISABLE_AUTO_MEMORY="1",
        CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1",
    )
    return env


def trial(
    study, binary, claude, op, arm, timeout, *, ledger_tools=LEDGER, server_options=None
):
    path = study / "transcripts" / f"{op['id']}-{arm}.json"
    if path.exists():
        return read(path)["completion"]
    hot = (DATA / "hot.txt").read_bytes().decode() if arm != "no_device" else ""
    system = SYSTEM[op["language"]] + ("\n\n" + hot if hot else "")
    has_device = arm in ("device", "device_jev")
    tools = ledger_tools + (DEVICE if has_device else [])
    exposed = ["mcp__workshop__" + t["name"] for t in tools]
    with tempfile.TemporaryDirectory(prefix="workshop-") as name:
        cwd = Path(name)
        log = cwd / "events.jsonl"
        server_env = {
            "TRIAL_ARCHIVE": str(study / "ledger.json"),
            "TRIAL_TOOL_LOG": str(log),
        }
        server_env.update(server_options or {})
        if has_device:
            with (
                sqlite3.connect(study / "baseline.db") as original,
                sqlite3.connect(cwd / "workshop.db") as db,
            ):
                original.backup(db)
            server_env.update(
                TRIAL_BINARY=str(binary), TRIAL_DB=str(cwd / "workshop.db")
            )
        server = {
            "mcpServers": {
                "workshop": {
                    "command": sys.executable,
                    "args": [str(study / "server.py")],
                    "env": server_env,
                }
            }
        }
        cmd = [
            claude,
            "-p",
            "--restricted",
            "--setting-sources",
            "",
            "--settings",
            compact(
                {
                    "disableAllHooks": True,
                    "autoMemoryEnabled": False,
                    "claudeMdExcludes": ["**/*"],
                }
            ),
            "--no-chrome",
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--no-session-persistence",
            "--tools",
            "",
            "--allowedTools",
            *exposed,
            "--permission-mode",
            "dontAsk",
            "--mcp-config",
            compact(server),
            "--system-prompt",
            system,
            "--model",
            "claude-sonnet-5-5",
            "--effort",
            "low",
            "--verbose",
            "--output-format",
            "stream-json",
        ]
        started = time.monotonic_ns()
        proc = subprocess.Popen(
            cmd,
            text=True,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            cwd=cwd,
            env=clean_env(),
            start_new_session=True,
        )
        events = []
        final = {}
        problem = []
        init = []

        def collect():
            for line in proc.stdout:
                at = time.monotonic_ns()
                try:
                    r = json.loads(line)
                except ValueError:
                    continue
                typ = r.get("type")
                if typ == "system" and r.get("subtype") == "init":
                    init.append(sorted(r["tools"]))
                    if set(r["tools"]) != set(exposed) or r.get("skills"):
                        problem.append("tool_inventory_mismatch")
                        try:
                            os.killpg(proc.pid, signal.SIGTERM)
                        except ProcessLookupError:
                            pass
                if typ in ("assistant", "user"):
                    for c in r.get("message", {}).get("content", []):
                        if c["type"] == "text" and typ == "assistant":
                            events.append({"type": "text", "at": at, "text": c["text"]})
                        elif c["type"] == "tool_use":
                            events.append(
                                {
                                    "type": "call",
                                    "at": at,
                                    "id": c["id"],
                                    "name": c["name"],
                                    "args": c["input"],
                                }
                            )
                        elif c["type"] == "tool_result":
                            events.append(
                                {
                                    "type": "delivery",
                                    "at": at,
                                    "id": c["tool_use_id"],
                                    "content": c["content"],
                                    "error": c.get("is_error", False),
                                }
                            )
                if typ == "result":
                    final.update(
                        {
                            k: r.get(k)
                            for k in (
                                "is_error",
                                "result",
                                "usage",
                                "modelUsage",
                                "num_turns",
                                "total_cost_usd",
                            )
                        }
                    )

        reader = threading.Thread(target=collect)
        reader.start()
        proc.stdin.write(op["prompt"])
        proc.stdin.close()
        try:
            proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            problem.append("time_budget")
            os.killpg(proc.pid, signal.SIGKILL)
            proc.wait()
        reader.join()
        tools_log = (
            [json.loads(l) for l in log.read_text().splitlines()]
            if log.exists()
            else []
        )
    if not init:
        problem.append("no_inventory")
    if final.get("is_error") or not final.get("result"):
        problem.append("no_completed_answer")
    if proc.returncode:
        problem.append("cli_failure")
    model_usage = final.get("modelUsage") or {}
    transcript = {
        "opportunity": op["id"],
        "arm": arm,
        "prompt": op["prompt"],
        "language": op["language"],
        "started_ns": started,
        "hot": hot,
        "tool_definitions": tools,
        "events": events,
        "server_events": tools_log,
        "answer": final.get("result", "") if not final.get("is_error") else "",
        "model_names": sorted(model_usage),
        "tokens": final.get("usage"),
        "model_usage": model_usage,
        "estimated_usd": final.get("total_cost_usd"),
        "errors": problem,
        "completion": "failed" if problem else "complete",
    }
    if final.get("is_error"):
        transcript["provider_error"] = str(final.get("result", ""))[:500]
    write(path, transcript)
    return transcript["completion"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("study", type=Path)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--claude", default="claude")
    parser.add_argument("--half", type=int, choices=[1, 2])
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--timeout", type=int, default=120)
    opts = parser.parse_args()
    os.umask(0o077)
    study = opts.study.resolve()
    binary = opts.binary.resolve()
    claude = shutil.which(opts.claude)
    if ROOT in study.parents:
        raise ValueError("keep private transcripts and mapping outside the checkout")
    prepare(study, binary, claude, opts.timeout)
    run_schedule(study, binary, claude, opts.timeout, opts.jobs, half=opts.half)


def run_schedule(
    study,
    binary,
    claude,
    timeout,
    jobs,
    *,
    half=None,
    ledger_tools=LEDGER,
    arm_env=None,
    variants=None,
):
    rows = {r["id"]: r for r in read(DATA / "opportunities.json")}
    schedule = [
        p for p in read(study / "allocation.json") if not half or p["half"] == half
    ]
    stop = threading.Event()

    def run_op(p):
        for arm in p["arms"]:
            if variants is not None and arm not in variants:
                continue
            if stop.is_set():
                break
            outcome = trial(
                study,
                binary,
                claude,
                rows[p["id"]],
                arm,
                timeout,
                ledger_tools=ledger_tools,
                server_options=(arm_env or {}).get(arm),
            )
            print(p["id"], arm, outcome, flush=True)
            if outcome != "complete":
                record = read(study / "transcripts" / f"{p['id']}-{arm}.json")
                error = record.get("provider_error", "").casefold()
                if (
                    "limit" in error
                    or "not logged" in error
                    or "tool_inventory_mismatch" in record["errors"]
                ):
                    stop.set()

    with ThreadPoolExecutor(max_workers=jobs) as pool:
        list(pool.map(run_op, schedule))
    if stop.is_set():
        raise SystemExit(
            "provider limit, authentication or tool-inventory interruption"
        )


if __name__ == "__main__":
    main()
