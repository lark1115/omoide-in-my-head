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
| `near_rules` が返った | 最大9件の元発言と `near_remaining` を確認し、元の登録引数に `--relation`、表示された版の `--against`、返った `--near-digest` を付けてもう一度登録する | 再び `near_rules` なら比較対象か頻度による点数が変わったので、新しい並びを読んで選び直す |
| `unresolved_conflicts` がある | 両方の規則と両方の元発言を利用者に示す | 利用者が選ぶまで現行版が残る |
| 利用者が採用する側を選んだ | `omoide resolve PENDING_VERSION --keep current` または `--keep pending` を実行する | 結果の `state` が `resolved` なら、`version_id` が採用された版 |

## 新しい会話から参照するとき

| 条件 | 実行すること | 結果の意味 |
|---|---|---|
| 同じ保存先を使う新しい会話で、作業を始める前 | `omoide recall --scene 'これから行う作業'` を実行する。把握している動作名と対象は同じ命令に添える | `found` の各記録に対象と適用除外がある。対象未指定なら候補の対象を個別に確認する |
| recall が incomplete_search または notice を返したとき | 台帳や原本など別の情報源を検索する | この応答で確定できた物がない。記録の不存在は確定していない |
| recall が found を返したとき | search.by_kind と search.incomplete を確認する | 返った物以外は未調査の場合がある |
| recall が candidates を返したとき | 採用を検討する候補の version_id で `omoide show VERSION_ID` を実行する | 本文・対象・条件・除外を照合してから使う。candidates_only は適用未確認 |
| 決まりの条件や出どころを確認するとき | `omoide show ID` を実行する | `source.text` が原文で、`sources` が出どころ一覧 |
| 版の変化を確かめるとき | `omoide show ID --history` を実行する | `history` で各版と元発言を対応づけられる |

## 決定・観察結果・使用を記録するとき

| 条件 | 実行すること | 結果の意味 |
|---|---|---|
| 利用者の決定や提案を元発言とともに記録する | README の decide 例で発言種別と決定状態を別々に渡す | pending は判断待ちで、利用者の了承には使えない |
| 渡された物や観察結果の事実を登録する | 出どころを指定して fact add を実行する | 伝聞は reported、確かさの省略は unknown。verified は観察情報と証拠がそろった場合だけ |
| recall が target_only を返した | 記録の本文・対象・範囲・時期を照合する | 対象だけが一致している。確認済みの事実や完了範囲は個別の証拠で判断する |
| 返された版を作業に実際に使った | used に version_id と新しい notice ID を指定する | 使用が1回記録される。同じ知らせの再送は同じ notice ID を使う |
| 検索結果の accounting が unlogged | 返った結果を読み、計測では該当呼び出しを記録漏れとして扱う | 検索結果は利用できるが、呼び出しログは保存できていない |

## 節・スキル・判定器を使うとき

| 条件 | 実行すること | 結果の意味 |
|---|---|---|
| 指示ファイルに初めて節を用意する | `omoide hot init ファイル` を実行する | current ならマーカーとヘッダーがある |
| 会話を始める前に節を点検する | `omoide hot status ファイル` を実行する | stale なら同じ上限で hot export を行う |
| repeated に対象の経験が複数ある | 各経験 ID と具体的な場面・動作を添えて skill propose を実行する | candidate は未確定。利用する前に confirm が必要 |
| skill confirm が near_rules を返す | 候補と元発言を照合して関係を指定する | unsure は、利用者が resolve するまで判断待ち |
| 判定器つきの検索が0件を返す | judge_state と matched_count を読む | used は正常な判定、failed は障害後の基本検索、not_configured は未設定 |
