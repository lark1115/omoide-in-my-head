"""Validate the finite distractor vocabulary, scope audit, archive and access budget."""
import argparse
import csv
import hashlib
import json
import sys
from collections import Counter
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from generate import BASE, ROOT, SEED, archive_bytes, build, opaque
from ledger import BUDGET, PAGE_BYTES, PAGE_ROWS, Ledger, payload


def check(ok, message):
    if not ok:
        raise ValueError(message)


def read(name):
    return json.loads((BASE / name).read_text(encoding="utf-8"))


def sha(data):
    return hashlib.sha256(data).hexdigest()


def certified_disjoint(r, subjects, origins, originals):
    # Different entities are distinct in this closed workshop vocabulary. For identical
    # entities there are exactly three independently checked reasons to disregard a row.
    if r["target"] not in subjects:
        return True
    if r["kind"] == "rule":
        return (r.get("condition") == "simulation-only" and
                r.get("exceptions") == ["ordinary operations"] and
                any(t in r["scene"] and t in r["text"] for t in ("講習用の模擬作業", "classroom simulation")) and
                any(t in r["text"] for t in ("実用品には適用しない", "Not for ordinary workshop stock")))
    if r["kind"] == "decision":
        seen = set()
        while r.get("state") == "superseded" and r["id"] not in seen:
            seen.add(r["id"])
            nxt = origins[r["successor"]]
            if nxt["target"] != r["target"] or nxt["at"] <= r["at"]:
                return False
            r = nxt
        return r["id"] in originals and r.get("state") == "decided"
    if r["kind"] == "experience":
        date = r.get("period", "")
        return (date.startswith("2041-") and date in r["text"] and
                any(t in r["text"] for t in ("旧ロットだけ", "closed lot dated")))
    return False


def inspect():
    data = build()
    raw = archive_bytes(data)
    rows = [r for _, _, r in data]
    originals = {r["id"] for _, k, r in data if k == "original"}
    origins = {r["id"]:r for r in rows}
    check(len(origins) == len(rows) == 3048 and len(originals) == 48, "duplicate ID or size")
    for base in read("memory.json"):
        remapped = dict(base, id=opaque(base["id"]))
        if "evidence" in remapped:
            remapped["evidence"] = opaque(remapped["evidence"])
        check(origins[remapped["id"]] == remapped, base["id"])
    extras = [r for r in rows if r["id"] not in originals]
    check(len({r["text"] for r in extras}) == 3000, "duplicate distractor text")
    for r in rows:
        check(len(payload([r], False)) <= PAGE_BYTES, "record cannot fit page")
        for field in ("successor","evidence"):
            check(field not in r or r[field] in origins, r["id"])
    for r in extras:
        if r["kind"] == "fact":
            check(r.get("certainty") == "reported", "extra fact elevated to verified")

    keys = {k["id"]:k for k in read("grading/key.json")}
    tasks = {t["id"] for t in read("opportunities.json")}
    with (ROOT/"scope.tsv").open(encoding="utf-8") as f:
        scopes = list(csv.DictReader(f, delimiter="\t"))
    check(len(scopes) == len(keys) == 48 and {s["id"] for s in scopes} == tasks == keys.keys(), "coverage")
    worst = 0
    for s in scopes:
        key = keys[s["id"]]
        subjects = set(s["subjects"].split("|"))
        check(key["target"] in subjects and s["reading"], s["id"])
        for r in extras:
            check(certified_disjoint(r, subjects, origins, originals), f"{s['id']}: {r['id']}")
        needed = {opaque(k) for k in key["memory"]}
        if not needed:
            check(s["witness"] == "-", s["id"])
            continue
        check({origins[k]["target"] for k in needed} <= subjects, s["id"])
        ledger = Ledger(rows)
        while needed:
            reply = ledger.page(s["witness"])
            check(reply, f"{s['id']}: witness exceeded budget")
            page = json.loads(reply)
            needed -= {r["id"] for r in page["rows"]}
            check(not needed or page["more"], f"{s['id']}: witness missed support")
        worst = max(worst,ledger.bytes_read)

    ledger = Ledger(rows)
    displayed = set()
    while out := ledger.page():
        page = json.loads(out)
        check(len(out) <= PAGE_BYTES and len(page["rows"]) <= PAGE_ROWS, "oversize")
        displayed.update(r["id"] for r in page["rows"])
        if not page["more"]:
            break
    check(ledger.bytes_read <= BUDGET < len(raw) and len(displayed) < len(rows), "dump possible")
    # A same-target distractor cannot become current merely by changing its label.
    for key, family, r in data:
        bad = dict(r)
        if family == "rehearsal":
            bad["condition"] = "ordinary operations"
        elif family == "revision":
            bad["state"] = "decided"
        elif family == "closed-lot":
            bad["period"] = "2042-03-08"
        else:
            continue
        check(not certified_disjoint(bad, {r["target"]}, origins, originals), key)
    # Switching from list to search must not reset the session's byte allowance.
    mixed = Ledger(rows)
    delivered = 0
    for term in [None, "room", "", "台紙"] * 200:
        out = mixed.page(term)
        delivered += len(out)
        check(delivered == mixed.bytes_read <= BUDGET, "shared byte allowance")
        if mixed.closed:
            break
    check(mixed.closed and mixed.page("reed") == b"", "exhaustion must persist")
    frozen = read("manifest.json")["sha256"]
    for name, digest in frozen.items():
        check(sha((BASE/name).read_bytes()) == digest, name)
    return dict(version=1, seed=SEED, file_format="canonical-jsonl-utf8-lf", rows=len(rows),
                families=dict(sorted(Counter(k for _, k, _ in data).items())),
                kinds=dict(sorted(Counter(r["kind"] for r in rows).items())),
                archive_bytes=len(raw), archive_sha256=sha(raw), frozen_inputs=frozen,
                scope_sha256=sha((ROOT/"scope.tsv").read_bytes()),
                per_session_bytes=BUDGET, per_page_bytes=PAGE_BYTES, max_rows_per_page=PAGE_ROWS,
                opportunities=len(scopes), excluded_from_condition=[], headline=44, planned_trials=144,
                scope_comparisons=len(scopes)*len(extras), maximum_witness_bytes=worst)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--archive", type=Path, help="also validate the generated artifact on disk")
    args = parser.parse_args()
    result = inspect()
    if args.archive:
        check(sha(args.archive.read_bytes()) == result["archive_sha256"], "generated file digest")
    check(result == json.loads((ROOT/"manifest.json").read_text()), "manifest mismatch")
    print(f"{result['rows']}件・48機会・{result['scope_comparisons']}組の適用範囲を確認")
