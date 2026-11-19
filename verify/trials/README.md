# Paired effectiveness study

From the repository root, use authenticated Claude Code and Codex CLIs. The first run used Claude Code 2.1.329 with claude-sonnet-5-5 and Codex CLI 0.170.0 with gpt-6.2-sol. Both used low effort. Model sampling and the provider seed were unavailable. The archive and rubric were frozen in commit 8ae72c33977ca71148a2f3ed4672aed733e22b2a.

```sh
cargo build --release -p omoide
study=$(mktemp -d)
python3 verify/trials/run.py "$study" target/release/omoide --jobs 6
python3 verify/trials/score.py blind "$study"
python3 verify/trials/score.py grade "$study" --jobs 6
python3 verify/trials/report.py "$study" verify/trials/results target/release/omoide
python3 -m unittest discover -s verify/trials
```

The host registers all 48 invented records. Each trial starts in an empty working directory and gets a fresh session. Each device session receives a copy of the seeded database. All arms have the same optional read-only ledger tools. The agent has 120 seconds, the device CLI has eight seconds per request. The agent receives the original request, the frozen common instruction for its language, and the additional inputs assigned by the manifest. A mismatched tool inventory stops the run. The 6 arm orders occur 8 times each. The first run scheduled two prespecified language- and type-stratified halves. Each opportunity kept its assigned order.

The private transcript records calls, results and visible text in order. The grading packet contains only the prompt and final work instruction, with route names and internal IDs neutralized and a random ticket. Each ticket is graded in a fresh Codex process in an empty directory with user settings, rules, host skills, plugins, apps, web search and shell tools disabled. The host sends only the ticket, workshop request, answer, general rubric and that opportunity's key. The grader must quote the answer. The analysis joins grades to conditions afterwards.

The first run's pre-final visible comments were reviewed for earlier commitments. They were searches and plans to read records. Timing uses the host's monotonic clock at stream receipt. A relevant hot record is available at session start. Input bytes include the two newline separators before hot.txt and the exact text in device tool results. Common ledger response bytes, tool schema bytes and provider-reported input tokens including cache are separate measures. Lookup delivery alone does not establish adoption. The original observations passed to the product CLI are retained in the private study directory. Public results include the blinded copies, grades, delivery IDs and product aggregates. Raw session identifiers and machine metadata stay private.

This run measures one agent and one grader on invented workshop decisions. The common ledger is a capable baseline. A paired change in these grades cannot establish a long-term change in owner corrections. No follow-up corrections were elicited. The eight controls per arm are the denominator for overapplication.

## Large ledger trial

Use the frozen dataset and paging contract under tests/trials-large. The four arms share the same original 48 requests, model, common instruction, time limit, hot section and grading criteria. Each trial gets a fresh 24,576-byte ledger budget. The ledger server uses the reference implementation, with separate cursors and a shared budget for list and search calls. Repeating a query resumes that cursor. No read-by-ID ledger tool is exposed. The device arms receive copies of the same verified full database. The generated file remains visible only to the host.

```sh
study=$(mktemp -d)
cargo build --locked --release -p omoide -p omoide-judge-jev
python3 verify/trials/large.py controls "$study" target/release/omoide
python3 verify/trials/large.py device "$study" target/release/omoide --adapter target/release/omoide-judge-jev
python3 verify/trials/score.py blind "$study"
python3 verify/trials/score.py grade "$study"
python3 verify/trials/large_report.py "$study" verify/trials/large-results target/release/omoide
```

The operator requested the two control arms before the retrieval change merged, and both device arms afterwards. Order is balanced within each block. The device versus control comparison therefore also spans the two execution periods. The Jev device receives the same tools and hot section as the core device. Only explicitly requested recalls can invoke the adapter. A private local socket connects the retrieval CLI to the adapter process. The acting and grading processes receive no provider credential. In the device arm with Jev, the documented selection threshold is 0.60, the provider deadline is 5.8 seconds, and the judge port deadline is 6.8 seconds inside the same eight-second tool budget as core. A failed judge is recorded separately from a failed lookup.

All 3,048 records are registered through the product CLI. The 36 historical decisions become genuine revised versions with successor links. Events retain their periods, facts their certainty and evidence, and rules their conditions and exclusions. Logical archive handles are opaque in the ledger. Only the analyst can translate internal record/version IDs to the original grading IDs. Source, conditions and full strict results count as delivery where identifiable. A short unconfirmed heading is reported separately until the agent reads it with show. Actual adoption and follow-up corrections remain unmeasured.

For the large run, the eight-second lookup deadline initially surfaced a Python exception containing local executable and temporary database paths in five actor sessions. The timeout handler was changed to a constant diagnostic while the run was in progress. All five assigned observations stay in the analysis. No actor session was replaced. The private transcripts, added-input strings, provider CLI streams and machine-level errors are excluded from publication. Public blinded text neutralizes lookup routes, internal handles and host paths. Published eval rows omit raw action text because the blinded answer is already present. Before publication, scan every changed file for machine paths, account/host identifiers, temporary working directory names and credentials.

Lookup diagnostics are separate from actor trials and grades. The frozen archive holds 885 current rules, exceeding the default 500-rule full-scan ceiling. Scene queries use the FTS index, with direct action lookup when provided. Evidence candidates have their own limit, so the combined candidate pool can exceed 500. A structured action-only judge request can also exclude rules not bound to the action. The per-lookup file distinguishes observed pool counts from offline replay counts, reports actual receipt assignment, and reports separate provider probes only on failing inputs. Each replay uses a fresh database copy. A store/process timing replay cannot identify the precise original stage of a lost response. The original run had no internal startup-versus-query timestamps. Unreceipted tokens are unknown.

After creating the grading copies, inspect the answer text itself for tool mentions and runtime metadata. A private editorial-edits.json can map the SHA-256 of a neutralized answer to a route-neutral answer preserving its chosen action, evidence and uncertainty. Running blind again assigns new random tickets to changed copies and preserves unchanged tickets. Run grade on the final tickets before joining observations.
