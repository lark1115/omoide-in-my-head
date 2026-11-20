"""Run every frozen case on every iteration and preserve all non-passes verbatim."""

import os
import tempfile
import argparse
import json
from pathlib import Path
import subprocess
import time

if __name__ == "__main__":
    p = argparse.ArgumentParser()
    for key in ("binary", "fault", "output"):
        p.add_argument(key, type=Path)
    p.add_argument("--runs", type=int, default=20)
    a = p.parse_args()
    with a.output.open("x") as out, tempfile.TemporaryDirectory() as tmp:
        for i in range(1, a.runs + 1):
            start = time.monotonic()
            trace = Path(tmp) / f"{i}.jsonl"
            env = {**os.environ, "OMOIDE_TEST_TIMING_LOG": str(trace)}
            result = subprocess.run(
                [
                    a.binary.resolve(),
                    "eval",
                    "run",
                    "--scenes",
                    "tests/scenes",
                    "--fault-binary",
                    a.fault.resolve(),
                ],
                capture_output=True,
                text=True,
                check=True,
                env=env,
            )
            report = json.loads(result.stdout)
            cases = report["cases"]
            if "failure_directory" in report:
                saved = [
                    json.loads(f.read_text())
                    for f in sorted(Path(report["failure_directory"]).glob("*.json"))
                ]
                assert saved == [r for r in cases if r["status"] != "pass"]
            assert len(cases) == len(set(r["case"] for r in cases)) == 62
            misses = [
                dict(case=r["case"], status=r["status"], errors=r.get("errors", []))
                for r in cases
                if r["status"] != "pass"
            ]
            print(
                json.dumps(
                    dict(
                        run=i,
                        count=len(cases),
                        ms=round((time.monotonic() - start) * 1000),
                        nonpasses=misses,
                        stops=[json.loads(l) for l in trace.read_text().splitlines()]
                        if trace.exists()
                        else [],
                    ),
                    ensure_ascii=False,
                ),
                file=out,
                flush=True,
            )
