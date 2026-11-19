"""Recompute counts from first-page observations and the frozen grading key."""

import json
import statistics
from pathlib import Path
from collections import Counter

ROOT = Path(__file__).resolve().parents[2]
RESULTS = Path(__file__).with_name("results")


def read(p):
    return [json.loads(l) for l in p.read_text().splitlines()]


def report():
    keys = {q["id"]: q for q in read(ROOT / "tests/discrimination/queries.jsonl")}
    table = [
        "| Split | First-page method | Required | Forbidden | Exact | None correct | Judge failures | Median / p95 ms | Input / output tokens with receipts |",
        "|---|---|---:|---:|---:|---:|---:|---|---|",
    ]
    for split, parity in [("DEV", 1), ("TEST", 0)]:
        data = [
            ("core baseline", ROOT / "verify/discrimination/results/core.jsonl"),
            (
                "Jev baseline (10 s)",
                ROOT / "verify/discrimination/results/jev-extended.jsonl",
            ),
        ]
        data.extend(
            [
                ("original fresh", RESULTS / "dev-original.jsonl"),
                ("trigger", RESULTS / "dev-trigger.jsonl"),
                ("instruction", RESULTS / "dev-instruction.jsonl"),
                ("instruction + loose filter", RESULTS / "dev-loose.jsonl"),
                ("instruction + structured filter", RESULTS / "dev-structured.jsonl"),
                ("core selected", RESULTS / "dev-final-core.jsonl"),
                ("original + structured filter, 0.65", RESULTS / "dev-selected.jsonl"),
                ("Jev selected, 0.60", RESULTS / "dev-picked.jsonl"),
                ("Jev selected, 1 s", RESULTS / "dev-default-time.jsonl"),
            ]
            if parity
            else [
                ("core selected", RESULTS / "test-core.jsonl"),
                ("Jev selected, 0.60", RESULTS / "test-jev.jsonl"),
            ]
        )
        for name, path in data:
            rows = [r for r in read(path) if int(r["id"][1:]) % 2 == parity]
            assert len(rows) == len({r["id"] for r in rows}) == 76
            st = Counter()
            for r in rows:
                q = keys[r["id"]]
                G = set(q["must"])
                F = set(q["must_not"])
                R = set(r["returned"])
                st.update(
                    dict(
                        hit=len(R & G),
                        must=len(G),
                        bad=bool(R & F),
                        exact=R == G,
                        empty=not G,
                        none=not G and not R,
                        failed=r.get("state", r.get("judge_state")) == "failed",
                    )
                )
            us = [r["usage"] for r in rows if r.get("usage")]
            ms = sorted(r["ms"] for r in rows)
            table.append(
                f"| {split} | {name} | {st['hit']}/{st['must']} | {st['bad']}/76 | {st['exact']}/76 | {st['none']}/{st['empty']} | {st['failed']} | {statistics.median(ms):.0f} / {ms[72]:.0f} | {sum(u['input_tokens'] for u in us)} / {sum(u['output_tokens'] for u in us)} ({len(us)} receipts) |"
            )
    (RESULTS / "summary.md").write_text("\n".join(table) + "\n")


if __name__ == "__main__":
    report()
