| Primary comparison (44 pairs) | Net passes | Reduction in known mistakes on 36 targets | Net control overapplications | Frozen verdict |
|---|---:|---:|---:|---|
| device_core vs no_device | +0 | -1 | +0 | no practical difference |
| device_jev vs no_device | +0 | +0 | +0 | no practical difference |

| Arm | Used search / 48 | Search calls per session (mean / median / max) | Search bytes per session (mean / median / max) | List calls per session (mean / median / max) | List bytes per session (mean / median / max) |
|---|---:|---|---|---|---|
| no_device | 38/48 | 2.21 / 2 / 6 | 3455.29 / 3419.5 / 8949 | 0.23 / 0 / 1 | 424.19 / 0 / 1851 |
| hot_only | 38/48 | 1.77 / 2 / 6 | 3046.85 / 2860 / 11270 | 0.08 / 0 / 1 | 154.25 / 0 / 1851 |
| device_core | 35/48 | 1.08 / 1 / 3 | 1922.21 / 2054.5 / 5261 | 0.00 / 0 / 0 | 0.00 / 0 / 0 |
| device_jev | 37/48 | 1.15 / 1 / 3 | 2013.81 / 2105 / 5484 | 0.00 / 0 / 0 | 0.00 / 0 / 0 |

| Arm | Qualifying opportunities | Returned to ledger | Already used ledger (returned / eligible) | No prior ledger use (returned / eligible) | No recall | Only nonqualifying replies | Unobservable / no pre-action delivery |
|---|---:|---:|---:|---:|---:|---:|---:|
| device_core | 33 | 29 | 2/3 | 27/30 | 9 | 6 | 0 |
| device_jev | 33 | 13 | 6/25 | 7/8 | 10 | 5 | 0 |

| First qualifying response | device_core returned / eligible | device_jev returned / eligible |
|---|---:|---:|
| no confirmed match | 13/13 | 10/14 |
| incomplete | 16/19 | 1/11 |
| both | 0/1 | 2/8 |

| Primary targets only | Called / 36 | Full supporting record before action / 36 | Pass with delivery | Pass without delivery |
|---|---:|---:|---:|---:|
| device_core | 36/36 | 22/36 | 22/22 | 13/14 |
| device_jev | 36/36 | 16/36 | 16/16 | 19/20 |
