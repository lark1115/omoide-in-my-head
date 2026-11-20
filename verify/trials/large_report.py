"""Join blinded grades after scoring, then apply the frozen large-trial hypotheses."""

import argparse
from itertools import combinations
from pathlib import Path
import sys
import json
import statistics
from run import read, write, DATA, ROOT
from report import report
from large import ARMS


def verdict(net, over, known_reduction):
    if net <= -3 or over >= 2:
        return "harm"
    if net >= 3 and known_reduction > 0:
        return "improvement"
    if -1 <= net <= 1:
        return "no practical difference"
    return "inconclusive"


def stats(values):
    return f"{sum(values)} / {statistics.median(values):g} / {max(values)}"


def summarize(
    study, out, binary, *, data_dir=DATA, handle=None, hot_ids=None, variants=ARMS
):
    report(study, out, binary, variants=variants, data=data_dir, hot_ids=hot_ids)
    # The public action wording is already in the blinded packets.
    evaluated = read(out / "eval.json")
    for batch in evaluated.values():
        for trial in batch["trials"]:
            trial.pop("action", None)
    write(out / "eval.json", evaluated)
    rows = read(out / "observed.json")
    if handle is None:
        sys.path.insert(0, str(ROOT / "tests/trials-large"))
        from generate import opaque as handle

    expected = {
        k["id"]: {handle(v) for v in k["memory"]}
        for k in read(data_dir / "grading/key.json")
    }
    data = []
    for r in rows:
        trial = read(
            study / "transcripts" / (r["opportunity"] + "-" + r["arm"] + ".json")
        )
        logs = trial["server_events"]
        ledger = [e for e in logs if e["tool"].startswith("ledger_")]
        sizes = {
            name: sum(len(e["text"].encode()) for e in ledger if e["tool"] == name)
            for name in ("ledger_list", "ledger_search")
        }
        total = sum(sizes.values())
        if total > 24576:
            raise ValueError("ledger budget exceeded")
        ledger_ids = set()
        for e in ledger:
            if e["text"]:
                ledger_ids.update(
                    item["id"] for item in json.loads(e["text"]).get("rows", [])
                )
        delivery = r["deliveries"]
        data.append(
            dict(
                arm=r["arm"],
                id=r["opportunity"],
                full_relevant_ledger=bool(ledger_ids & expected[r["opportunity"]]),
                ledger_bytes=total,
                ledger_calls=len(ledger),
                lists=sum(e["tool"] == "ledger_list" for e in ledger),
                searches=sum(e["tool"] == "ledger_search" for e in ledger),
                list_bytes=sizes["ledger_list"],
                search_bytes=sizes["ledger_search"],
                budget_closed=any(not e["text"] for e in ledger),
                device_calls=sum(
                    e["type"] == "call"
                    and e["name"].split("__")[-1] in ("recall", "show")
                    for e in trial["events"]
                ),
                device_errors=sum(d["error"] for d in delivery),
                judge_failures=sum(d.get("judge_state") == "failed" for d in delivery),
                candidate_before=any(
                    d["before_action"] and d.get("required_candidate") for d in delivery
                ),
                device_bytes=sum(d["bytes"] for d in delivery),
                hot_bytes=len(("\n\n" + trial["hot"]).encode()) if trial["hot"] else 0,
                delivered_before=r["device_related_before_action"],
                complete=trial["completion"],
                added=r["added_bytes"],
            )
        )
    write(out / "ledger-and-device.json", data)
    text = [
        "| All 48 | Ledger bytes (sum / median / max) | Ledger calls (sum / median / max) | List calls / bytes | Search calls / bytes | Budget closed | Hot bytes | Device calls / errors / judge failures | Device bytes | Added bytes | Full result before action | Required candidate before action | Relevant common ledger row delivered |",
        "|---|---|---|---|---|---:|---:|---|---:|---:|---:|---:|---:|",
    ]
    for arm in variants:
        g = [r for r in data if r["arm"] == arm]
        total = lambda field: sum(r[field] for r in g)
        text.append(
            f"| {arm} | {stats([r['ledger_bytes'] for r in g])} | {stats([r['ledger_calls'] for r in g])} | {total('lists')} / {total('list_bytes')} | {total('searches')} / {total('search_bytes')} | {total('budget_closed')} | {total('hot_bytes')} | {total('device_calls')} / {total('device_errors')} / {total('judge_failures')} | {total('device_bytes')} | {total('added')} | {total('delivered_before')} | {total('candidate_before')} | {total('full_relevant_ledger')} |"
        )
    by_arm = {
        a: {
            r["opportunity"]: r
            for r in rows
            if r["arm"] == a and r["cohort"] != "excluded"
        }
        for a in variants
    }
    text += [
        "",
        "| Baseline → comparison | Failure → pass | Pass → failure | Net |",
        "|---|---|---|---:|",
    ]
    for a, b in combinations(variants, 2):
        x, y = by_arm[a], by_arm[b]
        gains = sorted(
            k for k in x if x[k]["label"] != "pass" and y[k]["label"] == "pass"
        )
        losses = sorted(
            k for k in x if x[k]["label"] == "pass" and y[k]["label"] != "pass"
        )
        text.append(
            f"| {a} → {b} | {len(gains)} ({', '.join(gains)}) | {len(losses)} ({', '.join(losses)}) | {len(gains) - len(losses):+d} |"
        )

    def count(a, cohort, label):
        return sum(
            r["label"] == label and (not cohort or r["cohort"] == cohort)
            for r in by_arm[a].values()
        )

    core = "device_core" if "device_core" in variants else "device"
    net = count(core, None, "pass") - count("no_device", None, "pass")
    over = count(core, "control", "known_mistake") - count(
        "no_device", "control", "known_mistake"
    )
    fewer = count("no_device", "primary", "known_mistake") - count(
        core, "primary", "known_mistake"
    )
    text += [
        "",
        f"Frozen primary decision: **{verdict(net, over, fewer)}**. Net passes {net:+d}, net control overapplications {over:+d}, reduction in known mistakes on 36 target opportunities {fewer:+d}.",
    ]
    (out / "large-summary.md").write_text("\n".join(text) + "\n")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    for arg in ("study", "out", "binary"):
        p.add_argument(arg, type=Path)
    a = p.parse_args()
    summarize(a.study, a.out, a.binary.resolve())
