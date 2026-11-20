"""Check scope for all pairs and ledger search witnesses; no product or model runs."""
import argparse
import csv
from collections import Counter
import hashlib
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from generate import ROOT, SEED, archive_bytes, build, handle, hot_records, originals
# Import the unchanged budget and pagination contract, not the previous generator.
sys.path.insert(0, str(ROOT.parent / "trials-large" / "tools"))
from ledger import BUDGET, PAGE_BYTES, PAGE_ROWS, Ledger

TYPES = {"rule_omitted", "decision_reopened", "prior_resource", "work_repeated",
         "scope_overreach", "hearsay_verified"}
ARMS = ["no_device", "hot_only", "device_core", "device_jev"]


def check(condition, message):
    if not condition:
        raise ValueError(message)


def read(name):
    return json.loads((ROOT / name).read_text(encoding="utf-8"))


def sha(data):
    return hashlib.sha256(data).hexdigest()


def keyed(rows):
    result = {r["id"]:r for r in rows}
    check(len(result) == len(rows), "duplicate ID")
    return result


def safe(row, subjects, all_rows, base_ids):
    if row["target"] not in subjects:
        return True
    if row["kind"] == "rule":
        return (row.get("condition") == "scale-model-class"
                and row.get("exceptions") == ["ordinary workshop work"]
                and row["scene"] in row["text"]
                and any(s in row["text"] for s in ("工房の実作業は対象外", "Excludes ordinary workshop work")))
    if row["kind"] == "experience":
        date = row.get("period", "")
        return (date.startswith("2042-") and row.get("at", "").startswith(date)
                and date in row["text"] and any(s in row["text"] for s in ("旧ロットに限り", "closed lot dated")))
    if row["kind"] == "decision":
        seen = set()
        while row.get("state") == "superseded" and row["id"] not in seen:
            seen.add(row["id"])
            nxt = all_rows[row["successor"]]
            if nxt["target"] != row["target"] or nxt["at"] <= row["at"]:
                return False
            row = nxt
        return row["id"] in base_ids and row.get("state") == "decided"
    return False


