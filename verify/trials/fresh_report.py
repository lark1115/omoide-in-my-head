"""Trace each completed device reply and subsequent independent ledger calls."""

from collections import defaultdict, deque
import json
from pathlib import Path
import sys
from large_report import summarize
from fresh import FIXTURE, ARMS
from run import read, write


def diagnostics(trial, receipts, required, index):
    calls = {}
    pending = defaultdict(deque)
    for e in trial["server_events"]:
        signature = (
            e["tool"],
            json.dumps(e["args"], ensure_ascii=False, sort_keys=True),
        )
        pending[signature].append(e)
    action_events = [e["at"] for e in trial["events"] if e["type"] == "text"]
    action = action_events[-1] if action_events and trial["answer"] else None
    ledger_times = []
    queries = []
    for e in trial["events"]:
        if e["type"] == "call":
            name = e["name"].split("__")[-1]
            signature = (
                name,
                json.dumps(e["args"], ensure_ascii=False, sort_keys=True),
            )
            calls[e["id"]] = (
                name,
                pending[signature].popleft() if pending[signature] else None,
            )
            if name in ("ledger_list", "ledger_search"):
                ledger_times.append(e["at"])
        if e["type"] != "delivery" or e["id"] not in calls:
            continue
        name, log = calls.pop(e["id"])
        if name != "recall":
            continue
        v = e["content"]
        text = (
            v
            if isinstance(v, str)
            else "".join(c["text"] for c in v if c["type"] == "text")
        )
        try:
            r = json.loads(text)
        except ValueError:
            r = {}
        if not isinstance(r, dict):
            r = {}
        coverage = r.get("search", {})
        kinds = coverage.get("by_kind", {})
        batches = receipts.get(log.get("trace") if log else None, [])
        tokens = [b["usage"] for b in batches if b["usage"]]
        ids = lambda tier: {
            index[i["version_id"]] for i in r.get(tier, []) if i["version_id"] in index
        }
        reply_state = r.get("state")
        notice = bool(r.get("notice"))
        queries.append(
            dict(
                call=len(queries) + 1,
                reply_state=reply_state,
                notice=notice,
                incomplete=coverage.get("incomplete"),
                more_search=r.get("more_search_available"),
                cursor=bool(r.get("cursor")),
                omitted_candidates=r.get("candidates_remaining"),
                judge_error=r.get("judge_error"),
                rule_path=coverage.get("rule_path"),
                evidence_path=coverage.get("evidence_path"),
                pool=r.get("candidate_count"),
                by_kind=kinds,
                judge=r.get("judge_state"),
                judged=sum(v["judged"] for v in kinds.values()) if kinds else None,
                not_judged=coverage.get("not_judged"),
                judge_requested=coverage.get("judge_requested"),
                judge_remaining=coverage.get("judge_remaining"),
                fallback=coverage.get("fallback"),
                timeout=coverage.get("timeout"),
                tool_timeout="eight-second tool budget" in text,
                error=e["error"],
                failure=bool(e["error"] or not reply_state),
                bytes=len(text.encode()),
                server_ms=round((log["returned_ns"] - log["started_ns"]) / 1e6, 1)
                if log
                else None,
                batches=[
                    {k: b[k] for k in ("candidates", "status", "error", "usage")}
                    for b in batches
                ],
                reported_input_tokens=sum(u["input_tokens"] for u in tokens),
                reported_output_tokens=sum(u["output_tokens"] for u in tokens),
                required_full=sorted(ids("results") & required),
                required_short=sorted(ids("candidates") & required),
                before_action=e["at"] < action if action is not None else None,
                _at=e["at"],
            )
        )
    for q in queries:
        at = q.pop("_at")
        q["prior_ledger"] = any(t < at for t in ledger_times)
        q["later_ledger_before_action"] = (
            any(at < t < action for t in ledger_times) if action is not None else None
        )
        if q["failure"] or q["before_action"] is None:
            q["category"] = "unobservable"
        else:
            no_match = q["notice"] or q["reply_state"] in (
                "candidates_only",
                "none",
                "incomplete_search",
            )
            incomplete = (
                q["incomplete"]
                or q["reply_state"] == "incomplete_search"
                or q["more_search"]
            )
            q["category"] = (
                "both"
                if no_match and incomplete
                else "no confirmed match"
                if no_match
                else "incomplete"
                if incomplete
                else "neither"
            )
    for name, log in calls.values():
        if name == "recall":
            queries.append(
                dict(
                    call=len(queries) + 1,
                    reply_state=None,
                    category="unobservable",
                    failure=True,
                    before_action=None,
                    prior_ledger=None,
                    later_ledger_before_action=None,
                    tool_timeout=False,
                    judge=None,
                    judged=None,
                    not_judged=None,
                    judge_requested=None,
                    judge_remaining=None,
                    fallback=None,
                    batches=[
                        {k: b[k] for k in ("candidates", "status", "error", "usage")}
                        for b in receipts.get(log["trace"], [])
                    ]
                    if log
                    else [],
                )
            )
    return queries


