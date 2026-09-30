<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Windows and traps

Scope: the shared-memory windows that take guest accesses off the trapped vfio-user path, what still traps per CUDA operation and per compositor frame, and what one trap costs.

## Facts

### The windows

| Window | Env var | What it removes | What still traps | Evidence |
|---|---|---|---|---|
| Interrupt page | `CARAXES_INTR_PAGE=1` (needs `CARAXES_DOORBELL_POLL=spin\|hybrid`) | BAR0 page 0 (`NV_PMC`) and the VF CPU interrupt tree page 0xb81000 as sparse mmap areas of a memfd; 437.1 of 661.5 trapped accesses per Mutter frame (f3b) | the rest of BAR0, the doorbell sub-regions at 0xbb0090 included | docs/vfio/intr-page.md §1, §2, §7 |
| Display page | `CARAXES_DISPLAY_PAGE=1` (needs `CARAXES_INTR_PAGE=1` and `--display`) | the display FE interrupt status page 0x611000 (28 watched words, `LAST_DATA_RESET` and the other acks applied by the reactor) | core and window USERD, `RG_DPCA`, `FE_CORE_HEAD_STATE` | docs/vfio/intr-page.md §10 |
| Doorbell token pool | `CARAXES_DOORBELL_TOKENS=N` (default 64, at most 253) | the doorbell write at BAR0 0xbb0090: one datamatch ioeventfd per token, so a ring is one KVM exit ending in an eventfd, no VMM and no vfio-user message | each channel's first doorbell, nvidia-uvm's rings, any value outside the pool | docs/vfio/doorbell-token-pool.md Summary, (f) |
| Doorbell reactor | `CARAXES_DOORBELL_POLL=off\|spin\|hybrid` (`_IDLE_US` 200, `_CPU` auto) | no trap: the scheduler wake-up of the thread that reads the eventfds | unchanged | docs/vfio/doorbell-poll.md Design |
| BAR1 pool | `CARAXES_BAR1_PASSTHROUGH=pool` (`CARAXES_BAR1_POOL_MIB`, default 64) | guest BAR1 `[0, pool)` as a dma-buf sparse area: GPFIFO entries and USERD GPPut | guest BAR1 above the pool (Vulkan host-visible memory, console mapping, display imports) | docs/vfio/bar1-pool.md §1; docs/vfio/optimization-backlog.md §4 item 9 |
| BAR2 memfd | `CARAXES_BAR2_MEMFD=1` | region 3 (32 MiB) as a sealed memfd: the guest RM's page-table, instance-block and scrub writes, about 100000 trapped BAR2 writes per CUDA process | the flush page 0xfea000 (`CARAXES_BAR2_MEMFD_TRAP`), all of BAR0 (PRAMIN, MMU invalidate, BAR2 block, doorbells) | docs/vfio/bar2-memfd.md Design, §3; docs/vfio/optimization-backlog.md Summary 3 |
| MSI re-arm ioeventfd | `CARAXES_MSI_REARM_IOEVENTFD=1` (needs `CARAXES_DOORBELL_TOKENS=N`) | the `NV_XVE_CYA_2` write (BAR0 0x88704) once per interrupt, as an ioeventfd without datamatch read by the token pool's reactor | every other `NV_PCFG` write | docs/vfio/intr-page.md §4, §7 |

