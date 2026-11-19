"""Verify the frozen trial design. Does not run a model or the product."""
import hashlib
import json
import re
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PRIMARY = {
    "rule_omitted", "decision_reopened", "prior_resource", "work_repeated",
    "scope_overreach", "hearsay_verified",
}
FILES = ("memory.json", "opportunities.json", "hot.txt",
         "grading/key.json", "grading/examples.json")


def check(ok, message):
    if not ok:
        raise ValueError(message)


def load(name):
    def unique(pairs):
        obj = dict(pairs)
        check(len(obj) == len(pairs), f"{name}: duplicate key")
        return obj
    return json.loads((ROOT / name).read_text(encoding="utf-8"), object_pairs_hook=unique)


def index(rows, name):
    result = {r["id"]: r for r in rows}
    check(len(rows) == len(result), f"{name}: duplicate ID")
    return result


def validate():
    manifest = load("manifest.json")
    hashes = {f: hashlib.sha256((ROOT / f).read_bytes()).hexdigest() for f in FILES}
    check(manifest["sha256"] == hashes, "frozen artifact digest")
    tasks = index(load("opportunities.json"), "opportunities")
    keys = index(load("grading/key.json"), "grading key")
    memory = index(load("memory.json"), "memory")
    check(tasks.keys() == keys.keys() and len(tasks) >= 40, "unpaired opportunity")
    check(not tasks.keys() & memory.keys(), "ID collision")
    check(len({r["prompt"] for r in tasks.values()}) == len(tasks), "duplicate prompt")
    for key, record in memory.items():
        check(key == record["id"] and record["target"] and record["text"], key)
        kind = record["kind"]
        check(kind in {"rule", "decision", "fact", "experience"}, key)
        if kind == "rule":
            check(record["scene"] and record["action"] and "exceptions" in record, key)
        elif kind == "decision":
            check(record["state"] == "decided" and record["at"] and record["speech"], key)
        elif kind == "fact":
            check(record["certainty"] in {"reported", "verified"}, key)
            if record["certainty"] == "verified":
                evidence = memory[record["evidence"]]
                check(evidence["kind"] == "experience" and evidence["target"] == record["target"], key)
                check(record["at"] and record["method"], key)
        else:
            check(record["at"], key)

    by_type = {}
    by_lang = Counter()
    by_cohort = Counter()
    for key, task in tasks.items():
        grade = keys[key]
        check(set(task) == {"id", "language", "prompt"} and re.fullmatch(r"T\d{2}", key), key)
        check(set(grade) == {"id", "type", "cohort", "requirement", "memory", "action",
                             "target", "pass", "known_mistake"}, key)
        check(task["language"] in {"ja", "en"} and task["prompt"].strip(), key)
        check(all(grade[k].strip() for k in ("action", "target", "pass", "known_mistake")), key)
        check(grade["pass"] != grade["known_mistake"], key)
        typ = grade["type"]
        check(typ in PRIMARY | {"control", "outside"}, key)
        cohort = "primary" if typ in PRIMARY else ("control" if typ == "control" else "excluded")
        check(grade["cohort"] == cohort and bool(grade["memory"]) == (typ in PRIMARY), key)
        check(set(grade["memory"]) <= memory.keys(), key)
        check(grade["requirement"] in {"BR-13", "BR-15", "BR-16", "BR-17", "BR-18", "BR-19", "§4"}, key)
        check(all(memory[ref]["text"].casefold() not in task["prompt"].casefold()
                  for ref in grade["memory"]), f"{key}: prompt reproduces memory")
        by_type.setdefault(typ, Counter())[task["language"]] += 1
        by_lang[task["language"]] += 1
        by_cohort[cohort] += 1
    check(all(by_type[t] == {"ja": 3, "en": 3} for t in PRIMARY), "primary balance")
    check(by_type == manifest["by_type_language"] and by_lang == manifest["by_language"], "counts")
    check(by_cohort == manifest["by_cohort"], "cohort counts")

    arms = manifest["arms"]
    check(set(arms) == {"device", "no_device", "hot_only"}, "arm names")
    for name, arm in arms.items():
        check(arm == {"opportunities": "opportunities.json", "archive": "memory.json",
                      "hot": None if name == "no_device" else "hot.txt", "lookup": name == "device"},
              f"{name}: arm-dependent task, archive, grading leak or intervention mismatch")
    check(manifest["opportunities"] == len(tasks)
          and manifest["paired_trials"] == len(tasks) * len(arms), "planned denominator")
    hot = manifest["hot_record_ids"]
    check(len(set(hot)) == len(hot), "duplicate hot item")
    lines = []
    for ref in hot:
        r = memory[ref]
        check(r["kind"] == "rule", ref)
        lines.append(f"{r['scene']} / {r['target']} — {r['text']}\n")
    check("".join(lines).encode() == (ROOT / "hot.txt").read_bytes(), "hot input")
    examples = load("grading/examples.json")
    check(len({e["opportunity"] for e in examples}) == len(examples), "duplicate example")
    pairs = Counter((keys[e["opportunity"]]["type"], e["grade"]) for e in examples)
    check(pairs == {(t, g): 1 for t in PRIMARY for g in ("pass", "known_mistake")}, "worked examples")
    check(all(e["transcript"] and e["why"] for e in examples), "missing example rationale")
    print(f"{len(tasks)}機会・{len(tasks) * len(arms)}試行枠・{len(examples)}採点例を確認")


if __name__ == "__main__":
    validate()
