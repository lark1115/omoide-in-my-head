"""Linux wall time including CLI startup, and true per-child peak RSS."""

import json, os, statistics, subprocess, sys, tempfile, time
from pathlib import Path


def main():
    binary, snapshots = map(Path, sys.argv[1:])
    lookup = json.loads(Path(__file__).with_name("lookups.json").read_text())["T01"][0]
    with tempfile.TemporaryDirectory() as tmp:
        import sqlite3

        for size in (48, 500, 3048):
            db = Path(tmp) / f"{size}.db"
            with (
                sqlite3.connect(
                    f"file:{snapshots / str(size)}.db?immutable=1", uri=True
                ) as src,
                sqlite3.connect(db) as dst,
            ):
                src.backup(dst)
            times = []
            rss = []
            for _ in range(21):
                start = time.perf_counter_ns()
                with subprocess.Popen(
                    [binary, "--db", db, "recall", "--scene", lookup["scene"]],
                    stdout=subprocess.PIPE,
                ) as p:
                    data = json.loads(p.stdout.read())
                    _, status, usage = os.wait4(p.pid, 0)
                    p.returncode = os.waitstatus_to_exitcode(status)
                    if p.returncode:
                        raise RuntimeError("benchmark CLI failed")
                times.append((time.perf_counter_ns() - start) / 1e6)
                rss.append(usage.ru_maxrss)
            print(
                json.dumps(
                    dict(
                        records=size,
                        runs=len(times),
                        median_ms=round(statistics.median(times), 2),
                        p95_ms=round(sorted(times)[19], 2),
                        max_rss_kib=max(rss),
                        candidates=data["candidate_count"],
                    )
                )
            )


if __name__ == "__main__":
    main()
