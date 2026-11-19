# First-reply discrimination measurement

Run from the repository root with the adapter credential available in the environment. The output directory must have no previous arm files. The script refuses to overwrite an observation. To recompute the report from committed observations, omit both binary arguments.

```sh
cargo build --release --locked -p omoide -p omoide-judge-jev
python3 verify/discrimination/run.py study-output --binary target/release/omoide --adapter target/release/omoide-judge-jev
python3 verify/discrimination/run.py verify/discrimination/results
```

The script registers 72 active rules in file order into a single SQLite store. Near rules are retained as unrelated. It preserves the domain in the purpose and the rule text, and includes all exclusions in the source and rule text. The CLI accepts one exclusion string, so multiple exclusions are joined with a slash. Targets and actions are registered in their structured fields. No aliases, key phrases, grading IDs, expected answers or translations are used as retrieval input. Each arm starts from the same seeded snapshot. Each query launches a fresh release binary, passing just the frozen input fields. Routine call logging is enabled and no usage notices are sent. Scoring uses only the first returned page.

The arms use the same scan limit, byte budget and initial retrieval mode. Jev uses the production adapter and jev-1.14.0 at threshold 0.5. A local observer counts candidates in the actual judge request, redirects standard error to a private temporary file and replaces its process with the adapter. The same wall-clock limit includes that observer. A port or adapter failure is counted separately. Each valid core fallback is graded as returned. Missing token receipts have unknown cost. Timing includes startup, I/O, judging and the return of the first page on the local Apple silicon machine. Runs were sequential within each arm.

The extended run uses a 10-second port limit and a 9.5-second adapter budget. The default run uses 1 second and 900 milliseconds respectively. Both cover all 152 queries. These are retrieval results for the frozen invented puppet-theatre archive. They do not measure a downstream agent's actions.
