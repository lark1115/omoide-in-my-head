"""Register the entire frozen archive once. Trial processes receive independent DB copies."""

import json
from datetime import datetime
import subprocess


def populate(binary, records, db):
    def cli(args):
        p = subprocess.run(
            [binary, "--db", db, *args], text=True, capture_output=True, check=True
        )
        return json.loads(p.stdout)

    cli(["init"])
    index = {}

    def add(result, logical):
        for field in ("id", "version_id"):
            if field in result:
                index[result[field]] = logical
        return result["id"]

    by_id = {}
    # Register evidence first; the ledger order remains the frozen order.
    ordered = sorted(records, key=lambda r: r["kind"] != "experience")
    for r in ordered:
        target = ["--target", r["target"]]
        at = (
            [
                "--at",
                str(
                    int(
                        datetime.fromisoformat(
                            r["at"].replace("Z", "+00:00")
                        ).timestamp()
                        * 1000
                    )
                ),
            ]
            if "at" in r
            else []
        )
        if r["kind"] == "experience":
            result = cli(["event", "add", "--text", r["text"], *at, *target])
        else:
            if r.get("evidence"):
                source = by_id[r["evidence"]]
            else:
                speaker = (
                    "user" if r["source_role"] == "workshop owner" else "shift helper"
                )
                source = add(
                    cli(
                        ["say", "--speaker", speaker, "--text", r["text"], *at, *target]
                    ),
                    r["id"],
                )
            args = ["--source", source, "--text", r["text"], *target]
            if r["kind"] == "rule":
                args = [
                    "rule",
                    "add",
                    *args,
                    "--action",
                    r["action"],
                    "--scene",
                    r["scene"],
                ]
                for exclusion in r["exceptions"]:
                    args += ["--exclude", exclusion]
                result = cli(args)
                if result["state"] == "near_rules":
                    neighbor = result["near_rules"][0]["record"]["version_id"]
                    result = cli(
                        [
                            *args,
                            "--relation",
                            "unrelated",
                            "--against",
                            neighbor,
                            "--near-digest",
                            result["near_digest"],
                        ]
                    )
            elif r["kind"] == "decision":
                result = cli(
                    [
                        "decide",
                        *args,
                        "--speech",
                        {"directive": "instruction", "proposal": "proposal"}[
                            r["speech"]
                        ],
                        "--state",
                        r["state"],
                        *at,
                    ]
                )
            else:
                args = ["fact", "add", *args, "--certainty", r["certainty"], *at]
                if r.get("evidence"):
                    args += ["--evidence", source, "--verified-how", r["method"]]
                result = cli(args)
        by_id[r["id"]] = add(result, r["id"])
    assert len(by_id) == len(records) == 48
    return index
