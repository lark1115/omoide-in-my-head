"""Run the frozen scenes in fresh processes. Unmet expectations exit nonzero."""

import argparse
import json
import sqlite3
import subprocess
import sys
import os
import hashlib
import tempfile
from contextlib import ExitStack
from datetime import datetime, timezone
from pathlib import Path

from interruption import stopped_before_commit

SCENES = Path("tests/scenes")


def json_bytes(value):
    return (
        len(json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode()) + 1
    )


def invoke(binary, db, args):
    proc = subprocess.run([binary, "--db", db, *args], capture_output=True, text=True)
    if proc.returncode:
        try:
            reason = json.loads(proc.stdout)["message"]
        except (KeyError, ValueError):
            reason = proc.stderr.strip() or "child exited without JSON"
        raise RuntimeError(reason)
    return json.loads(proc.stdout)


def rule_args(record, source):
    args = [
        "rule",
        "add",
        "--source",
        source,
        "--text",
        record["text"],
        "--scene",
        record["scene"],
    ]
    for action in record.get("actions", [record.get("action")]):
        if action:
            args += ["--action", action]
    for field in ("purpose", "target"):
        if field in record:
            args += ["--" + field, record[field]]
    for exclusion in record.get("exceptions", []):
        args += ["--exclude", exclusion]
    if record.get("pinned"):
        args.append("--pinned")
    return args


def version(result):
    return result.get("version_id", result["id"])


def choose(near, base, relation):
    assert near["state"] == "near_rules"
    return [
        "--relation",
        relation,
        "--against",
        version(base),
        "--near-digest",
        near["near_digest"],
    ]


def register(binary, db, records, aliases):
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
            for alias in aliases.get(record["text"], []):
                args += ["--alias", alias]
            if "relation" in record:
                relation = record["relation"]
                kind = {"uncertain": "unsure", "revision": "restated"}.get(
                    relation["type"], relation["type"]
                )
                args += choose(invoke(binary, db, args), ids[relation["to"]], kind)
        elif record["kind"] in ("fact", "decision", "experience"):
            kind = record["kind"]
            args = {
                "fact": ["fact", "add"],
                "decision": ["decide"],
                "experience": ["event", "add"],
            }[kind]
            args += ["--text", record["text"]]
            for key in ("target", "certainty", "claim", "location", "period"):
                if key in record:
                    args += ["--" + key, record[key]]
            if "source" in record:
                args += ["--source", ids[record["source"]]["id"]]
            if "evidence" in record:
                args += ["--evidence", ids[record["evidence"]]["id"]]
            if "at" in record:
                at = datetime.fromisoformat(record["at"].replace("Z", "+00:00"))
                args += ["--at", str(int(at.timestamp() * 1000))]
            for scope in record.get("scope", []):
                args += ["--scope", scope]
            if kind == "decision":
                args += [
                    "--speech",
                    {"directive": "instruction", "proposal": "proposal"}[
                        record["speech"]
                    ],
                    "--state",
                    record["decision"],
                ]
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
            op = step["op"]
            actor = step.get("actor")
            if op == "seed_experiences":
                base = int(
                    datetime(2040, 4, 20, tzinfo=timezone.utc).timestamp() * 1000
                )
                for i in range(step["count"]):
                    rid = f"{step['id_prefix']}-{i}"
                    ids[rid] = invoke(
                        binary,
                        db,
                        [
                            "event",
                            "add",
                            "--text",
                            f"{step['text']} {i}",
                            "--at",
                            str(base + 60000 * i),
                            "--target",
                            "bakehouse",
                        ],
                    )
                continue
            if op == "initialize_section":
                limit = step["limit_bytes"]
                invoke(
                    binary,
                    db,
                    [
                        "hot",
                        "init",
                        str(db.with_suffix(".md")),
                        "--limit-bytes",
                        str(limit),
                    ],
                )
                continue
            if op == "export_section":
                invoke(
                    binary,
                    db,
                    [
                        "hot",
                        "export",
                        str(db.with_suffix(".md")),
                        "--limit-bytes",
                        str(limit),
                    ],
                )
                continue
            if op == "prepare_revision":
                base = records[step["base"]]
                replacement = {**base, "text": records[step["source"]]["text"]}
                args = rule_args(replacement, ids[step["source"]]["id"])
                args += choose(invoke(binary, db, args), ids[step["base"]], "restated")
                plans[actor] = (step["id"], args)
            elif op == "start_uncommitted_write":
                stack.enter_context(
                    stopped_before_commit(fault_binary, db, plans[actor][1])
                )
            elif op == "interrupt_before_commit":
                stack.close()
            elif op == "commit":
                args = [*plans[actor][1], "--operation-id", step["request"]]
                # The writer exits successfully; the caller deliberately loses the response.
                subprocess.run(
                    [binary, "--db", db, *args], check=True, stdout=subprocess.DEVNULL
                )
            elif op == "lose_reply_and_restart":
                continue  # Each call already starts a separate process; no response was retained.
            elif op == "retry":
                name, args = plans[actor]
                ids[name] = invoke(
                    binary, db, [*args, "--operation-id", step["request"]]
                )
            else:
                raise ValueError(op)


