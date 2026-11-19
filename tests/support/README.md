# 固定判定器と登録時の別表現

deterministic_judge.py の empty は正常な空の返事、timeout は期限を超える処理。oracle は s1-oracle.json の固定表で版ごとの点を返す。正解を知る oracle は通信・候補・採点の経路試験専用で、意味検索や実モデルの品質指標には使わない。

make_aliases.py は標準入力の登録本文と場面だけを読み、NLTK 3.9.1、WordNet 3.0 と Open Multilingual WordNet 1.4 の英語・日本語の辞書から候補を生成する。問い合わせ、期待値、採点理由は入力しない。NLTK とコーパスを用意した独立コンテナにこのスクリプトだけをコピーし、ネットワークを切り、ワークスペースのマウントを0にして同じ25項目を再生成した。結果が s1-registration-aliases.json。語義を絞らない機械的な候補には、元の場面とは違う語義も含まれる。

再生成時に使った NLTK データの SHA-256 は、wordnet.zip が `cbda5ea6eef7f36a97a43d4a75f85e07fccbb4f23657d27b4ccbc93e2646ab59`、omw-1.4.zip が `3b941e664852f3297b6040236626065796a2aaf7d7f9eec8779a3beaa1096c2d`。辞書生成は検証専用で、omoide 本体には同梱しない。

```sh
jq -c '.records[] | select(.kind=="rule") | {text,scene}' tests/scenes/01-s01.json | python3 tests/support/make_aliases.py
PYTHONPATH=crates/omoide/support python3 tests/support/test_scene_adapter.py
```

後者は timeout・valid-empty・no-key の起動引数を個別に検査する。S9・S12 の本番相当の合否には一時的な小さいデータを使わず、tests/scenes の固定ケースを eval run に渡す。
