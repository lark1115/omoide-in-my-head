"""Two optional live DEV lookups. Record safe diagnostics, never remote error bodies or credentials."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def proxy():
    request = json.load(sys.stdin)
    start = time.monotonic()
    proc = subprocess.run(
        [os.environ["SCALE_ADAPTER"], "--report-usage"],
        input=json.dumps(request),
        capture_output=True,
        text=True,
    )
    usage = json.loads(proc.stderr)["usage"] if proc.stderr.startswith("{") else None
    with open(os.environ["SCALE_BATCH_LOG"], "a") as f:
        print(
            json.dumps(
                dict(
                    count=len(request["candidates"]),
                    ms=round((time.monotonic() - start) * 1000),
                    status=proc.returncode,
                    usage=usage,
                )
            ),
            file=f,
        )
    sys.stdout.write(proc.stdout)
    return proc.returncode


if __name__ == "__main__":
    if sys.argv[1:] == ["proxy"]:
        sys.exit(proxy())
    p = argparse.ArgumentParser()
    for arg in ("binary", "db", "adapter", "output"):
        p.add_argument(arg, type=Path)
    a = p.parse_args()
    lookup = json.loads(Path(__file__).with_name("lookups.json").read_text())["T01"][0]
    with a.output.open("x") as out, tempfile.TemporaryDirectory() as td:
        for ms in (1000, 6000):
            log = Path(td) / str(ms)
            env = dict(
                os.environ,
                SCALE_ADAPTER=str(a.adapter.resolve()),
                SCALE_BATCH_LOG=str(log),
                JEV_TIMEOUT_MS="900",
            )
            start = time.monotonic()
            cmd = [
                a.binary,
                "--db",
                a.db,
                "recall",
                "--scene",
                lookup["scene"],
                "--judge",
                sys.executable,
                "--judge-arg",
                str(Path(__file__).resolve()),
                "--judge-arg",
                "proxy",
                "--threshold",
                "0.60",
                "--timeout-ms",
                str(ms),
            ]
            r = json.loads(subprocess.check_output(cmd, env=env))
            print(
                json.dumps(
                    dict(
                        limit_ms=ms,
                        elapsed_ms=round((time.monotonic() - start) * 1000),
                        search=r["search"],
                        state=r["judge_state"],
                        error=r.get("judge_error"),
                        batches=[json.loads(l) for l in log.read_text().splitlines()]
                        if log.exists()
                        else [],
                    )
                ),
                file=out,
                flush=True,
            )
