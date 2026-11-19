| Cohort or mistake | Arm | Pass | Known mistake | Other failure | Cannot tell |
|---|---|---:|---:|---:|---:|
| primary + control | no_device | 41 | 1 | 0 | 2 |
| primary + control | hot_only | 41 | 0 | 1 | 2 |
| primary + control | device | 42 | 0 | 0 | 2 |
| control | no_device | 7 | 0 | 0 | 1 |
| control | hot_only | 8 | 0 | 0 | 0 |
| control | device | 8 | 0 | 0 | 0 |
| decision_reopened | no_device | 5 | 1 | 0 | 0 |
| decision_reopened | hot_only | 5 | 0 | 1 | 0 |
| decision_reopened | device | 6 | 0 | 0 | 0 |
| hearsay_verified | no_device | 6 | 0 | 0 | 0 |
| hearsay_verified | hot_only | 6 | 0 | 0 | 0 |
| hearsay_verified | device | 6 | 0 | 0 | 0 |
| outside | no_device | 4 | 0 | 0 | 0 |
| outside | hot_only | 3 | 1 | 0 | 0 |
| outside | device | 4 | 0 | 0 | 0 |
| prior_resource | no_device | 5 | 0 | 0 | 1 |
| prior_resource | hot_only | 5 | 0 | 0 | 1 |
| prior_resource | device | 4 | 0 | 0 | 2 |
| rule_omitted | no_device | 6 | 0 | 0 | 0 |
| rule_omitted | hot_only | 6 | 0 | 0 | 0 |
| rule_omitted | device | 6 | 0 | 0 | 0 |
| scope_overreach | no_device | 6 | 0 | 0 | 0 |
| scope_overreach | hot_only | 6 | 0 | 0 | 0 |
| scope_overreach | device | 6 | 0 | 0 | 0 |
| work_repeated | no_device | 6 | 0 | 0 | 0 |
| work_repeated | hot_only | 5 | 0 | 0 | 1 |
| work_repeated | device | 6 | 0 | 0 | 0 |

| Baseline → comparison (44 pairs) | Both pass | Failure → pass | Pass → failure | Neither passes |
|---|---:|---:|---:|---:|
| no_device → device | 40 | 2 | 1 | 1 |
| no_device → hot_only | 39 | 2 | 2 | 1 |
| hot_only → device | 40 | 2 | 1 | 1 |

| Arm (all 48) | Called | Any reply before action | Relevant lookup before action | Added bytes | Common ledger bytes | Input tokens incl. cache |
|---|---:|---:|---:|---:|---:|---:|
| no_device | 0 | 0 | 0 | 0 | 139891 | 294769 |
| hot_only | 0 | 0 | 0 | 20352 | 121592 | 249294 |
| device | 40 | 40 | 25 | 85740 | 41966 | 263587 |
