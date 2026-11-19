"""Split-enforcing study driver. Each case exercises the real first recall page."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unicodedata
import re
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "verify/discrimination"))
from run import seed, lines, DATA


def words(text):
    # The discrimination fixture contains Latin words and Japanese kana/kanji.
    out = set()
    for jp, latin in re.findall(
        r"([ぁ-ゟァ-ヿ㐀-鿿々]+)|([^\W_]+)", unicodedata.normalize("NFKC", text).lower()
    ):
        if jp:
            out.update(
                [jp] if len(jp) == 1 else [jp[i : i + 2] for i in range(len(jp) - 1)]
            )
        elif latin:
            out.add(latin)
    return out


def judge():
    request = json.load(sys.stdin)
    original = request["candidates"]
    policy = os.environ.get("STUDY_FILTER")
    if policy:
        catalog = json.loads(Path(os.environ["STUDY_CATALOG"]).read_text())
        query = request["query"]

        def keep(c):
            r = catalog[c["version_id"]]
            if query.get("action") and query.get("target") and not query.get("scene"):
                return r["action"] == query["action"]
            if policy == "loose" and query.get("scene"):
                ts = words(query["scene"])
                doc = words(c["text"] + " " + (c.get("scene") or ""))
                return bool(ts & doc) and 1000 * len(ts & doc) >= 95 * len(ts)
            return True

        request["candidates"] = [c for c in original if keep(c)]
    p = subprocess.run(
        [os.environ["STUDY_ADAPTER"], "--report-usage"],
        input=json.dumps(request),
        text=True,
        capture_output=True,
    )
    if p.returncode:
        return 1
    result = json.loads(p.stdout)
    Path(os.environ["STUDY_CAPTURE"]).write_text(
        json.dumps(
            {
                "scores": result["answers"],
                "usage": json.loads(p.stderr)["usage"] if p.stderr else None,
            }
        )
    )
    if policy:
        admitted = {c["version_id"] for c in request["candidates"]}
        result["answers"].extend(
            {"version_id": c["version_id"], "score": 0}
            for c in original
            if c["version_id"] not in admitted
        )
    print(json.dumps(result))
    return 0


def run(a):
    if a.split == "holdout2":
        queries = lines(DATA / "holdout2.jsonl")
    else:
        queries = [
            q
            for q in lines(DATA / "queries.jsonl")
            if int(q["id"][1:]) % 2 == (a.split == "dev")
        ]
    if a.split != "dev":
        if not a.freeze:
            raise ValueError("provide the frozen git commit")
        subprocess.run(
            ["git", "diff", "--exit-code", a.freeze, "--", "crates"],
            check=True,
            stdout=subprocess.DEVNULL,
        )
    with tempfile.TemporaryDirectory() as td, a.output.open("x") as out:
        root = Path(td)
        db = root / "archive.db"
        mapping = seed(a.binary, db)
        catalog = {r["id"]: r for r in lines(DATA / "rules.jsonl")}
        (root / "catalog").write_text(
            json.dumps({v: catalog[k] for v, k in mapping.items()})
        )
        for q in queries:
            args = [a.binary, "--db", db, "recall"]
            for k, v in q["input"].items():
                args.extend(["--" + k, v])
            env = {k: v for k, v in os.environ.items() if not k.startswith("OMOIDE_")}
            if a.prefilter:
                env.update(
                    STUDY_FILTER=a.prefilter, STUDY_CATALOG=str(root / "catalog")
                )
            capture = root / q["id"]
            if a.adapter:
                env.update(
                    STUDY_ADAPTER=str(a.adapter.resolve()),
                    STUDY_CAPTURE=str(capture),
                    JEV_TIMEOUT_MS=str(a.adapter_ms),
                )
                args += [
                    "--judge",
                    sys.executable,
                    "--judge-arg",
                    str(Path(__file__).resolve()),
                    "--judge-arg",
                    "judge",
                    "--timeout-ms",
                    str(a.port_ms),
                    "--threshold",
                    str(a.threshold),
                ]
            start = time.perf_counter()
            raw_reply = subprocess.check_output(args, env=env)
            res = json.loads(raw_reply)
            row = dict(
                id=q["id"],
                ms=round((time.perf_counter() - start) * 1000, 2),
                state=res["judge_state"],
                returned=[mapping[r["version_id"]] for r in res["results"]],
            )
            if "candidates" in res:
                row.update(
                    candidates=[mapping[r["version_id"]] for r in res["candidates"]],
                    remaining_candidates=res["candidates_remaining"],
                    reply_state=res["state"],
                    bytes=len(raw_reply),
                )
            if capture.exists():
                raw = json.loads(capture.read_text())
                row.update(
                    scores={
                        mapping[s["version_id"]]: s["score"] for s in raw["scores"]
                    },
                    usage=raw["usage"],
                )
            print(json.dumps(row), file=out, flush=True)


if __name__ == "__main__":
    if sys.argv[1:] == ["judge"]:
        sys.exit(judge())
    p = argparse.ArgumentParser()
    p.add_argument("split", choices=["dev", "test", "holdout2"])
    p.add_argument("binary", type=Path)
    p.add_argument("output", type=Path)
    p.add_argument("--adapter", type=Path)
    p.add_argument("--threshold", type=float, default=0.5)
    p.add_argument("--port-ms", type=int, default=10000)
    p.add_argument("--adapter-ms", type=int, default=9000)
    p.add_argument("--freeze")
    p.add_argument("--prefilter", choices=["structured", "loose"])
    run(p.parse_args())
