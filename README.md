# omoide-in-my-head

`omoide` は、エージェントが利用者の発言から決まりを登録し、別の会話から出どころと一緒に引ける CLI です。保存先はローカルの SQLite です。インストール後の登録・検索にはネットワーク、GPU、常駐プロセスが要りません。D1 と Turso は未検証です。

## 測って分かっている事（2026-11）

この装置が、エージェントの間違いを減らすという証拠は、まだ無い。作り物の場面での測定で、分かっているのは次のとおり。

| 測った事 | 結果 |
|---|---|
| 問い合わせた時に、決めた記録が返るか（62 の場面） | 59 が合格。残る 3（語が 1 つも重ならない言い換え）は、判定の層を付けると合格 |
| 似た決まり 72 個の中から選べるか（新しい 80 の問い合わせ） | 判定の層なし: 当てはまると返した集まりが正解と完全に合ったのは 7。判定の層あり: 56。返すべき決まりは、候補の段を含めると 80 個のうち 79 個が返る |
| 記録 3,048 件の保存先で、要る記録が全部、最初の返事に入るか（確かめ用の 52 の問い合わせ） | 30。記録 48 件の時は 32 |
| 記録 48 件。装置あり・なしで、エージェントの答えの合格（44 件） | 差なし（41 と 42） |
| 記録 3,048 件。同じ比べ（1 回目。直す前の版） | 装置ありの方が悪い（装置なし 41、装置あり 37）。走らせる前に決めた線で「害がある」 |
| 記録 3,048 件。新しい依頼で測り直し（2 回目。今の版） | 差なし（4 つの形とも 43）。走らせる前に決めた線で「実用上の差なし」 |

2 回の比べから言える事:

- 記録が検索できる形で手元にあり、エージェントが探す気になっている場面では、この装置は、正しさも、読む量も、良くしない。台帳から読む量は半分になるが、装置の返事を読む量がそれを上回る。
- 1 回目に悪くなった理由は、装置が要る記録を届けられず、その時にエージェントが自分で探さなくなった事だった。今の版は、見つけられなかった時に「記録が無い証拠ではない。ほかの手段でも探せ」と返す。2 回目では、この返事のあと、エージェントは自分で探しに戻った。
- 判定の層なしでは、似た決まりを選び分けられない。

測っていない事: 記録が検索できる形になっていない場面、エージェントが探す事を思いつかない長い作業の途中、実際の利用者とのやり取りでの、同じ指摘のくり返し。

測定の記録は [verify/trials](verify/trials)、[verify/discrimination](verify/discrimination)、[verify/selection](verify/selection)、[verify/scale](verify/scale) にある。

## 空の保存先から試す

Rust 1.88 と C コンパイラのある環境で、リポジトリのルートから実行します。`cargo` が見つからない時は、PATH に `~/.cargo/bin` を足します（ログインでないシェルでは、入っていない事があります）。SQLite と FTS5 は実行ファイルに同梱されます。シェル例では JSON を読むために `jq` を使います。

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
- 場面検索は NFKC と小文字化、日本語の二字組、単字の問い合わせ、英数字の語を使います。動作名に結びついた規則を先に返します。次に本文一致、利用者の確定した決定・検証済みの事実、語の一致割合、明示された使用回数（最大32）、時刻、識別子を比較します。同じ半減期では経過時間による減衰と登録時刻の新しさの順は同じです。意味だけが同じ言い換えは取りこぼすため、登録時に別の表現を `--alias` で複数添えられます。
- `recall` は `state`（`found` / `candidates_only` / `none` / `incomplete_search`）、判定器の `judge_state`（`not_configured` / `used` / `failed`）、採点対象数 `candidate_count` と一致数 `matched_count` を別々に返します。`candidates_only` は未確認候補のみを示すので、使う前に候補の版IDで `omoide show VERSION_ID` を実行し、条件と除外を確かめます。今の規則が `--scan-limit`（既定500件）以下なら場面検索で全件を採点し、超えた場合は全文索引から `--fts-limit`（既定500件）まで取得します。打ち切りは `candidates_truncated` で確認できます。動作の一致は別に取得します。
- 問い合わせ結果は改行込みで4,000バイト以内です。続きは同じ問い合わせに `--cursor` を添えます。保存の版または使用回数の版が変わると `restarted` が真になり先頭から返します。`truncated` の本文は `show` で全体を読んでから使います。条件欄だけで上限を超える場合はエラーです。
- 保存する文に秘密を含めないでください。機密物は安全な保管先の参照だけを登録します。装置は本文から秘密を検出しません。

