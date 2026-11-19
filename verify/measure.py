"""Linux benchmark of the release CLI. Seed synthetic records outside the timed region."""
import argparse
import json
import platform
import sqlite3
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("binary", type=Path)
parser.add_argument("--dataset",choices=["rules","mixed"],default="rules")
parser.add_argument("--runs", type=int, default=31)
args = parser.parse_args()
binary = args.binary.resolve()
queries = {
    "rare": ["--scene", "zircon"],
    "frequent": ["--scene", "紙"],
    "action": ["--action", "check-0042"],
    "zero": ["--scene", "quartz"],
}

def seed(path):
    subprocess.run([binary, "--db", path, "init"], check=True, stdout=subprocess.DEVNULL)
    with sqlite3.connect(path) as db:
        db.execute("INSERT INTO utterances VALUES('source','紙を調べる','user',1,NULL)")
        for i in range(10000):
            identifier = f"record-{i:04d}"
            action = f"check-{i:04d}"
            word = "zircon" if i == 0 else "packing"
            if args.dataset=="mixed":
                kind="fact" if i%2==0 else "experience"
                details={"certainty":"reported"} if kind=="fact" else {}
                source="source" if kind=="fact" else None
                body=f"紙箱 {word}"
                db.execute("INSERT INTO memories(rowid,id,record_id,kind,body,source,created,details) VALUES(?,?,?,?,?,?,?,?)",
                           (i+1,identifier,identifier,kind,body,source,i,json.dumps(details)))
                db.execute("INSERT INTO evidence_index(rowid,words,singles,subject) VALUES(?,?,?,'')",
                           (i+1,body,"紙 箱"))
                continue
            db.execute("""INSERT INTO rules(rowid,id,version,state,body,scene,aliases,actions,source,created)
                          VALUES(?,?,?,'current','紙箱',?,'[]',?,'source',?)""",
                       (i + 1, identifier, f"v-{i}", word, json.dumps([action]), i))
            db.execute("INSERT INTO action_links VALUES(?,?)", (action, f"v-{i}"))
            db.execute("INSERT INTO rule_index(rowid,words,singles) VALUES(?,?,?)",
                       (i + 1, f"紙箱 {word}", "紙 箱"))

def sample(path, params):
    usage_path = path.with_suffix(".rss")
    start = time.perf_counter_ns()
    output = subprocess.check_output(["/usr/bin/time", "-o", usage_path, "-f", "%M",
                                      binary, "--db", path, "recall", *params])
    elapsed_ms = (time.perf_counter_ns() - start) / 1e6
    result = json.loads(output)
    return elapsed_ms, int(usage_path.read_text()), result["candidate_count"], result["candidates_truncated"]

with tempfile.TemporaryDirectory() as directory:
    path = Path(directory) / "sample.db"
    seed(path)
    print(platform.platform(), "binary bytes", binary.stat().st_size)
    for label, params in queries.items():
        if args.dataset=="mixed" and label=="action":
            continue
        first = sample(path, params)
        samples = [sample(path, params) for _ in range(args.runs)]
        times = sorted(row[0] for row in samples)
        assert max(row[1] for row in [first, *samples]) * 1024 <= 50_000_000
        assert binary.stat().st_size <= 20_000_000
        assert times[(95 * len(times) + 99) // 100 - 1] <= 100
        print(label, "first_ms", round(first[0], 3), "median_ms", round(statistics.median(times), 3),
              "p95_ms", round(times[(95*len(times)+99)//100-1], 3),
              "peak_KiB", max(row[1] for row in [first, *samples]),
              "candidates", first[2], "truncated", first[3])
