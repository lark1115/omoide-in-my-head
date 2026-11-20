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


class FailureArtifacts(unittest.TestCase):
    def test_nonpasses_are_written_immediately_and_passes_are_not(self):
        import json, os, tempfile

        with (
            tempfile.TemporaryDirectory() as root,
            patch.dict(os.environ, {"OMOIDE_EVAL_FAILURE_DIR": root}),
        ):
            dirs = []
            before = adapter.CaseResults("before", dirs)
            after = adapter.CaseResults("after", dirs)
            before.append({"case": "ok", "status": "pass"})
            self.assertEqual(dirs, [])
            row = {
                "case": "example",
                "status": "not_met",
                "errors": ["failure detail"],
                "observed": {"state": "current"},
            }
            before.append(row)
            self.assertEqual(len(dirs), 1)
            self.assertEqual(
                json.loads((dirs[0] / "before-0002.json").read_text()), row
            )
            other = {"case": "future", "status": "not_run", "errors": ["not available"]}
            after.append(other)
            self.assertEqual(
                json.loads((dirs[0] / "after-0001.json").read_text()), other
            )
            self.assertEqual(dirs[0].stat().st_mode & 0o777, 0o700)


if __name__ == "__main__":
    unittest.main()
