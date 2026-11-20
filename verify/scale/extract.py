"""Extract only distinct valid recall arguments; inspect the invented text before publishing."""

import argparse
import json
from collections import defaultdict
from pathlib import Path


def extract(folder):
    output = defaultdict(list)
    for path in sorted(folder.glob("*.json")):
        record = json.loads(path.read_text())
        for call in record["server_events"]:
            if call["tool"] != "recall":
                continue
            q = call["args"]
            if not (q.get("scene") or q.get("action")):
                continue
            if not set(q) <= {"scene", "action", "target", "kind"} or not all(
                isinstance(v, str) for v in q.values()
            ):
                raise ValueError("unexpected lookup field; inspect privately")
            if q not in output[record["opportunity"]]:
                output[record["opportunity"]].append(q)
    return output


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("private_transcripts", type=Path)
    parser.add_argument("output", type=Path)
    a = parser.parse_args()
    with a.output.open("x") as out:
        json.dump(extract(a.private_transcripts), out, ensure_ascii=False, indent=2)
        out.write("\n")