[要求](docs/requirements.md)、[保存の契約と設計](docs/adr/0001-layout-store-commands.md)、[エージェントが一巡する手順](AGENTS.md)

## 登録の続きと履歴

近い決まりは、本文・場面・別表現の内容語で比べます。動作名だけ、またはひらがなの助詞・語尾だけの一致では登録を止めません。

`near_rules` は根拠の強い順に最大9件です。`near_score` は比較順の点数、`near_remaining` は表示枠の外にある近い決まりの件数です。各要素の `record.version_id` と `source.text` を確認します。最初と同じ登録引数に次の3つを足してください。

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

表示外を含む全候補の版の並びが比較後に変わると、新しい `near_rules` を返します。登録の確定後に返事を失った場合は、同じ操作 ID と同じ引数で送り直してください。

`pending` の返事と、該当する問い合わせ結果の `unresolved_conflicts` には、現行版と提案版の各規則・元発言があります。両方を利用者に示し、判断後に `omoide resolve PENDING_VERSION --keep current` または `--keep pending` を実行します。並行した別の判断待ちがある場合は、その判断を先に確定してから現行版を置き換えます。対象が分からないときは元の規則も対象つきの例外も表示します。元の規則にすでに対象があれば、例外はその範囲内に登録します。

`omoide show ID --history` は同じ記録 ID の各版、その状態、後継、出どころを返します。版 ID を指定した通常の `show` はその版の本文を返します。証拠の全文を含む食い違いが4,000バイトに収まらない場合は、エラーにある版 ID を `show` で開きます。

## 決定・事実・経験を記録する

```sh
source_id=$(omoide say --text '印刷見本の受付は正午に締める。' | jq -er .id)
decision=$(omoide decide --source "$source_id" --text '見本の受付締切' --target 印刷室 \
  --speech instruction --state decided)
omoide recall --kind decision --scene '受付の締切' --target 印刷室
```

発言種別は `instruction` または `proposal`、決定状態は `pending`、`decided`、`revised` です。両方を明示します。出どころは利用者の元発言が必要です。判断待ちは承諾を意味しません。明示的な変更では、新しい元発言を用意し、同じ decide に `--revises 旧版のversion_id` を添えます。記録 ID は維持し、旧版を revised にして通常検索から外します。`show 記録ID --history` で版ごとの元発言を確認できます。

```sh
observation=$(omoide event add --text '作業台で丸い版を数えた。' --target 印刷室 | jq -er .id)
omoide fact add --source "$observation" --text '丸い版の棚卸が済んだ' \
  --certainty verified --target 印刷室 --verified-where 作業台 --evidence "$observation" \
  --scope 丸 --period 今週 --claim counted
omoide recall --kind fact --scene '棚卸の範囲' --target 印刷室 \
  --scope 丸 --scope 角 --period 今週 --claim counted
```

