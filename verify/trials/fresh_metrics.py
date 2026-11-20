"""Apply the two prospective verdicts and the secondary endpoint definitions."""

import statistics
from collections import Counter
from pathlib import Path
from run import read, write
from large_report import verdict
from fresh import ARMS


def summarize(out):
    observed = read(out / "observed.json")
    detailed = read(out / "ledger-and-device.json")
    lookups = read(out / "lookup-coverage.json")
    lines = [
        "| Primary comparison (44 pairs) | Net passes | Reduction in known mistakes on 36 targets | Net control overapplications | Frozen verdict |",
        "|---|---:|---:|---:|---|",
    ]

    def count(arm, label, cohorts):
        return sum(
            r["arm"] == arm and r["label"] == label and r["cohort"] in cohorts
            for r in observed
        )

    for arm in ARMS[2:]:
        net = count(arm, "pass", ("primary", "control")) - count(
            "no_device", "pass", ("primary", "control")
        )
        fewer = count("no_device", "known_mistake", ("primary",)) - count(
            arm, "known_mistake", ("primary",)
        )
        over = count(arm, "known_mistake", ("control",)) - count(
            "no_device", "known_mistake", ("control",)
        )
        lines.append(
            f"| {arm} vs no_device | {net:+d} | {fewer:+d} | {over:+d} | {verdict(net, over, fewer)} |"
        )
    lines += [
        "",
        "| Arm | Used search / 48 | Search calls per session (mean / median / max) | Search bytes per session (mean / median / max) | List calls per session (mean / median / max) | List bytes per session (mean / median / max) |",
        "|---|---:|---|---|---|---|",
    ]
    for arm in ARMS:
        group = [r for r in detailed if r["arm"] == arm]

        def values(field):
            v = [r[field] for r in group]
            return f"{statistics.mean(v):.2f} / {statistics.median(v):g} / {max(v)}"

        lines.append(
            f"| {arm} | {sum(r['searches'] > 0 for r in group)}/48 | {values('searches')} | {values('search_bytes')} | {values('lists')} | {values('list_bytes')} |"
        )
    lines += [
        "",
        "| Arm | Qualifying opportunities | Returned to ledger | Already used ledger (returned / eligible) | No prior ledger use (returned / eligible) | No recall | Only nonqualifying replies | Unobservable / no pre-action delivery |",
        "|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    per_session = []
    for arm in ARMS[2:]:
        counts = Counter()
        for t in lookups:
            if t["arm"] != arm:
                continue
            qs = t["lookups"]
            pre = [q for q in qs if q["before_action"] is True]
            first = next(
                (q for q in pre if q["category"] not in ("neither", "unobservable")),
                None,
            )
            status = (
                "qualifying"
                if first
                else "no recall"
                if not qs
                else "unobservable"
                if not pre or any(q["category"] == "unobservable" for q in pre)
                else "neither"
            )
            counts[status] += 1
            row = dict(opportunity=t["opportunity"], arm=arm, status=status)
            if first:
                prior = first["prior_ledger"]
                returned = first["later_ledger_before_action"]
                counts[("eligible", prior)] += 1
                counts[("returned", prior)] += int(returned)
                row.update(
                    category=first["category"],
                    first_call=first["call"],
                    prior_ledger=prior,
                    returned=returned,
                )
            per_session.append(row)
        lines.append(
            f"| {arm} | {counts['qualifying']} | {counts['returned', True] + counts['returned', False]} | {counts['returned', True]}/{counts['eligible', True]} | {counts['returned', False]}/{counts['eligible', False]} | {counts['no recall']} | {counts['neither']} | {counts['unobservable']} |"
        )
    write(out / "ledger-return.json", per_session)
    (out / "endpoints.md").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    import sys

    summarize(Path(sys.argv[1]))
