"""Fixture routing regressions; run with the adapter directory on PYTHONPATH."""

import unittest
from pathlib import Path
from unittest.mock import patch
import scene_adapter as adapter


class JudgeModes(unittest.TestCase):
    def test_only_timeout_requests_the_short_deadline(self):
        for mode in ("timeout", "valid-empty", "no-key"):
            answer = {
                "results": [],
                "judge_state": {
                    "timeout": "failed",
                    "valid-empty": "used",
                    "no-key": "not_configured",
                }[mode],
                "matched_count": 0,
                "candidate_count": 1,
            }
            if mode == "timeout":
                answer["judge_error"] = "judge timeout"
            with patch.object(adapter, "invoke", return_value=answer) as invoke:
                adapter.observe(
                    "binary",
                    Path("db"),
                    {"op": "recall", "scene": "test", "judge": mode},
                    {},
                    {},
                    Path("fixture.py"),
                    None,
                )
            args = invoke.call_args.args[-1]
            self.assertEqual("--judge" in args, mode != "no-key")
            self.assertEqual("--timeout-ms" in args, mode == "timeout")
            if mode == "timeout":
                self.assertEqual(args[args.index("--timeout-ms") + 1], "50")


if __name__ == "__main__":
    unittest.main()
