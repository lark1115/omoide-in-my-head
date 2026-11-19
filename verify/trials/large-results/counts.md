| Cohort or mistake | Arm | Pass | Known mistake | Other failure | Cannot tell |
|---|---|---:|---:|---:|---:|
| primary + control | no_device | 41 | 1 | 1 | 1 |
| primary + control | hot_only | 36 | 2 | 1 | 5 |
| primary + control | device | 37 | 1 | 0 | 6 |
| primary + control | device_jev | 37 | 1 | 0 | 6 |
| control | no_device | 8 | 0 | 0 | 0 |
| control | hot_only | 8 | 0 | 0 | 0 |
| control | device | 8 | 0 | 0 | 0 |
| control | device_jev | 8 | 0 | 0 | 0 |
| decision_reopened | no_device | 6 | 0 | 0 | 0 |
| decision_reopened | hot_only | 4 | 0 | 0 | 2 |
| decision_reopened | device | 4 | 1 | 0 | 1 |
| decision_reopened | device_jev | 6 | 0 | 0 | 0 |
| hearsay_verified | no_device | 6 | 0 | 0 | 0 |
| hearsay_verified | hot_only | 6 | 0 | 0 | 0 |
| hearsay_verified | device | 6 | 0 | 0 | 0 |
| hearsay_verified | device_jev | 6 | 0 | 0 | 0 |
| outside | no_device | 4 | 0 | 0 | 0 |
| outside | hot_only | 4 | 0 | 0 | 0 |
| outside | device | 4 | 0 | 0 | 0 |
| outside | device_jev | 4 | 0 | 0 | 0 |
| prior_resource | no_device | 5 | 1 | 0 | 0 |
| prior_resource | hot_only | 3 | 0 | 0 | 3 |
| prior_resource | device | 4 | 0 | 0 | 2 |
| prior_resource | device_jev | 3 | 0 | 0 | 3 |
| rule_omitted | no_device | 5 | 0 | 0 | 1 |
| rule_omitted | hot_only | 4 | 2 | 0 | 0 |
| rule_omitted | device | 4 | 0 | 0 | 2 |
| rule_omitted | device_jev | 3 | 1 | 0 | 2 |
| scope_overreach | no_device | 6 | 0 | 0 | 0 |
| scope_overreach | hot_only | 6 | 0 | 0 | 0 |
| scope_overreach | device | 5 | 0 | 0 | 1 |
| scope_overreach | device_jev | 5 | 0 | 0 | 1 |
| work_repeated | no_device | 5 | 0 | 1 | 0 |
| work_repeated | hot_only | 5 | 0 | 1 | 0 |
| work_repeated | device | 6 | 0 | 0 | 0 |
| work_repeated | device_jev | 6 | 0 | 0 | 0 |

| Baseline → comparison (44 pairs) | Both pass | Failure → pass | Pass → failure | Neither passes |
|---|---:|---:|---:|---:|
| no_device → device | 35 | 2 | 6 | 1 |
| no_device → hot_only | 35 | 1 | 6 | 2 |
| hot_only → device | 33 | 4 | 3 | 4 |
| no_device → device_jev | 35 | 2 | 6 | 1 |
| hot_only → device_jev | 33 | 4 | 3 | 4 |
| device → device_jev | 35 | 2 | 2 | 5 |

| Arm (all 48) | Called | Any reply before action | Relevant lookup before action | Added bytes | Common ledger bytes | Input tokens incl. cache |
|---|---:|---:|---:|---:|---:|---:|
| no_device | 0 | 0 | 0 | 0 | 197449 | 346291 |
| hot_only | 0 | 0 | 0 | 20352 | 149042 | 261471 |
| device | 40 | 40 | 17 | 136028 | 81129 | 326218 |
| device_jev | 40 | 40 | 9 | 99725 | 76057 | 296621 |
