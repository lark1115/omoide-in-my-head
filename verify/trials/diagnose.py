"""Offline lookup replay and optional, separately labelled provider diagnosis on the frozen archive."""

import argparse
from collections import Counter, defaultdict
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
from run import read, write


def capture():
    request = json.load(sys.stdin)
    Path(os.environ["DIAGNOSTIC_REQUEST"]).write_text(json.dumps(request))
    print('{"answers":[]}')


def match_receipts(study, rows, receipts):
    # A receipt was appended while its CLI call was in flight. Equal-size calls
    # can be ordered only when those intervals provably do not overlap.
    by_count = defaultdict(list)
    for r in rows:
        if r["status"] == "used":
            by_count[r["adapter_candidates"]].append(r)
    for count, group in by_count.items():
        source = [
            v["usage"] or {"input_tokens": 0, "output_tokens": 0}
            for v in receipts
            if v["candidates"] == count and v["status"] == 0
        ]
        spans = []
        for r in group:
            trial = read(
                study / "transcripts" / (r["opportunity"] + "-" + r["arm"] + ".json")
            )
            e = trial["server_events"][r["call"] - 1]
            # Actor-side events for all device trials were timestamped in one
            # driver process. Server clocks are only comparable within a call.
            matches = [
                event
                for event in trial["events"]
                if event["type"] == "call"
                and event["name"].split("__")[-1] == e["tool"]
                and event["args"] == e["args"]
            ]
            ordinal = sum(
                v["tool"] == e["tool"] and v["args"] == e["args"]
                for v in trial["server_events"][: r["call"] - 1]
            )
            call = matches[ordinal]
            end = next(
                event
                for event in trial["events"]
                if event["type"] == "delivery" and event["id"] == call["id"]
            )
            spans.append((call["at"], end["at"], r))
        spans.sort(key=lambda item: item[0])
        if len(spans) == len(source) and all(
            left[1] <= right[0] for left, right in zip(spans, spans[1:])
        ):
            for (_, _, row), usage in zip(spans, source):
                row["actual_tokens"] = usage
                row["receipt_basis"] = "candidate count and nonoverlapping call order"
        for _, _, row in spans:
            if "receipt_basis" not in row:
                row["receipt_basis"] = (
                    "identical same-size receipts"
                    if row.get("actual_tokens") is not None
                    else "ambiguous"
                )
            if row.get("actual_tokens") is not None:
                row.pop("possible_actual_receipts", None)


