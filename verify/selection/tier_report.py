"""First-reply metrics for independent strict and short candidate tiers."""

from pathlib import Path
import json
import statistics
from study import DATA, lines


def report(split, data):
    keys = {q["id"]: q for q in lines(data) if split != "dev" or int(q["id"][1:]) % 2}
    for arm in ("core", "jev"):
        rows = lines(
            Path(__file__).with_name("results") / (split + "-tier-" + arm + ".jsonl")
        )
        assert keys.keys() == {r["id"] for r in rows} and len(keys) == len(rows)
        n = h = u = b = e = neg = zero = no_strict = 0
        for r in rows:
            q = keys[r["id"]]
            g = set(q["must"])
            f = set(q["must_not"])
            s = set(r["returned"])
            c = set(r["candidates"])
            assert s.isdisjoint(c) and r["bytes"] <= 4000
            assert r["reply_state"] == (
                "found"
                if s
                else "candidates_only"
                if c or r["remaining_candidates"]
                else "none"
            )
            n += len(g)
            h += len(s & g)
            u += len((s | c) & g)
            b += bool(s & f)
            e += s == g
            neg += not g
            zero += not g and r["reply_state"] == "none"
            no_strict += not g and not s
        print(
            f"| {split} | {arm} | {e}/{len(rows)} | {b}/{len(rows)} | {h}/{n} | {u}/{n} | {statistics.mean(r['bytes'] for r in rows):.1f} | {zero}/{neg} | {no_strict}/{neg} |"
        )


if __name__ == "__main__":
    print(
        "| Split | Method | Strict exact | Forbidden in strict | Strict required | Union required | Mean bytes | None correct | No confirmed on negatives |"
    )
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|")
    report("dev", DATA / "queries.jsonl")
    if (DATA / "holdout2.jsonl").exists():
        report("holdout2", DATA / "holdout2.jsonl")
