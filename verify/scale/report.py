"""Rebuild delivery tables from the first-page evidence files. Empty support sets are N/A."""

import json
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).parent


def report():
    lines = [
        "| Split | Store | Method | Confirmed / supported inputs | Both tiers / supported inputs | Inputs missing any support | Prompt only | At least one observed lookup | At least one input per opportunity |",
        "|---|---:|---|---:|---:|---:|---:|---:|---:|",
    ]
    kinds = [
        "| Split | Store | Method | Kind | Confirmed | Both tiers | Absent | Support instances |",
        "|---|---:|---|---|---:|---:|---:|---:|",
    ]
    for split in ["dev", "test"]:
        for size in [48, 500, 3048]:
            for arm in ["baseline", "core"]:
                rows = [
                    r
                    for r in map(
                        json.loads,
                        (HERE / "results" / f"{arm}-{split}.jsonl")
                        .read_text()
                        .splitlines(),
                    )
                    if r["size"] == size
                ]
                assert len({r["op"] for r in rows}) == 24
                support = [r for r in rows if r["support"]]

                def delivered(row, strict=False):
                    return all(
                        x["tier"] == 1 if strict else bool(x["tier"])
                        for x in row["support"]
                    )

                def opportunity_count(batch):
                    denom = {r["op"] for r in batch}
                    return (
                        f"{len({r['op'] for r in batch if delivered(r)})}/{len(denom)}"
                    )

                strict = sum(delivered(r, True) for r in support)
                both = sum(delivered(r) for r in support)
                lines.append(
                    f"| {split} | {size} | {arm} | {strict}/{len(support)} | {both}/{len(support)} | {len(support) - both} | {opportunity_count([r for r in support if r['input'] == 0])} | {opportunity_count([r for r in support if r['input']])} | {opportunity_count(support)} |"
                )
                grouped = defaultdict(list)
                for r in rows:
                    for x in r["support"]:
                        grouped[x["kind"]].append(x["tier"])
                for k, tiers in sorted(grouped.items()):
                    kinds.append(
                        f"| {split} | {size} | {arm} | {k} | {tiers.count(1)} | {sum(bool(x) for x in tiers)} | {tiers.count(0)} | {len(tiers)} |"
                    )
    (HERE / "results" / "summary.md").write_text("\n".join(lines + [""] + kinds) + "\n")


if __name__ == "__main__":
    report()
