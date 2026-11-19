"""All release requirements now have tests; any finding fails the check."""

import json
import subprocess

findings = json.loads(subprocess.check_output(["miwa", "check", "--format", "json"]))[
    "findings"
]
if findings:
    raise SystemExit(f"Unexpected miwa findings: {findings}")
print("miwa: no drift or missing tests")
