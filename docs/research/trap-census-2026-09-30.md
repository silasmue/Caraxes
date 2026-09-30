<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Trap census and elimination plan (2026-09-30)

Scope: every guest access that still exits to Cloud Hypervisor (CH) and the device on today's
code (train-2026-09-30 c3e9bd5b, as train3 on Windows in C104, and the Windows lf24/final1 binaries), per workload on Linux
615 and Windows; what each costs; which fast paths exist and when they are on; the top ten
sources with the spec-true way to remove each. Census and plan only: the eliminations come
after the train merge. `R` = `/mnt/vmstore/caraxes/runs-2026-09-30/traps/`.

## Method

- Rows: the device profile (`CARAXES_PROFILE=1`, profile.rs): one row per trapped (region,
  direction, page, register) with the device time (message to answer, lock wait included)
  and the guest-seen gap (time since the previous trap ended, if under 2 ms).
- Windows: a workload is a guest RM client's lifetime in the exit dump (profile.rs
  `Lifetime`), or the window between two clients' opens. The KMD opens clients through
  the session, so every run has clients that span exactly one benchmark or stream
  [caraxes/tools/trap-census.py `--scope`, `--minus-scope`].
- Linux: the SotTR A/B arms' SIGUSR2 dumps around 25 s of benchmark rendering; the train A/B
  sessions; the per-frame records of 2026-09-25/28.
- Sources: registers grouped by what would remove them (trap-census.py `source`).
- vCPU share: modelled as rate × (RTT + device µs), RTT 25 µs (quiet host) for the records
  and the measured 38 µs for the L0 session; the guest-seen share is an upper bound (it also
  holds guest time between two traps under 2 ms apart).
- Host CPU: `/proc/<pid>/task/*/stat` of CH and the device, sampled every 10 s during the
  Windows agent's and codec agent's sessions (read-only) [R/procstat/, taskstat.py]. vCPU
  "host" time is utime + stime − guest_time: exits and KVM halt polling together.
- Instrumentation (branch census-2026-09-30, both lock-free, off unless the profile is on):
  - 71f4a99a: trapped doorbells by token (runlist, ChID) as window rows;
  - a7495f0e: trapped accesses the routes serve without the device mutex, per route
    (interrupt page, re-arm, display page, display status) with their device time. Before
    it the profile missed them (below).
- New session (Linux phase 2): one Linux B session on desktop-b-dmem (615, stock kernel,
  1 GiB at slot 4, 8 vCPUs), census binary a7495f0e = train c3e9bd5b plus the two counters,
  2026-09-30 08:08-08:16, beside a Windows VM and vm10 [R/L0-census/, census-linux.sh]:
  trap round trip (bench-trap, before the driver), idle desktop 60 s, glmark2 and vkmark 3 ×
  30 s at 1280x720, a Moonlight stream (vkcube, 60 s), a CUDA loop (`cuwork`, 40 s), each
  between SIGUSR2 dumps and /proc snapshots; 0 guest and 0 host Xid.
- Windows: no census session of my own (the permission system refused it); the numbers come
  from the Windows and codec agents' sessions (C96-C104, w3), train3 included (C104).

## Facts

### The fast paths and when they are on

| Fast path | Switch (default) | Removes | Mechanism | Linux B | vm2-615 | Windows |
|---|---|---|---|---|---|---|
| Interrupt page | `CARAXES_INTR_PAGE=1` (off) | BAR0 page 0 (PMC, `INTR_EN`), VF tree 0xb81000 | memfd area; guest stores to the tree applied by the reactor ("tagged") or, mapped read-only, trapped and applied at once ("exact") | on, tagged | on, tagged | on, exact: stores trap |
| Display FE page | `CARAXES_DISPLAY_PAGE=1` (off) | reads of 0x611000 | read-only memfd area, stores trap to a route | on | - | off |
| Display kick page | follows the display page | window/winimm `PUT` 0x690000-0x6d0000 | memfd area, reactor diffs `PUT` | on | - | off |
| BAR1 pool | `CARAXES_BAR1_PASSTHROUGH=pool` | guest BAR1 [0, 32 MiB) | dma-buf area | on | off | on |
| BAR2 memfd | `CARAXES_BAR2_MEMFD=1` (off) | BAR2 except the flush page 0xfea000 | memfd area, synced at the flush read and the invalidate | on | on | off |
| Token pool | `CARAXES_DOORBELL_TOKENS=64` | pool-token doorbells | datamatch ioeventfd | on | on | off |
| Guest tokens | `CARAXES_DOORBELL_GUEST_TOKENS` (auto) | `runlist << 16 \| chid` doorbells, ChID < 48 | datamatch ioeventfd, 253 fds per reply in all | 144 (runlists 0, 1, 7) | 144 | 240 (0, 1, 7, 2, 3) |
| MSI re-arm ioeventfd | `CARAXES_MSI_REARM_IOEVENTFD=1` (off) | `NV_XVE_CYA_2` 0x88704 | ioeventfd | on | on | off (routed trap) |
| Posted doorbell | `CARAXES_POSTED_DOORBELL` (on) | the lock wait only; still a trap | ring gate | on | on | on |
| Fast reads | `CARAXES_FAST_READS` (on) | the lock wait only: PTIMER, `TIME`, `TRIGGER`, flush page | atomics before the lock | on | on | on |
| Held invalidate | `CARAXES_SHADOW_INVALIDATE_ASYNC` (on) | the lock hold at `TRIGGER` | invgate.rs | on | on | on |