- The RM re-arms MSI by writing 0 to `NV_XVE_CYA_2` through `NV_PCFG`, and its comment says hardware only looks at the offset [615 tree: src/nvidia/src/kernel/gpu/bif/arch/maxwell/kernel_bif_gm107.c:150].
- The `NV_PMC_INTR_EN_SET/CLEAR` writes at every RM lock acquire and release are masked with `pmcRmOwnsIntrMask` [615 tree: src/nvidia/src/kernel/gpu/intr/arch/pascal/intr_gp100.c:62-66], which is 0 here, so the device only latches them [docs/vfio/intr-page.md §1].
- A work-submit token is `RUNLIST_ID` and `VECTOR` (ChID) [615 tree: src/nvidia/src/kernel/gpu/fifo/arch/turing/kernel_fifo_tu102.c:102-103], bits 22:16 and 11:0 [615 tree: src/common/inc/swref/published/turing/tu102/dev_ctrl.h:36-37], so pool tokens outside those fields never collide with an RM token [docs/vfio/doorbell-token-pool.md Summary 3].
- Page 0xbb0000 cannot become memory because the doorbell ioeventfd sits there, so the 11.8 `TIME_0/1` reads per Mutter frame stay trapped [docs/vfio/optimization-backlog.md §4 item 8].
- Prototype G (`CARAXES_BAR1_PASSTHROUGH=1`) is refused unless `CARAXES_BAR1_PASSTHROUGH_UNSAFE_LAYOUT=1`, because a guest BAR1 layout 2 MiB off the traced one mapped 128 area pages to FB pages the area did not back [docs/vfio/bar1-passthrough.md §9; run: caraxes/runs/cutover-615-2026-09-24/v2-610guest].
- The recommended configuration is memfd, pool, token pool, interrupt page and the re-arm switch [docs/vfio/README.md Status 2026-09-25].
- The hybrid reactor costs 3.1 % of a core against 96.0-97.6 % for spin with a short kernel equal to spin's (0.014 ms per launch in pass 2), and 5.1 % of a core with the interrupt page on [run: caraxes/runs/doorbell-poll-2026-09-24; docs/vfio/intr-page.md §9].
- How the pool, the memfd and the shadow keep guest memory consistent (syncs, remaps, the pool and memfd hang and its fix) is in [memory-model.md](memory-model.md); isolation and the Xid 31 of these series are in [isolation-and-xids.md](isolation-and-xids.md).

### Cost per trap

| Measurement | Value | Evidence |
|---|---|---|
| trapped BAR0 access answered by the model, bench-trap p50 | 23-27 us, device share 1-2 us | docs/vfio/mapping.md §6 |
| per-class medians in the census runs (busy host) | 28 to 35 us | docs/vfio/optimization-backlog.md §4 |
| trapped scratch register / doorbell value outside a 253-token pool (pure emulation, p50) | 25.5 us / 36.1 us | run: caraxes/runs/doorbell-tokens-2026-09-24 |
| pool token slot 0 / slot 252 (pure emulation, p50) | 4.1 us / 11.2 us | run: caraxes/runs/doorbell-tokens-2026-09-24 |
| GPU run, 64 tokens: value outside the pool / last slot 0xca70003f (write p50) | 36.8 us / 5.8 us | run: caraxes/runs/doorbell-tokens-gpu-2026-09-24/C-p21-tokens/trap.txt |
| KVM datamatch ioeventfd exit, slots 0-63 / bare exit to userspace | 1.24-1.43 us / 4.8 us; about 25 ns per datamatch entry scanned | docs/vfio/doorbell-token-pool.md (f) |
| BAR1 read, trapped against inside the P21 area | 47.9 us / 1.4 us | docs/vfio/README.md Status 2026-09-24 |
| MSI re-arm per interrupt | 30-50 us (p90 250-570 us under load) | docs/vfio/optimization-backlog.md §4 item 4 |
| BAR2 flush read carrying the memfd sync / memfd sync mean | about 200 us instead of 30 us / 126 us | docs/vfio/optimization-backlog.md §3.2 |
| eventfd ring to host doorbell write inside the device, p50 | 3.6-4.1 us | docs/vfio/doorbell-poll.md Measurement |
| profile (`CARAXES_PROFILE=1`) per trapped access / control / window hit / off | 84 ns (0.34 % of a 25 us trap) / 26 ns / 38 ns / 0.3 ns | docs/vfio/coverage-map.md §1.3 |

### What still traps per operation (census, vm3, 610 guest)

