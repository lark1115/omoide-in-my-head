"""Diagnose startup versus transaction time without changing the tested write or its SIGSTOP."""

import argparse, json, os, runpy, sqlite3, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HELPER = "crates/omoide/support/interruption.py"


def main(binary, fault):
    with tempfile.TemporaryDirectory() as temp:
        d = Path(temp)
        db = d / "test.db"
        wrapper = d / "slow-launch"
        wrapper.write_text(
            "#!"
            + sys.executable
            + '\nimport os,sys,time\ntime.sleep(16)\np=os.environ["NEAR_PROBE_BINARY"]\nos.execv(p,[p,*sys.argv[1:]])\n'
        )
        wrapper.chmod(0o700)
        subprocess.run(
            [binary, "--db", db, "init"], check=True, stdout=subprocess.DEVNULL
        )
        with sqlite3.connect(db) as conn:
            before = [
                conn.execute("SELECT count(*) FROM " + table).fetchone()[0]
                for table in ("utterances", "receipts")
            ]
        os.environ["NEAR_PROBE_BINARY"] = str(fault)
        old = d / "original.py"
        old.write_bytes(
            subprocess.check_output(["git", "-C", ROOT, "show", "67e0470:" + HELPER])
        )
        for label, path in [("original", old), ("separate_deadlines", ROOT / HELPER)]:
            stopped = runpy.run_path(str(path))["stopped_before_commit"]
            began = time.monotonic()
            observed = False
            error = None
            try:
                with stopped(
                    wrapper,
                    db,
                    ["say", "--text", "invented probe", "--operation-id", "probe"],
                ):
                    observed = True
            except RuntimeError as e:
                error = str(e)
            with sqlite3.connect(db) as conn:
                counts = [
                    conn.execute("SELECT count(*) FROM " + t).fetchone()[0]
                    for t in ["utterances", "receipts"]
                ]
            assert counts == before
            assert observed == (label == "separate_deadlines"), (label, observed, error)
            print(
                json.dumps(
                    dict(
                        helper=label,
                        injected_startup_seconds=16,
                        observed_sigstop=observed,
                        error=error,
                        elapsed_ms=round((time.monotonic() - began) * 1000),
                        utterances=counts[0],
                        receipts=counts[1],
                    )
                ),
                flush=True,
            )


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("binary", type=Path)
    p.add_argument("fault", type=Path)
    a = p.parse_args()
    main(a.binary.resolve(), a.fault.resolve())
