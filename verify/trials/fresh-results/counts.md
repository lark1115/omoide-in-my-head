| Cohort or mistake | Arm | Pass | Known mistake | Other failure | Cannot tell |
|---|---|---:|---:|---:|---:|
| primary + control | no_device | 43 | 0 | 1 | 0 |
| primary + control | hot_only | 43 | 0 | 1 | 0 |
| primary + control | device_core | 43 | 1 | 0 | 0 |
| primary + control | device_jev | 43 | 0 | 1 | 0 |
| control | no_device | 8 | 0 | 0 | 0 |
| control | hot_only | 8 | 0 | 0 | 0 |
| control | device_core | 8 | 0 | 0 | 0 |
| control | device_jev | 8 | 0 | 0 | 0 |
| decision_reopened | no_device | 6 | 0 | 0 | 0 |
| decision_reopened | hot_only | 6 | 0 | 0 | 0 |
| decision_reopened | device_core | 6 | 0 | 0 | 0 |
| decision_reopened | device_jev | 6 | 0 | 0 | 0 |
| hearsay_verified | no_device | 5 | 0 | 1 | 0 |
| hearsay_verified | hot_only | 5 | 0 | 1 | 0 |
| hearsay_verified | device_core | 6 | 0 | 0 | 0 |
| hearsay_verified | device_jev | 5 | 0 | 1 | 0 |
| outside | no_device | 4 | 0 | 0 | 0 |
| outside | hot_only | 4 | 0 | 0 | 0 |
| outside | device_core | 4 | 0 | 0 | 0 |
| outside | device_jev | 4 | 0 | 0 | 0 |
| prior_resource | no_device | 6 | 0 | 0 | 0 |
| prior_resource | hot_only | 6 | 0 | 0 | 0 |
| prior_resource | device_core | 6 | 0 | 0 | 0 |
| prior_resource | device_jev | 6 | 0 | 0 | 0 |
| rule_omitted | no_device | 6 | 0 | 0 | 0 |
| rule_omitted | hot_only | 6 | 0 | 0 | 0 |
| rule_omitted | device_core | 5 | 1 | 0 | 0 |
| rule_omitted | device_jev | 6 | 0 | 0 | 0 |
| scope_overreach | no_device | 6 | 0 | 0 | 0 |
| scope_overreach | hot_only | 6 | 0 | 0 | 0 |
| scope_overreach | device_core | 6 | 0 | 0 | 0 |
| scope_overreach | device_jev | 6 | 0 | 0 | 0 |
| work_repeated | no_device | 6 | 0 | 0 | 0 |
| work_repeated | hot_only | 6 | 0 | 0 | 0 |
| work_repeated | device_core | 6 | 0 | 0 | 0 |
| work_repeated | device_jev | 6 | 0 | 0 | 0 |

| Baseline → comparison (44 pairs) | Both pass | Failure → pass | Pass → failure | Neither passes |
|---|---:|---:|---:|---:|
| no_device → device_core | 42 | 1 | 1 | 0 |
| no_device → hot_only | 43 | 0 | 0 | 1 |
| hot_only → device_core | 42 | 1 | 1 | 0 |
| no_device → device_jev | 42 | 1 | 1 | 0 |
| hot_only → device_jev | 42 | 1 | 1 | 0 |
| device_core → device_jev | 42 | 1 | 1 | 0 |

| Arm (all 48) | Called | Any reply before action | Relevant lookup before action | Added bytes | Common ledger bytes | Input tokens incl. cache |
|---|---:|---:|---:|---:|---:|---:|
| no_device | 0 | 0 | 0 | 0 | 186215 | 275155 |
| hot_only | 0 | 0 | 0 | 24624 | 153653 | 253800 |
| device_core | 39 | 39 | 22 | 125255 | 92266 | 286681 |
| device_jev | 38 | 38 | 16 | 103558 | 96663 | 281657 |
