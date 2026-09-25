### rl

| count (whole process window) | ../opt-memfd-pool-2026-09-25/on (16.2 s) | on-rearm (11.9 s) |
|---|---|---|
| submissions (trapped doorbells + token-pool eventfd rings) | 80228 (4958/s) | 85148 (7153/s) |
| MSIs | 2453 (152/s) | 1986 (167/s) |
| BAR1 writes | 0 (0/s) | 0 (0/s) |
| intr pages (0x0, 0xb81000) | 0 (0/s) | 0 (0/s) |
| MSI re-arm NV_PCFG 0x88704 | 2274 (141/s) | 0 (0/s) |
| PCI config reads | 3591 (222/s) | 3078 (259/s) |
| MMU invalidate | 960 (59/s) | 960 (81/s) |
| BAR2 writes | 0 (0/s) | 0 (0/s) |
| BAR2 flush reads | 525 (32/s) | 525 (44/s) |
| GSP queue + falcon | 776 (48/s) | 776 (65/s) |
| other | 69 (4/s) | 69 (6/s) |
| all trapped accesses | 8215 (508/s) | 5428 (456/s) |
| MSI re-arm through the eventfd (writes) | 0 (0/s) | 1963 (165/s) |

### ppo

| count (whole process window) | ../opt-memfd-pool-2026-09-25/on (35.5 s) | on-rearm (33.8 s) |
|---|---|---|
| submissions (trapped doorbells + token-pool eventfd rings) | 770682 (21693/s) | 1102429 (32585/s) |
| MSIs | 5479 (154/s) | 5233 (155/s) |
| BAR1 writes | 0 (0/s) | 0 (0/s) |
| intr pages (0x0, 0xb81000) | 0 (0/s) | 0 (0/s) |
| MSI re-arm NV_PCFG 0x88704 | 5441 (153/s) | 0 (0/s) |
| PCI config reads | 7182 (202/s) | 8208 (243/s) |
| MMU invalidate | 1005 (28/s) | 1005 (30/s) |
| BAR2 writes | 0 (0/s) | 0 (0/s) |
| BAR2 flush reads | 540 (15/s) | 540 (16/s) |
| GSP queue + falcon | 776 (22/s) | 776 (23/s) |
| other | 69 (2/s) | 69 (2/s) |
| all trapped accesses | 15033 (423/s) | 10618 (314/s) |
| MSI re-arm through the eventfd (writes) | 0 (0/s) | 5226 (154/s) |

### short-auto

| count (whole process window) | ../opt-memfd-pool-2026-09-25/on (6.8 s) | on-rearm (3.6 s) |
|---|---|---|
| submissions (trapped doorbells + token-pool eventfd rings) | 190 (28/s) | 268 (75/s) |
| MSIs | 1272 (187/s) | 1097 (308/s) |
| BAR1 writes | 0 (0/s) | 0 (0/s) |
| intr pages (0x0, 0xb81000) | 0 (0/s) | 0 (0/s) |
| MSI re-arm NV_PCFG 0x88704 | 326 (48/s) | 0 (0/s) |
| PCI config reads | 1539 (226/s) | 1026 (288/s) |
| MMU invalidate | 830 (122/s) | 830 (233/s) |
| BAR2 writes | 0 (0/s) | 0 (0/s) |
| BAR2 flush reads | 477 (70/s) | 477 (134/s) |
| GSP queue + falcon | 748 (110/s) | 748 (210/s) |
| other | 69 (10/s) | 69 (19/s) |
| all trapped accesses | 4009 (589/s) | 3170 (890/s) |
| MSI re-arm through the eventfd (writes) | 0 (0/s) | 238 (67/s) |

