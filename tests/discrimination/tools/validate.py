"""Static checks for the shared-store discrimination corpus."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import unicodedata

ROOT = Path(__file__).resolve().parents[1]
VOCAB = re.compile(r"[a-z0-9]+|[ぁ-ゖァ-ヺー一-鿿々]")


def require(ok, message):
    if not ok:
        raise ValueError(message)


def parse(line):
    def unique(pairs):
        result = dict(pairs)
        require(len(result) == len(pairs), "duplicate JSON key")
        return result
    return json.loads(line, object_pairs_hook=unique)


def rows(name):
    return [parse(line) for line in (ROOT / name).read_text(encoding="utf-8").splitlines()]


def tokens(text):
    return VOCAB.findall(unicodedata.normalize("NFKC", text).lower())


def contains(sequence, phrase):
    return any(sequence[i:i + len(phrase)] == phrase for i in range(len(sequence)))


def level(query, rules):
    words = tokens(" ".join(query["input"].values()))
    reference = query["must"] or query["must_not"]
    partial = False
    for key in reference:
        rule = rules[key]
        if contains(words, tokens(rule["key_phrase"])):
            return "shared"
        vocab = tokens(" ".join(rule[k] for k in ("text", "scene", "action")))
        if set(words).intersection(vocab):
            partial = True
    return "partial" if partial else "zero"


def summarize(rs, qs):
    return {
        "format": 1,
        "rules": len(rs),
        "queries": len(qs),
        "variants": ["without_judge", "with_judge"],
        "by_overlap": dict(sorted(Counter(q["level"] for q in qs).items())),
        "by_required_count": dict(sorted(Counter(str(len(q["must"])) for q in qs).items())),
        "none_by_overlap": dict(sorted(Counter(q["level"] for q in qs if not q["must"]).items())),
        "action_target_only": sum(set(q["input"]) == {"action", "target"} for q in qs),
        "sha256": {f: hashlib.sha256((ROOT / f).read_bytes()).hexdigest()
                   for f in ("rules.jsonl", "queries.jsonl")},
    }


def validate():
    rs = rows("rules.jsonl")
    qs = rows("queries.jsonl")
    ids = [item["id"] for item in rs + qs]
    require(len(set(ids)) == len(ids), "duplicate id")
    require(60 <= len(rs) <= 80 and len(qs) >= 80, "corpus size")
    rules = {r["id"]: r for r in rs}
    for r in rs:
        require(all(r[k] for k in ("domain", "scene", "action", "text", "key_phrase")), r["id"])
        phrase = tokens(r["key_phrase"])
        require(phrase and any(contains(tokens(r[k]), phrase) for k in ("text", "scene", "action")),
                f"{r['id']}: key phrase not in record")
    seen = set()
    for q in qs:
        tag = q["id"]
        inputs = q["input"]
        require(set(inputs) in ({"scene"}, {"action", "target"}), tag)
        require(all(isinstance(s, str) and s.strip() for s in inputs.values()), tag)
        canonical = tuple(sorted(inputs.items()))
        require(canonical not in seen, f"{tag}: repeated input")
        seen.add(canonical)
        refs = q["must"] + q["must_not"]
        require(q["must_not"] and set(refs) <= rules.keys() and len(set(refs)) == len(refs), tag)
        require(q["level"] == level(q, rules), f"{tag}: incorrect overlap")
        require(bool(q["reason"].strip()) and len(q["reason"].splitlines()) == 1, tag)
    for side in ("must", "must_not"):
        require({k for q in qs for k in q[side]} == set(rules), f"missing rule on {side}")
    require({len(q["must"]) for q in qs} >= {0, 1, 2, 3}, "missing cardinality")
    require({q["level"] for q in qs if not q["must"]} == {"shared", "partial", "zero"}, "none controls")
    require({q["level"] for q in qs if q["must"]} == {"shared", "partial", "zero"}, "positive controls")
    require(any("target" in q["input"] and not q["must"] for q in qs), "missing structured negative")
    require(any("target" in q["input"] and q["must"] for q in qs), "missing structured positive")
    require(summarize(rs, qs) == parse((ROOT / "manifest.json").read_text()), "manifest mismatch")
    print(f"共有規則{len(rs)}件、照会{len(qs)}件の参照・難度・件数を確認")


if __name__ == "__main__":
    validate()
