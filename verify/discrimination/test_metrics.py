"""Exercise first-page metric denominators with the frozen mixed-arity key."""

import json
import tempfile
import unittest
from pathlib import Path

from run import DATA, lines, metrics


class Discrimination(unittest.TestCase):
    def test_partial_hit_fallback_and_no_response(self):
        keys = lines(DATA / "queries.jsonl")
        multi = next(q["id"] for q in keys if len(q["must"]) == 2)
        negative = next(q["id"] for q in keys if not q["must"])
        rows = []
        for q in keys:
            returned = list(q["must"])
            state = "used"
            if q["id"] == multi:
                returned = [returned[0], q["must_not"][0]]
                state = "failed"
            if q["id"] == negative:
                returned = None
            rows.append(
                dict(
                    id=q["id"],
                    returned=returned,
                    level=q["level"],
                    judge_state=state,
                    ms=1,
                    candidates=72,
                    usage=None,
                )
            )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "sample.jsonl").write_text(
                "".join(json.dumps(r) + "\n" for r in rows)
            )
            metrics(root)
            table = (root / "metrics.md").read_text()
            n = sum(len(q["must"]) for q in keys)
            self.assertIn(
                f"| sample | all | {n - 1}/{n} | 1/152 | 1 | 150/152 | 39/40 | 1 | 2/152 | 1 |",
                table,
            )
            rows[0] = rows[1]
            (root / "sample.jsonl").write_text(
                "".join(json.dumps(r) + "\n" for r in rows)
            )
            with self.assertRaisesRegex(ValueError, "incomplete arm"):
                metrics(root)