def observe(binary, db, query, ids, scene, judge, oracle):
    if query["op"] == "section":
        section = db.with_suffix(".md").read_text()
        rows = [
            {"id": version(ids[r["id"]])}
            for r in scene["records"]
            if r["kind"] == "rule"
            and json.dumps(r["text"], ensure_ascii=False) in section
        ]
        return (
            rows,
            {
                "max_section_bytes": len(section.encode()),
                "bytes": len(section.encode()),
            },
            None,
        )
    op = query["op"]
    if op in ("recall", "decision", "verification", "completion"):
        args = ["recall"]
        for key in ("scene", "action", "target", "period", "claim"):
            if key in query:
                args += ["--" + key, query[key]]
        mode = query.get("judge")
        if mode in ("timeout", "valid-empty") or oracle:
            args += [
                "--judge",
                sys.executable,
                "--judge-arg",
                str(judge),
                "--judge-arg",
                (
                    "oracle"
                    if oracle
                    else {"valid-empty": "empty", "timeout": "timeout"}[mode]
                ),
            ]
            if mode == "timeout":
                args += ["--timeout-ms", "50"]
            if oracle:
                args += ["--judge-arg", str(oracle)]
        if op != "recall":
            args += ["--kind", "decision" if op == "decision" else "fact"]
        for scope in query.get("scope", []):
            args += ["--scope", scope]
        if query.get("fault") == "fail_usage_accounting":
            # A real SQLite writer blocks only the observational write under WAL.
            with sqlite3.connect(db) as locker:
                locker.execute("BEGIN IMMEDIATE")
                result = invoke(binary, db, args)
                assert result["accounting"] == "unlogged"
            restarted = invoke(binary, db, args)
            assert restarted["accounting"] == "recorded"
            survives = result["results"] == restarted["results"] and all(
                invoke(binary, db, ["show", version(row)])["record"]["id"] == row["id"]
                for row in result["results"]
            )
        else:
            result = invoke(binary, db, args)
            survives = False
        if mode == "timeout" and result.get("judge_error") != "judge timeout":
            raise RuntimeError("expected a judge time limit, not a different failure")
        rows = list(result["results"])
        for row in result["results"]:
            for pair in row.get("unresolved_conflicts", []):
                for side in ("current", "pending"):
                    record, source = pair[side]["record"], pair[side]["source"]
                    assert record["source_id"] == source["id"]
                    assert (
                        invoke(binary, db, ["show", source["id"]])["record"]["text"]
                        == source["text"]
                    )
                    rows.append(record)
        return (
            rows,
            {
                **result,
                "lookup_calls": 1,
                "survives_restart": survives,
                "judge_state": {"not_configured": "unconfigured"}.get(
                    result["judge_state"], result["judge_state"]
                ),
                "result_count": result["matched_count"],
                "bytes": json_bytes(result),
            },
            result["candidate_count"],
        )
    if op == "sources":
        shown = invoke(binary, db, ["show", version(ids[query["record"]])])
        return shown["sources"], {"bytes": json_bytes(shown)}, None
    if op == "history":
        shown = invoke(binary, db, ["show", version(ids[query["record"]]), "--history"])
        return (
            [v["record"] for v in shown["history"]],
            {"exact_versions": len(shown["history"]), "bytes": json_bytes(shown)},
            None,
        )
    raise ValueError(op)


