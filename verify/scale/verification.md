# Verification

The frozen product commit is 7ddef5d3236c6ea5c202d414f68dfd2f1078727f. Subsequent changes add tests, evidence, the corrected requirement numbering from main and documentation. The even opportunities were not rerun. No actor sessions, grading or adoption notices were used in the offline comparison.

The 62 acceptance cases ran through real binaries and the prescribed fault fixtures. The unchanged three S1 misses were s01-part-ja, s01-zero-ja and s01-zero-en. Live Jev then passed all eight S1 cases, including those three. An initial S11 run failed to reach the pre-commit stop during startup; the complete rerun and the dedicated interruption, concurrency, resend and hot-file harnesses passed. No acceptance data was edited.

The CLI tests cover all five kinds including absent kinds, target/scope/period/claim filters, index misses versus complete empty scans, per-search and output limits, empty pages and the fallback instruction. Other tests cover independently ordered lexical and semantic results, accepted evidence following its fact, preserved negative judgments across partial batch failure, and a shared output cap and deadline. The adapter is checked at both exact serialized-byte limits, with multibyte inputs and oversize requests rejected before networking. Core tests include a property that adding unrelated terms to a field cannot increase its compact lexical match score.

Cargo-mutants 27.1.0 on the frozen CLI source (search, coverage, judgment, child exchange) generated 137 variants: 119 initially caught, three unviable and 15 survivors. Additional boundary, temporal-scope, partial-fallback, semantic ordering, coverage and cursor tests caught all 15 in an independent rerun. The adapter pass generated 43: 35 caught, six unviable and two survivors. Tests at exactly 24,000 and 48,000 serialized bytes caught both inclusive-boundary substitutions on rerun. An exploratory CLI mutation run started before the freeze was discarded. The final CLI run used an isolated Linux source copy.

The literal checks below are independent of cargo-mutants. The published script compiles each changed implementation, requires a failing assertion in the named test and restores the original source before the next substitution.

| File | Original | Replacement | Targeted test | Result |
|---|---|---|---|---|
| `omoide/coverage` | `r["kind"] == kind` | `r["state"] == kind` | `compare_tallies` | `caught` |
| `omoide/coverage` | `r["version_id"]` | `r["id"]` | `compare_tallies` | `caught` |
| `omoide/coverage` | `'$.period'` | `'$.claim'` | `eligible_counts_use_structured_scope` | `caught` |
| `omoide/coverage` | `'$.scope'` | `'$.period'` | `eligible_counts_use_structured_scope` | `caught` |
| `omoide/coverage` | `'$.claim'` | `'$.period'` | `eligible_counts_use_structured_scope` | `caught` |
| `omoide/search` | `let evidence = record["evidence"]` | `let evidence = record["source_hint"]` | `eligible_counts_use_structured_scope` | `caught` |
| `omoide/search` | `record["period"].as_str().is_some_and` | `record["purpose"].as_str().is_some_and` | `temporal_scope_affects` | `caught` |
| `omoide/search` | `json!("incomplete_search")` | `json!("none")` | `coverage_distinguishes` | `caught` |
| `omoide/search` | `Some("judge timeout")` | `Some("judge output limit")` | `fallback_keeps_only` | `caught` |
| `omoide/search` | `ordered(&scores.confirmed)` | `ordered(&scores.candidates)` | `semantic_order_survives` | `caught` |
| `omoide/judging` | `"kind":r["kind"]` | `"kind":r["scene"]` | `bounded_batches_keep` | `caught` |
| `omoide-judge-jev/protocol` | `Some("decision")` | `Some("rule")` | `choose_question_by_kind` | `caught` |
| `omoide-judge-jev/protocol` | `Some("experience")` | `Some("skill")` | `choose_question_by_kind` | `caught` |
| `omoide-judge-jev/protocol` | `Some("fact")` | `Some("skill")` | `choose_question_by_kind` | `caught` |

The core pass generated 118 variants, caught 113 and had five unviable. Across all final passes there were no unexplained survivors or timeouts. The 14 deliberate field/literal swaps compiled and failed their targeted tests.

Linux ARM64 / Debian bookworm, Rust 1.88 release: the same representative DEV query at 48, 500 and 3,048 records took median 35.67, 45.68 and 48.32 ms including process startup. Corresponding p95 wall times were 46.14, 55.70 and 55.36 ms; maximum per-child RSS was 14,896, 19,132 and 22,076 KiB. One concurrent test workload and one query shape limit generalization.
