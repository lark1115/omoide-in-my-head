# AGENTS.md

## このリポジトリを変更するとき

| 条件 | 実行すること | 結果の意味 |
|---|---|---|
| 機能の追加に着手する前 | `docs/requirements.md` の該当する業務要求を確認する | 要求が見つからない場合は、先に要求の記録を更新する |
| 要件またはテストを変更した後 | `python3 verify/check_miwa.py` を実行する | 終了コード0なら、要求・要件・テストのずれはない |
| 公開用の例や試験を追加する前 | 人名、実在する利用者のデータ、その環境固有のパスや設定が含まれないことを確かめる | 含まれている場合は、架空の場面に置き換える |

## エージェントが記憶を登録するとき

| 条件 | 実行すること | 結果の意味 |
|---|---|---|
| CLI を未インストール | README に従って `cargo install --locked --path crates/omoide` を実行する | 完了すると `omoide` が使える |
| 保存先を初めて使う | 共有するファイルを `OMOIDE_DB` に指定して `omoide init` を実行する | `ready` と `store_id` が初期化済みの印 |
| 指示ファイルから移す決まりを選んだ | 決まりの出どころとなった元発言を確認する | 原文が見つかった決まりだけを登録する |
| 元発言が確認できた | `omoide say --text '原文'` に本文を一字も変えずに渡す | `stored` の `id` が出どころ |
| 出どころの ID を取得した | README の例に沿って `omoide rule add --source ID …` を実行する | `activated` は有効、`near_rules` は比較のみ |
| `near_rules` が返った | 元の登録引数に `--relation`、選んだ版の `--against`、返った `--near-digest` を付けてもう一度登録する | 再び `near_rules` なら比較対象が変わったので、新しい並びを読んで選び直す |
| `unresolved_conflicts` がある | 両方の規則と両方の元発言を利用者に示す | 利用者が選ぶまで現行版が残る |
| 利用者が採用する側を選んだ | `omoide resolve PENDING_VERSION --keep current` または `--keep pending` を実行する | 結果の `state` が `resolved` なら、`version_id` が採用された版 |

## 新しい会話から参照するとき

| 条件 | 実行すること | 結果の意味 |
|---|---|---|
| 同じ保存先を使う新しい会話で、作業を始める前 | `omoide recall --scene 'これから行う作業'` を実行する。把握している動作名と対象は同じ命令に添える | `found` の各記録に対象と適用除外がある。対象未指定なら候補の対象を個別に確認する |
| 決まりの条件や出どころを確認するとき | `omoide show ID` を実行する | `source.text` が原文で、`sources` が出どころ一覧 |
| 版の変化を確かめるとき | `omoide show ID --history` を実行する | `history` で各版と元発言を対応づけられる |
