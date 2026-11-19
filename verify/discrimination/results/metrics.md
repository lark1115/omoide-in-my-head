| Arm | Overlap | Required returned | Forbidden opportunities | Forbidden IDs | Exact | None correct | No response | Forbidden upper bound | Judge failures |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| core | all | 102/144 | 127/152 | 382 | 1/152 | 0/40 | 0 | 127/152 | 0 |
| core | shared | 86/88 | 72/80 | 231 | 1/80 | 0/16 | 0 | 72/80 | 0 |
| core | partial | 16/25 | 38/41 | 124 | 0/41 | 0/16 | 0 | 38/41 | 0 |
| core | zero | 0/31 | 17/31 | 27 | 0/31 | 0/8 | 0 | 17/31 | 0 |
| jev-default | all | 130/144 | 45/152 | 76 | 87/152 | 30/40 | 0 | 45/152 | 7 |
| jev-default | shared | 74/88 | 22/80 | 32 | 44/80 | 12/16 | 0 | 22/80 | 5 |
| jev-default | partial | 25/25 | 8/41 | 18 | 27/41 | 11/16 | 0 | 8/41 | 2 |
| jev-default | zero | 31/31 | 15/31 | 26 | 16/31 | 7/8 | 0 | 15/31 | 0 |
| jev-extended | all | 131/144 | 45/152 | 69 | 90/152 | 30/40 | 0 | 45/152 | 0 |
| jev-extended | shared | 76/88 | 20/80 | 28 | 49/80 | 12/16 | 0 | 20/80 | 0 |
| jev-extended | partial | 24/25 | 8/41 | 13 | 27/41 | 11/16 | 0 | 8/41 | 0 |
| jev-extended | zero | 31/31 | 17/31 | 28 | 14/31 | 7/8 | 0 | 17/31 | 0 |

core: median 11.2 ms, p95 21.0 ms, max 30.3 ms. Candidate range unobserved. Token receipts 0/152, input 0, output 0.

jev-default: median 314.8 ms, p95 845.2 ms, max 1053.1 ms. Candidate range (16, 72). Token receipts 145/152, input 1329704, output 172056.

jev-extended: median 278.1 ms, p95 396.5 ms, max 586.3 ms. Candidate range (16, 72). Token receipts 152/152, input 1391923, output 180096.
