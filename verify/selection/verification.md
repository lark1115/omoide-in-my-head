# Verification evidence

The final policy was frozen at e8bb0833977bda8718cef94803dd8c4f1eb68494 before the even IDs were run. Later edits add tests, documentation, observations and reporting. The core and adapter product code is byte-identical to the frozen commit.

All 62 frozen cases ran through the actual binaries. Core and the fixtures required by S12 passed 59. The only misses were s01-part-ja, s01-zero-ja and s01-zero-en. The live Jev adapter at 0.60 passed all eight S1 cases with judge_state used. Thus each of the 62 cases passed on its prescribed route. The write and hot-file contention harnesses also passed. The registration, interruption and usage paths received no production changes.

Eight separate source-literal mutations compiled and failed the CLI tests. The tests run for each mutation were weak_lexical_match_and_structured_judge_candidates or target_and_action_words_do_not_supply_extra_trigger_evidence.

| Mutated field or condition | Replacement | Observable failure |
|---|---|---|
| Core gate kind rule | skill | Weak rule returned |
| Trigger identity target | purpose | Shared target boosts the weak rule |
| Trigger identity actions | aliases | Shared action boosts the weak rule |
| Trigger tokens scene | purpose | Relative cut loses the strong trigger evidence |
| Judge input scene absent | target absent | Incorrect candidate set |
| Judge input target present | scene present | Incorrect candidate set |
| Judge input action present | scene present | Incorrect candidate set |
| Judge filter kind rule | skill | Other-action rule admitted |

The original core mutation run included an intermediate coverage-only module. Excluding that module gives 69 caught and four unviable. All 21 mutants in the final selection module were then caught, yielding 90 caught and four unviable on the final core source. No scored survivor or timeout.

The full binary pass over judge.rs and search.rs generated 68 mutants. It caught 67 and missed one substitution of multiplication for addition when computing the best rule score. A new CLI test checks a three-word trigger within a four-hit rule. Multiplication makes its relative threshold exceed its own score. A targeted cargo-mutants rerun caught the survivor. Across the two changed binary files all 68 mutations are now caught.

## Two-tier confirmation

The two-tier product was frozen at d4ed3303e25b903efdce01b29800f7933f4e3fc1 before loading holdout2. All four arms completed 80 queries exactly once. Both live judge arms succeeded on all 80. No even-numbered query from the first discrimination set was rerun for this change.

The workspace suite passed 108 tests. The three known S1 core misses passed through the live Jev adapter at 0.60. Confirmed-page priority, two probability boundaries, disjoint tier membership, the first sentence and short-field allowlist, UTF-8 including the final newline, and the count of omitted candidates have direct coverage. The page property varied candidate counts and multibyte text lengths. The relation, interruption and hot-file contract harnesses passed. The unchanged frozen scene adapter still evaluates the confirmed result list.

Eleven independent literal/field substitutions all compiled and failed targeted tests. Six exchanged short-reference fields (id/version_id, kind/target, actions/scene in both directions). One extracted the sentence from scene instead of text. The other four changed candidates_only to none, routed confirmed records from candidate scores, routed candidates from confirmed scores, and changed the public guidance field name. The core mutation run found 99 variants, caught 94 and rejected five at compile time.

The two changed binary files had 67 variants, 66 caught and one unviable (the replacement attempted to construct a default judge reply without a Default implementation). Neither run had a survivor or timeout.
