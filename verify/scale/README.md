# First-page delivery as the archive grows

The offline measure runs the real CLI in fresh processes without an agent, judge, provider credential or grading session. It uses the public workshop data and the logical IDs in the key. It never reads a trial grade. The full archive has 3,048 records, including the original 48. At 500, all originals stay in, then distractors are chosen in SHA-256 order using `delivery-v1:` plus the opaque record ID. A sampled historical decision brings its successor chain. At 3,048 the experiment reused the verified snapshot from the large trial. All comparisons share snapshots across builds; record writes, call log entries and new opaque IDs are not inputs to the measurement.

Each opportunity contributes its prompt as a scene query, plus every unique valid recall argument set observed from either device arm in the large trial. Only the invented scene, target, action and kind inputs were extracted. Duplicate argument dictionaries within an opportunity count once. Five invalid attempts had neither scene nor action, and are excluded from the valid-input denominator. Eight opportunities had no valid agent lookup and still contribute their prompt. The published lookups contain no answers, returned records, usernames or tool diagnostics.

The raw output identifies every input by opportunity and index (0 = prompt, subsequent indices follow lookups.json). Each supporting record is marked 1 for results, 2 for candidates, or 0 if absent from the first page. In particular, a second-tier reference is available to read; it is not confirmation that a rule applies. A query counts as complete only if **all** its supporting IDs appear on that first page. An opportunity with no required IDs is N/A and still has a raw observation. The per-kind table counts supporting instances, so e.g. a fact and its underlying experience are counted separately. The opportunity table also gives prompt-only and observed-lookup-only results, and counts each supported opportunity once for the best of its inputs. It does not pool partial hits across pages or across queries.

The initial baseline binary was main at 59e04b8. Odd-numbered T IDs were the only development data. The final product was frozen at 7ddef5d3236c6ea5c202d414f68dfd2f1078727f. Both binaries were confirmed on the even IDs once. No retrieval parameters were changed afterwards. See results/summary.md for all sizes, modes and kinds and the four baseline/core JSONL files for individual cases.

From the repository root, build current and baseline in separate worktrees, then run:

```sh
cargo build --locked --release --workspace
python3 verify/scale/measure.py seed "$SNAPSHOTS" --binary "$BASELINE"
python3 verify/scale/measure.py dev "$SNAPSHOTS" --binary "$BASELINE" --output baseline-dev.jsonl
python3 verify/scale/measure.py dev "$SNAPSHOTS" --binary target/release/omoide --output core-dev.jsonl
```

Use a private snapshot directory. The TEST command requires an explicit frozen build commit and a new output file. Published results should be inspected directly to preserve the original holdout. The separate optional judge_probe.py sends only one invented DEV query against the large archive in each of two timing conditions. It does not drive an acting model. The Linux performance sample uses perf.py and that same DEV lookup. Its 21 fresh CLI invocations per size include startup and serialization. Peak memory is per-child RSS reported by wait4 on Linux ARM64 / Debian bookworm, Rust 1.88 release.

## What changed delivery

The baseline admitted many lexical neighbors into a byte-limited reply. At 3,048 records it delivered all support for 12/50 DEV inputs. Matching the query to compact bodies and subjects improved that to 33/50. Taking period scope into account and moving already-admitted observations beside the facts citing them reached 38/50. English stemming and partial subject matches helped related inflections and compounds. The final coverage fields also consume reply bytes. Final DEV delivery was 39/50, 41/50, 39/50. The harder even split reached 32/52, 31/52, 30/52. In the large store this is an improvement over 20/52, but the no-decline objective is not met per input. At least one tested input delivered all supporting records for 15/18 held-out opportunities at each size. Neither number establishes an improvement in an agent's actual actions.

## Judge size and time

The old port already supplied decisions, facts and experiences, but described them all as rules. The adapter now chooses the relevance question for the candidate kind without treating a reported fact as verified or a pending decision as consent. Its allowlist of candidate content is unchanged.

The large DEV query originally associated with rejected requests had 962 candidates. Cold probes at 1 and 6 seconds expired without completing a batch. On the separate warm run, 151 candidates were judged within the one-second budget and 811 were explicitly left unjudged; with six seconds all 962 completed in 26 successful batches. These elapsed times include the measurement subprocess. The service reports a 64k request / 32k per-question context limit for Jev. The implementation uses smaller serialized UTF-8 byte ceilings, with framing headroom, and validates the actual adapter request before transmission. Size validation cannot prevent unrelated network, authentication or service errors. [TypeSafe model limits](https://docs.typesafe.ai/go/models.md)
