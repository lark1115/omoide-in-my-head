"""Local judge transport. Only the host adapter process receives the provider credential."""

from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import socket
import socketserver
import subprocess
import sys
import tempfile
import threading


@contextmanager
def host(adapter, stats):
    if not os.environ.get("TYPESAFE_API_KEY"):
        raise ValueError("Jev credential missing")
    lock = threading.Lock()

    class Handler(socketserver.StreamRequestHandler):
        def handle(self):
            request = self.rfile.read()
            value = json.loads(request)
            trace = value.pop("_trial_trace", None)
            request = json.dumps(value).encode()
            query_digest = hashlib.sha256(
                json.dumps(value["query"], ensure_ascii=False, sort_keys=True).encode()
            ).hexdigest()
            result = None
            try:
                result = subprocess.run(
                    [adapter, "--report-usage"],
                    input=request,
                    capture_output=True,
                    env={
                        "TYPESAFE_API_KEY": os.environ["TYPESAFE_API_KEY"],
                        "JEV_TIMEOUT_MS": "5800",
                        "PATH": os.defpath,
                    },
                    timeout=7,
                )
            except subprocess.TimeoutExpired:
                pass
            message = (
                result.stderr.decode().strip() if result else "bridge deadline exceeded"
            )
            usage = json.loads(message)["usage"] if message.startswith("{") else None
            cause = (
                message
                if message.startswith("TypeSafe ") and len(message) < 80
                else "adapter failure"
            )
            with lock, stats.open("a") as out:
                print(
                    json.dumps(
                        dict(
                            trace=trace,
                            candidates=len(value["candidates"]),
                            query_digest=query_digest,
                            error=(
                                "bridge deadline exceeded"
                                if result is None
                                else cause
                                if result.returncode
                                else None
                            ),
                            status=result.returncode if result else -1,
                            usage=usage,
                        )
                    ),
                    file=out,
                )
            if result is not None and result.returncode == 0:
                self.wfile.write(result.stdout)

    with tempfile.TemporaryDirectory(prefix="judge-", dir="/tmp") as tmp:
        path = str(Path(tmp) / "port")
        with socketserver.ThreadingUnixStreamServer(path, Handler) as server:
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                yield path
            finally:
                server.shutdown()
                thread.join()


if __name__ == "__main__":
    with socket.socket(socket.AF_UNIX) as peer:
        peer.settimeout(6.5)
        peer.connect(sys.argv[1])
        value = json.load(sys.stdin)
        if len(sys.argv) > 2:
            value["_trial_trace"] = sys.argv[2]
        peer.sendall(json.dumps(value).encode())
        peer.shutdown(socket.SHUT_WR)
        response = peer.makefile("rb").read()
        if not response:
            sys.exit(1)
        sys.stdout.buffer.write(response)
