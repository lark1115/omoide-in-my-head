"""Run the frozen scenes in fresh processes. Unmet expectations exit nonzero."""
import argparse
import json
import sqlite3
import subprocess
import tempfile
from contextlib import ExitStack
from datetime import datetime, timezone
from pathlib import Path

from interruption import stopped_before_commit

SCENES = Path(__file__).resolve().parents[1] / "tests" / "scenes"


def invoke(binary, db, args):
    proc = subprocess.run([binary, "--db", db, *args], capture_output=True, text=True)
    if proc.returncode:
        raise RuntimeError(proc.stdout or proc.stderr)
    return json.loads(proc.stdout)


def rule_args(record, source):
    args = ["rule", "add", "--source", source, "--text", record["text"], "--scene", record["scene"]]
    for action in record.get("actions", [record.get("action")]):
        if action:
            args += ["--action", action]
    for field in ("purpose", "target"):
        if field in record:
            args += ["--" + field, record[field]]
    for exclusion in record.get("exceptions", []):
        args += ["--exclude", exclusion]
    return args


def version(result):
    return result.get("version_id", result["id"])


def choose(near, base, relation):
    assert near["state"] == "near_rules"
    return ["--relation", relation, "--against", version(base), "--near-digest", near["near_digest"]]


def register(binary, db, records):
    ids = {}
    for record in records:
        if record["kind"] == "utterance":
            args = ["say", "--text", record["text"]]
            if "at" in record:
                timestamp = datetime.fromisoformat(record["at"].replace("Z", "+00:00"))
                args += ["--at", str(int(timestamp.timestamp() * 1000))]
            if "target" in record:
                args += ["--target", record["target"]]
        elif record["kind"] == "rule":
            args = rule_args(record, ids[record["source"]]["id"])
            if "relation" in record:
                relation = record["relation"]
                kind = {"uncertain": "unsure", "revision": "restated"}.get(relation["type"], relation["type"])
                args += choose(invoke(binary, db, args), ids[relation["to"]], kind)
        elif record["kind"] in ("fact","decision","experience"):
            kind=record["kind"]
            args={"fact":["fact","add"],"decision":["decide"],"experience":["event","add"]}[kind]
            args += ["--text",record["text"]]
            for key in ("target","certainty","claim","location","period"):
                if key in record:
                    args += ["--"+key,record[key]]
            if "source" in record:
                args += ["--source",ids[record["source"]]["id"]]
            if "evidence" in record:
                args += ["--evidence",ids[record["evidence"]]["id"]]
            if "at" in record:
                at=datetime.fromisoformat(record["at"].replace("Z","+00:00"))
                args+=["--at",str(int(at.timestamp()*1000))]
            for scope in record.get("scope",[]):
                args+=["--scope",scope]
            if kind=="decision":
                args+=["--speech",{"directive":"instruction","proposal":"proposal"}[record["speech"]],"--state",record["decision"]]
        else:
            raise ValueError(record["kind"])
        result = invoke(binary, db, args)
        assert result["kind"] == record["kind"]
        ids[record["id"]] = result
    return ids


def prepare(binary, fault_binary, db, scene, case, ids):
    records = {r["id"]: r for r in scene["records"]}
    plans = {}
    with ExitStack() as stack:
        for step in scene["preparations"][case["before_query"]]:
            actor, op = step["actor"], step["op"]
            if op == "prepare_revision":
                base = records[step["base"]]
                replacement = {**base, "text": records[step["source"]]["text"]}
                args = rule_args(replacement, ids[step["source"]]["id"])
                args += choose(invoke(binary, db, args), ids[step["base"]], "restated")
                plans[actor] = (step["id"], args)
            elif op == "start_uncommitted_write":
                stack.enter_context(stopped_before_commit(fault_binary, db, plans[actor][1]))
            elif op == "interrupt_before_commit":
                stack.close()
            elif op == "commit":
                args = [*plans[actor][1], "--operation-id", step["request"]]
                # The writer exits successfully; the caller deliberately loses the response.
                subprocess.run([binary, "--db", db, *args], check=True, stdout=subprocess.DEVNULL)
            elif op == "lose_reply_and_restart":
                continue  # Each call already starts a separate process; no response was retained.
            elif op == "retry":
                name, args = plans[actor]
                ids[name] = invoke(binary, db, [*args, "--operation-id", step["request"]])
            else:
                raise ValueError(op)


