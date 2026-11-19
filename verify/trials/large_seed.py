"""Seed the device snapshot once and verify the full record and history census."""

import hashlib
import os
import sqlite3
import sys
import tempfile
from pathlib import Path
from run import ROOT, read, write
from seed import populate


def seed_large(study, binary):
    dest = study / "baseline.db"
    if dest.exists() and (study / "seed.json").exists():
        return
    sys.path.insert(0, str(ROOT / "tests/trials-large"))
    from generate import build

    archive = build()
    names = {
        r["id"]: k if family == "original" else r["id"] for k, family, r in archive
    }
    with tempfile.TemporaryDirectory(dir=study) as td:
        db = Path(td) / "seed.db"
        index = populate(binary, read(study / "ledger.json"), db, expected_count=3048)
        with sqlite3.connect(db) as conn:
            counts = dict(
                conn.execute("SELECT kind,count(*) FROM memories GROUP BY kind")
            )
            counts["rule"] = conn.execute(
                "SELECT count(*) FROM rules WHERE state='current'"
            ).fetchone()[0]
            revised = conn.execute(
                "SELECT count(*) FROM memories WHERE json_extract(details,'$.decision')='revised'"
            ).fetchone()[0]
            assert (
                counts == {"decision": 627, "experience": 717, "fact": 819, "rule": 885}
                and revised == 36
            )
            assert conn.execute("PRAGMA integrity_check").fetchone() == ("ok",)
            conn.execute("PRAGMA wal_checkpoint(TRUNCATE)")
        # Prefer version handles when several old decisions share a public record id.
        write(study / "id-map.json", {k: names[v] for k, v in index.items()})
        os.replace(db, dest)
        write(
            study / "seed.json",
            dict(
                counts=counts,
                revised=revised,
                archive_sha256=hashlib.sha256(
                    (study / "archive.jsonl").read_bytes()
                ).hexdigest(),
            ),
        )


if __name__ == "__main__":
    seed_large(Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve())
