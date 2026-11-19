#!/usr/bin/env python3
"""Protocol fixture. Oracle scores demonstrate wiring only; never a quality estimate."""

import json
import sys
import time

request = json.load(sys.stdin)
mode = sys.argv[1]
if mode == "timeout":
    time.sleep(60)
if mode == "empty":
    print('{"answers":[]}')
elif mode == "oracle":
    with open(sys.argv[2], encoding="utf-8") as stream:
        expected = json.load(stream)[request["query"]["scene"]]
    print(
        json.dumps(
            {
                "answers": [
                    {"version_id": c["version_id"], "score": int(c["text"] in expected)}
                    for c in request["candidates"]
                ]
            }
        )
    )
else:
    raise SystemExit("unknown fixture mode")
