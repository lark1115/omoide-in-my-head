# omoide-judge-jev

TypeSafe の System One API に、各候補の規則が問い合わせの状況に当てはまる確率を尋ねます。応答は omoide の既存の判定器プロトコルです。omoide 本体だけをインストールした場合、Jev 用のバイナリや HTTP クライアントは入りません。

## 有効にする

```sh
cargo install --locked --path crates/omoide-judge-jev
omoide recall --kind rule --scene '焼き上がったパンを冷ます' --judge omoide-judge-jev --threshold 0.6
```

キーは環境変数 `TYPESAFE_API_KEY` から読み取ります。継続してこの判定器を使うシェルでは `OMOIDE_JUDGE=omoide-judge-jev` も設定できます。通常のインストールやキーの存在だけで外部通信は始まりません。

## 送信する内容

固定 URL `https://api.typesafe.ai/v1/systemone` に HTTPS POST を1回送ります。モデルは `jev-1.14.0` 固定です。共有する state は問い合わせの **scene、action、target**。各 Noul の instructions に、その候補の **text、scene、aliases** と「この規則は共有された状況に当てはまるか」という固定の問いを入れます。規則の別の言い方も肯定の対象とし、別の作業・範囲外の状況を否定の基準にしています。入力の版 ID はローカルに保持します。質問には q0、q1 など、その呼び出しだけの番号を付けます。元発言、履歴、機会 ID、追加の入力フィールドは送信しません。キーは Authorization ヘッダーにだけ渡し、リダイレクトは追いません。

同じ state を持つ全候補を1リクエストにまとめます。Jev 1.14 の標準コンテキストはリクエスト全体64k、質問単体32kトークンで、別途の質問数上限はありません。コンテキスト超過を含む HTTP エラーで、自動再送・入力の切り詰めは行いません。候補0件では HTTP を呼びません。公式の仕様は [System One API](https://docs.typesafe.ai/api/system-one.md)、[モデルと上限](https://docs.typesafe.ai/models.md)、[Noul](https://docs.typesafe.ai/primitives/noul.md) にあります。

## 結果と期限

返った各質問名を元の版 ID に対応づけ、Noul の確率を score としてそのまま渡します。omoide が閾値を適用します。モデル名違い、欠け・余分・重複した回答、型・確率範囲の違反、不正な JSON、通信障害、時間切れ、キー未設定では、スコアを出力せず非ゼロで終了します。設定済みの判定器が失敗した場合、omoide は `judge_state=failed` で語彙検索を返します。判定器自体を指定しない場合は `not_configured` です。

アダプターの期限は `JEV_TIMEOUT_MS`（既定900 ms）、プロセス起動を含む外側の期限は `omoide recall --timeout-ms`（既定1,000 ms）です。アダプターは使用済み時間を差し引いた予算で HTTP を呼び、応答処理の後にも残り時間を検査します。設定を変更する際はプロセス起動・入出力に使う差を残してください。`--report-usage` をアダプターに渡すと、成功した API 呼び出しのモデル名と入出力トークン数だけを標準エラーに出します。標準出力には常に判定結果だけを出します。

## 固定場面 S1 の計測

```sh
cargo build --release --locked -p omoide -p omoide-judge-jev
python3 verify/measure_jev.py target/release/omoide target/release/omoide-judge-jev
```

上の明示的な計測は API を利用します。各回で本物の `omoide eval run --scene S1` を起動し、評価の `--binary` に計測用ラッパーを指定します。ラッパーは実 CLI の recall にだけ Jev を接続し、起動から JSON を受け取るまでの時間を測ります。8ケースを3回判定し、同じ確率を実ポートに再生して0.3・0.7でも採点します。正例の候補は正解規則1件だけ、負例では同じ1件が不正解候補です。正例で複数の紛らわしい候補を順位づけする能力は、このデータだけでは測れません。

既存の24件の観測から、キーも API 通信も使わず閾値比較を再実行できます。

```sh
python3 verify/measure_jev.py target/release/omoide target/release/omoide-judge-jev \
  --replay-from verify/jev-s1-observations.jsonl
```

単体試験は記録済み応答とメモリ内の HTTP 応答を使います。ポートの結合検証では、実アダプターのキー欠落時の失敗と、製品と同じデコーダーに記録済みの API 応答を渡す試験ドライバーを使用します。

```sh
cargo test --locked -p omoide-judge-jev
cargo build --release --locked -p omoide-judge-jev --example recorded
python3 verify/jev_port.py target/release/omoide target/release/omoide-judge-jev target/release/examples/recorded
```

大きな候補集合では本体が入力を分割し、束ごとに一度送信します。規則とスキルでは適用、決定は選択との関連、事実・経験は状況の証拠との関連を尋ねます。通常のモデル枠（要求64k、質問32k）に余裕を取り、トークン数の保守的な代用として UTF-8 の直列化バイト数を使います。要求48,000バイト、共有状態と単独質問24,000バイトを超えるペイロードは送信前に拒否します。本体は質問ごとに1,024バイトの余裕を加えて分割し、判定の全体期限を共有します。失敗時にも完了した束の判定結果を使い、未判定分だけ語彙検索に戻します。
