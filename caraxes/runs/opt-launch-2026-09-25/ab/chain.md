## ab

### A/launch (3 device logs)

| step | p50 us (median over reps) | mean us | n per rep |
|---|---|---|---|
| lock | 0.6 | 80.46 | 9106 |
| dispatch | 1.2 | 1.69 | 8326 |
| ran-ahead check | 1.5 | 1.74 | 8326 |
| plan | 0.2 | 0.22 | 8326 |
| worker wait | 0.1 | 0.07 | 8326 |
| BAR1 flush | 1.4 | 1.59 | 8326 |
| USERD read | 1.5 | 1.69 | 8326 |
| host doorbell write | 0.1 | 0.09 | 8326 |
| tail after the host doorbell | 4.2 | 5.81 | 8322 |
| ring to host doorbell (sum) | 6.4 | 7.52 | 8322 |
| host doorbell to the next ring | 120.0 | 21956.59 | 9102 |
| host doorbell to host event | 1068071.8 | 276498.81 | 871 |
| host event to raise | 2.5 | 4.50 | 876 |
| raise to MSI | 15.2 | 211.75 | 793 |

rings 9106 (807 after a blocking wait), host doorbells 8386; host events: 4 before any doorbell, 158 < 1 ms, 253 < 100 ms, 444 older (medians over reps)
- rep 1: MSIs for raises 1547; hybrid 200 us idle
- rep 2: MSIs for raises 774; hybrid 200 us idle
- rep 3: MSIs for raises 800; hybrid 200 us idle

guest: h2d pinned 1M GB/s 5.845 [4.651-10.871], h2d pageable 1M GB/s 4.468 [4.062-6.6], h2d pinned 16M GB/s 6.553 [6.541-12.868], d2d device 16M GB/s 33.433 [16.497-148.097], dev_cpu_ms 1730 [530-7990], interval_mean_us 22.21 [21.25-36.2], interval_p50_us 18.94 [18.2-19.45], interval_p90_us 20.18 [20.04-20.81], launch_mean_us 5.9 [4.5-10.13], launch_p50_us 4.23 [4.02-5.49], launch_p90_us 4.73 [4.46-6.31], sync_mean_us 16.68 [16.23-26.02], sync_p50_us 14.1 [13.74-14.64], sync_p90_us 15.01 [14.58-15.56], ms/launch 0.021 [0.019-0.123]

### A/ppo (2 device logs)

| step | p50 us (median over reps) | mean us | n per rep |
|---|---|---|---|
| lock | 0.6 | 1.12 | 844935 |
| dispatch | 1.5 | 2.21 | 837149 |
| ran-ahead check | 2.0 | 2.36 | 837149 |
| plan | 0.2 | 0.26 | 837149 |
| worker wait | 0.1 | 0.08 | 837149 |
| BAR1 flush | 1.5 | 1.75 | 837149 |
| USERD read | 2.0 | 2.22 | 837149 |
| host doorbell write | 0.1 | 0.09 | 837149 |
| tail after the host doorbell | 4.8 | 6.56 | 834389 |
| ring to host doorbell (sum) | 8.0 | 9.48 | 834389 |
| host doorbell to the next ring | 11.8 | 4181.69 | 844931 |
| host doorbell to host event | 535.0 | 230225.33 | 12890 |
| host event to raise | 3.2 | 4.63 | 12894 |
| raise to MSI | 17.6 | 202.32 | 12398 |

rings 844935 (33833 after a blocking wait), host doorbells 837169; host events: 4 before any doorbell, 8270 < 1 ms, 3269 < 100 ms, 1351 older (medians over reps)
- rep 1: MSIs for raises 12145; hybrid 200 us idle
- rep 2: MSIs for raises 12949; hybrid 200 us idle

guest: ppo steps/s 25158 [16223-34093], dev_cpu_ms 32030 [26970-37090]

### A/rl (3 device logs)

