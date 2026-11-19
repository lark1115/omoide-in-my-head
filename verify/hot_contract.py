"""Check optimistic revision rechecks and publication under a per-target lock."""

import json
import os
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from stage_one_scenes import invoke

binary, fault = map(Path, sys.argv[1:])
with tempfile.TemporaryDirectory() as directory:
    db = Path(directory) / "memo.db"
    target = db.with_suffix(".md")

    def run(*args):
        return invoke(binary.resolve(), db, list(args))

    run("init")
    target.write_bytes(b"original\r\n")
    run("hot", "init", str(target))
    source = run("say", "--text", "synthetic instruction")["id"]
    record = run(
        "rule",
        "add",
        "--source",
        source,
        "--scene",
        "stock",
        "--action",
        "pack",
        "--text",
        "check the seal",
        "--pinned",
    )
    args = ["--db", db, "hot", "export", target]

    def pause():
        p = subprocess.Popen(
            [fault.resolve(), *args],
            env={**os.environ, "OMOIDE_TEST_STOP_HOT": "1"},
            stdout=subprocess.PIPE,
        )
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            pid, status = os.waitpid(p.pid, os.WNOHANG | os.WUNTRACED)
            if pid and os.WIFSTOPPED(status):
                return p
            if pid:
                raise RuntimeError("exporter exited before rename")
            time.sleep(0.01)
        p.kill()
        p.wait()
        raise RuntimeError("exporter did not stop")

    before = target.read_bytes()
    killed = pause()
    assert target.read_bytes() == before
    killed.kill()
    killed.wait()
    assert target.read_bytes() == before
    slow = pause()
    fast = subprocess.Popen([binary.resolve(), *args], stdout=subprocess.PIPE)
    try:
        run("used", "--version", record["version_id"], "--notice-id", "during-render")
        assert target.read_bytes() == before
        os.kill(slow.pid, signal.SIGCONT)
        first = json.loads(slow.communicate(timeout=20)[0])
        second = json.loads(fast.communicate(timeout=20)[0])
        assert first["header"] == second["header"] and first["header"]["usage"] == 1
        data = target.read_bytes()
        assert (
            data.startswith(b"original\r\n")
            and data.count(b"<!-- omoide:begin -->") == 1
        )
        assert (
            b"check the seal" in data
            and run("hot", "status", str(target))["state"] == "current"
        )
    finally:
        for child in (slow, fast):
            if child.poll() is None:
                child.kill()
                child.wait()
    print(
        "PASS: killed before rename, two exporters, concurrent usage and coherent final header"
    )
