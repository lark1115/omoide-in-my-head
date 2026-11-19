"""Join sealed single-ticket grades and actual delivery events after grading."""

import argparse
import json
from collections import Counter
from pathlib import Path
from run import ARMS, DATA, read, write
from score import LABELS
from workshop import compact


def observe(t, key, hot_ids, index):
    action_events = [e for e in t["events"] if e["type"] == "text"]
    # Review pre-final text separately: a prior operational commitment invalidates this timestamp.
    action = action_events[-1]["at"] if action_events and t["answer"] else None
    hot = ("\n\n" + t["hot"]) if t["hot"] else ""
    added = [hot]
    hits = set(hot_ids) if hot else set()
    required = set(key["memory"])
    start = t["started_ns"]
    arrived = start if required & hits else None
    names = {}
    device_calls = []
    common_bytes = 0
    deliveries = []
    for e in t["events"]:
        if e["type"] == "call":
            name = e["name"].split("__")[-1]
            names[e["id"]] = name
            if name in ("show", "recall"):
                device_calls.append(e["at"])
        if e["type"] != "delivery":
            continue
        value = e["content"]
        text = (
            value
            if isinstance(value, str)
            else "".join(i["text"] for i in value if i["type"] == "text")
        )
        if names[e["id"]] not in ("show", "recall"):
            common_bytes += len(text.encode())
            continue
        added.append(text)
        try:
            data = json.loads(text)
        except ValueError:
            data = {}
        records = (
            data.get("results", [data.get("record", data)])
            if isinstance(data, dict)
            else []
        )
        got = {
            index[r["id"]]
            for r in records
            if isinstance(r, dict) and r.get("id") in index
        }
        hits.update(got)
        if arrived is None and got & required:
            arrived = e["at"]
        deliveries.append(
            {
                "ids": sorted(got),
                "before_action": None if action is None else e["at"] < action,
                "bytes": len(text.encode()),
                "error": e["error"],
            }
        )

    def relative(ns):
        return ns - start if ns is not None else None

    observation = dict(
        opportunity=t["opportunity"],
        variant=t["arm"],
        called=bool(device_calls),
        delivered_at=relative(arrived),
        acted_at=relative(action),
        used=False if not hot else None,
        action=t["answer"] or None,
        corrected=None,
        added_input="".join(added),
    )
    audit = dict(
        deliveries=deliveries,
        common_ledger_bytes=common_bytes,
        tool_definition_bytes=len(compact(t["tool_definitions"]).encode()),
        added_bytes=len(observation["added_input"].encode()),
        all_required=required <= hits if required else None,
        device_related_before_action=any(
            d["before_action"] and required.intersection(d["ids"]) for d in deliveries
        ),
        unrelated_device_ids=sum(len(set(d["ids"]) - required) for d in deliveries),
        total_input_tokens=sum(
            (t["tokens"] or {}).get(k, 0)
            for k in (
                "input_tokens",
                "cache_creation_input_tokens",
                "cache_read_input_tokens",
            )
        ),
    )
    return observation, audit


def report(study, output, binary):
    import subprocess

    tickets = read(study / "tickets.json")
    keys = {k["id"]: k for k in read(DATA / "grading/key.json")}
    index = read(study / "id-map.json")
    hot_ids = read(DATA / "manifest.json")["hot_record_ids"]
    packets = read(study / "blind.json")
    grades = [read(study / "grades" / (p["ticket"] + ".json")) for p in packets]
    rows = []
    observations = []
    for grade in grades:
        stem = tickets[grade["ticket"]]
        trial = read(study / "transcripts" / (stem + ".json"))
        key = keys[trial["opportunity"]]
        obs, audit = observe(trial, key, hot_ids, index)
        observations.append(obs)
        rows.append(
            {
                "ticket": grade["ticket"],
                "opportunity": trial["opportunity"],
                "arm": trial["arm"],
                "type": key["type"],
                "cohort": key["cohort"],
                "label": grade["label"],
                "called": obs["called"],
                **audit,
            }
        )
    for key in keys:
        arms = [r["arm"] for r in rows if r["opportunity"] == key]
        if arms and sorted(arms) != sorted(ARMS):
            raise ValueError("incomplete comparison")
    output.mkdir(exist_ok=True, parents=True)
    write(output / "blinded.json", packets)
    write(output / "grades.json", grades)
    write(output / "observed.json", rows)
    write(study / "observations.json", observations)
    evaluated = json.loads(
        subprocess.check_output(
            [
                binary,
                "eval",
                "run",
                "--scene",
                "S2",
                "--trials",
                study / "observations.json",
            ]
        )
    )
    write(output / "eval.json", evaluated["observations"])
    lines = [
        "| Cohort or mistake | Arm | Pass | Known mistake | Other failure | Cannot tell |",
        "|---|---|---:|---:|---:|---:|",
    ]
    for typ in ["primary + control", *sorted({r["type"] for r in rows})]:
        for arm in ARMS:
            counts = Counter(
                r["label"]
                for r in rows
                if r["arm"] == arm
                and (
                    r["cohort"] in ("primary", "control")
                    if typ == "primary + control"
                    else r["type"] == typ
                )
            )
            lines.append(
                "| "
                + " | ".join(map(str, [typ, arm, *[counts[l] for l in LABELS]]))
                + " |"
            )
    lines += [
        "",
        "| Baseline → comparison (44 pairs) | Both pass | Failure → pass | Pass → failure | Neither passes |",
        "|---|---:|---:|---:|---:|",
    ]
    for a, b in [
        ("no_device", "device"),
        ("no_device", "hot_only"),
        ("hot_only", "device"),
    ]:
        side = lambda arm: {
            r["opportunity"]: r["label"] == "pass"
            for r in rows
            if r["arm"] == arm and r["cohort"] in ("primary", "control")
        }
        x, y = side(a), side(b)
        c = Counter((x[k], y[k]) for k in x)
        lines.append(
            "| "
            + " | ".join(
                map(
                    str,
                    [
                        a + " → " + b,
                        *[
                            c[k]
                            for k in [
                                (True, True),
                                (False, True),
                                (True, False),
                                (False, False),
                            ]
                        ],
                    ],
                )
            )
            + " |"
        )
    lines += [
        "",
        "| Arm (all 48) | Called | Any reply before action | Relevant lookup before action | Added bytes | Common ledger bytes | Input tokens incl. cache |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]
    for arm in ARMS:
        group = [r for r in rows if r["arm"] == arm]
        values = [
            sum(r["called"] for r in group),
            sum(any(d["before_action"] for d in r["deliveries"]) for r in group),
            sum(r["device_related_before_action"] for r in group),
            *[
                sum(r[k] for r in group)
                for k in ("added_bytes", "common_ledger_bytes", "total_input_tokens")
            ],
        ]
        lines.append("| " + " | ".join(map(str, [arm, *values])) + " |")
    (output / "counts.md").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in ("study", "output", "binary"):
        parser.add_argument(name, type=Path)
    a = parser.parse_args()
    report(a.study, a.output, a.binary.resolve())
