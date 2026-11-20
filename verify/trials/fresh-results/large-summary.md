| All 48 | Ledger bytes (sum / median / max) | Ledger calls (sum / median / max) | List calls / bytes | Search calls / bytes | Budget closed | Hot bytes | Device calls / errors / judge failures | Device bytes | Added bytes | Full result before action | Required candidate before action | Relevant common ledger row delivered |
|---|---|---|---|---|---:|---:|---|---:|---:|---:|---:|---:|
| no_device | 186215 / 3890.5 / 8949 | 117 / 3 / 6 | 11 / 20361 | 106 / 165854 | 0 | 0 | 0 / 0 / 0 | 0 | 0 | 0 | 0 | 35 |
| hot_only | 153653 / 2883 / 11270 | 89 / 2 / 6 | 4 / 7404 | 85 / 146249 | 0 | 24624 | 0 / 0 / 0 | 0 | 24624 | 0 | 0 | 33 |
| device_core | 92266 / 2054.5 / 5261 | 52 / 1 / 3 | 0 / 0 | 52 / 92266 | 0 | 24624 | 41 / 2 / 0 | 100631 | 125255 | 22 | 0 | 28 |
| device_jev | 96663 / 2105 / 5484 | 55 / 1 / 3 | 0 / 0 | 55 / 96663 | 0 | 24624 | 42 / 3 / 2 | 78934 | 103558 | 16 | 3 | 29 |

| Baseline → comparison | Failure → pass | Pass → failure | Net |
|---|---|---|---:|
| no_device → hot_only | 0 () | 0 () | +0 |
| no_device → device_core | 1 (L05) | 1 (L31) | +0 |
| no_device → device_jev | 1 (L05) | 1 (L30) | +0 |
| hot_only → device_core | 1 (L05) | 1 (L31) | +0 |
| hot_only → device_jev | 1 (L05) | 1 (L30) | +0 |
| device_core → device_jev | 1 (L31) | 1 (L30) | +0 |

Frozen device_core primary decision: **no practical difference**. Net passes +0, net control overapplications +0, reduction in known mistakes on 36 target opportunities -1.