def observe(binary, db, query, ids):
    op = query["op"]
    if op in ("recall","decision","verification","completion"):
        args = ["recall"]
        for key in ("scene", "action", "target","period","claim"):
            if key in query:
                args += ["--" + key, query[key]]
        if op != "recall":
            args+=["--kind","decision" if op=="decision" else "fact"]
        for scope in query.get("scope",[]):
            args+=["--scope",scope]
        if query.get("fault")=="fail_usage_accounting":
            # A real SQLite writer blocks only the observational write under WAL.
            with sqlite3.connect(db) as locker:
                locker.execute("BEGIN IMMEDIATE")
                result=invoke(binary,db,args)
                assert result["accounting"]=="unlogged"
            restarted=invoke(binary,db,args)
            assert restarted["accounting"]=="recorded"
            survives=result["results"]==restarted["results"] and all(
                invoke(binary,db,["show",version(row)])["record"]["id"]==row["id"] for row in result["results"])
        else:
            result=invoke(binary,db,args)
            survives=False
        rows = list(result["results"])
        for row in result["results"]:
            for pair in row.get("unresolved_conflicts", []):
                for side in ("current", "pending"):
                    record, source = pair[side]["record"], pair[side]["source"]
                    assert record["source_id"] == source["id"]
                    assert invoke(binary, db, ["show", source["id"]])["record"]["text"] == source["text"]
                    rows.append(record)
        return rows, {**result,"lookup_calls":1,"survives_restart":survives},result["candidate_count"]
    if op == "sources":
        shown = invoke(binary, db, ["show", version(ids[query["record"]])])
        return shown["sources"], {}, None
    if op == "history":
        shown = invoke(binary, db, ["show", version(ids[query["record"]]), "--history"])
        return [v["record"] for v in shown["history"]], {"exact_versions": len(shown["history"])}, None
    raise ValueError(op)


def evaluate(case, rows, meta, ids):
    reverse = {version(actual): symbolic for symbolic, actual in ids.items()}
    found = {}
    for r in rows:
        item = dict(r)
        if r.get("source_id") is not None:
            item["source"] = reverse[r["source_id"]]
        if r.get("evidence") in reverse:
            item["evidence"]=reverse[r["evidence"]]
        if r.get("speech")=="instruction":
            item["speech"]="directive"
        if r.get("successor"):
            item["successor"] = reverse[r["successor"]]
        if "created" in r:
            item["at"] = datetime.fromtimestamp(r["created"] / 1000, timezone.utc).isoformat().replace("+00:00", "Z")
        if "exclusion" in r:
            item["exceptions"] = "unknown" if r["exclusion"] == "unknown" else [r["exclusion"]]
        found[reverse[version(r)]] = item
    expect = case["expect"]
    errors = ["missing " + k for k in expect["must"] if k not in found]
    errors += ["unexpected " + k for k in expect["must_not"] if k in found]
    for k, fields in expect["fields"].items():
        for field, expected in fields.items():
            if found.get(k, {}).get(field) != expected:
                errors.append(f"{k}.{field} mismatch")
    for k, expected in expect["meta"].items():
        if meta.get(k) != expected:
            errors.append("metadata " + k)
    return errors, len(found)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--stage", choices=(1, 2, 3), type=int, default=1)
    parser.add_argument("--fault-binary", type=Path)
    opts = parser.parse_args()
    binary = opts.binary.resolve()
    manifest = json.loads((SCENES / "manifest.json").read_text())
    active = {"S1", "S2", "S6"}
    if opts.stage >= 2:
        active.update({"S7", "S10", "S11"})
    if opts.stage==3:
        active.update({"S3","S4","S5","S8"})
    failures = 0
    for entry in manifest["scenes"]:
        if entry["scene"] not in active:
            for c in json.loads((SCENES/entry["file"]).read_text())["cases"]:
                print(c["id"],"NOT RUN")
            continue
        scene = json.loads((SCENES / entry["file"]).read_text())
        for case in scene["cases"]:
            if ("fault" in case["query"] and opts.stage<3) or ("before_query" in case and not opts.fault_binary):
                print(case["id"], "NOT RUN")
                continue
            with tempfile.TemporaryDirectory() as directory:
                db = Path(directory) / "memory.db"
                invoke(binary, db, ["init"])
                ids = register(binary, db, scene["records"])
                if "before_query" in case:
                    prepare(binary, opts.fault_binary.resolve(), db, scene, case, ids)
                rows, meta, candidates = observe(binary, db, case["query"], ids)
                errors, matches = evaluate(case, rows, meta, ids)
            print(case["id"], "NOT MET" if errors else "PASS", f"candidates={candidates}",
                  f"matches={matches}", *errors)
            failures += bool(errors)
    return int(failures > 0)


if __name__ == "__main__":
    raise SystemExit(main())