| Operation | `bare` | `off` | `on-nopool` | `on` (memfd + pool) | `on-rearm` |
|---|---|---|---|---|---|
| cuInit | 4235 | 3290 | 174 | 174.3 | 174.3 |
| cuCtxCreate | 87141 | 75070 | 1884 | 1228.3 | 1189.7 |
| cuCtxDestroy | 27718 | 21507 | 924 | 925.7 | 1067.0 |
| cuMemAlloc 1 MiB | 36.0 | 5.6 | 3.6 | 3.8 | 3.5 |
| cuMemAlloc 64 MiB | 266 | 188 | 8.0 | 8.2 | 7.8 |
| cuMemAllocHost 1 MiB | 905 | 807 | 8.2 | 8.2 | 7.5 |
| cuMemHostRegister 1 MiB | 1209 | 1049 | 15.2 | 15.4 | 15.0 |
| launch + sync, 1000 back to back, per launch | 4.5 | 1.0 | 3.6 | 0.0 | 0.0 |
| D2D 16 MiB | 6.9 | 1.4 | 3.2 | 0.7 | 10.3 |
| idle, per 100 ms | 247 | 35.9 | 66.4 | 38.6 | 25.6 |

[docs/vfio/optimization-backlog.md §2; run: caraxes/runs/trap-census-2026-09-25; run: caraxes/runs/opt-msi-ioeventfd-2026-09-25/summary.md; gross counts; `off` is interrupt page, pool 32 MiB, token pool 64, hybrid reactor]

- In `off` a submission, a copy, an event and a synchronise trap 0 times net of the background, and PPO submits 28000 times per second with no trap per submission [docs/vfio/optimization-backlog.md Summary 1].
- In `off` cuCtxCreate still traps 72736 BAR2 writes, 753 PCI config reads, 488 MMU invalidate, 391 MSI re-arm, 273 flush reads, 255 GSP falcon, 85 GSP queue, 60 usermode page and 20 first doorbells [docs/vfio/optimization-backlog.md §2.1].
- In `on-nopool` cuCtxCreate keeps 642 BAR1 writes, 488 invalidate accesses, 273 flush reads, 255 GSP falcon and 85 GSP queue accesses, 53 re-arm writes and 20 first doorbells [docs/vfio/optimization-backlog.md §2.1].
- One guest mapping change costs 24 trapped accesses: 18 RM-lock interrupt-enable writes, 5 for the MMU invalidate and 1 flush read [docs/vfio/alloc-path.md Summary].
- With interrupt page, memfd and lazy plans, cuMemAlloc 1 MiB traps 4.3 times and 64 MiB 13.9 times, against 45.5 and 370.6 without windows [docs/vfio/alloc-path.md §7; run: caraxes/runs/alloc-path-2026-09-24/night].
- PPO steady state traps about 16500 times per second in `bare` (BAR1 10350, doorbells 3450, interrupt pages 2310) and about 400 in `off` (re-arm 148, PCI config 0-234, invalidate 25) [docs/vfio/optimization-backlog.md §3.5].
- The re-arm ioeventfd takes re-arm traps from 141 to 0 per second in rlprobe and 153 to 0 in PPO, and all traps from 508 to 456 and 423 to 314 per second [run: caraxes/runs/opt-msi-ioeventfd-2026-09-25/workload-rates.md].
- With the full stack what still traps per second is the PCI config capability walk (200-290 reads), the MMU invalidate and flush page, and the GSP queue [docs/vfio/intr-page.md §4].
- Idle interrupt forwarding of other VMs' work ran at 331 MSIs per second in the coverage map, six times the census's 55 per second [docs/vfio/coverage-map.md §4 item 9; docs/vfio/optimization-backlog.md Summary 6].

### What still traps per frame (display)

