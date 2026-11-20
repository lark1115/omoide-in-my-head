"""Independent frozen manual/common-word registration cases, exercised via the CLI."""

import argparse, csv, json, sqlite3, subprocess, tempfile
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent


def data():
    with (HERE / "store-context.tsv").open() as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    base = [r for r in rows if not r["related_to"]]
    positive = [r for r in rows if r["related_to"]]
    assert Counter(r["group"] for r in base) == {"manual": 10, "common": 15}
    assert (
        len({r["scene"] for r in base}) == len(base) == 25
        and len(set(r["id"] for r in rows)) == 30
    )
    assert len(positive) == 5 and all(
        any(b["id"] == r["related_to"] and b["scene"] == r["scene"] for b in base)
        for r in positive
    )
    return base, positive


def run(binary):
    base, pos = data()
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        archive = root / "base.db"
        ids = {}
        sources = {}

        def cli(db, args):
            return json.loads(subprocess.check_output([binary, "--db", db, *args]))

        def args(r):
            return [
                "rule",
                "add",
                "--source",
                sources[r["id"]],
                "--scene",
                r["scene"],
                "--text",
                r["text"],
                "--action",
                r["action"],
                "--target",
                r["target"],
            ]

        cli(archive, ["init"])
        incremental = {}
        for r in base + pos:
            sources[r["id"]] = cli(archive, ["say", "--text", r["text"]])["id"]
        for r in base:
            cmd = args(r)
            reply = cli(archive, cmd)
            neighbors = reply.get("near_rules", [])
            incremental[r["id"]] = [ids[n["record"]["version_id"]] for n in neighbors]
            if neighbors:
                reply = cli(
                    archive,
                    cmd
                    + [
                        "--relation",
                        "unrelated",
                        "--against",
                        neighbors[0]["record"]["version_id"],
                        "--near-digest",
                        reply["near_digest"],
                    ],
                )
            ids[reply["version_id"]] = r["id"]
        reverse = {k: v for v, k in ids.items()}
        found = {}
        for r in base + pos:
            db = root / (r["id"] + ".db")
            with sqlite3.connect(archive) as src, sqlite3.connect(db) as dst:
                src.backup(dst)
                if r in base:
                    dst.execute(
                        "UPDATE rules SET state='superseded' WHERE version=?",
                        (reverse[r["id"]],),
                    )
            result = cli(db, args(r))
            found[r["id"]] = [
                ids[n["record"]["version_id"]] for n in result.get("near_rules", [])
            ]

        def stats(group):
            sizes = [len(found[r["id"]]) for r in base if r["group"] == group]
            return dict(
                prompts=sum(bool(n) for n in sizes),
                neighbors=sum(sizes),
                maximum=max(sizes),
            )

        return dict(
            manual=stats("manual"),
            common=stats("common"),
            required=sum(r["related_to"] in found[r["id"]] for r in pos),
            positive_extras=sum(
                len(set(found[r["id"]]) - {r["related_to"]}) for r in pos
            ),
            sequential_prompts=sum(bool(v) for v in incremental.values()),
            sequential_max=max(map(len, incremental.values())),
            sequential=incremental,
            found=found,
        )


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("binary", type=Path)
    p.add_argument("output", type=Path)
    a = p.parse_args()
    with a.output.open("x") as f:
        json.dump(run(a.binary.resolve()), f, ensure_ascii=False, indent=2)
        f.write("\n")
