"""Pass recorded provider responses through the production decoder and omoide's real judge port."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("binary", type=Path)
parser.add_argument("adapter", type=Path)
parser.add_argument("recorded_driver", type=Path)
args = parser.parse_args()
env = dict(os.environ)
env.pop("TYPESAFE_API_KEY", None)
fixtures = (
    Path(__file__).resolve().parents[1] / "crates/omoide-judge-jev/tests/fixtures"
)

with tempfile.TemporaryDirectory() as temp:
    root = Path(temp)

    def run(*command):
        return json.loads(
            subprocess.check_output(
                [args.binary.resolve(), "--db", root / "test.db", *command], env=env
            )
        )

    run("init")
    source = run("say", "--text", "Wrap with paper")["id"]
    rule = run(
        "rule",
        "add",
        "--text",
        "Wrap with paper",
        "--scene",
        "packing",
        "--source",
        source,
    )["id"]

    def check(command, state, ids):
        r = run(
            "recall", "--scene", "packing", "--timeout-ms", "4000", "--judge", *command
        )
        assert r["judge_state"] == state and [v["id"] for v in r["results"]] == ids, r

    check([args.adapter.resolve()], "failed", [rule])
    check(
        [args.recorded_driver.resolve(), "--judge-arg", fixtures / "s1-response.json"],
        "used",
        [rule],
    )
    check(
        [
            args.recorded_driver.resolve(),
            "--judge-arg",
            fixtures / "s1-negative-response.json",
        ],
        "used",
        [],
    )
    raw = json.loads((fixtures / "s1-response.json").read_text())
    for answers in [
        {},
        {"q0": {"type": "choice", "noul": 0.9}},
        {"q0": {"type": "noul", "noul": 1.1}},
        {**raw["answers"], "unknown": raw["answers"]["q0"]},
    ]:
        path = root / "reply.json"
        path.write_text(json.dumps({**raw, "answers": answers}))
        check([args.recorded_driver.resolve(), "--judge-arg", path], "failed", [rule])
    print(
        "PASS: absent key, recorded yes/no, missing/wrong/out-of-range/unknown answers"
    )