| step | p50 us (median over reps) | mean us | n per rep |
|---|---|---|---|
| lock | 0.5 | 3.63 | 77280 |
| dispatch | 1.6 | 1.82 | 76251 |
| ran-ahead check | 2.5 | 2.73 | 76251 |
| plan | 0.2 | 0.15 | 76251 |
| worker wait | 0.1 | 0.06 | 76251 |
| BAR1 flush | 2.3 | 2.53 | 76251 |
| USERD read | 2.5 | 2.61 | 76251 |
| host doorbell write | 0.1 | 0.06 | 76251 |
| tail after the host doorbell | 4.2 | 4.93 | 76116 |
| ring to host doorbell (sum) | 9.8 | 10.27 | 76116 |
| host doorbell to the next ring | 7.5 | 9029.07 | 77276 |
| host doorbell to host event | 2405492.9 | 599189.53 | 1234 |
| host event to raise | 2.0 | 3.31 | 1241 |
| raise to MSI | 10.9 | 84.29 | 1202 |

rings 77280 (1232 after a blocking wait), host doorbells 76271; host events: 7 before any doorbell, 355 < 1 ms, 246 < 100 ms, 633 older (medians over reps)
- rep 1: MSIs for raises 2232; hybrid 200 us idle
- rep 2: MSIs for raises 1205; hybrid 200 us idle
- rep 3: MSIs for raises 510; hybrid 200 us idle

guest: rl stage4 s 2.4 [2.2-3], rl xfer/s 1192 [965-1289], dev_cpu_ms 3280 [3120-5000]

### B/launch (3 device logs)

| step | p50 us (median over reps) | mean us | n per rep |
|---|---|---|---|
| lock | 0.6 | 100.49 | 8931 |
| dispatch | 0.1 | 0.22 | 8319 |
| ran-ahead check | 0.9 | 1.13 | 8319 |
| plan | 0.2 | 0.26 | 8319 |
| worker wait | 0.1 | 0.07 | 8319 |
| BAR1 flush | 0.1 | 0.06 | 8319 |
| USERD read | 0.8 | 0.91 | 8319 |
| host doorbell write | 0.1 | 0.18 | 8319 |
| tail after the host doorbell | 1.9 | 4.98 | 8315 |
| ring to host doorbell (sum) | 3.0 | 3.64 | 8315 |
| host doorbell to the next ring | 130.0 | 14118.57 | 8928 |
| host doorbell to host event | 2115226.7 | 502163.79 | 1399 |
| host event to raise | 2.6 | 3.78 | 656 |
| raise to MSI | 13.3 | 309.78 | 634 |

rings 8931 (269 after a blocking wait), host doorbells 8379; host events: 7 before any doorbell, 145 < 1 ms, 322 < 100 ms, 799 older (medians over reps)
- rep 1: MSIs for raises 620; flushes 1636 done / 9008 skipped; gate 656 at once / 1058 held / 135 releases; hybrid 200 us idle, adaptive up to 2000 us, last window 200 us
- rep 2: MSIs for raises 757; flushes 1636 done / 9007 skipped; gate 839 at once / 573 held / 54 releases; hybrid 200 us idle, adaptive up to 2000 us, last window 1902 us
- rep 3: MSIs for raises 659; flushes 1636 done / 9009 skipped; gate 656 at once / 181 held / 23 releases; hybrid 200 us idle, adaptive up to 2000 us, last window 200 us

guest: h2d pinned 1M GB/s 6.085 [5.951-6.091], h2d pageable 1M GB/s 4.401 [4.339-4.57], h2d pinned 16M GB/s 6.553 [6.523-6.556], d2d device 16M GB/s 15.199 [10.123-16.889], dev_cpu_ms 1930 [620-3360], interval_mean_us 20.48 [18.68-21.33], interval_p50_us 13.37 [12.33-15.05], interval_p90_us 15.3 [13.29-16.19], launch_mean_us 5.53 [5.14-7.15], launch_p50_us 4.57 [3.94-4.91], launch_p90_us 5.32 [4.17-5.49], sync_mean_us 15.26 [11.49-15.76], sync_p50_us 9.33 [7.66-10.01], sync_p90_us 10.79 [8.03-10.84], ms/launch 0.02 [0.016-0.04]