- 経験は `event add` で保存し、出来事の本文・対象・時点を持ちます。時点は Unix ミリ秒の `--at` で指定でき、省略時は記録時刻です。
- 事実は発言または経験を出どころにします。確かさは伝聞の `reported`、検証済みの `verified`（別表記 `self-verified`）、不明の `unknown`（既定）です。検証済みには場所・時刻・方法と証拠の参照が必要です。`--verified-where`、`--verified-when`、`--verified-how`、`--evidence` で指定します。元の経験があれば、その記録済みの対象・時刻・観察本文・ID を省略欄に使います。`--location` は物の所在も記録します。
- `--claim` は完全一致する主張名です。依頼と受領確認、入力と利用可能の確認には別の主張名・記録を使います（例 `requested` / `delivered`、`entered` / `usable`）。照会に `--claim` があれば、`verified` はその主張の確認状況を返します。対象・期間を指定していない照会で、特定の対象・期間だけの記録を一般的な確認に使いません。`--scope` は複数指定でき、明示範囲のうち検証済みの証拠がないものが `unproven_scope` に残ります。
- 決定・事実・経験は常に全文索引を使います。対象指定時は対象名も独立した索引から引きます。結果には `match_basis: lexical`（本文の語が一致）または `target_only`（対象だけ一致）が付きます。対象だけの候補は本文一致の後で、利用者の確定した決定・検証済みの事実、時刻、ID の順です。規則を対象名だけで発動することはありません。範囲と期間は順位付けの前に絞ります。索引の候補上限は規則群とそれ以外の群に別々に適用します。各ページの `remaining` は採点範囲内の残数で、索引段階の打ち切りは `candidates_truncated` が示します。

## 使用の知らせと計測

```sh
reply=$(omoide recall --action ignite --target 工房 --opportunity-id kiln-start-1)
version=$(printf '%s\n' "$reply" | jq -er '.results[0].version_id')
call=$(printf '%s\n' "$reply" | jq -er .call_id)
omoide used --version "$version" --from-call "$call" --notice-id use-1
omoide acted --version "$version" --action ignite --opportunity-id kiln-start-1 --notice-id acted-1
omoide outcome --version "$version" --result success --opportunity-id kiln-start-1 --notice-id outcome-1
omoide stats --correction-threshold 2
```

- 返したことは呼び出しログに、実際の採用は `used`、実行は `acted`、成否は `outcome --result success|failure`、指摘は `corrected` に分けます。同じ知らせの再送には同じ notice ID を使います。operation ID と両方指定する場合は同じ値を使います。確定済みの版への知らせは保存できますが、古い版が通常検索に戻ることはありません。
- 照会を引き継ぐ `--from-call` は、その版を返した実在の recall の ID です。機会 ID は元の照会から継承でき、両方に指定する場合は一致が必要です。動作名は規則に一つだけ結びつく場合は省略でき、複数なら `acted` に明示します。通知の確定ごとに使用回数側の版を一度だけ進めます。
- stats は現行の各規則について、登録後の指摘と、その規則の動作・対象に対応する機会数を返します。機会は ID つきの recall または acted によって観測します。同じ機会の指摘・動作は各1回、機会 ID のない指摘は通知 ID ごとに数えます。機会が0なら `not_measured`。指摘が指定回数（既定2）以上なら `check_candidates` に載せます。
- SQLite の `calls` に成功した呼び出しの ID、種別、開始時刻、経過マイクロ秒、返した ID・版 ID、改行込みの出力バイト数、機会と動作を保存します。問い合わせ本文の既定は NULL です。必要な照会だけ `--log-query` を付けるか、`OMOIDE_LOG_QUERY=true` を設定します。書き込み不能なら本体の結果を返して警告し、recall では `accounting: unlogged` を示します。記録前の停止でも、確定済みの本体と受け取りは残ります。

段階1〜3の SQLite は初回に1トランザクションで形式4へ移行します。発言・規則・事実・受け取りを維持し、既存の事実から検索索引を構築します。切り戻しが必要な場合は事前に SQLite のバックアップ機能でコピーを作成してください。

## 常に読み込む節

```sh
omoide hot init AGENTS.md --limit-bytes 2048
omoide hot export AGENTS.md --limit-bytes 2048
omoide hot status AGENTS.md --limit-bytes 2048
```

init はマーカーがなければファイルの末尾に節を用意し、その時点の現行版を入れます。export は既存の開始・終了マーカーの間を更新します。上限は開始・終了マーカー、ヘッダー、末尾改行込みの UTF-8 バイト数で、既定は4,096です。改行やタグを含む登録文は1行のデータにエスケープします。対象・適用除外・スキルの参照先があれば同行に残します。`rule add --pinned` で固定した規則を先に全件入れ、残りは使用回数・新しさ・版 ID 順に、行全体が入るものを採ります。固定分が上限を超えた場合は超過バイト数を返し、ファイルは変更しません。入らなかったものも recall で探せます。

