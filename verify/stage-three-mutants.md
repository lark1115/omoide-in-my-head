# 段階3の変異試験

Rust 1.88.0、cargo-mutants 27.1.0、Linux ARM64。対象ごとに別のソースコピー・別のビルドディレクトリを使い、各コピー内では1変異ずつ試した。タイムアウトは180秒。CI には入れない。

| 対象 | 生成 | 検出 | 未検出 | コンパイル不能 | 時間切れ |
|---|---:|---:|---:|---:|---:|
| omoide-core 全体 | 57 | 53 | 0 | 4 | 0 |
| omoide / activity.rs | 17 | 16 | 0 | 1 | 0 |
| omoide / args.rs | 11 | 5 | 0 | 6 | 0 |
| omoide / db.rs | 28 | 24 | 0 | 4 | 0 |
| omoide / main.rs | 7 | 6 | 0 | 1 | 0 |
| omoide / memory.rs | 12 | 11 | 0 | 1 | 0 |
| omoide / search.rs | 38 | 38 | 0 | 0 | 0 |
| omoide / write.rs | 34 | 33 | 0 | 1 | 0 |
| 合計 | 204 | 186 | 0 | 18 | 0 |

等価として許容した生き残りは0件。対象ファイルの決定箇所にフィルターをかけていない。初回の未検出を受けて、独立した候補上限、判断待ちだけが持つ動作、範囲の一部だけが確認された場合、操作 ID と入力の境界、競合の期限を検証した。

```sh
cargo mutants -p omoide-core --timeout 180
for file in activity args db main memory search write; do
  cargo mutants -p omoide --file "crates/omoide/src/$file.rs" --timeout 180
done
```

## 文字列の意味と、判定の片側を落とす変更

上表の「未検出0」は **cargo-mutants が生成した集合**の結果。27.1.0 の `--list` で該当箇所を確認すると、完了判定の `== "verified"` にはリテラルを保った `!= "verified"` が生成される。順位の OR 式には `||` を `&&` にする変異と各比較の反転が生成される。監査で試した次の2変異はその集合に入っていなかった。

- **B1** `== "verified"` を `!= "unknown"` に変更する。従来の部分範囲テストで未検証側の確かさは省略値の unknown だったため、この2条件は同じ結果を返した。新しい CLI テストは明示的な reported の1件だけを登録する。対象、期間、範囲、主張、証拠参照が一致し、検索でその ID が返っても、verified は false、unproven_scope には要求範囲がそのまま残ることを確かめる。
- **B2** 順位の `decision == "decided" || certainty == "verified"` から決定側の項を削る。新しい CLI テストでは、本文、対象、使用回数が同じ4件に異なる時刻を与える。期待順は、確定決定（時刻1）、検証済み事実（0）、伝聞（3）、判断待ち（2）。新しさだけで確定決定が偶然先頭に来る余地を除いた。

3ファイルのコードを別々のコピーに置き、以下の変更を1件ずつコンパイルして、実 CLI を起動する evidence_activity テストで検証した。各行の複数の変更も、個別に適用して元に戻した。

| ファイル | 入れ替えた条件・値 | 検出するテスト（関数名の先頭） |
|---|---|---|
| search.rs | B1、検証の verified → reported | a_reported_fact_with_evidence、partial_proof_requires |
| search.rs | B2、順位の decided → pending、順位の verified → reported | decided_and_verified_records |
| search.rs | 規則の選択 rule → fact、条件欄の付与対象 rule → fact | independent_candidate_caps、rule_and_evidence_fts_limits |
| search.rs | 元発言の付与対象 decision → fact | decision_state_is_independent |
| search.rs | 順位に使う target_only → lexical、出力ラベル lexical → target_only | reported_use_only_changes、decided_and_verified_records |
| search.rs | claim フィルターに period を渡す、delivered の照会に requested を渡す | verification_requires_explicit |
| memory.rs | 決定の出どころ utterance → experience、話者 user → agent | decision_state_is_independent |
| memory.rs | 検証済みとして扱う2表記 verified / self-verified のそれぞれを reported に変更、観察元 experience → utterance | verification_requires_explicit |
| activity.rs | 動作名が必要な通知 acted → used、分母に使う通知 acted → used、指摘の corrected → used、集計する規則の current → pending | distinguish_return_use |

手動変異は21件すべてテストが検出した。コンパイル失敗を検出件数に含めていない。
