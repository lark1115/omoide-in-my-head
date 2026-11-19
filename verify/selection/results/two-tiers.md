| Split | Method | Strict exact | Forbidden in strict | Strict required | Union required | Mean bytes | None correct | No confirmed on negatives | Candidates only on negatives | Judge failures | Median / p95 ms | Input / output tokens |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| dev | tier-core | 14/76 | 49/76 | 52/68 | 56/68 | 3292.8 | 0/24 | 4/24 | 4/24 | 0 | 6 / 7 | 0 / 0 |
| dev | tier-jev | 51/76 | 13/76 | 56/68 | 63/68 | 1272.6 | 12/24 | 18/24 | 6/24 | 0 | 251 / 294 | 681238 / 88160 |
| holdout2 | base-core | 1/80 | 67/80 | 59/80 | 59/80 | 3460.1 | 1/20 | 1/20 | 0/20 | 0 | 11 / 23 | 0 / 0 |
| holdout2 | base-jev | 58/80 | 15/80 | 69/80 | 69/80 | 913.1 | 19/20 | 19/20 | 0/20 | 0 | 252 / 317 | 722042 / 93264 |
| holdout2 | tier-core | 7/80 | 51/80 | 44/80 | 60/80 | 3438.3 | 1/20 | 3/20 | 2/20 | 0 | 6 / 6 | 0 / 0 |
| holdout2 | tier-jev | 56/80 | 8/80 | 60/80 | 79/80 | 1154.4 | 12/20 | 20/20 | 8/20 | 0 | 245 / 289 | 702915 / 90816 |
