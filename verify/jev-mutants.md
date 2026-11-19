# Jev アダプターの検証

Rust 1.88.0、Linux ARM64、cargo-mutants 27.1.0。記録済みの応答とメモリ内の HTTP 交換を使用した。`cargo mutants -p omoide-judge-jev --timeout 120` の生成32件のうち検出26件、生き残り0件、コンパイル不能6件、時間切れ0件。初回は、名前付き回答の代わりに配列が来たときのデシリアライザーの診断を空にする変異が生き残った。配列の拒否とエラーの対象を明示するテストを追加した。

## 文字列・対応付けの取り違え

次の22変更を別々に適用した。すべてコンパイルに成功し、対応するテストが失敗した。テストの API キーは実環境から読み込まず、通信は行っていない。

| 変更（列挙したものは各1件） | 検出したテスト |
|---|---|
| モデルの固定値を latest に変更、state キーを scene に変更、Noul を Choice に変更 | one_question_per_rule_with_only_allowlisted_content |
| 規則本文に場面を渡す、別表現に本文を渡す、肯定と否定の基準を交換する、質問名にローカル版 ID を載せる | one_question_per_rule_with_only_allowlisted_content |
| モデル名の検査を旧版の除外だけに弱める | require_complete_typed_probabilities_and_match_names_not_response_order |
| 応答の質問名 q0 を別接頭辞で探す、回答の noul タグを choice に変える、判定フィールド type を kind に変える | recorded_probability_and_usage_map_to_the_local_version |
| 真の確率 noul の代わりに confidence を読む | require_complete_typed_probabilities_and_match_names_not_response_order |
| ポートの出力キー score を confidence に変える、入出力トークン数を逆にする | recorded_probability_and_usage_map_to_the_local_version |
| API のパス変更、POST → GET、Authorization ヘッダーの改名、Bearer → Basic、JSON → text/plain | http_contract_and_failures_without_network |
| キー・タイムアウトの環境変数名の取り違え、利用量オプション名の変更 | recognizes_only_the_documented_key_and_timeout_names_offline、cli_failures_never_supply_scores_or_require_network |

記録済み Noul の値をポートにそのまま渡すことと、欠け・余分・重複・不正な値が1件でもあれば全体を失敗させることを検証した。S1 のライブ成績の再現用データは jev-s1-observations.jsonl にある。
