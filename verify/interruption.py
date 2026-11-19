"""Import the same fault-control helper used by the embedded offline evaluator."""

import runpy
from pathlib import Path

helper = (
    Path(__file__).resolve().parents[1]
    / "crates"
    / "omoide"
    / "support"
    / "interruption.py"
)
stopped_before_commit = runpy.run_path(str(helper))["stopped_before_commit"]
