"""Evaluate odd lower-ID pairs only. Confirm on reserved pairs and the new fixture after freeze."""

import sys, itertools, json, re
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from sweep import prepare, strength
from calibrate import features, terms
from measure import read_lines, labels


def scene_terms(r):
    s = re.sub(r"(?:場面|場合|とき|時|際)$", "", r["scene"])
    return terms(s, True) - terms((r.get("target") or "") + " " + r["action"], True)


def observations():
    rows = read_lines("rules.jsonl")
    lab = labels(rows)
    docs = {r["id"]: features(r, True)[0] for r in rows}
    df = Counter(w for doc in docs.values() for w in doc)
    for a in rows:
        for b in rows:
            if a == b:
                continue
            pair = tuple(sorted([a["id"], b["id"]]))
            old = strength(prepare(a, b), 25, 8, 8, True)
            da, db = docs[a["id"]], docs[b["id"]]
            sa, sb = scene_terms(a), scene_terms(b)
            ia = features(a, True)[2]
            ib = features(b, True)[2]
            anchor = da & db - (ia | ib)
            cross = (sa & db) | (sb & da)
            exact = bool(sa and sa == sb) or bool(a["text"] == b["text"])
            sibling = prepare(a, b)[1]
            yield (
                a,
                b,
                lab.get(pair) if int(pair[0][1:]) % 2 else None,
                old,
                anchor,
                cross,
                sa & sb,
                exact,
                sibling,
                min(len(da), len(db)),
                {w: df[w] - 1 for w in da & db},
            )


def main():
    observations_list = list(observations())
    for slope, floor, gate, minimum in itertools.product(
        (0, 100, 200, 300, 400),
        (0, 2, 4, 6, 8, 10, 12),
        ("none", "cross", "scene"),
        (1, 40, 50, 60, 75),
    ):
        counts = Counter()
        for (
            a,
            b,
            label,
            old,
            anchor,
            cross,
            shared_scene,
            exact,
            sibling,
            den,
            df,
        ) in observations_list:
            if not label or not old:
                continue
            weight = lambda w: max(0, 100 - slope * max(0, df[w] - 1) // 70)
            signals = {
                "none": anchor,
                "cross": cross & anchor,
                "scene": shared_scene & anchor,
            }[gate]
            accept = (
                exact
                or sibling
                or (
                    any(weight(w) >= minimum for w in signals)
                    and sum(weight(w) for w in anchor) >= floor * den
                )
            )
            if accept:
                counts[label] += 1
        print(
            json.dumps(
                dict(
                    slope=slope,
                    floor=floor,
                    gate=gate,
                    minimum=minimum,
                    positive=counts["positive"],
                    negative=counts["negative"],
                )
            )
        )


if __name__ == "__main__":
    main()
