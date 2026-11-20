# Current-store registration evidence and S11 diagnostics

The two new Japanese shapes and their labels were written at 674fb09 before measuring the baseline. The unrelated set has ten distinct scene/manual instructions and fifteen different procedures containing common words. Five further instructions are related to named manuals. No private rules were used. The fixed original 30+10 rows and the 72 discrimination rules were untouched.

## Registration results

| Dataset | Method | False prompts | False neighbor directions | Maximum displayed for an unrelated draft |
|---|---|---:|---:|---:|
| Ten manuals, each against the other 24 independent rules | PR 25 | 10/10 | 90 | 9 |
| Same | Revised | 0/10 | 0 | 0 |
| Fifteen common-word procedures against the other 24 | PR 25 | 14/15 | 26 | 3 |
| Same | Revised | 11/15 | 20 | 2 |

All five required pairs were delivered by both builds. The baseline returned 40 extra unrelated neighbors on those positive drafts; the revision returned zero. Sequential import of the 25 independent rules went from 17 prompts / max 9 to 7 prompts / max 2. The cap target is met, but 11 common-word rules still prompt on the leave-one-out measure. No claim about the private pilot's success rate follows. On the earlier Japanese file, all 435 unrelated pairs in both directions remained separate, and all ten concrete related pairs were shown in both directions.

| Split | Method | Broad positive directions delivered | Negative directions returned | Mean / max displayed |
|---|---|---:|---:|---:|
| DEV | PR 25 | 212/320 | 10/2,272 | 5.31 / 9 |
| DEV | Revised | 198/320 | 0/2,272 | 4.72 / 7 |
| TEST | PR 25 | 146/256 | 4/2,264 | 5.03 / 8 |
| TEST | Revised | 134/256 | 2/2,264 | 4.56 / 7 |
| All | PR 25 | 358/576 | 14/4,536 | 5.17 / 9 |
| All | Revised | 332/576 | 2/4,536 | 4.64 / 7 |

The lower numeric rule ID sets pair parity as in PR 25. Mean/max group incoming drafts by their own parity. DEV screening allowed either cross-field evidence or shared trigger evidence, and compared frequency slopes 0/100/200/300/400 and trigger coverage. Rarity on shared body terms alone retained 212 positive and 10 false directions on DEV. Requiring a shared informative scene word without minimum scene coverage gave 208 and 4. Requiring weighted coverage equivalent to 20% of the shorter trigger gave 198 and zero with slope 200, compared to 196 and zero with slope 300. A minimum distinguishing weight of 75 was the stricter of tied settings. The selected 4% minimum document mass excludes weak names in very long instructions. An exact or equivalent trigger, explicit rare alias, and witnessed rare subject retain the conflict-preserving paths described in ADR-0004. Missing real conflicts remains costly: TEST lost 12 more broad positive directions than PR 25.

The numerical policy was frozen at ffc3e7b before the single confirmatory runs. A subsequent code review restricted the exact-body shortcut for a repeatedly identical literal instruction in different scenes. This concerns identical bodies only; all 72 bodies in the mechanical comparison are distinct, as are all 30 bodies in the new file. Neither confirmation was rerun or used to choose a new threshold. A separate regression covers the identical-literal edge. Run screen.py to reproduce the DEV screen. The real first-page IDs are in selected-72.json and the prior study's selected.json; use context_check.py to exercise the new Japanese archive.

## Interruption diagnosis

A fresh macOS release/fault build of PR 25 ran the full 62-case set 20 times. S11-current-ja failed in run 1 with “writer did not reach the pre-commit stop”. The usual three S1 lexical misses occurred on every run. Three full iterations completed during a parallel clean all-target build. The exact error from the architect's earlier run was not retained; its specific cause cannot be recovered. A pre-PR-25 report and the verification note in PR 21 already document the same S11 pre-commit-stop timeout.

The old watchdog charged executable startup and transaction execution to one 15-second clock and could exit its loop without a final waitpid observation. The revised fault binary signals entry to Rust main on a private test channel before argument parsing. The runner allows 120 seconds before readiness and keeps 15 seconds for the actual pre-commit stop. An older fault binary that omits the marker can still pass only by actually stopping. The runner checks child status before declaring a timeout, always kills and reaps the child, and never accepts mere elapsed time as a successful interruption.

Two series of 20 full revised runs completed with no S11 failure (240/240 S11 cases). Four full iterations completed during a 91-second clean build. In the first revised series an existing environment filter also stripped the optional diagnostic output settings. The second series preserved the two diagnostic settings and checked the saved per-case files against the returned rows. Among its 80 real stops, the first writer took **39,634 ms from launch until readiness was observed, then 13 ms until SIGSTOP**. Median observed startup was 9 ms; the longest readiness-to-stop interval was 35 ms. No specific OS service is identified by this measurement. The measured pre-readiness delay alone exceeds the original timeout.

A separate controlled delayed-launch probe used the same real write, with a 16-second delay before exec. The old harness timed out; the new one observed SIGSTOP, killed the writer and left the database at zero utterances and its original initialization receipt. The store contention test independently exhibited the same combined-startup-deadline testing assumption; its worker now signals entry, the other writers are launched while the victim holds its uncommitted transaction, and timeout cleanup kills/reaps the victim. No product write or rollback condition was weakened.

scenes-before.jsonl, scenes-after.jsonl and scenes-timed.jsonl contain all 60 iteration summaries and every non-pass with its error text. Every new eval invocation saves the complete JSON row of each non-pass as it occurs, in a private run directory. The default location is target/omoide-eval-failures; OMOIDE_EVAL_FAILURE_DIR overrides it. The result reports failure_directory. Piping stdout into a counter no longer discards the only diagnostic.

## Checks and cost

The final healthy six S11 cases include current version, history, sources, interruption/resend and usage faults. The relation, contention, kill/resend and hot-file harnesses passed. All eight live S1 cases passed after warming the adapter; an earlier live pass had a partial-Japanese miss and a judge failure. The prescribed routes cover all 62 acceptance cases.

Both censes run inside the existing revision-protected read batch, including skill confirmation on previously stored candidates. They use prepared, indexed term lookups restricted to current rules, with no schema migration, retained cache, extra read transaction or judge. Per Linux ARM64 Debian/Rust 1.88 release measurements on one invented rule, at 3,048 records the 21 fresh registration previews had median 12.31 ms vs 11.44 ms before; max observed child RSS was 13,248 KiB in both. Smaller-store times and slow tails are in linux-performance.jsonl. These single-input measurements do not establish a general speedup.

The final workspace run passed 142 tests (two ignored helpers/benchmarks), and two Python adapter tests passed. Cargo-mutants 27.1.0, Linux ARM64: on the final core source, 243 caught and five unviable (192 caught outside Corpus methods and all 51 final Corpus variants caught). The changed CLI files had 101 caught and 13 unviable. All three generated store-statement mutations were unviable because Statement has no Default implementation; compiling and running independent SQL swaps was therefore important. Ten new deliberate SQL/field/decision swaps and the eleven prior field/cap/digest substitutions all compiled and failed targeted assertions. No unexplained survivors or timeouts. A final full run after the identical-body edge test again recorded every non-pass. Format, locked clippy/build, miwa, SQLite smoke and the publication scan were run locally.
