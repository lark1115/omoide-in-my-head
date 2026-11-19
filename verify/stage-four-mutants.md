# 段階4の変異検証

Linux ARM64、Rust 1.88.0、cargo-mutants 27.1.0。核全体と、変更したバイナリ・保存クレートの Rust 実装ファイルを検査した。作業コピーとビルド先は実行ごとに独立させ、同じファイルを複数に分けた場合は変異名の重複がないことを確認した。保存クレートはワークスペースのテストも実行した。タイムアウトは180秒。

| 対象 | 生成 | 検出 | 生き残り | コンパイル不能 | 時間切れ |
|---|---:|---:|---:|---:|---:|
| core | 73 | 69 | 0 | 4 | 0 |
| store/lib.rs | 56 | 48 | 0 | 8 | 0 |
| store/evidence.rs | 9 | 3 | 0 | 6 | 0 |
| store/lookup.rs | 19 | 12 | 0 | 7 | 0 |
| hot.rs | 63 | 62 | 0 | 1 | 0 |
| eval.rs | 46 | 43 | 0 | 3 | 0 |
| search.rs | 42 | 42 | 0 | 0 | 0 |
| args.rs | 11 | 5 | 0 | 6 | 0 |
| main.rs | 7 | 6 | 0 | 1 | 0 |
| skills.rs | 14 | 11 | 0 | 3 | 0 |
| activity.rs | 17 | 16 | 0 | 1 | 0 |
| db.rs | 28 | 24 | 0 | 4 | 0 |
| judge.rs | 16 | 16 | 0 | 0 | 0 |
| relations.rs | 39 | 32 | 0 | 7 | 0 |
| write.rs | 51 | 49 | 0 | 2 | 0 |
| 合計 | 491 | 438 | 0 | 53 | 0 |

等価として許容した生き残りはない。再現例は `cargo mutants -p omoide-core --timeout 180`、`cargo mutants -p omoide --file crates/omoide/src/hot.rs --timeout 180`、`cargo mutants -p omoide-store --file crates/omoide-store/src/lib.rs --test-workspace true --timeout 180`。

## 自動生成の対象外になる意味の入れ替え

比較演算子を反転しても、「正しい種類名を別の有効な種類名と取り違えた」実装は必ずしも検証できない。下表の各変更を1つずつ適用し、コンパイルが成功した実装でテストが失敗することを確かめた。複数の変更を同じ行に記した場合も、各変更は独立した試験である。

| 箇所 | 手で入れ替えたもの | 検出する試験または観測 |
|---|---|---|
| skills / repeated・propose | 繰り返しを集める experience → fact、出典として受け付ける experience → decision | experience_to_skill_to_use_to_export_and_optional_demotion |
| skills / confirm | 読む候補の skill → rule、candidate → current | experience_to_skill_to_use_to_export_and_optional_demotion |
| skills / 関門 | 照合相手の rule → skill、current → candidate | unsure_skill_needs_resolution_and_preserves_both_origins |
| skills / 同一・未解決 | 同一の判定 same → activated、未解決の証拠を更新するキー unresolved_conflicts → 別名 | skill_relations_keep_experience_sources_and_reject_stale_choices、unsure_skill_needs_resolution_and_preserves_both_origins |
| write / 成否 | 対象 skill → rule、連続失敗に数える failure → success、降格対象 current → candidate | experience_to_skill_to_use_to_export_and_optional_demotion |
| relations / 解決 | 採用側の種類を返す判断 pending → current | unsure_skill_needs_resolution_and_preserves_both_origins |
| activity | 集計する規則の rule → skill、書き出しのログ種別 hot_export → hot_status | export_log_describes_published_bytes_and_current_rules_only_are_measured |
| search / 絞り込み | 対象種別一覧の skill → experience、rule → fact、最後の種別照合に kind の代わりに state を使用 | current_skills_and_rules_obey_kind_filters_and_action_priority |
| search / 順位 | 動作で直接引いた skill と rule の区別を反転 | current_skills_and_rules_obey_kind_filters_and_action_priority |
| search / 判定後の優先 | decided → pending、verified → reported | judged_equal_scores_preserve_both_priority_arms |
| search / 判定状態 | used → failed、failed → used | judge_failures_return_lexical_results_and_empty_is_a_success |
| store/lookup / 入力フィルター | フィルター名 skill → experience、rule → decision、SQLite の skill → rule | current_skills_and_rules_obey_kind_filters_and_action_priority（候補数も照合） |
| store/lookup / 近傍 | 近い現行規則の rule → skill | digest_and_exact_version_bind_the_choice |
| store/lookup / 失敗数 | 結果 failure → success、通知種別 outcome → used | experience_to_skill_to_use_to_export_and_optional_demotion |
| hot / 区画 | 開始印 → 終了印、終了印 → 開始印、予約マーカー名 → 別名、ヘッダーの接頭辞 → 終了印 | section_is_bounded_and_preserves_surrounding_bytes |
| hot / ヘッダー | store と config、revision と usage の各方向のキー取り違え（4件） | marker_offsets_cover_end_line_and_preserve_the_surrounding_text |
| hot / 掲載 | 掲載する current → candidate、使用回数の used → acted、固定指定の pinned → failure_count | cap_includes_markers_exactly_and_optional_rows_stay_searchable、experience_to_skill_to_use_to_export_and_optional_demotion |
| eval / モード | 正解表の対象 S1 → S2、許される試行条件の device、no_device、hot_only をそれぞれ別名に変更 | eval_run_and_paired_build_diff_use_independent_stores、trials_count_uncalled_opportunities_and_unknown_order_without_credit |
| eval / 計測 | 装置なしの判定 no_device → device、節のみの判定 hot_only → no_device、成功に数える before → after、後着 after → before、順序不明 unknown → before | trials_count_uncalled_opportunities_and_unknown_order_without_credit |
| eval / 差分キー | ケース識別子 case → id | eval_run_and_paired_build_diff_use_independent_stores |

Rust/SQL は50件を検出した。旧段階から継承した確かさ・決定状態の検査は [段階3の記録](stage-three-mutants.md) にある。

オフラインの Python アダプターも次の15件を別々に検証した。固定データを変更せず、操作の取り違えを短時間で再現する補助試験だけで経験2件を用いた。受入結果の S9 は指定通り各ケースで600件を登録した。

| 振り分け | 入れ替え | 検出根拠 |
|---|---|---|
| S9 準備・読む場所 | seed_experiences、initialize_section、export_section の各操作を相互に取り違える（3件）、section → history、掲載行の判定 rule → fact | 実 DB の経験件数、export ログ、節の内容、旧版非出現 |
| S12 判定 | 短い期限の timeout → no-key、タイムアウト理由を別の失敗理由に変更、valid-empty に timeout を渡す、timeout のモード指定を no-key に変更、未設定の状態を used に変換 | judge_state・result_count と失敗理由、test_scene_adapter.py の起動引数 |
| 採点・段階選択 | max_section_bytes の上限判定を別キーへ、interleaving と grow_and_export の未実行条件を取り違える、S12 を有効な場面から外す | S9 の実測バイト上限、実行件数、S12 の状態別ケース |
| 登録時の入力 | pinned の参照キーを別名に変更、別表現の検索キーを本文から場面に変更 | 実 CLI の show で固定指定と登録済み aliases を照合 |
