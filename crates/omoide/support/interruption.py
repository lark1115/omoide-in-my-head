"""Process control used only by the opt-in S11 acceptance harness."""

import json
import os
import signal
import subprocess
import tempfile
import time
from contextlib import contextmanager

READY = b"OMOIDE_FAULT_READY\n"


@contextmanager
def stopped_before_commit(binary, db, args, *, call_log=False):
    """Separate cold executable startup from the transaction watchdog; require real SIGSTOP."""
    hook = "OMOIDE_TEST_STOP_CALL_LOG" if call_log else "OMOIDE_TEST_STOP_BEFORE_COMMIT"
    env = {**os.environ, hook: "1", "OMOIDE_TEST_SIGNAL_START": "1"}
    with tempfile.TemporaryFile() as stdout:
        launched = time.monotonic()
        child = subprocess.Popen([binary, "--db", db, *args], env=env, stdout=stdout)
        ready = None
        try:
            while True:
                now = time.monotonic()
                if ready is None and os.pread(stdout.fileno(), len(READY), 0) == READY:
                    ready = now
                # Always observe the child before testing the deadline, including
                # when the parent was descheduled between polls.
                pid, status = os.waitpid(child.pid, os.WNOHANG | os.WUNTRACED)
                if pid:
                    if os.WIFSTOPPED(status) and os.WSTOPSIG(status) == signal.SIGSTOP:
                        break
                    child.returncode = os.waitstatus_to_exitcode(status)
                    detail = (
                        os.pread(stdout.fileno(), 1024, 0)
                        .removeprefix(READY)
                        .decode(errors="replace")
                        .strip()
                    )
                    raise RuntimeError(
                        f"writer exited before the pre-commit stop (status {child.returncode}): {detail}"
                    )
                if ready is None and now - launched >= 120:
                    raise RuntimeError(
                        "no executable readiness marker or pre-commit stop within 120 s"
                    )
                if ready is not None and now - ready >= 15:
                    raise RuntimeError(
                        "writer started but did not reach the pre-commit stop within 15 s"
                    )
                time.sleep(0.005)
            if path := os.environ.get("OMOIDE_TEST_TIMING_LOG"):
                timing = {
                    "startup_ms": None
                    if ready is None
                    else round(1000 * (ready - launched)),
                    "ready_to_stop_ms": None
                    if ready is None
                    else round(1000 * (now - ready)),
                }
                fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
                with os.fdopen(fd, "a") as f:
                    print(json.dumps(timing), file=f)
            yield
        finally:
            if child.returncode is None:
                child.kill()
                child.wait()
