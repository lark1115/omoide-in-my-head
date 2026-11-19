"""Run the frozen stage-one scenes against an installed binary. A miss exits nonzero."""
import argparse
import json
import subprocess
import tempfile
from datetime import datetime
from pathlib import Path

SCENES = Path(__file__).resolve().parents[1] / "tests" / "scenes"


def invoke(binary, db, args):
    proc = subprocess.run([binary, "--db", db, *args], capture_output=True, text=True)
    if proc.returncode:
        raise RuntimeError(proc.stdout or proc.stderr)
    return json.loads(proc.stdout)


def register(binary, db, records):
    ids = {}
    for record in records:
        if record["kind"] == "utterance":
            args = ["say", "--text", record["text"]]
            if "at" in record:
                timestamp = datetime.fromisoformat(record["at"].replace("Z", "+00:00"))
                args += ["--at", str(int(timestamp.timestamp() * 1000))]
        elif record["kind"] == "rule":
            args = ["rule", "add", "--source", ids[record["source"]], "--text",
                    record["text"], "--scene", record["scene"]]
            for action in record.get("actions", [record.get("action")]):
                if action:
                    args += ["--action", action]
            if "purpose" in record:
                args += ["--purpose", record["purpose"]]
            for exclusion in record.get("exceptions", []):
                args += ["--exclude", exclusion]
        else:
            raise ValueError(f"Unsupported record kind: {record['kind']}")
        if "target" in record:
            args += ["--target", record["target"]]
        result = invoke(binary, db, args)
        ids[record["id"]] = result["id"]
    return ids


def run_case(binary, scene, case):
    with tempfile.TemporaryDirectory() as directory:
        db = Path(directory) / "memory.db"
        invoke(binary, db, ["init"])
        ids = register(binary, db, scene["records"])
        query = case["query"]
        assert query["op"] == "recall"
        args = ["recall"]
        for key in ("scene", "action", "target"):
            if key in query:
                args += ["--" + key, query[key]]
        calls = 0
        result = invoke(binary, db, args)
        calls += 1
        reverse = {actual: symbolic for symbolic, actual in ids.items()}
        found = {reverse[r["id"]]: r for r in result["results"]}
        expect = case["expect"]
        errors = []
        for key in expect["must"]:
            if key not in found:
                errors.append("missing " + key)
        for key in expect["must_not"]:
            if key in found:
                errors.append("unexpected " + key)
        for key, fields in expect["fields"].items():
            for field, expected in fields.items():
                actual = found.get(key, {}).get(field)
                if field == "exceptions" and key in found:
                    v = found[key]["exclusion"]
                    actual = "unknown" if v == "unknown" else [v]
                if actual != expected:
                    errors.append(f"{key}.{field}: {actual!r} != {expected!r}")
        actual_meta = {"lookup_calls": calls}
        for key, expected in expect["meta"].items():
            if actual_meta[key] != expected:
                errors.append(f"metadata {key}")
        return errors, result["candidate_count"], result["matched_count"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    binary = parser.parse_args().binary.resolve()
    manifest = json.loads((SCENES / "manifest.json").read_text())
    failures = 0
    for entry in manifest["scenes"]:
        if entry["scene"] not in {"S1", "S2", "S6"}:
            print(entry["scene"], "NOT RUN", entry["cases"], "cases")
            continue
        scene = json.loads((SCENES / entry["file"]).read_text())
        for case in scene["cases"]:
            errors, candidates, matches = run_case(binary, scene, case)
            print(case["id"], "NOT MET" if errors else "PASS", f"candidates={candidates}",
                  f"matches={matches}", *errors)
            failures += bool(errors)
    return int(failures > 0)


if __name__ == "__main__":
    raise SystemExit(main())
