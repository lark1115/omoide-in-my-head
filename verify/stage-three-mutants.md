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
