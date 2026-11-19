# 段階2の核の変異試験

cargo-mutants 27.1.0 で `cargo mutants -p omoide-core` を実行。Linux ARM64・Rust 1.88.0 の専用コピーで、1件ずつ同じテストを回した。51件のうち検出47件、生き残り0件、コンパイル不能4件、時間切れ0件。等価として許容した生き残りはない。

| コンパイル不能だった変更 | 理由 |
|---|---|
| Rank の cmp を Default::default() にする | Ordering が Default を実装していない |
| Rank の partial_cmp を Some(Default::default()) にする | Ordering が Default を実装していない |
| registration を Default::default() にする | Outcome が Default を実装していない |
| rank を vec![Default::default()] にする | Rank が Default を実装していない |

段階1の監査で見つかった順位・近さ・ページ境界は、核のテストだけで検証する。ページの長さは、次のカーソルと末尾改行を含めて別に組み立てた JSON と比較する。段階2では共通のバイト数判定を1か所にまとめたため、段階1とは生成される変異の総数が異なる。