def summarize_fresh(study, out, binary):
    sys.path.insert(0, str(FIXTURE))
    from generate import handle, hot_records, originals

    hot_ids = [r["id"] for r in hot_records(originals())]
    summarize(
        study,
        out,
        binary,
        data_dir=FIXTURE,
        handle=handle,
        hot_ids=hot_ids,
        variants=ARMS,
    )
    return instrument(study, out)


def instrument(study, out):
    out.mkdir(parents=True, exist_ok=True)
    keys = {r["id"]: set(r["memory"]) for r in read(FIXTURE / "grading/key.json")}
    index = read(study / "id-map.json")
    receipts = defaultdict(list)
    for line in (study / "usage.jsonl").read_text().splitlines():
        row = json.loads(line)
        receipts[row["trace"]].append(row)
    data = []
    for t in sorted((study / "transcripts").glob("*.json")):
        trial = read(t)
        records = diagnostics(trial, receipts, keys[trial["opportunity"]], index)
        data.append(
            dict(
                opportunity=trial["opportunity"],
                arm=trial["arm"],
                completion=trial["completion"],
                lookups=records,
            )
        )
    if len(data) != 192 or len({(t["opportunity"], t["arm"]) for t in data}) != 192:
        raise ValueError("incomplete allocation")
    write(out / "lookup-coverage.json", data)
    warm = []
    for f in sorted((study / "warmups").glob("*.json")):
        batches = receipts["warm-" + f.stem]
        warm.append(
            dict(
                opportunity=f.stem,
                attempts=read(f),
                batches=[
                    {k: b[k] for k in ("candidates", "status", "error", "usage")}
                    for b in batches
                ],
            )
        )
    if len(warm) != 48:
        raise ValueError("warm-up count mismatch")
    write(out / "warmups.json", warm)
    table = [
        "| Arm | Sessions | Attempted recall | Every attempted recall unavailable | No successful semantic judgment among callers | Recalls | Tool timeouts | Replies with fallback |",
        "|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for arm in ARMS:
        sessions = [r for r in data if r["arm"] == arm]
        calls = [q for r in sessions for q in r["lookups"]]
        attempted = [r for r in sessions if r["lookups"]]
        semantic = (
            sum(
                not any(
                    q["judge"] == "used" or (q["judged"] or 0) > 0 for q in r["lookups"]
                )
                for r in attempted
            )
            if arm == "device_jev"
            else "n/a"
        )
        table.append(
            f"| {arm} | {len(sessions)} | {len(attempted)} | {sum(all(q['failure'] for q in r['lookups']) for r in attempted)} | {semantic} | {len(calls)} | {sum(q['tool_timeout'] for q in calls)} | {sum(bool(q['fallback']) for q in calls)} |"
        )
    (out / "availability.md").write_text("\n".join(table) + "\n")
    return data


if __name__ == "__main__":
    import argparse

    p = argparse.ArgumentParser()
    for arg in ("study", "out", "binary"):
        p.add_argument(arg, type=Path)
    a = p.parse_args()
    summarize_fresh(a.study, a.out, a.binary.resolve())
