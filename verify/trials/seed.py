"""Register the entire frozen archive once. Trial processes receive independent DB copies."""

import json
from datetime import datetime
import subprocess


def populate(
    binary, records, db, *, expected_count=48, default_fact_speaker="workshop owner"
):
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
    versions = {}
    predecessor = {r["successor"]: r["id"] for r in records if r.get("successor")}

    def depth(r):
        key = r["id"]
        seen = set()
        while key in predecessor:
            if key in seen:
                raise ValueError("revision cycle")
            seen.add(key)
            key = predecessor[key]
        return len(seen)

    # Register evidence first; the ledger order remains the frozen order.
    ordered = sorted(records, key=lambda r: (r["kind"] != "experience", depth(r)))
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
            period = ["--period", r["period"]] if "period" in r else []
            result = cli(["event", "add", "--text", r["text"], *at, *target, *period])
        else:
            if r.get("evidence"):
                source = by_id[r["evidence"]]
            else:
                role = r.get(
                    "source_role",
                    default_fact_speaker if r["kind"] == "fact" else "workshop owner",
                )
                speaker = "user" if role in ("owner", "workshop owner") else role
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
                        "decided" if r["state"] == "superseded" else r["state"],
                        *(
                            ["--revises", versions[predecessor[r["id"]]]]
                            if r["id"] in predecessor
                            else []
                        ),
                        *at,
                    ]
                )
            else:
                args = ["fact", "add", *args, "--certainty", r["certainty"], *at]
                if r.get("evidence"):
                    args += ["--evidence", source, "--verified-how", r["method"]]
                result = cli(args)
        by_id[r["id"]] = add(result, r["id"])
        versions[r["id"]] = result.get("version_id", result["id"])
    if len(by_id) != expected_count or len(records) != expected_count:
        raise ValueError("registered archive count mismatch")
    return index