def evaluate(case, rows, meta, ids, recorded_times):
    reverse = {version(actual): symbolic for symbolic, actual in ids.items()}
    found = {}
    for r in rows:
        item = dict(r)
        if r.get("source_id") is not None:
            item["source"] = reverse[r["source_id"]]
        if r.get("evidence") in reverse:
            item["evidence"] = reverse[r["evidence"]]
        if r.get("speech") == "instruction":
            item["speech"] = "directive"
        if r.get("successor"):
            item["successor"] = reverse[r["successor"]]
        if "created" in r:
            item["at"] = (
                datetime.fromtimestamp(r["created"] / 1000, timezone.utc)
                .isoformat()
                .replace("+00:00", "Z")
            )
        if "exclusion" in r:
            item["exceptions"] = (
                "unknown" if r["exclusion"] == "unknown" else [r["exclusion"]]
            )
        found[reverse[version(r)]] = item
    expect = case["expect"]
    errors = ["missing " + k for k in expect["must"] if k not in found]
    errors += ["unexpected " + k for k in expect["must_not"] if k in found]
    for k, fields in expect["fields"].items():
        for field, expected in fields.items():
            if found.get(k, {}).get(field) != expected:
                errors.append(f"{k}.{field} mismatch")
    for k, expected in expect["meta"].items():
        if k == "max_section_bytes" and meta[k] <= expected:
            continue
        if meta.get(k) != expected:
            errors.append("metadata " + k)
    stable_ids = {**{r["id"]: symbol for symbol, r in ids.items()}, **reverse}

    def normalize(value):
        if isinstance(value, list):
            return [normalize(v) for v in value]
        if isinstance(value, dict):
            return {
                k: normalize(v) for k, v in value.items() if k not in ("created", "at")
            }
        if isinstance(value, str):
            return stable_ids.get(value, value)
        return value

    observations = []
    for key, value in found.items():
        fields = normalize(value)
        if key in recorded_times and "at" in value:
            fields["at"] = value["at"]
        observations.append({"record": key, "fields": fields})
    return errors, sorted(found), observations


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--stage", choices=(1, 2, 3, 4), type=int, default=1)
    parser.add_argument("--fault-binary", type=Path)
    parser.add_argument("--scenes", type=Path, default=SCENES)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--scene")
    parser.add_argument("--compare-binary", type=Path)
    parser.add_argument("--compare-fault", type=Path)
    parser.add_argument("--judge-fixture", type=Path)
    parser.add_argument("--oracle-table", type=Path)
    parser.add_argument("--aliases", type=Path)
    opts = parser.parse_args()
    # The reference study is deterministic even when the calling agent uses ambient settings.
    for key in tuple(os.environ):
        if key.startswith("OMOIDE_"):
            del os.environ[key]
    judge = (
        opts.judge_fixture or opts.scenes.parent / "support" / "deterministic_judge.py"
    ).resolve()
    oracle = opts.oracle_table.resolve() if opts.oracle_table else None
    aliases = json.loads(opts.aliases.read_text()) if opts.aliases else {}
    report = []
    compared = []
    binary = opts.binary.resolve()
    manifest_bytes = (opts.scenes / "manifest.json").read_bytes()
    manifest = json.loads(manifest_bytes)
    digest = hashlib.sha256(manifest_bytes)
    for entry in manifest["scenes"]:
        digest.update((opts.scenes / entry["file"]).read_bytes())
    config = {
        "dataset_digest": digest.hexdigest(),
        "variant": "oracle-path-only"
        if oracle
        else "registration-aliases"
        if aliases
        else "baseline",
    }
    config["settings_digest"] = hashlib.sha256(
        json.dumps(
            {
                "scene": opts.scene,
                "aliases": aliases,
                "stage": opts.stage,
                "judge": hashlib.sha256(judge.read_bytes()).hexdigest()
                if judge.is_file()
                else None,
                "oracle": hashlib.sha256(oracle.read_bytes()).hexdigest()
                if oracle
                else None,
            },
            sort_keys=True,
            ensure_ascii=False,
        ).encode()
    ).hexdigest()
    config["settings_digest"] = hashlib.sha256(
        json.dumps(
            {
                "stage": opts.stage,
                "scene": opts.scene,
                "aliases": aliases,
                "judge": hashlib.sha256(judge.read_bytes()).hexdigest()
                if judge.is_file()
                else None,
                "oracle": hashlib.sha256(oracle.read_bytes()).hexdigest()
                if oracle
                else None,
            },
            sort_keys=True,
            ensure_ascii=False,
        ).encode()
    ).hexdigest()
    active = {"S1", "S2", "S6"}
    if opts.stage >= 2:
        active.update({"S7", "S10", "S11"})
    if opts.stage >= 3:
        active.update({"S3", "S4", "S5", "S8"})
    if opts.stage >= 4:
        active.update({"S9", "S12"})
    failures = 0
    for entry in manifest["scenes"]:
        if opts.scene and opts.scene != entry["scene"]:
            continue
        if entry["scene"] not in active:
            for c in json.loads((opts.scenes / entry["file"]).read_text())["cases"]:
                report.append({"case": c["id"], "status": "not_run"})
            continue
        scene = json.loads((opts.scenes / entry["file"]).read_text())
        for case in scene["cases"]:
            if (
                (
                    opts.compare_binary
                    and case.get("before_query") == "interleaving"
                    and not opts.compare_fault
                )
                or ("fault" in case["query"] and opts.stage < 3)
                or (
                    case.get("before_query") == "interleaving" and not opts.fault_binary
                )
            ):
                report.append({"case": case["id"], "status": "not_run"})
                if opts.compare_binary:
                    compared.append(report[-1])
                continue
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                cases_to_run = [(binary, opts.fault_binary, root / "before.db", report)]
                try:
                    invoke(binary, cases_to_run[0][2], ["init"])
                    ids = register(
                        binary, cases_to_run[0][2], scene["records"], aliases
                    )
                except (
                    RuntimeError,
                    ValueError,
                    KeyError,
                    AssertionError,
                    OSError,
                    subprocess.SubprocessError,
                ) as exc:
                    report.append(
                        {
                            "case": case["id"],
                            "status": "not_met",
                            "errors": ["setup: " + str(exc)],
                        }
                    )
                    if opts.compare_binary:
                        compared.append(
                            {
                                "case": case["id"],
                                "status": "not_run",
                                "errors": ["shared initial store unavailable"],
                            }
                        )
                    failures += 1
                    continue
                if opts.compare_binary:
                    with (
                        sqlite3.connect(root / "before.db") as source,
                        sqlite3.connect(root / "after.db") as destination,
                    ):
                        source.backup(destination)
                    cases_to_run.append(
                        (
                            opts.compare_binary.resolve(),
                            opts.compare_fault,
                            root / "after.db",
                            compared,
                        )
                    )
                for exe, fault, db, results in cases_to_run:
                    names = dict(ids)
                    try:
                        if "before_query" in case:
                            prepare(
                                exe,
                                fault.resolve() if fault else None,
                                db,
                                scene,
                                case,
                                names,
                            )
                        rows, meta, candidates = observe(
                            exe, db, case["query"], names, scene, judge, oracle
                        )
                        errors, matches, observations = evaluate(
                            case,
                            rows,
                            meta,
                            names,
                            {r["id"] for r in scene["records"] if "at" in r},
                        )
                        results.append(
                            {
                                "case": case["id"],
                                "status": "not_met" if errors else "pass",
                                "returned": matches,
                                "observed": observations,
                                "candidates": candidates,
                                "judge_state": meta.get("judge_state"),
                                "bytes": meta["bytes"],
                                "errors": errors,
                            }
                        )
                        failures += bool(errors)
                    except (
                        RuntimeError,
                        ValueError,
                        KeyError,
                        AssertionError,
                        OSError,
                        subprocess.SubprocessError,
                    ) as exc:
                        results.append(
                            {
                                "case": case["id"],
                                "status": "not_met",
                                "errors": [f"{type(exc).__name__}: {exc}"],
                            }
                        )
                        failures += 1

    if not report:
        raise ValueError("no matching cases")
    if opts.json:
        if opts.compare_binary:
            print(
                json.dumps(
                    {
                        "before": {"cases": report, **config},
                        "after": {"cases": compared, **config},
                    },
                    ensure_ascii=False,
                )
            )
            return 0
        print(json.dumps({"cases": report, **config}, ensure_ascii=False))
        return 0
    for r in report:
        print(r["case"], r["status"], r.get("errors", ""))
    return int(failures > 0)


if __name__ == "__main__":
    raise SystemExit(main())
