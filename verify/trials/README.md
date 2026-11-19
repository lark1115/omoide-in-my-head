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
