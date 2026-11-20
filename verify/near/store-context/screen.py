"""Pre-registration dataset calibration. Use only original odd lower-ID labels."""

import itertools, json
from collections import Counter
from dev import observations, scene_terms
from sweep import units


def main():
    data = list(observations())
    rows = {a["id"]: a for a, *_ in data}.values()
    all_units = Counter(u for r in rows for u in units(r["text"] + " " + r["scene"]))
    for slope, minimum, scene_floor in itertools.product(
        (0, 200, 300, 400), (40, 60, 75), (0, 10, 20, 30)
    ):
        counts = Counter()
        for a, b, label, old, anchor, cross, triggers, exact, sibling, den, df in data:
            if not old or not label:
                continue
            weight = lambda freq: max(0, 100 - slope * max(0, freq - 1) // 70)
            named = [units(r["target"]) for r in (a, b) if r.get("target")]
            subjects = set.intersection(
                units(a["scene"]), units(b["scene"]), *named
            ) - units(a["action"])
            rare_sibling = sibling and any(
                weight(all_units[u] - 1) >= minimum for u in subjects
            )
            evidence = triggers & anchor
            sig = sum(weight(df[w]) for w in evidence)
            strong = any(weight(df[w]) >= minimum for w in evidence)
            if (
                exact
                or rare_sibling
                or (
                    strong
                    and sig
                    >= scene_floor * min(len(scene_terms(a)), len(scene_terms(b)))
                    and sum(weight(df[w]) for w in anchor) >= 4 * den
                )
            ):
                counts[label] += 1
        print(
            json.dumps(
                dict(
                    slope=slope,
                    minimum=minimum,
                    scene_floor=scene_floor,
                    positive=counts["positive"],
                    negative=counts["negative"],
                )
            )
        )


if __name__ == "__main__":
    main()
