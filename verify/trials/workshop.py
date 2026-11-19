"""Read-only stdio MCP server for the frozen workshop. No filesystem or arbitrary-command tool."""

import json
import os
from pathlib import Path
import subprocess
import sys
import time


def compact(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def schema(name, description, fields, required=()):
    return {
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": {
                k: {"type": "string", "description": v} for k, v in fields.items()
            },
            "required": list(required),
            "additionalProperties": False,
        },
    }


LEDGER = [
    schema(
        "ledger_list", "List record IDs, types and subjects in the workshop ledger.", {}
    ),
    schema(
        "ledger_search",
        "Search the workshop ledger by case-insensitive literal substring. Returns matching original records.",
        {"text": "Text to find"},
        ["text"],
    ),
    schema(
        "ledger_read",
        "Read a complete original workshop ledger record.",
        {"id": "Ledger ID"},
        ["id"],
    ),
]
DEVICE = [
    schema(
        "recall",
        "Find applicable workshop records by situation or intended action. Subject and kind are optional.",
        {
            "scene": "Situation in free text",
            "action": "Action name",
            "target": "Subject or project",
            "kind": "rule, decision, fact or experience",
        },
    ),
    schema(
        "show",
        "Read one returned record and its full source.",
        {"id": "Returned record or version ID"},
        ["id"],
    ),
]


def serve(*, ledger_tools=LEDGER, ledger_call=None):
    records = json.loads(Path(os.environ["TRIAL_ARCHIVE"]).read_text())
    tools = ledger_tools + (DEVICE if "TRIAL_DB" in os.environ else [])
    definitions = {t["name"]: t for t in tools}

    def call(name, args):
        spec = definitions[name]["inputSchema"]
        if (
            not isinstance(args, dict)
            or not set(args) <= spec["properties"].keys()
            or not set(spec["required"]) <= args.keys()
        ):
            raise ValueError("invalid tool arguments")
        if ledger_call and name.startswith("ledger_"):
            return ledger_call(name, args, records)
        if not all(isinstance(v, str) for v in args.values()):
            raise ValueError("expected string arguments")
        if name == "ledger_list":
            return compact(
                [{k: r[k] for k in ("id", "kind", "target")} for r in records]
            )
        if name == "ledger_read":
            return compact(next((r for r in records if r["id"] == args["id"]), None))
        if name == "ledger_search":
            term = args["text"].casefold()
            return compact([r for r in records if term in compact(r).casefold()])
        cli = [os.environ["TRIAL_BINARY"], "--db", os.environ["TRIAL_DB"], name]
        if name == "show":
            cli.append(args["id"])
        else:
            for key, value in args.items():
                cli += ["--" + key, value]
        if name == "recall" and os.environ.get("TRIAL_JUDGE_SOCKET"):
            cli += [
                "--judge",
                sys.executable,
                "--judge-arg",
                str(Path(__file__).with_name("jev_bridge.py")),
                "--judge-arg",
                os.environ["TRIAL_JUDGE_SOCKET"],
                "--timeout-ms",
                "6800",
                "--threshold",
                "0.60",
            ]
        proc = subprocess.run(
            cli,
            capture_output=True,
            text=True,
            timeout=8,
            env={"PATH": os.defpath, "LANG": "en_US.UTF-8"},
        )
        # Keep the complete JSON including states, conditions and truncation markers.
        value = json.loads(proc.stdout)
        if proc.returncode:
            raise ValueError(value["message"])
        return proc.stdout.rstrip("\n")

    for line in sys.stdin:
        req = json.loads(line)
        if "id" not in req:
            continue
        method = req["method"]
        reply = {"jsonrpc": "2.0", "id": req["id"]}
        if method == "initialize":
            result = {
                "protocolVersion": req["params"]["protocolVersion"],
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "workshop", "version": "1"},
            }
        elif method == "tools/list":
            result = {"tools": tools}
        elif method == "tools/call":
            p = req["params"]
            began = time.monotonic_ns()
            try:
                raw = call(p["name"], p.get("arguments", {}))
                error = False
            except subprocess.TimeoutExpired:
                raw = compact({"error": "lookup exceeded the eight-second tool budget"})
                error = True
            except (KeyError, ValueError) as exc:
                raw = compact({"error": str(exc)})
                error = True
            result = {"content": [{"type": "text", "text": raw}], "isError": error}
            with Path(os.environ["TRIAL_TOOL_LOG"]).open("a") as log:
                print(
                    compact(
                        {
                            "tool": p["name"],
                            "args": p.get("arguments", {}),
                            "text": raw,
                            "error": error,
                            "started_ns": began,
                            "returned_ns": time.monotonic_ns(),
                        }
                    ),
                    file=log,
                )
        elif method == "ping":
            result = {}
        else:
            reply["error"] = {"code": -32601, "message": "Method not found"}
            print(compact(reply), flush=True)
            continue
        reply["result"] = result
        print(compact(reply), flush=True)


if __name__ == "__main__":
    serve()
