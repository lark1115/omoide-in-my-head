"""Kill and replay every stage-two write, using independent CLI processes against SQLite."""

import argparse
import sqlite3
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from interruption import stopped_before_commit
from stage_one_scenes import choose, invoke, version


def snapshot(db):
    with sqlite3.connect(db) as con:
        con.execute("BEGIN")
        return {
            table: con.execute("SELECT * FROM " + table + " ORDER BY rowid").fetchall()
            for table in (
                "meta",
                "receipts",
                "utterances",
                "rules",
                "action_links",
                "rule_index",
                "extra_sources",
                "memories",
                "evidence_index",
                "notices",
            )
        }


def fixture(binary, db, kind):
    def run(args):
        return invoke(binary, db, args)

    run(["init"])
    source = run(["say", "--text", "Keep paper dry"])["id"]
    new_source = run(["say", "--text", "Use the red carton"])["id"]
    if kind.startswith("skill-"):
        event = run(["event", "add", "--text", "inspect", "--target", "shed"])
        proposal = [
            "skill",
            "propose",
            "--source",
            event["id"],
            "--text",
            "inspect containers",
            "--scene",
            "stock",
            "--action",
            "pack",
            "--target",
            "shed",
        ]
        if kind == "skill-propose":
            return proposal
        if kind in ("skill-confirm", "skill-failure", "skill-demote", "skill-success"):
            proposed = run(proposal)
            confirm = ["skill", "confirm", version(proposed)]
            if kind == "skill-confirm":
                return confirm
            current = run(confirm)
            return [
                "outcome",
                "--version",
                version(current),
                "--result",
                "success" if kind == "skill-success" else "failure",
                *(["--demote-after", "1"] if kind == "skill-demote" else []),
            ]
        base = run(
            [
                "rule",
                "add",
                "--source",
                source,
                "--text",
                "pack",
                "--scene",
                "stock",
                "--action",
                "pack",
            ]
        )
        proposed = run(proposal)
        confirm = ["skill", "confirm", version(proposed)]
        relation = kind.removeprefix("skill-")
        near = run(confirm)
        if relation.startswith("resolve-"):
            pending = run(confirm + choose(near, base, "unsure"))
            return ["resolve", version(pending), "--keep", relation.split("-")[1]]
        return confirm + choose(near, base, relation)
    if kind == "decide":
        return [
            "decide",
            "--source",
            source,
            "--text",
            "decision",
            "--speech",
            "instruction",
            "--state",
            "decided",
        ]
    if kind == "revise-decision":
        base = run(
            [
                "decide",
                "--source",
                source,
                "--text",
                "consider packing",
                "--speech",
                "proposal",
                "--state",
                "pending",
            ]
        )
        return [
            "decide",
            "--source",
            new_source,
            "--text",
            "pack",
            "--speech",
            "instruction",
            "--state",
            "decided",
            "--revises",
            version(base),
        ]
    if kind == "experience":
        return ["event", "add", "--text", "checked the stock"]
    if kind == "fact":
        return ["fact", "add", "--source", source, "--text", "summary"]
    common = ["--action", "pack", "--scene", "stock"]
    base = run(["rule", "add", "--source", source, "--text", "dry", *common])
    if kind in (
        "used",
        "acted",
        "outcome",
        "corrected",
        "skill-propose",
        "skill-confirm",
        "skill-same",
        "skill-restated",
        "skill-exception",
        "skill-unrelated",
        "skill-unsure",
        "skill-resolve-current",
        "skill-resolve-pending",
        "skill-demote",
        "skill-success",
        "skill-failure",
    ):
        return [
            kind,
            "--version",
            base["version_id"],
            "--opportunity-id",
            "trial",
            *(["--result", "success"] if kind == "outcome" else []),
        ]
    if kind == "verified":
        e = run(["event", "add", "--text", "counted the boxes", "--target", "depot"])
        return [
            "fact",
            "add",
            "--text",
            "boxes counted",
            "--source",
            e["id"],
            "--certainty",
            "verified",
        ]
    draft = ["rule", "add", "--source", new_source, "--text", "red", *common]
    if kind == "exception":
        draft += ["--target", "annex"]
    near = run(draft)
    if kind.startswith("resolve-"):
        pending = run(draft + choose(near, base, "unsure"))
        return ["resolve", pending["version_id"], "--keep", kind.split("-")[1]]
    return draft + choose(near, base, kind)


def verify(binary, fault, kind):
    with tempfile.TemporaryDirectory() as temp:
        db = Path(temp) / "test.db"
        args = [*fixture(binary, db, kind), "--operation-id", "same-request"]
        before = snapshot(db)
        with stopped_before_commit(fault, db, args):
            assert snapshot(db) == before, "reader saw uncommitted data"
        assert snapshot(db) == before, "partial transaction survived the kill"
        with ThreadPoolExecutor(max_workers=6) as pool:
            writes = [pool.submit(invoke, binary, db, args) for _ in range(6)]
            replies = [write.result() for write in writes]
        assert all(reply == replies[0] for reply in replies)
        after = snapshot(db)
        usage = kind in (
            "used",
            "acted",
            "outcome",
            "corrected",
            "skill-demote",
            "skill-success",
            "skill-failure",
        )
        assert after["meta"][0][1] == before["meta"][0][1] + int(
            not usage or kind == "skill-demote"
        )
        assert after["meta"][0][2] == before["meta"][0][2] + int(usage)
        assert len(after["evidence_index"]) == len(after["memories"])
        assert len(after["receipts"]) == len(before["receipts"]) + 1
        assert len(after["rules"]) == len(after["rule_index"])
        assert invoke(binary, db, args) == replies[0]
        assert snapshot(db) == after
        print(
            kind, "PASS; rollback, six concurrent senders, one receipt and one revision"
        )


def verify_call_log(binary, fault):
    with tempfile.TemporaryDirectory() as temp:
        db = Path(temp) / "calls.db"
        invoke(binary, db, ["init"])
        args = ["recall", "--scene", "sample", "--call-id", "once"]
        before = snapshot(db)

        def count():
            with sqlite3.connect(db) as con:
                return con.execute(
                    "SELECT count(*) FROM calls WHERE id='once'"
                ).fetchone()[0]

        with stopped_before_commit(fault, db, args, call_log=True):
            assert count() == 0
        assert count() == 0
        with ThreadPoolExecutor(max_workers=6) as pool:
            replies = list(pool.map(lambda _: invoke(binary, db, args), range(6)))
        assert [r["accounting"] for r in replies].count("recorded") == 1
        assert all(r["results"] == [] and r["state"] == "none" for r in replies)
        assert count() == 1 and snapshot(db) == before
        print(
            "call log PASS; interrupted row absent, one stored call ID across six concurrent retries"
        )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("fault_binary", type=Path)
    args = parser.parse_args()
    for kind in (
        "same",
        "restated",
        "exception",
        "unrelated",
        "unsure",
        "resolve-current",
        "resolve-pending",
        "fact",
        "verified",
        "experience",
        "decide",
        "revise-decision",
        "used",
        "acted",
        "outcome",
        "corrected",
        "skill-propose",
        "skill-confirm",
        "skill-same",
        "skill-restated",
        "skill-exception",
        "skill-unrelated",
        "skill-unsure",
        "skill-resolve-current",
        "skill-resolve-pending",
        "skill-demote",
        "skill-success",
        "skill-failure",
    ):
        verify(args.binary.resolve(), args.fault_binary.resolve(), kind)
    verify_call_log(args.binary.resolve(), args.fault_binary.resolve())


if __name__ == "__main__":
    main()
