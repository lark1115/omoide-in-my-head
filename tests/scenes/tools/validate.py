"""Validate the portable scene corpus without running omoide."""
import json
import re
import unicodedata
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ORDER = [1, 2, 6, 7, 10, 11, 3, 4, 5, 8, 9, 12]
CLAUSE = re.compile(r"(REQ-[A-Z]+-1)((?:\([a-z]\))+)")


def require(ok, message):
    if not ok:
        raise ValueError(message)


def read(path):
    def unique_keys(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, f"{path.name}: repeated JSON key {key}")
            result[key] = value
        return result
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_keys)


def tokens(text):
    # Diagnostic vocabulary only: unlike the product it needs no external tokenizer.
    text = unicodedata.normalize("NFKC", text).lower()
    return set(re.findall(r"[a-z0-9]+|[ぁ-ゖァ-ヺー一-鿿々]", text))


def validate():
    manifest = read(ROOT / "manifest.json")
    entries = manifest["scenes"]
    require(manifest["version"] == 1, "unsupported format")
    require([e["scene"] for e in entries] == [f"S{n}" for n in ORDER], "scene order")
    paths = [e["file"] for e in entries]
    require(len(set(paths)) == len(paths), "duplicate manifest filename")
    require(set(paths) == {p.name for p in ROOT.glob("*-s*.json")}, "manifest files")
    clauses = {
        p.stem: set(re.findall(r"^- \(([a-z])\)", p.read_text(), re.M))
        for p in (ROOT.parents[1] / "req").glob("REQ-*.md")
    }
    ids = set()

    def declare(key):
        require(key not in ids, f"duplicate id: {key}")
        ids.add(key)

    total = 0
    for entry in entries:
        data = read(ROOT / entry["file"])
        scene = data["scene"]
        require(scene == entry["scene"], "scene mismatch")
        records = {}
        for record in data["records"]:
            key = record["id"]
            declare(key)
            for ref in (record.get("source"), record.get("evidence"),
                        record.get("relation", {}).get("to")):
                require(ref is None or ref in records, f"{key}: unknown prior record {ref}")
            records[key] = record

        prepared = {}
        for name, steps in data.get("preparations", {}).items():
            produced = set()
            for step in steps:
                if step["op"] == "prepare_revision":
                    require(step["base"] in records and step["source"] in records, name)
                    declare(step["id"])
                    produced.add(step["id"])
                if step["op"] == "seed_experiences":
                    for index in range(step["count"]):
                        key = f"{step['id_prefix']}-{index}"
                        declare(key)
                        produced.add(key)
            prepared[name] = produced

        cases = data["cases"]
        require(len(cases) == entry["cases"], f"{scene}: manifest count")
        levels = Counter(c["level"] for c in cases if "level" in c)
        require(dict(levels) == entry["levels"], f"{scene}: level counts")
        require({(c["language"], c["polarity"]) for c in cases}
                == {(l, p) for l in ("ja", "en") for p in ("positive", "negative")},
                f"{scene}: language or polarity")
        for c in cases:
            key = c["id"]
            declare(key)
            known = set(records)
            if "before_query" in c:
                known.update(prepared[c["before_query"]])
            expected = c["expect"]
            must, excluded = expected["must"], expected["must_not"]
            require(len(set(must + excluded)) == len(must + excluded), f"{key}: repeated expectation")
            require(set(must + excluded) <= known, f"{key}: unknown expected ID")
            require(set(expected["fields"]) <= set(must), f"{key}: field on unexpected record")
            for fields in expected["fields"].values():
                for role in ("source", "successor", "evidence"):
                    require(role not in fields or fields[role] in known, f"{key}: {role}")
            query = c["query"]
            require(query.get("record") is None or query["record"] in known, key)
            require(must or excluded or expected["meta"], f"{key}: empty oracle")
            reason = c["reason"]
            refs = CLAUSE.findall(reason)
            require(refs and len(reason.splitlines()) == 1, f"{key}: missing clause or multiline reason")
            for req, letters in refs:
                require(set(re.findall("[a-z]", letters)) <= clauses.get(req, set()), f"{key}: {req}{letters}")
            if scene == "S1":
                require(c.get("level") in {"shared", "partial", "zero"}, key)
                rule = next(r for r in records.values() if r["kind"] == "rule")
                words = tokens(" ".join(rule[k] for k in ("text", "scene", "action")))
                overlap = words & tokens(query["scene"])
                require(bool(overlap) == (c["level"] != "zero"), f"{key}: overlap {overlap}")
        if scene == "S1":
            for lang in ("ja", "en"):
                require({c["level"] for c in cases if c["language"] == lang and c["polarity"] == "positive"}
                        == {"shared", "partial", "zero"}, f"S1: {lang} paraphrases")
        total += len(cases)
    require(total == manifest["total_cases"], "manifest total")
    print(f"{len(entries)}場面・{total}件を検証しました。")


if __name__ == "__main__":
    validate()
