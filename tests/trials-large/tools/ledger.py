"""Reference byte accounting for an isolated session; no model or subprocess."""
import json
import unicodedata

BUDGET = 24576
PAGE_BYTES = 4096
PAGE_ROWS = 8


def fold(s):
    return unicodedata.normalize("NFKC", s).casefold()


def payload(rows, more):
    return json.dumps({"rows":rows, "more":more}, sort_keys=True, ensure_ascii=False,
                      separators=(",", ":")).encode("utf-8")


class Ledger:
    def __init__(self, records):
        self.records = records
        self.bytes_read = 0
        self.calls = 0
        self.closed = False
        self.offsets = {}

    def page(self, term=None):
        """List, or literal substring search; repeating an operation continues its cursor."""
        self.calls += 1
        if self.closed:
            return b""
        if term is not None and (not isinstance(term, str) or not term.strip()):
            out = b'{"error":"expected a nonempty search string"}'
            if self.bytes_read + len(out) > BUDGET:
                self.closed = True
                return b""
            self.bytes_read += len(out)
            return out
        needle = None if term is None else fold(term.strip())
        remaining = min(PAGE_BYTES, BUDGET-self.bytes_read)
        cursor = self.offsets.get(needle, 0)
        selected = []
        more = False
        for i in range(cursor, len(self.records)):
            row = self.records[i]
            if needle is not None and not any(
                needle in fold(row.get(k,"")) for k in ("target", "scene", "action", "text")
            ):
                continue
            if len(selected) == PAGE_ROWS or len(payload(selected + [row], False)) > remaining:
                self.offsets[needle] = i
                more = True
                break
            selected.append(row)
        else:
            self.offsets[needle] = len(self.records)
        if more and not selected:
            self.closed = True
            return b""
        out = payload(selected, more)
        if len(out) > remaining:
            self.closed = True
            return b""
        self.bytes_read += len(out)
        return out
