"""During stage three, permit only the exact deferred no-test findings."""
import json
import subprocess

deferred = {"REQ-HOT-1"}
out = subprocess.check_output(["miwa", "check", "--format", "json"])
findings = json.loads(out)["findings"]
actual = {(f["kind"], f["target"]) for f in findings}
expected = {("NoTest", req) for req in deferred}
if actual != expected:
    raise SystemExit(f"Unexpected miwa findings: {findings}")
print("No drift. Deferred no-test requirements:", ", ".join(sorted(deferred)))
