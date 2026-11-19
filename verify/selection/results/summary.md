| Split | First-page method | Required | Forbidden | Exact | None correct | Judge failures | Median / p95 ms | Input / output tokens with receipts |
|---|---|---:|---:|---:|---:|---:|---|---|
| DEV | core baseline | 56/68 | 62/76 | 0/76 | 0/24 | 0 | 11 / 26 | 0 / 0 (0 receipts) |
| DEV | Jev baseline (10 s) | 59/68 | 20/76 | 46/76 | 17/24 | 0 | 281 / 405 | 696568 / 90120 (76 receipts) |
| DEV | original fresh | 59/68 | 19/76 | 47/76 | 17/24 | 0 | 255 / 338 | 696568 / 90120 (76 receipts) |
| DEV | trigger | 50/68 | 9/76 | 47/76 | 19/24 | 0 | 277 / 342 | 782112 / 90120 (76 receipts) |
| DEV | instruction | 54/68 | 16/76 | 50/76 | 19/24 | 0 | 277 / 439 | 767016 / 90120 (76 receipts) |
| DEV | instruction + loose filter | 42/68 | 6/76 | 53/76 | 23/24 | 0 | 249 / 453 | 124195 / 12508 (71 receipts) |
| DEV | instruction + structured filter | 54/68 | 17/76 | 47/76 | 19/24 | 0 | 297 / 440 | 750118 / 88160 (76 receipts) |
| DEV | core selected | 52/68 | 49/76 | 14/76 | 4/24 | 0 | 8 / 15 | 0 / 0 (0 receipts) |
| DEV | original + structured filter, 0.65 | 51/68 | 5/76 | 55/76 | 21/24 | 0 | 272 / 329 | 681238 / 88160 (76 receipts) |
| DEV | Jev selected, 0.60 | 57/68 | 13/76 | 52/76 | 18/24 | 0 | 275 / 339 | 681238 / 88160 (76 receipts) |
| DEV | Jev selected, 1 s | 57/68 | 12/76 | 52/76 | 19/24 | 0 | 265 / 325 | 681238 / 88160 (76 receipts) |
| TEST | core baseline | 46/76 | 65/76 | 1/76 | 0/16 | 0 | 11 / 19 | 0 / 0 (0 receipts) |
| TEST | Jev baseline (10 s) | 72/76 | 25/76 | 44/76 | 13/16 | 0 | 273 / 388 | 695355 / 89976 (76 receipts) |
| TEST | core selected | 38/76 | 44/76 | 14/76 | 0/16 | 0 | 8 / 10 | 0 / 0 (0 receipts) |
| TEST | Jev selected, 0.60 | 66/76 | 17/76 | 48/76 | 15/16 | 0 | 265 / 336 | 680025 / 88024 (76 receipts) |
