"""Kill and replay every stage-two write, using independent CLI processes against SQLite."""
import argparse
import sqlite3
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from interruption import stopped_before_commit
from stage_one_scenes import choose, invoke


def snapshot(db):
    with sqlite3.connect(db) as con:
        con.execute("BEGIN")
        return {table: con.execute("SELECT * FROM " + table + " ORDER BY rowid").fetchall()
                for table in ("meta", "receipts", "utterances", "rules", "action_links",
                              "rule_index", "extra_sources", "facts")}


def fixture(binary, db, kind):
    def run(args):
        return invoke(binary, db, args)
    run(["init"])
    source = run(["say", "--text", "Keep paper dry"])["id"]
    new_source = run(["say", "--text", "Use the red carton"])["id"]
    if kind == "fact":
        return ["fact", "add", "--source", source, "--text", "summary"]
    common = ["--action", "pack", "--scene", "stock"]
    base = run(["rule", "add", "--source", source, "--text", "dry", *common])
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
        assert after["meta"][0][1] == before["meta"][0][1] + 1
        assert len(after["receipts"]) == len(before["receipts"]) + 1
        assert len(after["rules"]) == len(after["rule_index"])
        assert invoke(binary, db, args) == replies[0]
        assert snapshot(db) == after
        print(kind, "PASS; rollback, six concurrent senders, one receipt and one revision")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("fault_binary", type=Path)
    args = parser.parse_args()
    for kind in ("same", "restated", "exception", "unrelated", "unsure",
                 "resolve-current", "resolve-pending", "fact"):
        verify(args.binary.resolve(), args.fault_binary.resolve(), kind)


if __name__ == "__main__":
    main()