| Configuration | Mutter traps / callbacks | Weston traps / callbacks | Evidence |
|---|---|---|---|
| no page (f3b, f4) | 662 / 41.5 Hz | 157 / 58.4 Hz | docs/vfio/display-e.md §6 |
| no page (run A) | 638.9 / 48.83 Hz | 155.4 / 59.71 Hz | run: caraxes/runs/intr-page-2026-09-24/A-page-off |
| interrupt page (B3) | 229.8 / 59.71 Hz | 103.0 / 59.78 Hz | run: caraxes/runs/intr-page-2026-09-24/B3-page-on |
| interrupt page (series B) | 230.24 / 59.72 Hz | 106.46 / 59.71 Hz | run: caraxes/runs/display-page-2026-09-24/B-intr-page |
| plus display page, 610 guest (series C) | 139.86 / 59.86 Hz | 31.08 / 57.41 Hz | run: caraxes/runs/display-page-2026-09-24/C-display-page |
| plus display page, 615 guest | 138.92 / 58.62 Hz | 29.25 / 59.79 Hz | docs/vfio/display-615.md §5 |

| Row per frame, 615 guest, both pages | Mutter | Weston |
|---|---:|---:|
| `NV_PCFG` config mirror 0x88704 (re-arm) | 10.21 | 8.89 |
| MMU invalidate writes / `PRIV_MMU_INVALIDATE` reads | 6.08 / 4.05 | 0 / 0 |
| 0x696000 display channel kick | 1.11 | 1.11 |
| VF `TIME_0/1` reads | 17.53 | 17.02 |
| BAR1 USERD GPPut writes | 3.00 | 2.00 |
| BAR2 writes / reads | 73.63 / 23.05 | 0 / 0 |

[docs/vfio/display-615.md §5; `CARAXES_INTR_PAGE=1 CARAXES_DISPLAY_PAGE=1`, token pool, hybrid reactor, no BAR2 memfd, re-arm trapped]

- Without the pages every MSI costs about 70 trapped accesses of interrupt-tree walk and PMC enable toggles, 15-19 ms of vCPU exits per Mutter frame at 23-27 us per trap [docs/vfio/display-e.md §6].
- A doorbell ioeventfd removes 3 of about 670 traps per Mutter frame and cannot move the frame rate [docs/vfio/display-e.md §6].
- 610 delta: the 610 guest makes about 2 more config-mirror writes per frame (Mutter 12.17 against 10.21) with more MSIs (6.61 against 4.66 per frame), and every other row is within two traps per frame [docs/vfio/display-615.md §5].

## Consequence for Leandro VFIO

- CUDA's steady state (submission, copy, event, synchronise) is trap-free with the recommended stack, so its remaining gap to native is device and host work, not exits (see [performance-facts.md](performance-facts.md)).
- The next windows in the backlog are the MMU invalidate and flush page as memory (item 3), progress-gated interrupt forwarding (item 5), a poll-mode GSP queue (item 6) and the PCI config walk (item 7) [docs/vfio/optimization-backlog.md §4].
- Mutter's largest remaining per-frame source is BAR2 page-table churn, which the BAR2 memfd targets with an expected saving of about 2 ms per frame [docs/vfio/optimization-backlog.md §4 item 1].
- Every window is opt-in and falls back to the trapped path when the client does not register it [docs/vfio/intr-page.md §4; docs/vfio/bar2-memfd.md Design 5].

## Open

- Mutter and Weston have not been measured with the BAR2 memfd and the re-arm ioeventfd (vm0 not borrowed); the expected re-arm saving is 9.6 traps per Mutter frame [docs/vfio/intr-page.md §4].
- The guest caller of the PCI config capability walk (offsets 0x06, 0x34, 0x60, 0x68, 0x78) is not identified [docs/vfio/optimization-backlog.md §3.3].
- SotTR has no trap table, so its BAR1 mappings outside the pool are unknown [docs/vfio/optimization-backlog.md §3.6].
- The traps Weston keeps with the display page are not yet named from `C-display-page/traps-weston/` [docs/vfio/intr-page.md §10.6].
- The interrupt page with a spin reactor instead of hybrid was not measured [docs/vfio/intr-page.md §9].