同じファイルの書き手は隣の `.omoide.lock` を共有します。ロック内で対象を読み、同じディレクトリの一時ファイルに書きます。SQLite の書き込み予約を取って保存 ID・データ版・使用版を再照合し、一致する間に rename します。競合中は読み直し、継続する場合は失敗を返します。破損または重複したマーカーを持つファイルは更新しません。status は保存 ID、両方の版、上限と書式から作った設定ダイジェストを比較します。

編集者がロックを使わず同時に書いた内容は保護できません。強制終了で未公開の一時ファイルが残る場合があります。更新前から続いている会話がすでに読み込んだ指示は取り消せません。新しい会話が参照するファイルの節だけが更新されます。

## 判定器を指定する

`recall --judge 実行ファイル --judge-arg 引数` で外部プロセスを明示します。引数は複数回指定でき、シェルを経由しません。未設定時は従来の語彙採点を使います。起動した判定器は現在の OS 環境を継承し、標準入力に問い合わせ（scene、action、target）と候補一覧（version_id、本文、場面、別表現）の JSON を受け取ります。設定した実行ファイルにこの内容を渡してよい場合にだけ使ってください。装置にネットワーククライアントはありません。

```json
{"answers":[{"version_id":"入力で受け取った版ID","score":0.85}]}
```

空でない返事は各候補をちょうど一度含み、点は0〜1です。`{"answers":[]}` は正常な「該当なし」です。採用の下限は `--threshold`（既定0.5、以上を採用）。同点なら従来の優先条件で決めます。小規模なら対象に合う全現行規則・スキルを渡すため、語の重なりがなくても候補を選べます。時間・出力上限は `--timeout-ms`（既定1,000）、`--output-bytes`（既定262,144）。失敗時は `judge_state=failed` と理由を付け、元の結果を返します。正常な空結果は `judge_state=used` と `matched_count=0` です。

### 検索結果と未確認候補

`results` は適用条件を通った記録です。`candidates` は確認用の短い見出しです。候補を使う前に `omoide show VERSION_ID` で本文・場面・対象・除外を読みます。`found` は確定結果あり、`candidates_only` は未確認候補のみ、`none` は対象をすべて調べて両欄が空の状態です。`incomplete_search` は確定結果がなく、未調査の範囲が残る状態です。両方の欄が4,000バイトを共有します。`candidates_remaining` はそのページに載らなかった未確認候補数です。

### Jev を使う場合

`omoide-judge-jev` は TypeSafe の `jev-1.14.0` を呼ぶ任意の判定器です。本体とは別にインストールし、`TYPESAFE_API_KEY` が設定された環境からコマンド名を指定します。

```sh
cargo install --locked --path crates/omoide-judge-jev
omoide recall --kind rule --scene '焼成後のパンをどこに置くか' --judge omoide-judge-jev --threshold 0.6
```

明示的にこの判定器を使うと、共有する問い合わせの場面・動作・対象と、候補ごとの本文・場面・別表現が TypeSafe に送信されます。送信内容、期限と再現手順は[アダプターの README](crates/omoide-judge-jev/README.md)にあります。

## 経験をスキルにする

```sh
first=$(omoide event add --text '搬入口で留め金を点検した' --target 工房 | jq -er .id)
second=$(omoide event add --text '搬入口で留め金を点検した' --target 工房 | jq -er .id)
omoide repeated --min 2
proposal=$(omoide skill propose --source "$first" --source "$second" --scene '資材の搬入' \
  --action unload --target 工房 --text '留め金を点検する' --reference manual/receiving.md)
omoide skill confirm "$(printf '%s\n' "$proposal" | jq -er .version_id)"
```

repeated は同じ対象と同じ正規化語集合を持つ経験を集めます。意味だけが同じ別の語までは判定しません。候補には1件以上の実在する重複のない経験 ID が必要です。候補の本文は呼び出し側が作ります。confirm で近い現行規則が返った場合は、規則登録と同じ `--relation`、`--against`、`--near-digest` を使います。same は経験を既存規則の出典に加え、restated は旧版を置き換え、exception は対象を指定した例外にします。unsure は判断待ちです。利用者が resolve で採る側を決めるまでスキルを有効にしません。

