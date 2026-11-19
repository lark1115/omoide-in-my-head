"""Check the full contents of every publication file without printing matched private values."""

import argparse
import getpass
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import tempfile


def check(root, paths):
    user = getpass.getuser()
    identity = {user, socket.gethostname(), socket.gethostname().split(".")[0]}
    for name in ("user.name", "user.email"):
        result = subprocess.run(
            ["git", "config", name], capture_output=True, text=True, cwd=root
        )
        if result.stdout.strip():
            identity.add(result.stdout.strip())
    if os.uname().sysname == "Darwin":
        for k in ("ComputerName", "LocalHostName"):
            result = subprocess.run(
                ["scutil", "--get", k], capture_output=True, text=True
            )
            if result.returncode == 0:
                identity.add(result.stdout.strip())
    identities = [
        re.compile(r"(?<![\w-])" + re.escape(x) + r"(?![\w-])", re.I)
        for x in identity
        if len(x) >= 3
    ]
    private = [str(Path.home()), str(root.resolve()), tempfile.gettempdir().rstrip("/")]
    secrets = [
        os.environ[k]
        for k in (
            "TYPESAFE_API_KEY",
            "GH_TOKEN",
            "GH_TOKEN_LARK",
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "CLAUDE_CODE_OAUTH_TOKEN",
        )
        if len(os.environ.get(k, "")) >= 12
    ]
    local_path = re.compile(
        r"/(?:Users|Volumes|private/var|var/folders|home)/[A-Za-z0-9_.-]+|/tmp/[A-Za-z0-9_.-]+"
    )
    transient = re.compile(
        r"\b(?:workshop|judge)-[a-z0-9_]{8}\b|\btoolu_[A-Za-z0-9]+|\b(?:sk|ghp)-[A-Za-z0-9_]{20,}"
    )
    failures = []
    for path in paths:
        raw = path.read_text()
        for label, violated in [
            (
                "host path",
                any(x and x in raw for x in private) or local_path.search(raw),
            ),
            ("account or host name", any(r.search(raw) for r in identities)),
            ("temporary name or credential-like token", transient.search(raw)),
            ("credential value", any(x in raw for x in secrets)),
        ]:
            if violated:
                failures.append((str(path.relative_to(root)), label))
    return failures


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("base", default="origin/main", nargs="?")
    a = p.parse_args()
    root = Path(
        subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"], text=True
        ).strip()
    )
    names = set(
        subprocess.check_output(
            ["git", "diff", "--name-only", "--diff-filter=ACMRT", a.base],
            cwd=root,
            text=True,
        ).splitlines()
    )
    names.update(
        subprocess.check_output(
            ["git", "ls-files", "--others", "--exclude-standard"], cwd=root, text=True
        ).splitlines()
    )
    findings = check(root, [root / n for n in sorted(names)])
    if findings:
        raise SystemExit(json.dumps(findings, ensure_ascii=False))
    print(f"公開対象 {len(names)} ファイルの全文検査に合格")
