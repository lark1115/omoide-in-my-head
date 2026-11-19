import itertools
import unittest
from collections import Counter
from run import ARMS, DATA, plan, read
from score import neutral
from report import observe


class Protocol(unittest.TestCase):
    def test_orders_and_half(self):
        schedule = plan(
            read(DATA / "opportunities.json"), read(DATA / "grading/key.json")
        )
        self.assertEqual(
            Counter(tuple(r["arms"]) for r in schedule),
            {x: 8 for x in itertools.permutations(ARMS)},
        )
        self.assertEqual(Counter(r["half"] for r in schedule), {1: 24, 2: 24})

    def test_neutral_keeps_action(self):
        self.assertEqual(
            neutral("ledger_read m009: keep the door closed."),
            "records : keep the door closed.",
        )

    def test_distinguish_common_and_device(self):
        trial = dict(
            started_ns=100,
            opportunity="A",
            arm="device",
            hot="section",
            answer="Wait",
            tool_definitions=[],
            tokens={},
            events=[
                dict(type="call", at=101, id="1", name="ledger_read"),
                dict(type="delivery", at=102, id="1", content="ledger", error=False),
                dict(type="call", at=103, id="2", name="recall"),
                dict(
                    type="delivery",
                    at=104,
                    id="2",
                    content='{"results":[{"id":"a"}]}',
                    error=False,
                ),
                dict(type="text", at=105, text="Wait"),
            ],
        )
        o, a = observe(trial, {"memory": ["m1"]}, [], {"a": "m1"})
        self.assertEqual(
            (o["delivered_at"], o["acted_at"], o["used"], o["corrected"]),
            (4, 5, None, None),
        )
        self.assertTrue(a["all_required"])
        self.assertTrue(a["device_related_before_action"])
        self.assertEqual(a["common_ledger_bytes"], 6)
        self.assertNotIn("ledger", o["added_input"])


class LargeProtocol(unittest.TestCase):
    def test_frozen_thresholds(self):
        from large_report import verdict

        for net, over, reduction, expected in [
            (-3, 0, 1, "harm"),
            (4, 2, 1, "harm"),
            (3, 1, 1, "improvement"),
            (3, 0, 0, "inconclusive"),
            (2, 0, 1, "inconclusive"),
            (-2, 0, 1, "inconclusive"),
            (-1, 0, 0, "no practical difference"),
            (1, 1, 1, "no practical difference"),
        ]:
            self.assertEqual(verdict(net, over, reduction), expected)

    def test_short_candidate_is_visible_without_confirmed_delivery(self):
        trial = dict(
            started_ns=100,
            opportunity="A",
            arm="device_jev",
            hot="",
            answer="Check",
            tool_definitions=[],
            tokens={},
            events=[
                dict(type="call", at=101, id="1", name="recall"),
                dict(
                    type="delivery",
                    at=102,
                    id="1",
                    content='{"results":[],"candidates":[{"id":"a","version_id":"v"}],"judge_state":"used"}',
                    error=False,
                ),
                dict(type="text", at=103, text="Check"),
            ],
        )
        obs, a = observe(trial, {"memory": ["m1"]}, [], {"v": "m1"})
        self.assertIsNone(obs["delivered_at"])
        self.assertFalse(a["device_related_before_action"])
        self.assertTrue(a["deliveries"][0]["required_candidate"])


class ErrorPrivacy(unittest.TestCase):
    def test_timeout_contains_only_constant_diagnostic(self):
        import io
        import json
        import os
        import subprocess
        import tempfile
        from pathlib import Path
        from unittest.mock import patch
        import workshop

        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            archive = root / "ledger.json"
            archive.write_text("[]")
            message = {
                "id": 1,
                "method": "tools/call",
                "params": {"name": "recall", "arguments": {"scene": "workshop"}},
            }
            output = io.StringIO()
            with (
                patch.dict(
                    os.environ,
                    {
                        "TRIAL_ARCHIVE": str(archive),
                        "TRIAL_TOOL_LOG": str(root / "log"),
                        "TRIAL_BINARY": str(root / "private-binary"),
                        "TRIAL_DB": str(root / "private-db"),
                    },
                ),
                patch("sys.stdin", io.StringIO(json.dumps(message) + "\n")),
                patch("sys.stdout", output),
                patch.object(
                    subprocess,
                    "run",
                    side_effect=subprocess.TimeoutExpired(
                        [root / "private-binary", root / "private-db"], 8
                    ),
                ),
            ):
                workshop.serve()
            result = json.loads(output.getvalue())["result"]
            self.assertTrue(result["isError"])
            self.assertEqual(
                json.loads(result["content"][0]["text"]),
                {"error": "lookup exceeded the eight-second tool budget"},
            )
            self.assertNotIn(td, output.getvalue())


class EditorialReview(unittest.TestCase):
    def test_work_instruction_edits_get_a_new_ticket(self):
        from pathlib import Path
        import hashlib
        import tempfile
        from run import read, write
        from score import blind

        with tempfile.TemporaryDirectory() as td:
            study = Path(td)
            (study / "transcripts").mkdir()
            write(
                study / "transcripts" / "T00-device.json",
                {
                    "prompt": "Which pan?",
                    "answer": "I checked the record. Use the blue pan.",
                },
            )
            blind(study)
            original = read(study / "blind.json")[0]
            digest = hashlib.sha256(original["answer"].encode()).hexdigest()
            write(study / "editorial-edits.json", {digest: "Use the blue pan."})
            blind(study)
            updated = read(study / "blind.json")[0]
            self.assertNotEqual(original["ticket"], updated["ticket"])
            self.assertEqual(updated["answer"], "Use the blue pan.")
            blind(study)
            self.assertEqual(read(study / "blind.json"), [updated])