### B/ppo (2 device logs)

| step | p50 us (median over reps) | mean us | n per rep |
|---|---|---|---|
| lock | 0.6 | 8.12 | 875695 |
| dispatch | 0.2 | 0.76 | 869594 |
| ran-ahead check | 1.1 | 1.82 | 869594 |
| plan | 0.2 | 0.38 | 869594 |
| worker wait | 0.1 | 0.09 | 869594 |
| BAR1 flush | 0.1 | 0.08 | 869594 |
| USERD read | 1.1 | 1.35 | 869594 |
| host doorbell write | 0.2 | 0.36 | 869594 |
| tail after the host doorbell | 2.0 | 4.24 | 866838 |
| ring to host doorbell (sum) | 3.4 | 5.48 | 866838 |
| host doorbell to the next ring | 17.9 | 2400.12 | 875691 |
| host doorbell to host event | 1430.0 | 560773.08 | 14051 |
| host event to raise | 3.1 | 5.06 | 12015 |
| raise to MSI | 83.5 | 431.92 | 10991 |

rings 875695 (14415 after a blocking wait), host doorbells 869614; host events: 13 before any doorbell, 7085 < 1 ms, 4181 < 100 ms, 2785 older (medians over reps)
- rep 1: MSIs for raises 10760; flushes 461 done / 266298 skipped; gate 11577 at once / 3012 held / 284 releases; hybrid 200 us idle, adaptive up to 2000 us, last window 1363 us
- rep 2: MSIs for raises 11893; flushes 461 done / 1473543 skipped; gate 12453 at once / 1088 held / 90 releases; hybrid 200 us idle, adaptive up to 2000 us, last window 2000 us

guest: ppo steps/s 25773.5 [8800-42747], dev_cpu_ms 40550 [31930-49170]

### B/rl (3 device logs)

| step | p50 us (median over reps) | mean us | n per rep |
|---|---|---|---|
| lock | 0.6 | 3.59 | 82420 |
| dispatch | 0.1 | 0.19 | 81059 |
| ran-ahead check | 1.4 | 1.47 | 81059 |
| plan | 0.2 | 0.17 | 81059 |
| worker wait | 0.1 | 0.06 | 81059 |
| BAR1 flush | 0.1 | 0.06 | 81059 |
| USERD read | 1.2 | 1.33 | 81059 |
| host doorbell write | 0.1 | 0.18 | 81059 |
| tail after the host doorbell | 1.9 | 2.66 | 80867 |
| ring to host doorbell (sum) | 3.7 | 4.04 | 80867 |
| host doorbell to the next ring | 13.5 | 1775.48 | 82417 |
| host doorbell to host event | 2869125.4 | 663639.35 | 2162 |
| host event to raise | 3.8 | 4.55 | 832 |
| raise to MSI | 12.1 | 80.72 | 805 |

rings 82420 (662 after a blocking wait), host doorbells 81079; host events: 8 before any doorbell, 431 < 1 ms, 283 < 100 ms, 674 older (medians over reps)
- rep 1: MSIs for raises 982; flushes 455 done / 78426 skipped; gate 832 at once / 2187 held / 268 releases; hybrid 200 us idle, adaptive up to 2000 us, last window 200 us
- rep 2: MSIs for raises 826; flushes 455 done / 84679 skipped; gate 784 at once / 573 held / 71 releases; hybrid 200 us idle, adaptive up to 2000 us, last window 200 us
- rep 3: MSIs for raises 1606; flushes 455 done / 81366 skipped; gate 1623 at once / 575 held / 65 releases; hybrid 200 us idle, adaptive up to 2000 us, last window 200 us

guest: rl stage4 s 2.7 [2.6-3], rl xfer/s 1052 [947-1126], dev_cpu_ms 4120 [3870-8360]

