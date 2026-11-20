"""DEV-only screen on labelled pairs and frozen Japanese cases, with and without a common target."""

import itertools, json, re
from calibrate import components, features, japanese, terms
from measure import labels, read_lines


def units(text):
    return {w for w in terms(text, True) if w.isascii()} | (
        set(re.findall("[㐀-鿿]", text)) - set("時前後際中場合")
    )


def prepare(a, b):
    f = components(features(a, True), features(b, True))
    named = [units(r["target"]) for r in (a, b) if r.get("target")]
    sibling = (
        f[6]
        and a.get("target") != b.get("target")
        and bool(
            set.intersection(units(a["scene"]), units(b["scene"]), *named)
            - units(a["action"])
        )
    )
    witnessed = bool(
        a.get("target")
        and a["target"] == b.get("target")
        and set.intersection(units(a["scene"]), units(b["scene"]), units(a["target"]))
        - units(a["action"])
    )
    return f, sibling, witnessed


def strength(pair, threshold, prior, scene_bonus, use_family):
    (
        (hits, anchors, cov, triggers, exact_scene, exact_body, act, target),
        family,
        witnessed,
    ) = pair
    s = (
        cov
        + scene_bonus * min(2, triggers)
        + prior * act
        + prior * (act and witnessed)
        + 100 * (exact_scene or exact_body)
        if anchors
        else 0
    )
    if use_family and family:
        s = max(s, threshold)
    return s if s >= threshold else 0


def main():
    rows = read_lines("rules.jsonl")
    lab = labels(rows)
    jp = [
        (prepare(dict(a, target=t), dict(b, target=t)), v)
        for a, b, v in japanese()
        for t in (None, "共同作業")
    ]
    pairs = {
        (a["id"], b["id"]): prepare(a, b) for a, b in itertools.combinations(rows, 2)
    }
    for t, p, b, f in itertools.product(
        (12, 15, 18, 20, 22, 25, 28, 30, 32, 35, 40),
        (0, 2, 3, 4, 5, 6, 8),
        (0, 4, 8),
        (False, True),
    ):
        args = (t, p, b, f)
        ja_good = sum(bool(strength(pair, *args)) for pair, v in jp if v)
        ja_bad = sum(bool(strength(pair, *args)) for pair, v in jp if not v)
        results = {
            k: sum(
                bool(strength(pair, *args))
                for ids, pair in pairs.items()
                if lab.get(ids) == k and int(ids[0][1:]) % 2
            )
            for k in ("positive", "negative")
        }
        print(
            json.dumps(
                dict(
                    threshold=t,
                    prior=p,
                    trigger=b,
                    family=f,
                    japanese=[ja_good, ja_bad],
                    **results,
                )
            )
        )


if __name__ == "__main__":
    main()


def policy_report():
    rows = read_lines("rules.jsonl")
    lab = labels(rows)
    prepared = {
        (a["id"], b["id"]): prepare(a, b) for a, b in itertools.combinations(rows, 2)
    }
    jay = [
        (prepare(dict(a, target=t), dict(b, target=t)), v)
        for a, b, v in japanese()
        for t in (None, "共同作業")
    ]
    for t, p, b, f, cap in [
        (15, 0, 0, False, 72),
        (25, 8, 8, False, 72),
        (25, 8, 8, True, 72),
        (25, 8, 8, True, 12),
        (25, 8, 8, True, 9),
        (25, 8, 8, True, 8),
        (25, 8, 8, True, 6),
        (25, 8, 8, True, 4),
    ]:
        results = {}
        for a in rows:
            ranked = []
            for other in rows:
                if a == other:
                    continue
                pair = tuple(sorted((a["id"], other["id"])))
                value = strength(prepared[pair], t, p, b, f)
                if value:
                    ranked.append((-value, other["scene"], other["text"], other["id"]))
            results[a["id"]] = [row[-1] for row in sorted(ranked)[:cap]]
        counts = [0, 0]
        for (left, right), label in lab.items():
            if int(left[1:]) % 2:
                counts[label == "negative"] += int(right in results[left]) + int(
                    left in results[right]
                )
        sizes = [len(results[a["id"]]) for a in rows if int(a["id"][1:]) % 2]
        print(
            json.dumps(
                dict(
                    floor=t,
                    prior=p,
                    trigger=b,
                    family=f,
                    cap=cap,
                    dev=counts,
                    mean=sum(sizes) / len(sizes),
                    maximum=max(sizes),
                    ja_pos=sum(bool(strength(x, t, p, b, f)) for x, v in jay if v),
                    ja_fp=sum(bool(strength(x, t, p, b, f)) for x, v in jay if not v),
                )
            )
        )
