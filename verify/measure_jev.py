"""Measure the eight invented S1 cases through the real CLI. The live run requires explicit invocation."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def case_path(scene, suffix):
    return Path(os.environ["JEV_STUDY_RUN_DIR"]) / (
        hashlib.sha256(scene.encode()).hexdigest() + suffix
    )


def judge():
    request = json.load(sys.stdin)
    candidates = request["candidates"]
    scene = request["query"]["scene"]
    capture = case_path(scene, ".json")
    # Local version IDs change in each fresh store. Replay only the same question and candidate content.
    content = [{k: v for k, v in c.items() if k != "version_id"} for c in candidates]
    if os.environ["JEV_STUDY_MODE"] == "live":
        proc = subprocess.run(
            [os.environ["JEV_STUDY_ADAPTER"], "--report-usage"],
            input=json.dumps(request),
            text=True,
            capture_output=True,
        )
        if proc.returncode:
            # Store only the adapter's fixed diagnostics, never a remote error body or environment.
            return proc.returncode
        response = json.loads(proc.stdout)
        scores = {a["version_id"]: a["score"] for a in response["answers"]}
        observation = {
            "query": request["query"],
            "content": content,
            "scores": [scores[c["version_id"]] for c in candidates],
            "usage": json.loads(proc.stderr)["usage"],
        }
        capture.write_text(json.dumps(observation))
    else:
        observation = json.loads(capture.read_text())
        if (
            observation["query"] != request["query"]
            or observation["content"] != content
            or len(observation["scores"]) != len(candidates)
        ):
            raise ValueError("replay inputs differ from live inputs")
        response = {
            "answers": [
                {"version_id": c["version_id"], "score": p}
                for c, p in zip(candidates, observation["scores"])
            ]
        }
    print(json.dumps(response))
    return 0


def proxy(args):
    cmd = [os.environ["JEV_STUDY_BINARY"], *args]
    if args[2:3] != [
        "recall"
    ]:  # The scene adapter prefixes every invocation with --db <path>.
        os.execv(cmd[0], cmd)
    scene = args[args.index("--scene") + 1]
    cmd += [
        "--judge",
        sys.executable,
        "--judge-arg",
        os.environ["JEV_STUDY_DRIVER"],
        "--judge-arg",
        "judge",
        "--threshold",
        os.environ["JEV_STUDY_THRESHOLD"],
        "--timeout-ms",
        os.environ["JEV_STUDY_PORT_MS"],
    ]
    start = time.perf_counter_ns()
    result = subprocess.run(cmd, stdout=subprocess.PIPE)
    elapsed = (time.perf_counter_ns() - start) / 1e6
    response = json.loads(result.stdout)
    case_path(scene, ".time").write_text(
        json.dumps(
            {
                "ms": elapsed,
                "judge_state": response.get("judge_state"),
                "error": response.get("judge_error"),
            }
        )
    )
    sys.stdout.buffer.write(result.stdout)
    return result.returncode


def launcher(directory, role):
    path = directory / role
    path.write_text(
        "#!/usr/bin/env python3\nimport os, sys\nos.execv(sys.executable, "
        f"[sys.executable, os.environ['JEV_STUDY_DRIVER'], '{role}', *sys.argv[1:]])\n"
    )
    path.chmod(0o700)
    return path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("adapter", type=Path)
    parser.add_argument("--scenes", type=Path, default=Path("tests/scenes"))
    parser.add_argument("--adapter-ms", type=int, default=900)
    parser.add_argument("--port-ms", type=int, default=1000)
    parser.add_argument(
        "--replay-from",
        type=Path,
        help="Reuse successful live JSONL observations of this frozen single-rule fixture; no API calls.",
    )
    args = parser.parse_args()
    if not args.replay_from and not os.environ.get("TYPESAFE_API_KEY"):
        parser.error("missing TYPESAFE_API_KEY")
    fixture_bytes = (args.scenes / "01-s01.json").read_bytes()
    fixture_hash = hashlib.sha256(fixture_bytes).hexdigest()
    fixture = json.loads(fixture_bytes)
    all_used = True
    (rule,) = [r for r in fixture["records"] if r["kind"] == "rule"]
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        wrapped = launcher(root, "proxy")
        env = {
            **os.environ,
            "JEV_STUDY_BINARY": str(args.binary.resolve()),
            "JEV_STUDY_ADAPTER": str(args.adapter.resolve()),
            "JEV_STUDY_DRIVER": str(Path(__file__).resolve()),
            "JEV_TIMEOUT_MS": str(args.adapter_ms),
            "JEV_STUDY_PORT_MS": str(args.port_ms),
        }
        previous = {}
        if args.replay_from:
            for line in args.replay_from.read_text().splitlines():
                row = json.loads(line)
                if row["mode"] == "live":
                    if (
                        row["judge_state"] != "used"
                        or row["fixture_sha256"] != fixture_hash
                        or (row["run"], row["case"]) in previous
                    ):
                        raise ValueError(
                            "replay requires unique, successful live observations"
                        )
                    previous[row["run"], row["case"]] = row
        for run in range(1, 4):
            run_dir = root / str(run)
            run_dir.mkdir()
            env["JEV_STUDY_RUN_DIR"] = str(run_dir)
            if args.replay_from:
                for case in fixture["cases"]:
                    row = previous[run, case["id"]]
                    score = (
                        row["correct_probability"]
                        if rule["id"] in case["expect"]["must"]
                        else row["best_wrong_probability"]
                    )
                    if not isinstance(score, (float, int)) or not 0 <= score <= 1:
                        raise ValueError("invalid recorded probability")
                    query = {
                        k: case["query"].get(k) for k in ("scene", "action", "target")
                    }
                    content = [
                        {
                            "text": rule["text"],
                            "scene": rule["scene"],
                            "aliases": rule.get("aliases", []),
                        }
                    ]
                    name = hashlib.sha256(query["scene"].encode()).hexdigest() + ".json"
                    (run_dir / name).write_text(
                        json.dumps(
                            {
                                "query": query,
                                "content": content,
                                "scores": [score],
                                "usage": row["usage"],
                            }
                        )
                    )
                    print(json.dumps(row), flush=True)
            for threshold in (0.3, 0.7) if args.replay_from else (0.5, 0.3, 0.7):
                mode = "live" if threshold == 0.5 else "replay"
                env.update(JEV_STUDY_MODE=mode, JEV_STUDY_THRESHOLD=str(threshold))
                run_env = env.copy()
                if mode == "replay":
                    run_env.pop("TYPESAFE_API_KEY", None)
                report = json.loads(
                    subprocess.check_output(
                        [
                            args.binary.resolve(),
                            "eval",
                            "run",
                            "--scenes",
                            args.scenes.resolve(),
                            "--scene",
                            "S1",
                            "--binary",
                            wrapped,
                        ],
                        env=run_env,
                    )
                )
                observed = {row["case"]: row for row in report["cases"]}
                for case in fixture["cases"]:
                    path = (
                        run_dir
                        / hashlib.sha256(case["query"]["scene"].encode()).hexdigest()
                    )
                    timing = json.loads(path.with_suffix(".time").read_text())
                    record = (
                        json.loads(path.with_suffix(".json").read_text())
                        if path.with_suffix(".json").exists()
                        else {}
                    )
                    if record and (
                        len(record["scores"]) != 1
                        or record["content"][0]["text"] != rule["text"]
                    ):
                        raise RuntimeError("unexpected candidate set for frozen S1")
                    correct = rule["id"] in case["expect"]["must"]
                    score = record.get("scores", [None])[0]
                    actual = observed[case["id"]]
                    all_used &= timing["judge_state"] == "used"
                    print(
                        json.dumps(
                            {
                                "run": run,
                                "case": case["id"],
                                "mode": mode,
                                "fixture_sha256": fixture_hash,
                                "threshold": threshold,
                                "status": actual["status"],
                                **timing,
                                "correct_probability": score if correct else None,
                                "best_wrong_probability": None if correct else score,
                                "usage": record.get("usage")
                                if mode == "live"
                                else None,
                            }
                        ),
                        flush=True,
                    )

    return 0 if all_used else 1


if __name__ == "__main__":
    if sys.argv[1:2] == ["proxy"]:
        sys.exit(proxy(sys.argv[2:]))
    if sys.argv[1:2] == ["judge"]:
        sys.exit(judge())
    sys.exit(main())
