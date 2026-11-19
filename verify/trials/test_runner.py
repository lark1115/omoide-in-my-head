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
