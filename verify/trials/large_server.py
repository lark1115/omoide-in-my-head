"""Expose the exact frozen paging implementation from the host."""

from workshop import schema, serve

LARGE_LEDGER = [
    schema(
        "ledger_list",
        "Read the next page of original workshop records in filing order.",
        {},
    ),
    schema(
        "ledger_search",
        "Read the next page matching a literal substring of subject, scene, action or text. Repeating the search continues it.",
        {"text": "Text to find"},
        ["text"],
    ),
]
if __name__ == "__main__":
    import os, json, fcntl
    from pathlib import Path
    from ledger import Ledger

    def call(name, args, records):
        path = Path(os.environ["TRIAL_TOOL_LOG"]).with_suffix(".budget")
        with path.open("a+") as f:
            fcntl.flock(f, fcntl.LOCK_EX)
            f.seek(0)
            previous = f.read()
            ledger = Ledger(records)
            if previous:
                state = json.loads(previous)
                ledger.bytes_read = state["bytes"]
                ledger.closed = state["closed"]
                ledger.offsets = dict(state["cursors"])
            result = ledger.page(
                None if name == "ledger_list" else args.get("text", "")
            )
            f.seek(0)
            f.truncate()
            json.dump(
                {
                    "bytes": ledger.bytes_read,
                    "closed": ledger.closed,
                    "cursors": list(ledger.offsets.items()),
                },
                f,
            )
            return result.decode()

    serve(ledger_tools=LARGE_LEDGER, ledger_call=call)
