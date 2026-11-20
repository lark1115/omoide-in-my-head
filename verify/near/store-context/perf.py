"""Linux fresh-process registration preview of one invented existing rule at each frozen scale."""

import json, os, statistics, subprocess, sys, time, sqlite3
from pathlib import Path

root = Path(__file__).resolve().parents[3]
subject = next(
    r
    for r in json.loads((root / "tests/trials/memory.json").read_text())
    if r["id"] == "m001"
)
for size in (48, 500, 3048):
    db = Path(sys.argv[1]) / f"{size}.db"
    with sqlite3.connect(db) as c:
        body, scene, action, target, source = c.execute(
            "SELECT body,scene,actions,target,source FROM rules WHERE body=? AND state='current'",
            (subject["text"],),
        ).fetchone()
    args = [
        "--db",
        db,
        "rule",
        "add",
        "--text",
        body,
        "--scene",
        scene,
        "--source",
        source,
        "--target",
        target,
    ]
    for a in json.loads(action):
        args += ["--action", a]
    for label, binary in zip(["original", "selected"], sys.argv[2:]):
        times = []
        rss = []
        for _ in range(21):
            t = time.perf_counter()
            with subprocess.Popen([binary, *args], stdout=subprocess.PIPE) as proc:
                result = json.loads(proc.stdout.read())
                _, status, usage = os.wait4(proc.pid, 0)
                proc.returncode = os.waitstatus_to_exitcode(status)
                assert proc.returncode == 0 and result["state"] == "near_rules"
            times.append(1000 * (time.perf_counter() - t))
            rss.append(usage.ru_maxrss)
        print(
            json.dumps(
                dict(
                    size=size,
                    build=label,
                    runs=21,
                    median_ms=round(statistics.median(times), 2),
                    p95_ms=round(sorted(times)[19], 2),
                    max_rss_kib=max(rss),
                )
            )
        )