検索・節に載せるのは現行のスキルです。採用した版には used、実行の結果には outcome を送ります。検索結果の failure_count はその版の失敗総数、consecutive_failures は連続失敗数です。成功を通知すると連続数が0になります。既定では自動降格しません。`outcome --demote-after N` または `OMOIDE_DEMOTE_AFTER` が正なら、N回以上連続して失敗した現行スキルを候補に戻します。この変更と通知・受け取りは同時に確定します。

## オフライン評価

```sh
cargo build --locked --release
cargo build --locked --features fault-injection --target-dir target/fault
target/release/omoide eval run --scenes tests/scenes --fault-binary target/fault/debug/omoide > report.json
target/release/omoide eval diff before.json report.json --allow accepted.json
```

eval run は Python 3 標準ライブラリを使う同梱アダプターを起動し、各ケースを独立した DB と新しい CLI プロセスで試します。通常の登録・検索・書き出しには Python は不要です。JSON の各ケースには pass / not_met / not_run、返った記録、意味を保った観測値、返したバイト数を記録します。評価そのものが完了すれば未達も含めて正常終了します。中断可能な CLI がない場合は、故障位置が必要なケースを not_run と表示します。

`eval diff BUILD_A BUILD_B --builds` は同一の登録済みデータベースのバックアップを各ビルド用に複製し、同じケースと設定で比較します。S11 の中断位置を試す場合は `--before-fault` と `--after-fault` に各ビルドの検証用 CLI を指定します。保存済み JSON の比較もできます。認める差は例えば `[{"case":"example","field":"bytes","before":210,"after":224}]` のように、ケース、項目、前後の値を指定します。結果は allowed と unexplained に分かれます。

S1 の比較では通常の eval run に対し、`--scene S1 --oracle` は固定正解表を使った経路試験、`--scene S1 --aliases tests/support/s1-registration-aliases.json` は登録文から辞書で作った別表現だけの検索です。正解表の結果をモデル品質の測定には使いません。S12 の検証用コマンドは tests/support/deterministic_judge.py です。

`eval run --trials trials.json` は装置あり・なし・節のみの観測も集計します。入力は試行の JSON 配列です。各行に opportunity、variant（device / no_device / hot_only）、called、delivered_at、acted_at、used、action、corrected、実際に足した文字列 added_input を渡します。両時刻は同じ単位で、観測不能なら null。装置が呼ばれなかった機会も分母に残します。到着と動作の順序が分からない場合は unknown で、間に合った使用に数えません。追加量は入力文字列の実際の UTF-8 バイト数です。公開例と試験には架空の場面だけを使ってください。

## ローカル検証

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 verify/check_miwa.py
cargo build --locked --release
target/release/omoide eval run --scene S2
```

全段階の固定場面と故障試験は、上のオフライン評価の手順で実行します。故障注入の CLI は別途ビルドします。既定のビルドには中断機能は入りません。

```sh
cargo build --locked --features fault-injection --target-dir target/fault
target/release/omoide eval run --fault-binary target/fault/debug/omoide
python3 verify/relations_contract.py target/release/omoide target/fault/debug/omoide
python3 verify/hot_contract.py target/release/omoide target/fault/debug/omoide
```

Linux では Python 3 と GNU time を用意し、`python3 verify/measure.py target/release/omoide` で規則1万件を、同じ命令に `--dataset mixed` を添えて事実・経験各5千件の計測を再実行できます。

### 検索範囲を読む

確定結果が空なら notice が返ります。未確認候補の原文を読み、台帳など別の情報源も探してください。found でも search.incomplete が真なら検索範囲の一部を確認できていません。search.by_kind は種類ごとに eligible（対象）、scored（語彙採点）、judged（正常判定）を返します。rule_path、evidence_path、action_index が実際の経路、rule_cap、evidence_cap、reply_cap が上限に当たった箇所です。timeout、fallback、judge_remaining で時間切れ・基本検索への復帰・要求したが未判定の件数を確認できます。
