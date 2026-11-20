# Registration calibration

Fixture and pair split were frozen in 45aba5c before the baseline. Product selection was frozen in 786462d before the new TEST measurement. Only invented descriptions from the public fixtures enter this repository. An additional local replay of the supplied two-rule field reproduction changed from a near prompt on both action variants to immediate activation on both.

## Measurements

The CLI registered each of the 72 discrimination rules against the other 71 active rules. The harness builds a common base, copies it into separate SQLite stores and marks the incoming rule's existing version non-current. The incoming draft uses only scene, text, structured target and action. The domain, gold groups and key_phrase never enter its registration. In a fresh store opaque version IDs differ, but equal strengths sort by scene, body and then version ID. The 72 descriptions are distinct.

| Pair split | Build | Positive comparisons delivered | Negative comparisons returned | Mean neighbors | Maximum |
|---|---|---:|---:|---:|---:|
| DEV | Baseline | 320/320 | 364/2,272 | 18.78 | 35 |
| DEV | Selected | 212/320 | 10/2,272 | 5.31 | 9 |
| TEST | Baseline | 256/256 | 360/2,264 | 17.33 | 31 |
| TEST | Selected | 146/256 | 4/2,264 | 5.03 | 8 |
| All | Baseline | 576/576 | 724/4,536 | 18.06 | 35 |
| All | Selected | 358/576 | 14/4,536 | 5.17 | 9 |

Pair membership uses the parity of the lower numeric rule ID. Each undirected pair contributes two registration directions. The mean and maximum split the 72 incoming drafts by their own parity. Nine was the smallest display budget retaining all selected DEV positives: eight dropped one direction, six dropped 18. None of the actual selected lists of 72 trials exceeded nine. A separate CLI test constructs a larger set, verifies ordering, rejects a guessed hidden version and invalidates the previous digest when only an omitted neighbor changes.

All 16 pairs with identical explicit target and action passed both ways; their lower IDs happen to be odd. The even split contains only same-domain co-list positives. Of 128 such positive pairs, 55 had neither an independent lexical anchor nor a witnessed sibling subject and were missed. Confusable instruments with the same action and otherwise unrelated wording, and translations without lexical anchors, are examples of the limit. The mechanically labelled recall loss is real; these numbers cannot establish recall for every true conflict. The fixed Japanese acceptance cases covered ten concrete conflicts and 435 unrelated pairs.

## Development alternatives

Keeping all hiragana at a floor of 25, with untethered target and action priors and a trigger bonus, flagged all 435 Japanese negatives sharing a coarse target/action. Filtering grammar alone still left eight false prompts with that unconditional target bonus. The selected prior needs the common subject in both triggers. An unstructured 15% coverage cut after filtering, with no priors, detected 168/320 DEV positives and wrongly flagged 20/2,272 negatives. At 25%, corroborated priors and trigger weighting without the sibling-subject route produced 176/320 and 10/2,272. Allowing a meaningful named subject component in both triggers under the same action recovered 36 positive directions, at the same negative count. For a separate DEV-only sensitivity check, computing both numerator and denominator after removing every identifier token on its own side gave 210/320 positives and 16/2,272 negatives. The selected rule requires an independent anchor and preserves other corroborating content in the coverage fraction. A common ideograph in arbitrary trigger text was less discriminating. The labelled disagreement is left visible; no judge, translation dictionary or domain-specific exceptions were added.

From the root, use `python3 verify/near/measure.py BINARY OUTPUT.json` on each build. baseline.json and selected.json preserve the returned logical IDs and omitted counts. `python3 verify/near/sweep.py --report` rebuilds the compact DEV ablation from the frozen cases. Run it without that flag to inspect the full parameter screen. The independent Python tokenizer covers the scripts in these fixtures; the reported before/after results came from the actual binary.

## Verification

The final CLI and core were tested with the workspace suite, relation/interruption/resend harness, the hot-file contract and the frozen scene adapter. Core retained the usual three S1 misses without a judge. S10 and S11 passed; all eight S1 cases passed with the live adapter on a warm verification run (the first run had one failed judge state). All 62 cases passed on their prescribed routes.

Cargo-mutants 27.1.0 on an isolated Rust 1.88 Linux copy: the core pass generated 192 mutations (182 caught initially, five unviable, five survivors); targeted tests killed all five survivors on rerun. The two binary files generated 114 mutations, 106 caught and eight unviable, with no survivor or timeout. The targeted core tests exercise empty input, the multiplication in coverage, an identifier present on only one side, unknown targets and target names absent from triggers.

Independent literal/field substitutions compiled and were caught for aliases, actions, target, body, trigger, score field, omitted-count field, hidden digest membership, priority order, maximum and version tie order. See literal_checks.py. Tests that deliberately store similar rules to exercise recall pagination and independent FTS caps now register their explicit unrelated relation before checking retrieval.
