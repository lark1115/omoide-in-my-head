| All 48 | Ledger bytes (sum / median / max) | Ledger calls (sum / median / max) | List calls / bytes | Search calls / bytes | Budget closed | Hot bytes | Device calls / errors / judge failures | Device bytes | Added bytes | Full result before action | Required candidate before action | Relevant common ledger row delivered |
|---|---|---|---|---|---:|---:|---|---:|---:|---:|---:|---:|
| no_device | 197449 / 4272.5 / 14613 | 158 / 3 / 15 | 15 / 32370 | 143 / 165079 | 0 | 0 | 0 / 0 / 0 | 0 | 0 | 0 | 0 | 35 |
| hot_only | 149042 / 2591 / 9931 | 105 / 2 / 8 | 10 / 21580 | 95 / 127462 | 0 | 20352 | 0 / 0 / 0 | 0 | 20352 | 0 | 0 | 31 |
| device | 81129 / 1882 / 5630 | 59 / 1 / 4 | 1 / 2158 | 58 / 78971 | 0 | 20352 | 53 / 10 / 0 | 115676 | 136028 | 17 | 0 | 27 |
| device_jev | 76057 / 1876.5 / 6603 | 56 / 1 / 3 | 0 / 0 | 56 / 76057 | 0 | 20352 | 54 / 11 / 10 | 79373 | 99725 | 9 | 5 | 26 |

| Baseline → comparison | Failure → pass | Pass → failure | Net |
|---|---|---|---:|
| no_device → hot_only | 1 (T32) | 6 (T12, T13, T18, T35, T44, T46) | -5 |
| no_device → device | 2 (T32, T47) | 6 (T01, T05, T07, T12, T35, T44) | -4 |
| no_device → device_jev | 2 (T32, T47) | 6 (T01, T05, T12, T13, T15, T44) | -4 |
| hot_only → device | 4 (T13, T18, T46, T47) | 3 (T01, T05, T07) | +1 |
| hot_only → device_jev | 4 (T18, T35, T46, T47) | 3 (T01, T05, T15) | +1 |
| device → device_jev | 2 (T07, T35) | 2 (T13, T15) | +0 |

Frozen primary decision: **harm**. Net passes -4, net control overapplications +0, reduction in known mistakes on 36 target opportunities +0.
