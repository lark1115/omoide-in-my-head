"""Independent DEV screening of bounded lexical policies (no TEST outcomes used)."""

import csv
import itertools
import json
import re
import unicodedata
from measure import ROOT, labels, read_lines

STOP = set(
    "a an the to of for in on at as and or by is are it with from then when".split()
)
JP = r"[ぁ-ゖゝ-ゟァ-ヺー-ヿ㐀-鿿々〇]"


def words(s):
    s = unicodedata.normalize("NFKC", s).lower()
    return {
        w
        for m in re.finditer(f"({JP}+)|([a-z0-9]+)", s)
        for w in (
            [m[0]]
            if not m[1] or len(m[0]) == 1
            else [m[0][i : i + 2] for i in range(len(m[0]) - 1)]
        )
    }


def terms(s, clean):
    return {
        w
        for w in words(s)
        if not clean or w not in STOP and not all("ぁ" <= c <= "ゟ" for c in w)
    }


def features(r, clean):
    scene = terms(r["scene"], clean)
    body = terms(r["text"], clean)
    identity = terms((r.get("target") or "") + " " + r["action"], clean)
    return (
        body | scene,
        scene - identity,
        identity,
        body,
        r["action"],
        r.get("target"),
    )


def components(x, y):
    a, sa, ia, ba, act, t = x
    b, sb, ib, bb, other, u = y
    h = a & b
    anchor = h - (ia | ib)
    trigger = sa & sb
    cov = 100 * len(h) // max(1, min(len(a), len(b)))
    return (
        len(h),
        len(anchor),
        cov,
        len(trigger),
        bool(sa and sa == sb),
        bool(ba and ba == bb),
        act == other,
        bool(t and t == u),
    )


def score(f, floor, prior, scene_bonus):
    hits, anchors, coverage, triggers, same_scene, same_body, action, target = f
    if not anchors:
        return 0
    strength = (
        coverage
        + prior * action
        + prior * (action and target)
        + scene_bonus * min(triggers, 2)
    )
    if same_scene or same_body:
        strength += 100
    return strength if strength >= floor else 0


def japanese():
    def read(file):
        return list(csv.DictReader((ROOT / "tests/near" / file).open(), delimiter="\t"))

    base = [dict(r, action="report") for r in read("unrelated.tsv")]
    return [(a, b, False) for a, b in itertools.combinations(base, 2)] + [
        (base[int(r["base"]) - 1], dict(r, action="other"), True)
        for r in read("related.tsv")
    ]


if __name__ == "__main__":
    rows = read_lines("rules.jsonl")
    lab = labels(rows)
    jp = japanese()
    for clean in (False, True):
        feats = {r["id"]: features(r, clean) for r in rows}
        pairs = {
            p: components(feats[p[0]], feats[p[1]])
            for p in itertools.combinations(feats, 2)
        }
        ja = [
            (components(features(a, clean), features(b, clean)), label)
            for a, b, label in jp
        ]
        for floor, prior, bonus in itertools.product(
            (10, 15, 20, 25, 30, 35, 40), (0, 4, 8), (0, 4, 8)
        ):
            p = sum(bool(score(f, floor, prior, bonus)) for f, v in ja if v)
            n = sum(bool(score(f, floor, prior, bonus)) for f, v in ja if not v)
            counts = [0, 0]
            for pair, kind in lab.items():
                if int(pair[0][1:]) % 2 and score(pairs[pair], floor, prior, bonus):
                    counts[kind == "negative"] += 1
            print(
                json.dumps(
                    dict(
                        clean=clean,
                        floor=floor,
                        prior=prior,
                        scene=bonus,
                        ja_pos=p,
                        ja_fp=n,
                        dev_pos=counts[0],
                        dev_fp=counts[1],
                    )
                )
            )
