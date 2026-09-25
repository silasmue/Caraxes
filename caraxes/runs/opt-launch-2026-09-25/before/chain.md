## before

### launch1k (3 device logs)

| step | p50 us (median over reps) | mean us | n per rep |
|---|---|---|---|
| lock | 2.3 | 2.23 | 3 |

rings 3 (1 after a blocking wait), host doorbells 0; host events: 0 before any doorbell, 0 < 1 ms, 0 < 100 ms, 0 older (medians over reps)
- rep 1: MSIs for raises 1; hybrid 200 us idle
- rep 2: MSIs for raises 1; hybrid 200 us idle
- rep 3: MSIs for raises 0; hybrid 200 us idle

guest: dev_cpu_ms 0 [0-0]

### rl (2 device logs)

| step | p50 us (median over reps) | mean us | n per rep |
|---|---|---|---|
| lock | 0.7 | 24.44 | 69297 |
| dispatch | 1.7 | 3.59 | 67890 |
| ran-ahead check | 2.2 | 3.27 | 67890 |
| plan | 0.2 | 0.41 | 67890 |
| worker wait | 0.1 | 0.09 | 67890 |
| BAR1 flush | 2.1 | 2.90 | 67890 |
| USERD read | 2.2 | 2.97 | 67890 |
| host doorbell write | 0.1 | 0.13 | 67890 |
| tail after the host doorbell | 5.5 | 9.71 | 67209 |
| ring to host doorbell (sum) | 9.4 | 13.98 | 67209 |
| host doorbell to the next ring | 16.7 | 30330.53 | 69293 |
| host doorbell to host event | 4930967.8 | 1229886.35 | 2744 |
| host event to raise | 4.8 | 11.60 | 2748 |
| raise to MSI | 23.5 | 212.41 | 2297 |

rings 34648 (2516 after a blocking wait), host doorbells 33955; host events: 2 before any doorbell, 381 < 1 ms, 288 < 100 ms, 702 older (medians over reps)
- rep 1: MSIs for raises 2349; hybrid 200 us idle
- rep 2: MSIs for raises 0; hybrid 200 us idle

guest: rl stage4 s 5.8 [5.8-5.8], rl xfer/s 499 [499-499], dev_cpu_ms 4790 [4790-4790]

### short-auto (3 device logs)

| step | p50 us (median over reps) | mean us | n per rep |
|---|---|---|---|
| lock | 0.7 | 738.46 | 339 |
| dispatch | 1.5 | 1.97 | 229 |
| ran-ahead check | 2.3 | 2.32 | 229 |
| plan | 0.2 | 0.19 | 229 |
| worker wait | 0.1 | 0.07 | 229 |
| BAR1 flush | 2.2 | 2.26 | 229 |
| USERD read | 2.3 | 2.27 | 229 |
| host doorbell write | 0.1 | 0.09 | 229 |
| tail after the host doorbell | 4.3 | 5.20 | 228 |
| ring to host doorbell (sum) | 8.9 | 9.85 | 228 |
| host doorbell to the next ring | 24.1 | 49689.07 | 335 |
| host doorbell to host event | 471512.3 | 133623.61 | 129 |
| host event to raise | 2.8 | 2.95 | 137 |
| raise to MSI | 10.3 | 305.65 | 111 |

rings 339 (38 after a blocking wait), host doorbells 249; host events: 4 before any doorbell, 8 < 1 ms, 65 < 100 ms, 56 older (medians over reps)
- rep 1: MSIs for raises 118; hybrid 200 us idle
- rep 2: MSIs for raises 112; hybrid 200 us idle
- rep 3: MSIs for raises 111; hybrid 200 us idle

guest: ms/launch 0.023 [0.02-0.025], dev_cpu_ms 530 [480-660]

