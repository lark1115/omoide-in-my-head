# omoide-in-my-head

`omoide` は、エージェントが利用者の発言から決まりを登録し、別の会話から出どころと一緒に引ける CLI です。段階1の保存先はローカルの SQLite です。インストール後の登録・検索にはネットワーク、GPU、常駐プロセスが要りません。D1 と Turso は未検証です。

## 空の保存先から試す

Rust 1.88 と C コンパイラのある環境で、リポジトリのルートから実行します。SQLite と FTS5 は実行ファイルに同梱されます。シェル例では JSON を読むために `jq` を使います。

```sh
cargo install --locked --path crates/omoide
export OMOIDE_DB="$PWD/workshop.db"
omoide init
source_id=$(omoide say --operation-id kiln-words-1 --text '窯に火を入れる前に、扉の留め金を点検する。' | jq -er .id)
omoide rule add --operation-id kiln-rule-1 --source "$source_id" \
  --scene '窯の点火' --alias '焼成を始める' --action ignite --target 工房 \
  --text '扉の留め金を点検する。' --purpose '扉の開放を防ぐ' --exclude 展示室
```

新しいシェル・新しい会話で同じディレクトリを開きます。前の発言や ID を問い合わせに渡す必要はありません。

```sh
export OMOIDE_DB="$PWD/workshop.db"
reply=$(omoide recall --scene '焼成を始める前の準備' --target 工房)
printf '%s\n' "$reply" | jq '.results[] | {id,kind,state,actions,text}'
omoide show "$(printf '%s\n' "$reply" | jq -er '.results[0].id')" | jq '.source'
omoide recall --action ignite --target 工房
```

`show` は決まり・版・発言の ID を受け取ります。決まりには `record` と `source`、発言には原文、話者、時刻（Unix ミリ秒）、対象を含む `record` が返ります。`say --at` で元の発言の時刻を指定できます。

## 結果の読み方

- 結果は JSON です。失敗は終了コードが非ゼロで、`kind` と `message` を返します。保存先は `--db` または `OMOIDE_DB` で指定します。既定はカレントディレクトリの `omoide.db` です。
- 再送に備え、変更には最初から `--operation-id` を指定してください。返事が届かなければ同じ ID と同じ入力で送り直すと、確定済みの結果を取得できます。同じ ID で入力が違えば失敗します。省略時は ID を自動生成して返します。
- `rule add` の `activated` は登録済みです。`near_rules` は未登録で、近い決まり、元の発言、版の並びの要約値を返します。関係を選んで登録を続ける機能は段階2です。
- 動作名・対象・適用除外は完全一致です。対象を省いた規則は全対象に使えます。対象つきの規則を引くには問い合わせにも対象が必要です。目的や除外を省くと検索時に `unknown` を返します。
- 場面検索は NFKC と小文字化、日本語の二字組、単字の問い合わせ、英数字の語を使います。動作一致を優先し、語の一致割合、登録時刻、ID の順で比較します。意味だけが同じ言い換えは取りこぼすため、登録時に別の表現を `--alias` で複数添えられます。
- `recall` は `state`（`found` / `none`）、判定器の `judge_state`（段階1では `not_configured`）、採点対象数 `candidate_count` と一致数 `matched_count` を別々に返します。今の規則が `--scan-limit`（既定500件）以下なら場面検索で全件を採点し、超えた場合は全文索引から `--fts-limit`（既定500件）まで取得します。打ち切りは `candidates_truncated` で確認できます。動作の一致は別に取得します。
- 問い合わせ結果は改行込みで4,000バイト以内です。続きは同じ問い合わせに `--cursor` を添えます。保存の版が変わると `restarted` が真になり先頭から返します。`truncated` の本文は `show` で全体を読んでから使います。条件欄だけで上限を超える場合はエラーです。
- 保存する文に秘密を含めないでください。機密物は安全な保管先の参照だけを登録します。装置は本文から秘密を検出しません。

[要求](docs/requirements.md)、[保存の契約と設計](docs/adr/0001-layout-store-commands.md)、[エージェントが一巡する手順](AGENTS.md)

## ローカル検証

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 verify/check_miwa.py
cargo build --locked --release
python3 verify/stage_one_scenes.py target/release/omoide
```

場面の採点は未達があれば非ゼロで終了します。段階2以降は未実行と表示します。Linux では Python 3 と GNU time を用意し、`python3 verify/measure.py target/release/omoide` で1万件の計測を再実行できます。