def diagnose(study, out, binary, adapter):
    rows = []
    receipts = [json.loads(l) for l in (study / "usage.jsonl").read_text().splitlines()]
    for p in sorted((study / "transcripts").glob("*.json")):
        t = read(p)
        if t["arm"] not in ("device", "device_jev"):
            continue
        for i, e in enumerate(t["server_events"]):
            if e["tool"] != "recall":
                continue
            value = json.loads(e["text"])
            msg = value.get("error", "")
            actual_ms = round((e["returned_ns"] - e["started_ns"]) / 1e6, 2)
            timeout = "timed out" in msg or "eight-second" in msg
            condition = (
                "tool_timeout"
                if timeout
                else "invalid_input"
                if msg
                else value["judge_state"]
            )
            r = dict(
                opportunity=t["opportunity"],
                arm=t["arm"],
                call=i + 1,
                status=condition,
                elapsed_ms=actual_ms,
                observed_pool=value.get("candidate_count"),
                judge_error=value.get("judge_error"),
                path_metadata_exposed=any(
                    prefix in msg
                    for prefix in ("/Volumes/", "/Users/", "/private/", "/var/folders/")
                ),
            )
            args = e["args"]
            r["rules_path"] = (
                "not requested"
                if args.get("kind") not in (None, "rule", "skill")
                else "action index"
                if not args.get("scene")
                else "lexical index plus action index"
                if args.get("action")
                else "lexical index"
            )
            if condition == "invalid_input":
                r["rules_path"] = "no usable reply"
                rows.append(r)
                continue
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                db = root / "snapshot.db"
                shutil.copyfile(study / "baseline.db", db)
                cmd = [binary, "--db", db, "recall"]
                for k, v in args.items():
                    cmd += ["--" + k, v]
                env = dict(
                    PATH=os.environ["PATH"],
                    LANG="en_US.UTF-8",
                    DIAGNOSTIC_REQUEST=str(root / "request"),
                )
                if t["arm"] == "device_jev":
                    cmd += [
                        "--judge",
                        sys.executable,
                        "--judge-arg",
                        str(Path(__file__).resolve()),
                        "--judge-arg",
                        "capture",
                        "--timeout-ms",
                        "6800",
                        "--threshold",
                        "0.6",
                    ]
                start = time.perf_counter_ns()
                pr = subprocess.run(cmd, capture_output=True, env=env, timeout=30)
                r["offline_replay_ms"] = round(
                    (time.perf_counter_ns() - start) / 1e6, 2
                )
                replay = json.loads(pr.stdout)
                r["replay_pool"] = replay.get("candidate_count")
                if (root / "request").exists():
                    raw = (root / "request").read_bytes()
                    r["adapter_candidates"] = len(json.loads(raw)["candidates"])
                    if adapter and condition in ("failed", "tool_timeout"):
                        env = {
                            "TYPESAFE_API_KEY": os.environ["TYPESAFE_API_KEY"],
                            "JEV_TIMEOUT_MS": "5800",
                            "PATH": os.defpath,
                        }
                        start = time.perf_counter()
                        try:
                            diagnostic = subprocess.run(
                                [adapter, "--report-usage"],
                                input=raw,
                                env=env,
                                capture_output=True,
                                timeout=7,
                            )
                            message = diagnostic.stderr.decode().strip()
                            r["separate_provider_probe"] = (
                                "ok"
                                if diagnostic.returncode == 0
                                else message
                                if message.startswith("TypeSafe ") and len(message) < 80
                                else "adapter failure"
                            )
                            r["probe_usage"] = (
                                json.loads(message)["usage"]
                                if message.startswith("{")
                                else None
                            )
                        except subprocess.TimeoutExpired:
                            r["separate_provider_probe"] = "bridge 7 s expired"
                        r["probe_ms"] = round((time.perf_counter() - start) * 1000, 2)
            if condition == "not_configured":
                r["actual_tokens"] = {"input_tokens": 0, "output_tokens": 0}
            if condition == "used":
                options = [
                    a["usage"] or {"input_tokens": 0, "output_tokens": 0}
                    for a in receipts
                    if a["status"] == 0
                    and a["candidates"] == r.get("adapter_candidates")
                ]
                r["possible_actual_receipts"] = [
                    json.loads(k)
                    for k in sorted({json.dumps(x, sort_keys=True) for x in options})
                ]
                r["actual_tokens"] = (
                    options[0]
                    if options and all(x == options[0] for x in options)
                    else None
                )
            rows.append(r)
    match_receipts(study, rows, receipts)
    out.mkdir(parents=True, exist_ok=True)
    write(out / "lookup-diagnostics.json", rows)
    write(out / "provider-receipts.json", receipts)
    lines = [
        "| Arm (48 sessions) | Recall attempted | All attempted recalls failed at tool level | No successful Jev judgment among attempts | 8 s tool timeouts | Judge failures with core fallback |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for arm in ("device", "device_jev"):
        g = defaultdict(list)
        for r in rows:
            if r["arm"] == arm:
                g[r["opportunity"]].append(r["status"])
        flat = Counter(x for a in g.values() for x in a)
        unavailable = sum(
            not any(x in ("used", "failed", "not_configured") for x in v)
            for v in g.values()
        )
        no_judge = (
            sum("used" not in v for v in g.values()) if arm == "device_jev" else "n/a"
        )
        lines.append(
            f"| {arm} | {len(g)} | {unavailable} | {no_judge} | {flat['tool_timeout']} | {flat['failed']} |"
        )
    (out / "availability.md").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    if sys.argv[1:] == ["capture"]:
        capture()
        sys.exit(0)
    p = argparse.ArgumentParser()
    for k in ("study", "out", "binary"):
        p.add_argument(k, type=Path)
    p.add_argument("--probe-adapter", type=Path)
    a = p.parse_args()
    diagnose(
        a.study,
        a.out,
        a.binary.resolve(),
        a.probe_adapter.resolve() if a.probe_adapter else None,
    )