def inspect():
    catalog = build()
    rows = [r for _,_,r in catalog]
    registry = keyed(rows)
    base = keyed(originals())
    base_ids = {handle(k) for k in base}
    extras = [r for r in rows if r["id"] not in base_ids]
    check(len(base)==48 and len(extras)==3000, "archive size")
    check(len({r["text"] for r in extras}) == len(extras), "duplicate distractor")
    for key, r in base.items():
        expected = dict(r,id=handle(key))
        expected.pop("language")
        if "evidence" in r:
            expected["evidence"] = handle(r["evidence"])
            check(base[r["evidence"]]["kind"]=="experience" and
                  base[r["evidence"]]["target"]==r["target"], key)
        check(expected == registry[handle(key)], key)
    check(all(r.get("certainty")=="reported" for r in extras if r["kind"]=="fact"), "distractor elevated")
    # No reused references, prompt text or supporting statements from the development set.
    prev = ROOT.parent/"trials"
    prior_tasks = json.loads((prev/"opportunities.json").read_text())
    prior_memory = json.loads((prev/"memory.json").read_text())
    tasks = keyed(read("opportunities.json"))
    keys = keyed(read("grading/key.json"))
    check(set(tasks).isdisjoint(r["id"] for r in prior_tasks), "old opportunity ID")
    prompts = {r["prompt"] for r in tasks.values()}
    check(len(prompts)==48 and prompts.isdisjoint(r["prompt"] for r in prior_tasks), "repeated prompt")
    check({r["text"] for r in base.values()}.isdisjoint(r["text"] for r in prior_memory), "reused memory")
    with (ROOT/"scope.tsv").open(encoding="utf-8") as f:
        scopes = list(csv.DictReader(f,delimiter="\t"))
    check(len(scopes)==48 and {s["id"] for s in scopes}==tasks.keys()==keys.keys(), "scope coverage")
    counter = Counter()
    included = Counter()
    used = set()
    worst = 0
    for s in scopes:
        key = keys[s["id"]]
        task = tasks[s["id"]]
        subjects = set(s["subjects"].split("|"))
        typ = key["type"]
        cohort = "primary" if typ in TYPES else ("control" if typ=="control" else "excluded")
        check(typ in TYPES|{"outside","control"} and key["cohort"]==cohort, s["id"])
        check(set(task)=={"id","language","prompt"} and key["target"] in subjects, s["id"])
        check(key["pass"] and key["known_mistake"] and key["action"], s["id"])
        needed = set(key["memory"])
        check(needed<=base.keys() and bool(needed)==(cohort=="primary"), s["id"])
        check(all(base[n]["target"] in subjects for n in needed), s["id"])
        # Other fresh originals also must not introduce a second applicable answer.
        others = {n for n,r in base.items() if r["target"] in subjects}
        check(others == needed, f"{s['id']}: additional fresh support")
        for r in extras:
            check(safe(r,subjects,registry,base_ids), f"{s['id']}: overlapping row {r['id']}")
        included[cohort]+=1
        counter[typ,task["language"]]+=1
        used.update(needed)
        if not needed:
            check(s["witness"]=="-", s["id"])
            continue
        remaining = {handle(n) for n in needed}
        ledger = Ledger(rows)
        while remaining:
            out = ledger.page(s["witness"])
            check(out and len(out)<=PAGE_BYTES, f"{s['id']}: witness over budget")
            page=json.loads(out)
            remaining.difference_update(r["id"] for r in page["rows"])
            check(not remaining or page["more"], f"{s['id']}: witness misses record")
        worst=max(worst,ledger.bytes_read)
    check(used==base.keys() and included=={"primary":36,"control":8,"excluded":4}, "composition")
    expected={(name,lang):n for name,n in [(t,3) for t in TYPES]+[("control",4),("outside",2)]
              for lang in ("ja","en")}
    check(counter==expected, "language/type balance")
    # The guard must reject ordinary-work, current-decision and current-lot mutations.
    for _,family,row in catalog:
        mutated=dict(row)
        if family=="sample":
            mutated["condition"]="ordinary"
        elif family=="former":
            mutated["state"]="decided"
        elif family=="closed-lot":
            mutated["period"]="2043-05-07"
        else:
            continue
        check(not safe(mutated,{row["target"]},registry,base_ids), "scope canary passed")
    selected=hot_records(base.values())
    hot="".join(r["scene"]+" / "+r["target"]+" — "+r["text"]+"\n" for r in selected).encode()
    check(hot==(ROOT/"hot.txt").read_bytes(), "hot selection")
    examples=read("grading/examples.json")
    check(len({e["opportunity"] for e in examples})==12, "example IDs")
    check(Counter((keys[e["opportunity"]]["type"],e["grade"]) for e in examples)
          == {(t,v):1 for t in TYPES for v in ("pass","known_mistake")}, "examples")
    raw=archive_bytes(catalog)
    check(len(raw)>BUDGET, "full dump fits")
    files=("memory.json","opportunities.json","hot.txt","grading/key.json","grading/examples.json","scope.tsv")
    return dict(version=1,seed=SEED,archive_rows=len(rows),archive_bytes=len(raw),
                archive_sha256=sha(raw),families=dict(sorted(Counter(f for _,f,_ in catalog).items())),
                frozen_inputs={f:sha((ROOT/f).read_bytes()) for f in files},
                budget=[BUDGET,PAGE_BYTES,PAGE_ROWS],ledger_contract_sha256=sha(
                    (ROOT.parent/"trials-large/tools/ledger.py").read_bytes()),
                arms=ARMS,opportunities=48,planned_trials=192,headline_per_arm=44,
                cohorts=dict(included),per_type_language={t:{l:counter[t,l] for l in ("ja","en")}
                    for t in sorted(TYPES|{"outside","control"})},hot_ids=[r["id"] for r in selected],
                comparisons=len(extras)*len(tasks),maximum_witness_bytes=worst,
                excluded_from_condition=[])


if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("--archive", type=Path)
    args=parser.parse_args()
    result=inspect()
    check(result==read("manifest.json"), "frozen manifest mismatch")
    if args.archive:
        check(sha(args.archive.read_bytes())==result["archive_sha256"], "generated file digest")
    print(f"新規48機会・4条件・{result['comparisons']}組の範囲と検索予算を確認")
