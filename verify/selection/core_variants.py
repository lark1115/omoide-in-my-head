"""Reproduce the DEV-only core experiment in a temporary checkout of the baseline."""

import itertools
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from study import DATA, ROOT, lines, seed

BASE = "d9be551"
INSTRUMENT = """
    let parameter = |name: &str| std::env::var(name).ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
    let floor = parameter("STUDY_FLOOR");
    let relative = parameter("STUDY_RELATIVE");
    let emphasize = parameter("STUDY_CONDITION");
    let structured = parameter("STUDY_STRUCTURED") > 0;
    let scores: BTreeMap<_,_> = ranked.iter().filter(|r| pool[r.id].1["kind"] == "rule").map(|r| {
        let record = &pool[r.id].1;
        let condition = tokens(record["scene"].as_str().unwrap_or_default());
        let common = tokens(&format!("{} {}",record["target"].as_str().unwrap_or_default(),record["actions"]));
        let specific = Tokens {words:condition.words.difference(&common.words).cloned().collect(),
                               singles:condition.singles.difference(&common.singles).cloned().collect()};
        (r.id, r.hits + emphasize * omoide_core::hits(&terms,&specific))
    }).collect();
    let best = scores.values().copied().max().unwrap_or(0);
    ranked.retain(|r| {
        let Some(score)=scores.get(r.id) else {return true};
        if r.action {return true}
        if structured && query.action.is_some() && query.target.is_some() {return false}
        score * 100 >= best * relative && r.hits * 100 >= terms.words.len() * floor
    });
"""


def main(out):
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        with subprocess.Popen(
            ["git", "-C", ROOT, "archive", BASE], stdout=subprocess.PIPE
        ) as archive:
            subprocess.run(["tar", "-x", "-C", root], stdin=archive.stdout, check=True)
        path = root / "crates/omoide/src/search.rs"
        s = path.read_text()
        path.write_text(
            s.replace(
                "    let mut judge_state =", INSTRUMENT + "    let mut judge_state ="
            )
        )
        subprocess.run(
            ["cargo", "build", "--locked", "--release", "-p", "omoide"],
            cwd=root,
            check=True,
        )
        binary = root / "target/release/omoide"
        db = root / "rules.db"
        mapping = seed(binary, db)
        qs = [q for q in lines(DATA / "queries.jsonl") if int(q["id"][1:]) % 2]
        results = Path(__file__).with_name("results")
        grid = [
            r["params"]
            for f in ("dev-core-grid", "dev-core-refinement", "dev-core-compatible")
            for r in lines(results / (f + ".jsonl"))
        ]
        for params in grid:
            env = {
                **os.environ,
                **dict(
                    zip(
                        (
                            "STUDY_FLOOR",
                            "STUDY_RELATIVE",
                            "STUDY_CONDITION",
                            "STUDY_STRUCTURED",
                        ),
                        map(str, params),
                    )
                ),
            }
            nums = [0] * 6
            for q in qs:
                args = [
                    binary,
                    "--db",
                    db,
                    "recall",
                    *itertools.chain.from_iterable(
                        ("--" + k, v) for k, v in q["input"].items()
                    ),
                ]
                r = json.loads(subprocess.check_output(args, env=env))
                ret = {mapping[x["version_id"]] for x in r["results"]}
                must = set(q["must"])
                for i, v in enumerate(
                    [
                        len(ret & must),
                        len(must),
                        bool(ret.intersection(q["must_not"])),
                        ret == must,
                        not ret and not must,
                        not must,
                    ]
                ):
                    nums[i] += v
            print(
                json.dumps(
                    dict(
                        params=params,
                        required=nums[:2],
                        forbidden=nums[2],
                        exact=nums[3],
                        none=nums[4:],
                    )
                ),
                file=out,
                flush=True,
            )


if __name__ == "__main__":
    main(sys.stdout)
