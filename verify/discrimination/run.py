"""Evaluate the first real CLI reply in each arm against the frozen discrimination set."""

import argparse
import json
import os
from pathlib import Path
import sqlite3
import statistics
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "tests/discrimination"


def lines(path):
    return [json.loads(l) for l in path.read_text().splitlines()]


def wrapper():
    raw = sys.stdin.buffer.read()
    directory = Path(os.environ["DISCRIM_TRACE"])
    (directory / "count").write_text(str(len(json.loads(raw)["candidates"])))
    with (
        tempfile.TemporaryFile() as request,
        (directory / "usage").open("wb") as diagnostic,
    ):
        request.write(raw)
        request.seek(0)
        os.dup2(request.fileno(), 0)
        os.dup2(diagnostic.fileno(), 2)
        adapter = os.environ["DISCRIM_ADAPTER"]
        os.execv(adapter, [adapter, "--report-usage"])


def seed(binary, db):
    def cli(args):
        return json.loads(subprocess.check_output([binary, "--db", db, *args]))

    cli(["init"])
    mapping = {}
    for r in lines(DATA / "rules.jsonl"):
        exclusions = " / ".join(r["excludes"])
        text = (
            r["text"]
            + " ["
            + r["domain"]
            + "]"
            + (" Excludes: " + exclusions if exclusions else "")
        )
        source = cli(
            [
                "say",
                "--text",
                " ".join(filter(None, (r["scene"], r["action"], r["target"], text))),
            ]
        )["id"]
        args = [
            "rule",
            "add",
            "--scene",
            r["scene"],
            "--text",
            text,
            "--action",
            r["action"],
            "--source",
            source,
            "--purpose",
            r["domain"],
        ]
        if exclusions:
            args += ["--exclude", exclusions]
        if r["target"]:
            args += ["--target", r["target"]]
        outcome = cli(args)
        if outcome["state"] == "near_rules":
            outcome = cli(
                [
                    *args,
                    "--relation",
                    "unrelated",
                    "--against",
                    outcome["near_rules"][0]["record"]["version_id"],
                    "--near-digest",
                    outcome["near_digest"],
                ]
            )
        mapping[outcome["version_id"]] = r["id"]
    if len(mapping) != 72:
        raise ValueError("archive has missing versions")
    return mapping


def run(binary, adapter, output):
    subprocess.run([sys.executable, DATA / "tools/validate.py"], check=True)
    if not os.environ.get("TYPESAFE_API_KEY"):
        raise ValueError("set TYPESAFE_API_KEY for the live arm")
    output.mkdir(parents=True, exist_ok=True)
    queries = lines(DATA / "queries.jsonl")
    with tempfile.TemporaryDirectory() as path:
        root = Path(path)
        base = root / "base.db"
        mapping = seed(binary, base)
        rerun = False
        for arm, port, inner in [
            ("core", 0, 0),
            ("jev-default", 1000, 900),
            ("jev-extended", 10000, 9500),
        ]:
            if arm == "jev-extended" and not rerun:
                break
            db = root / (arm + ".db")
            with sqlite3.connect(base) as src, sqlite3.connect(db) as dest:
                src.backup(dest)
            env = {k: v for k, v in os.environ.items() if not k.startswith("OMOIDE_")}
            result_path = output / (arm + ".jsonl")
            with result_path.open("x") as stream:
                for q in queries:
                    with tempfile.TemporaryDirectory(dir=root) as trace:
                        trace = Path(trace)
                        args = [binary, "--db", db, "recall"]
                        for k, v in q["input"].items():
                            args += ["--" + k, v]
                        if port:
                            env.update(
                                DISCRIM_TRACE=str(trace),
                                DISCRIM_ADAPTER=str(adapter),
                                JEV_TIMEOUT_MS=str(inner),
                            )
                            args += [
                                "--judge",
                                sys.executable,
                                "--judge-arg",
                                str(Path(__file__).resolve()),
                                "--judge-arg",
                                "judge",
                                "--timeout-ms",
                                str(port),
                                "--threshold",
                                "0.5",
                            ]
                        start = time.perf_counter()
                        p = subprocess.run(
                            args, env=env, capture_output=True, text=True, timeout=20
                        )
                        ms = round((time.perf_counter() - start) * 1000, 2)
                        result = json.loads(p.stdout) if p.returncode == 0 else {}
                        returned = [
                            mapping[r["version_id"]]
                            for r in result.get("results", [])
                            if r["kind"] == "rule"
                        ]
                        diag = (
                            (trace / "usage").read_text().strip()
                            if (trace / "usage").exists()
                            else ""
                        )
                        usage = (
                            json.loads(diag)["usage"] if diag.startswith("{") else None
                        )
                        error = (
                            diag
                            if diag.startswith("TypeSafe ") and len(diag) < 80
                            else result.get("judge_error")
                        )
                        row = dict(
                            id=q["id"],
                            level=q["level"],
                            returned=returned if result else None,
                            judge_state=result.get("judge_state"),
                            error=error,
                            ms=ms,
                            candidates=int((trace / "count").read_text())
                            if (trace / "count").exists()
                            else None,
                            usage=usage,
                            remaining=result.get("remaining"),
                        )
                        if port and row["judge_state"] != "used":
                            rerun = True
                        stream.write(json.dumps(row, ensure_ascii=False) + "\n")
                        stream.flush()
            print(arm, len(queries), flush=True)


