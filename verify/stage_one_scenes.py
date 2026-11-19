"""Legacy verification entry point; use omoide eval run for a machine-readable report."""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "crates" / "omoide" / "support"))
from scene_adapter import choose, invoke, main, version

__all__ = ["choose", "invoke", "main", "version"]

if __name__ == "__main__":
    raise SystemExit(main())