- Built, off, not GPU-validated for games: the MMU invalidate page (`CARAXES_INVALIDATE_PAGE`,
  inv_page.rs), the usermode page as read-write memory (`CARAXES_USERMODE_PAGE`, um_page.rs;
  it gives up the doorbell ioeventfds), the flush page as memory
  (`CARAXES_BAR2_FLUSH_PAGE=memory`), the GSP queue-head ioeventfd (`CARAXES_RPC_IOEVENTFD`)
  [intr-page.md §11.3, §12.2; gsp-transport-615.md §11.2; optimization-backlog.md item 3].
- No MSI-X anywhere: every board has `msix=false`; MSI and INTx only [board.rs:227-230].
- Settings: Linux B = desktop-b-dmem.sh; vm2-615 = train/ab/A1/setup.txt; Windows =
  windows-plain/C101-final1/env.json plus run-ch-gpu.sh's read-only areas.
- **Profile gap (fixed in a7495f0e):** accesses served by `Routes` (device.rs) returned before
  `profile::access`. Windows C101: 1392638 routed traps (the interrupt page's own count)
  beside 4.32 M access rows; C100: 1700385 beside 3.88 M. Linux B on train routes its display
  page stores the same way [windows-plain/C10{0,1}*/gpu.log "trapped access(es) served
  without the device mutex"].

### Cost per trap

| Access | Cost | Evidence |
|---|---|---|
| trapped access, no device work (KVM exit, CH, vfio-user round trip) | 23-27 µs p50 on a quiet host; 35.5 µs p50 (p99 93) read, 40.0 µs (p99 116) write on train beside two VMs | mapping.md §6; R/L0-census/rtt.txt |
| trapped read answered before the device lock (PTIMER, usermode `TIME`) | 38.3 / 38.6 µs p50: the lock is not the cost, the exit and the socket are | R/L0-census/rtt.txt |
| back-to-back trapped BAR2 writes, Windows under CH | 27-32 µs guest-seen | R/phase1/win-C8{1,3}.md, win-C96.md |
| datamatch ioeventfd: MMIO address / inside a read-only memslot | 1.28 / 1.91 µs | R/kvm-ro-ioeventfd-probe.txt |
| write into a read-only memslot, no match (userspace exit, empty handler) | 6.5 µs | same |
| read of a mapped page | ~1 ns | same |
| device time behind a trap | invalidate `TRIGGER` write 150-890 µs; GSP queue head 67-1520 µs; Linux FE ack 280-1070 µs; L2 flush write 750-835 µs; flush-page read 50-145 µs; Windows window `PUT` 48-66 µs; `TIME`/PTIMER 0.3-68 µs; BAR2 write 2-5 µs | R/phase1/*.md |

- One vfio-user socket carries every vCPU's traps, so long device work delays unrelated
  reads: guest-seen `TIME` 39-94 µs (Linux SotTR) and 107-160 µs (Windows 3D) for 0.3-68 µs of
  device time.
- The probe settles the usermode-page question of windows-traps-exit-2026-09-29.md: KVM
  emulates a write into a `KVM_MEM_READONLY` slot as MMIO, so a datamatch ioeventfd fires
  there without a userspace exit, and other values still exit (6.5 µs) [R/kvm-ro-ioeventfd-probe.txt].

### Per workload

| Workload (record) | traps/s | modelled vCPU % (seen) | top sources, /s |
|---|---:|---:|---|
| Linux idle GNOME desktop, 60 s (L0, train) | 421 | 1.6 (1.1) | `TIME` 360 (120 hi-lo-hi triples), display acks 60 (routed, 1.2 µs) |
| Linux glmark2, score 4410, 91 s (L0) | 3206 | 13.9 (19.6) | flush read 1407, `TIME` 907, invalidate 704 (141 × 5), display acks 179 |
| Linux vkmark, score 3922, 91 s (L0) | 3061 | 14.2 (19.9) | flush read 1385, `TIME` 852, invalidate 626, display acks 179 |
| Linux Moonlight stream, 60 fps (vkcube + NVENC), 60 s (L0) | 4300 | 23.2 (33.0) | flush read 1620, invalidate 1323, `TIME` 1177, display acks 180 |
| Linux CUDA loop (`cuwork`), 40 s (L0) | 474 | 2.2 (1.7) | the idle desktop's `TIME` 361 and acks 60; invalidate 22, GSP 5 RPC; submissions 6087 rings/s all on the token pool |
| Linux SotTR benchmark, 25 s, arms A/D/F (sottr-ab) | 1670-2788 | 19-81 (25-58) | invalidate 699-1214, `TIME` 532-588, flush read 240-628, doorbell 84-120, FE acks 58-62, GSP 5-36 RPC |
| Linux SotTR stall window, 20 s (sottr-stall) | 4878 | 96 (59) | invalidate 3120, flush read 1368, `TIME` 334, FE acks 56 |
| Linux vkmark, 2026-09-28 record (older binary) | ~3000 | - | per 60 Hz period 22.5 flush reads, 13.8 `TIME`, 2 invalidates (10), 3 FE writes, 1.1 kicks [vkmark-frame-2026-09-28.md] |
| Linux Mutter desktop, 2026-09-25 record | 2400-3000 | - | 40-50 per frame: 23 flush reads, `TIME` 925/s fixed, 10 invalidate accesses, 1.3 kicks [optimization-backlog.md item 3] |
| Linux vm2-615 boot + CUDA context bench (train/ab A1) | 1507 | 8 (9) | invalidate 404, BAR1 288 (no pool), PROM 198, flush read 185, GSP 40 RPC |
| Windows Heaven 4.0, 133 fps, 292 s (C100 client 0xc1d000a4) | 3290 + ~5330 routed | 9.9 (46) + ~14 | interrupt tree stores ~5330, BAR2 writes 1228, PTIMER 896, doorbell 497, `TIME` 295, window `PUT` 138, ARMED reads 90 |
| Windows Superposition 1080p medium, 80.8 fps, 208 s (C101 0xc1d000d7) | 2373 + ~4570 routed | 7.5 (27) + ~12 | tree stores ~4570, PTIMER 902, BAR2 525, `TIME` 306, doorbell 291, window `PUT` 138, ARMED 91 |
| Windows Superposition on train3, 161 s (C104 0xc1d000d7) | 2577 + ~4780 routed | 8.1 (28) + ~13 | tree stores ~4780, PTIMER 882, BAR2 719, doorbell 326, `TIME` 301, window `PUT` 135, ARMED 88 |
| Windows Sunshine NVENC stream, 21 fps, 106 s (w3 0xc1d00145) | 478 + ~2380 routed | 1.6 (7.6) + ~6 | tree stores ~2380, doorbell 170, GSP 45 RPC (+133 falcon), BAR2 83 |
| Windows Sunshine x264 stream, 116 s (w3 0xc1d0019a) | 2217 + ~750 routed | 6.4 (6.7) + ~2 | BAR2 writes 2133, tree stores ~750, invalidate 26 |
| Windows without a desktop session, 677 s (C96 0xc1d0002f) | 720 + ~650 routed | 1.8 (5.1) + ~2 | PTIMER 360, `TIME` 360 (a 120 Hz hi-lo-hi pair), tree stores ~650 |
| Windows desktop start and the session's first ssh steps, 47 s (C100 0xc1d00044 − 0xc1d00089) | 21380 | 65 (74) | BAR2 writes 20567, invalidate 271, GSP 49 RPC |
| Windows boot (C93-C96, w1, same counts) | ~1 M per boot | ~27 s of vCPU | BAR2 0.83-1.0 M, PROM 80948, SEC2 25259, config 19075, PRAMIN 17535, ROM 15154 |

- Routed tree stores per window are the interrupt page's applied stores (acks, `LEAF_EN_SET`,
  `LEAF_EN_CLEAR`, `TOP_EN_SET`, `TOP_EN_CLEAR` window rows) plus the re-arms at the
  session's re-arms/acks ratio (1.11-1.41); in C101 stores plus re-arms equal the routed
  count exactly (1202028 + 190610 = 1392638) [R/phase1, window rows of each client scope].
- Windows takes about 9 tree stores per interrupt: `LEAF_EN_SET` 2.3, `LEAF_EN_CLEAR` 2.25,
  `TOP_EN_SET`/`CLEAR` 1.1 each, 1 ack, 1.3 re-arms (Heaven: 588 acks/s, 710 vector raises/s).
  Linux's tagged page takes the same stores as memory: 0 traps.
- Windows per presented frame in Heaven (133 fps): PTIMER 6.7, doorbell 3.7, `TIME` 2.2,
  BAR2 9.2, window `PUT` 1.04, ARMED 0.68, window-immediate `PUT` 0.34, scanline 0.34, tree ~40.
- Windows clock: PTIMER and usermode `TIME` come as hi-lo-hi triples, one of each per
  timestamp; without a session exactly 120 pairs per second [R/phase1/win-bg-C96.md].
- Windows doorbells: C91 trapped 172949 rings against 153774 guest-token rings. The auto set
  covers ChIDs < 48 on runlists 0, 1, 7, 2, 3 (253 fds per reply); the misses are higher ChIDs
  (UVMLite 34-51 [device-ce-2026-09-29.md]) or other runlists [C91 gpu.log; tokens.rs:290-395].
- Host CPU, Windows Superposition (161 s): vCPU threads 246 % of a core, 38 % of it host
  side (exits and halt polling); device: token reactor 15.4 %, socket thread 14.6 %,
  shadow-userd 6.6 %. NVENC stream: vCPU host 23.8 % against guest 31.4 %; socket thread 10.6 %
  [R/procstat/C101-final1, R/W-w3-procstat].
- Linux on train (L0, modelled with the measured 38 µs): the device time behind a trap is
  small now (flush read 0.4-0.8 µs, invalidate 22-50 µs, 267 µs per GSP RPC) against
  74-252 µs and 150-890 µs in the 2026-09-29 SotTR arms; what remains is the exit itself
  (guest-seen 68-88 µs per flush read or invalidate access in the stream) [R/L0-census/L-*.md].
- Linux fast paths on train (per second): glmark2 token-pool rings 8674 (8914 guest writes),
  re-arm eventfd 6306, interrupt-page scans 14054, BAR2 memfd syncs 9081; vkmark 8199 rings
  (19528 writes); trapped doorbells ~0: the new token rows show only runlists 2 and 3, ChIDs
  34-54 (copy runlists outside the guest set 0, 1, 7) [R/L0-census/L-vkmark.md, L-cuda.md].
- Host CPU on train, Linux B: glmark2 vCPU threads 344 % of a core, 237 % of it host side
  (exits, ioeventfds, halt polling: not separable without root); token reactor 77 % (hybrid);
  device socket thread 5 %. Idle desktop: host side 23 %, reactor 23 %. CUDA loop: reactor
  83 % [R/L0-census/L-*-cpu.txt].
- Linux fast paths at work in SotTR (per second): token-pool rings 207-789 (636-1323 guest
  writes), re-arm eventfd 364-540, interrupt-page scans 1212-2284 [R/phase1/linux-sottr-*.md].
- Writes: Linux SotTR 31-40 % of the traps, most of the device time; Windows Heaven 1919 of
  3290 profiled traps/s plus all ~5330 tree stores.

## Consequence for Leandro VFIO

### The ranked table

| # | Source | Linux, /s | Windows, /s | Cost | vCPU share (modelled) |
|---|---|---|---|---|---|
| 1 | Interrupt tree stores (exact mode) | 0 | Heaven ~5330, Superposition ~4570, NVENC stream ~2380, idle ~650 | ~27 µs, device few µs | Windows 3D 12-14 % |
| 2 | MMU invalidate (5 accesses each) | stream 1323, glmark2 704, vkmark 626; SotTR 699-1214 (stall 3120); CUDA bench 404 | 3D 5-14, desktop start 271 | 35-40 µs each + sync at `TRIGGER`: 22-50 µs on train, 150-890 µs on the 09-29 SotTR binaries | stream 11.7 %, glmark2 4.2 %; SotTR 12-34 % |
| 3 | BAR2 writes (Windows, no memfd) | 0 | Heaven 1228, Superposition 525, x264 stream 2133, desktop start 20567, ~0.9 M per boot | 27-32 µs | Windows 3D 1.5-3.6 %, desktop start 58 % |
| 4 | Clock: PTIMER + usermode `TIME` | `TIME` idle 360, glmark2 907, vkmark 852, stream 1177, SotTR 532-603 | PTIMER 896-902 + `TIME` 295-306 in 3D; 360 + 360 idle | 35-40 µs, seen 40-160 µs | Linux 1.4-4.5 %, Windows 3D ~3 % |
| 5 | BAR2 flush-page read (Linux memfd sync point) | stream 1620, glmark2 1407, vkmark 1385; SotTR 240-628 (stall 1368); CUDA bench 185 | 2-64 | 35-40 µs; sync 0.4-0.8 µs on train (deferred), 50-145 µs on 09-29 | 5.3-6.3 % (seen 9.5-13.4 %); SotTR 2-8 % |
| 6 | Trapped doorbells | ~0 on train (copy runlists 2, 3); SotTR 84-120 on 09-29 binaries | Heaven 497, Superposition 291, NVENC stream 170 | 25-30 µs (posted: device 2-3 µs) | Windows 3D ~1.4 % |
| 7 | Windows display USERD: `PUT`, ARMED, scanline, GET, cursor | kick page on | 3D ~330 (`PUT` 138, ARMED 90, winimm 45, scanline 45, GET 11-14); cursor 4 per move | `PUT` 48-66 µs device | Windows 3D ~1.7 % |
| 8 | GSP RPC (queue head + 3 falcon accesses) | 5-40 RPC/s | NVENC stream 45 RPC/s, desktop start 49, 3D 1-3 | head 67-1520 µs device | Linux SotTR 0.3-5.5 % |
| 9 | Linux display FE acks (read-only display page stores, routed) | idle 60, benchmarks and stream 179-180 (3 per frame) | - | 1.2-2.3 µs device on train (280-1070 µs on 09-29) plus the exit | 0.7 % |
| 10 | Boot and recovery: PROM window, SEC2, PRAMIN, config, ROM | ~75k per boot | ~160k per boot (+ BAR2) | ~25-30 µs | ~2 s (Linux), ~5 s + BAR2 ~25 s (Windows) per boot, FLR or TDR recovery |

- Next after ten: the L2 flush and sysmem invalidate pair (0x70010, 0xb80f00; Linux SotTR 19
  pairs/s, 750-835 µs per write, about 3 % of a vCPU).

### The elimination plan: how the card avoids each round trip

| # | Spec-true elimination | Gain | Linux/Windows | Risk | Effort | Built? |
|---|---|---|---|---|---|---|
| 1 | The tree page as the VF's own memory: Windows on the tagged (read-write) page Linux uses; on the card these are posted MMIO stores to VF-local registers | −4.6-5.3 k traps/s in Windows 3D, ~12-14 % of a vCPU; −2.4 k/s in a stream | Windows | high: the tagged page gave TDRs at the first D3D11 device under QEMU (P37 arm 2), cause not found; exact mode was the fix | A/B under CH, then the TDR's cause | yes (Linux default) |
| 2 | BAR2 as memory: the Windows BAR2 memfd (BAR2 is a plain aperture on the card) | −0.5-2.1 k/s in 3D and streams, −20 k/s at desktop start, −0.9 M traps (~25 s of vCPU) per boot | Windows | medium: C80's 54 Xid 31 (PTEs the shadow walk missed); 1c31cac2 syncs before every re-plan; the Windows agent's A/B | A/B | yes |
| 3 | The clock as read-only memory: the usermode page and PTIMER page 0x9000 mapped read-only (the CH fork's read-only areas), `TIME` and PTIMER published as one aligned qword each from one clock; the doorbell at 0xbb0090 keeps its ioeventfds (fire inside a read-only slot, 1.91 µs); other stores trap as today | Linux −530-925/s (1.5-5 %); Windows −1.2 k/s in 3D, −720/s idle (~3 %); vkmark's vblank ISR loses 6 traps | both | medium: freshness (um_page publishes at most every µs while the reactor runs, stands still up to the hybrid reactor's 1 ms sleep); Windows readers of the pair unknown | moderate: a read-only mode for um_page.rs, a PTIMER page model with every readable register | half: um_page.rs (read-write), CH read-only areas |
| 4 | Posted MMIO writes in CH's vfio-user client (vfio-user `No_reply`; QEMU 11.1's client posts by default, `x-no-posted-writes=off`), except stores into read-only mapped areas, whose later reads bypass the socket | a trapped write 25-30 → ~6 µs; the device time behind GSP heads, L2 flushes, display `PUT`s and SEC2/PRAMIN boot writes leaves the vCPU: Linux SotTR ~4-10 % of a vCPU; Windows ~4 % in 3D, ~45 % at desktop start while BAR2 traps. Not the Windows tree stores or the Linux FE acks (read-only mapped pages) | both | low-medium: ordering holds on the one socket (a later trapped read waits), not for reads of mapped pages (hence the exception); errors of posted writes go unreported, as on PCIe | small CH patch; the vendored server already honours `No_reply` (vendor/vfio-user-rs/src/lib.rs:2046) | no |
| 5 | The invalidate page (the VF's `PRIV_MMU_INVALIDATE` words as memory, `TRIGGER` is the card's own busy bit), then a cheaper sync behind `TRIGGER` | Linux SotTR −700-1200 traps/s, ~125 µs of exits per invalidate (1.7-3 %); the 150-890 µs sync needs its own work (page compares, dirty tracking) | Linux | medium: completion waits for the reactor (up to 1 ms asleep); the untrapped-GPPut barrier sits at the invalidate | validation, then the sync | yes |
| 6 | Doorbells: every guest token an ioeventfd (the card never traps a doorbell). Per-ChID eventfds shared by all runlists of that ChID, or more ChIDs on the hot runlists; the new token rows name the misses | Windows −170-500/s (~1.4 %) | Windows mainly | low | small | half: guest tokens (240) |
| 7 | GSP queue head as a doorbell (posted, ioeventfd), and its falcon status words served from memory | −4 traps per RPC: Linux 5-40 RPC/s (SotTR up to 5.5 %), Windows streams 45 RPC/s | both | low (fail-closed, tested) | A/B | yes: `CARAXES_RPC_IOEVENTFD` |
| 8 | Windows display: the kick page for windows and window-immediates, the ARMED half as read-only memory published at latch; scanline stays a trap (a counter cannot be memory without a clock) | −270/s in 3D (~1.4 %), less flip latency (`PUT` 48-66 µs) | Windows (Linux has the kick page) | medium: TDRs if kicks lag | moderate | half: kick page |
| 9 | The flush page as memory once the sync is cheap (a flush read orders posted writes on the card; it does no work) | Linux −240-1370/s (2-8 %) | Linux | medium: every doorbell then pays a sync (P1 run: ring to host doorbell 10 → 56 µs) | depends on 5 | yes: `CARAXES_BAR2_FLUSH_PAGE=memory` |
| 10 | Boot: the PROM window as read-only memory holding the VBIOS image; SEC2/PRAMIN loads posted (4) | Windows ~2-5 s and Linux ~1-2 s per boot and recovery | both | low | small | no |

- Order (gain per effort, shared first): 4 (small, both, every write) → 2 (Windows, an A/B
  away) → 1 (Windows' largest, needs the TDR hunt) → 3 (both) → 6, 7 (small) → 5, 9
  (Linux games) → 8 → 10. Last, for what remains: a faster round trip (busy-polling the
  vfio-user socket on both ends, vkmark-frame fix 5: 25-30 → ≤ 10 µs per trap).
- Expected, Windows Heaven: ~8600 → ~250 traps/s with 1, 2, 3, 6, 8 (left: scanline, GET,
  invalidates, GSP); modelled vCPU ~24 % → ~1 %.
- Expected, Linux desktop and 3D on train: glmark2 3206 → ~190 traps/s and the stream 4300 →
  ~190 with 3, 5, 9 (left: the display acks, GSP); modelled vCPU 14-23 % → ~1 %.
- Expected, Linux SotTR: 1700-2800 → ~200-400 traps/s with 3, 5, 7, 9 (left: GSP falcon
  reads, doorbells outside the tokens, L2 flushes).

## Open

- SotTR on train is unmeasured: its arms ran 2026-09-29 binaries (their doorbell and `TRIGGER`
  device times are upper bounds for train). The L0 round trip was taken beside two VMs; a
  quiet-host repeat would show whether 23-27 µs still holds.
- Windows idle desktop with DWM is unmeasured, and census-windows.sh has not run (refused by
  the permission system). The per-token doorbell rows and routed-trap rows have not run on a
  guest yet: which doorbell tokens miss the guest set is still inferred.
- The Windows re-arm count per window is estimated from the session ratio.
- Who reads Windows' clock pair, and how fresh it must be.
