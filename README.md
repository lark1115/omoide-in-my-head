# omoide-in-my-head

`omoide` は、エージェントが利用者の発言から決まりを登録し、別の会話から出どころと一緒に引ける CLI です。保存先はローカルの SQLite です。インストール後の登録・検索にはネットワーク、GPU、常駐プロセスが要りません。D1 と Turso は未検証です。

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
- `rule add` の `activated` は登録済みです。`near_rules` は未登録で、近い決まり、元の発言、版の並びの要約値を返します。比較を終える手順は次節にあります。
- 動作名・対象・適用除外は完全一致です。対象を省いた規則は全対象に使えます。対象を指定しない問い合わせは対象不明として、対象つきの規則も対象名と一緒に返します。対象を指定すると、別の対象向けの規則は返りません。適用除外の文は結果にも残るため、完全一致で除外されなかった範囲は呼び出し側が判断します。目的や除外を省くと検索時に `unknown` を返します。
- 場面検索は NFKC と小文字化、日本語の二字組、単字の問い合わせ、英数字の語を使います。動作一致を優先し、語の一致割合、登録時刻、ID の順で比較します。意味だけが同じ言い換えは取りこぼすため、登録時に別の表現を `--alias` で複数添えられます。
- `recall` は `state`（`found` / `none`）、判定器の `judge_state`（現在は `not_configured`）、採点対象数 `candidate_count` と一致数 `matched_count` を別々に返します。今の規則が `--scan-limit`（既定500件）以下なら場面検索で全件を採点し、超えた場合は全文索引から `--fts-limit`（既定500件）まで取得します。打ち切りは `candidates_truncated` で確認できます。動作の一致は別に取得します。
- 問い合わせ結果は改行込みで4,000バイト以内です。続きは同じ問い合わせに `--cursor` を添えます。保存の版が変わると `restarted` が真になり先頭から返します。`truncated` の本文は `show` で全体を読んでから使います。条件欄だけで上限を超える場合はエラーです。
- 保存する文に秘密を含めないでください。機密物は安全な保管先の参照だけを登録します。装置は本文から秘密を検出しません。

[要求](docs/requirements.md)、[保存の契約と設計](docs/adr/0001-layout-store-commands.md)、[エージェントが一巡する手順](AGENTS.md)

## 登録の続きと履歴

`near_rules` の各要素の `record.version_id` と `source.text` を確認します。最初と同じ登録引数に次の3つを足してください。

```text
--relation same|restated|exception|unrelated|unsure
--against 選んだ現行版のversion_id
--near-digest 最初のnear_digest
```

| 関係 | 登録結果 |
|---|---|
| same | 同じ版に出どころを追加する。元の発言は `show` の `sources` に残る |
| restated | 同じ記録 ID に新版を作る。旧版には `superseded` と後継の版 ID が付く |
| exception | 対象名つきの別の規則を作る。対象が合う問い合わせでは例外を返し、`parent_id` に元の規則の記録 ID を示す |
| unrelated | 独立した現行規則を登録する |
| unsure | 現行版を残し、同じ記録に判断待ちの版を作る |

比較後に現行版の並びが変わると、新しい `near_rules` を返します。登録の確定後に返事を失った場合は、同じ操作 ID と同じ引数で送り直してください。

`pending` の返事と、該当する問い合わせ結果の `unresolved_conflicts` には、現行版と提案版の各規則・元発言があります。両方を利用者に示し、判断後に `omoide resolve PENDING_VERSION --keep current` または `--keep pending` を実行します。並行した別の判断待ちがある場合は、その判断を先に確定してから現行版を置き換えます。対象が分からないときは元の規則も対象つきの例外も表示します。元の規則にすでに対象があれば、例外はその範囲内に登録します。

`omoide show ID --history` は同じ記録 ID の各版、その状態、後継、出どころを返します。版 ID を指定した通常の `show` はその版の本文を返します。証拠の全文を含む食い違いが4,000バイトに収まらない場合は、エラーにある版 ID を `show` で開きます。

発言を要約した事実は `omoide fact add --source 発言ID --text '要約' --certainty reported` で登録し、返った ID を `show` で読みます。確かさの省略値は `unknown` です。この段階の事実は ID で参照できます。事実の検索・検証済みの証拠は後続段階で扱います。

段階1の SQLite ファイルは、段階2の CLI が最初に開く際に1トランザクションで形式2へ移行します。記録・版・発言の ID、全文索引の対応、確定済み操作の受け取りを引き継ぎます。移行後は段階2以降の CLI を使ってください。切り戻しが必要な場合は事前に SQLite のバックアップ機能でコピーを作成します。

## ローカル検証

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 verify/check_miwa.py
cargo build --locked --release
python3 verify/stage_one_scenes.py target/release/omoide
```

場面の採点は未達があれば非ゼロで終了します。段階1〜2の全対象を試すには、次のコマンドで中断位置を制御できる検証用 CLI を別途ビルドします。既定のビルドには中断機能は入りません。

```sh
cargo build --locked --features fault-injection --target-dir target/fault
python3 verify/stage_one_scenes.py target/release/omoide --stage 2 --fault-binary target/fault/debug/omoide
python3 verify/relations_contract.py target/release/omoide target/fault/debug/omoide
```

Linux では Python 3 と GNU time を用意し、`python3 verify/measure.py target/release/omoide` で1万件の計測を再実行できます。
