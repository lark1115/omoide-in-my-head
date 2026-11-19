"""Screen thresholds on odd IDs only. Validate the selected cut through the actual first-page port."""

import json
from pathlib import Path
from collections import defaultdict


def read(path):
    return [json.loads(l) for l in Path(path).read_text().splitlines()]


def sweep():
    rules = {r["id"]: r for r in read("tests/discrimination/rules.jsonl")}
    keys = {
        q["id"]: q
        for q in read("tests/discrimination/queries.jsonl")
        if int(q["id"][1:]) % 2
    }
    for arm in ("original", "trigger", "instruction"):
        rows = read(f"verify/selection/results/dev-{arm}.jsonl")
        if set(r["id"] for r in rows) != keys.keys():
            raise ValueError("expected full dev split")
        for threshold in (0.3, 0.35, 0.4, 0.45, 0.5, 0.55, 0.6, 0.65, 0.75, 0.85, 0.9):
            for gap in (1, 0.2, 0.1):
                hit = required = bad = exact = none = empty = 0
                for r in rows:
                    q = keys[r["id"]]
                    must = set(q["must"])
                    ban = set(q["must_not"])
                    best = defaultdict(float)
                    group = lambda k: (rules[k]["action"], rules[k]["target"])
                    scores = r["scores"]
                    for k, v in scores.items():
                        best[group(k)] = max(best[group(k)], v)
                    ret = {
                        k
                        for k, v in scores.items()
                        if v >= threshold and best[group(k)] - v <= gap
                    }
                    hit += len(ret & must)
                    required += len(must)
                    bad += bool(ret & ban)
                    exact += ret == must
                    none += not must and not ret
                    empty += not must
                print(
                    json.dumps(
                        dict(
                            arm=arm,
                            threshold=threshold,
                            gap=gap,
                            required=[hit, required],
                            forbidden=bad,
                            exact=exact,
                            none=[none, empty],
                        )
                    )
                )


if __name__ == "__main__":
    sweep()
