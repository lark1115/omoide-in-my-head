"""Process control used only by the opt-in S11 acceptance harness."""
import os
import signal
import subprocess
import time
from contextlib import contextmanager


@contextmanager
def stopped_before_commit(binary, db, args, *, call_log=False):
    """Yield only once the real CLI has inserted its receipt but has not committed."""
    hook = "OMOIDE_TEST_STOP_CALL_LOG" if call_log else "OMOIDE_TEST_STOP_BEFORE_COMMIT"
    env = {**os.environ, hook: "1"}
    child = subprocess.Popen([binary, "--db", db, *args], env=env, stdout=subprocess.DEVNULL)
    try:
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            pid, status = os.waitpid(child.pid, os.WNOHANG | os.WUNTRACED)
            if pid:
                if os.WIFSTOPPED(status) and os.WSTOPSIG(status) == signal.SIGSTOP:
                    break
                child.returncode = os.waitstatus_to_exitcode(status)
                raise RuntimeError("writer exited before the pre-commit stop; use the fault-injection binary")
            time.sleep(0.005)
        else:
            raise RuntimeError("writer did not reach the pre-commit stop")
        yield
    finally:
        if child.returncode is None:
            child.kill()
            child.wait()