def metrics(output):
    keys = {q["id"]: q for q in lines(DATA / "queries.jsonl")}
    text = [
        "| Arm | Overlap | Required returned | Forbidden opportunities | Forbidden IDs | Exact | None correct | No response | Forbidden upper bound | Judge failures |",
        "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    timing = []
    for path in sorted(output.glob("*.jsonl")):
        rows = lines(path)
        if len(rows) != len(keys) or {r["id"] for r in rows} != keys.keys():
            raise ValueError("incomplete arm")
        for level in ("all", "shared", "partial", "zero"):
            g = [r for r in rows if level == "all" or r["level"] == level]
            n = len(g)
            need = hit = bad = forbidden = exact = none = empty = missing = failed = 0
            for r in g:
                q = keys[r["id"]]
                required = set(q["must"])
                banned = set(q["must_not"])
                need += len(required)
                empty += not required
                failed += r["judge_state"] == "failed"
                if r["returned"] is None:
                    missing += 1
                    continue
                returned = set(r["returned"])
                hit += len(returned & required)
                bad += bool(returned & banned)
                forbidden += len(returned & banned)
                exact += returned == required
                none += not required and not returned
            text.append(
                f"| {path.stem} | {level} | {hit}/{need} | {bad}/{n} | {forbidden} | {exact}/{n} | {none}/{empty} | {missing} | {bad + missing}/{n} | {failed} |"
            )
        ms = sorted(r["ms"] for r in rows)
        us = [r["usage"] for r in rows if r["usage"]]
        counts = [r["candidates"] for r in rows if r["candidates"] is not None]
        timing.append(
            f"{path.stem}: median {statistics.median(ms):.1f} ms, p95 {ms[144]:.1f} ms, max {ms[-1]:.1f} ms. Candidate range {str((min(counts), max(counts))) if counts else 'unobserved'}. Token receipts {len(us)}/152, input {sum(u['input_tokens'] for u in us)}, output {sum(u['output_tokens'] for u in us)}."
        )
    (output / "metrics.md").write_text(
        "\n".join(text) + "\n\n" + "\n\n".join(timing) + "\n"
    )


if __name__ == "__main__":
    if sys.argv[1:] == ["judge"]:
        wrapper()
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--adapter", type=Path)
    a = parser.parse_args()
    if a.binary:
        run(a.binary.resolve(), a.adapter.resolve(), a.output)
    metrics(a.output)
