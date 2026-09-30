<!-- SPDX-License-Identifier: MIT -->
# Traps and fast paths

A trap is a guest access to a BAR offset that the device offers neither as memory nor as an
ioeventfd. It costs the vCPU 23 to 40 us, almost all of it the exit and the vfio-user round
trip. The fast paths turn hot registers into memory or eventfds and keep the card's semantics
by syncing at points the driver itself orders. Numbers without another citation are from
[research: trap-census-2026-09-30](../research/trap-census-2026-09-30.md); switches and
defaults are those of 2026-09-30 ([device-configuration](device-configuration.md),
[run: flags-2026-09-30](../../caraxes/runs/flags-2026-09-30/NOTE.md)).

## The path of one trap

1. The guest loads or stores at a BAR address without a KVM memory slot; the vCPU exits to KVM.
2. Cloud Hypervisor takes the MMIO exit and sends one `VFIO_USER_REGION_READ` or `_WRITE`
   over the device's socket; it waits for the reply, for writes too.
3. The device answers (`crates/vfio-user-nvgpu/src/device.rs` `SharedDevice::region_read`,
   `region_write`, the models in `bar0.rs` and `fb.rs`), and the vCPU runs on.

- One socket carries every vCPU's traps: a `TIME` read with 0.3 to 68 us of device time is
  seen by the guest as 39 to 94 us (Linux SotTR) and 107 to 160 us (Windows 3D).
- Around the socket: a sparse mmap area (a KVM memory slot), a datamatch ioeventfd (KVM
  signals an eventfd inside the exit), a read-only slot (reads are memory, writes exit, a
  datamatch ioeventfd still fires). Accesses served before the device lock (`device.rs`
  `Routes`, `fastread.rs`) still pay the exit. The device profile counts every trap
  ([verification-methods](verification-methods.md)).

## Cost

| Access | Cost, p50 |
|---|---|
| trapped access without device work, quiet host | 23 to 27 us ([vfio/mapping.md](../vfio/mapping.md) §6) |
| trapped read / write beside two VMs (2026-09-30) | 35.5 us / 40.0 us |
| trapped read answered before the device lock (PTIMER, `TIME`) | 38.3 to 38.6 us |
| back-to-back trapped BAR2 writes, Windows | 27 to 32 us guest-seen |
| datamatch ioeventfd at an MMIO address / inside a read-only slot | 1.28 / 1.91 us |
| write into a read-only slot without a match | 6.5 us |
| read of a mapped page | about 1 ns |

- Device time behind a trap, Linux B on 2026-09-30: flush read 0.4 to 0.8 us, invalidate 22
  to 50 us, 267 us per GSP RPC, display ack 1.2 to 2.3 us; on 2026-09-29: L2 flush write 750
  to 835 us, Windows window `PUT` 48 to 66 us, BAR2 write 2 to 5 us.

## The fast paths

| Fast path | Switch | Removes | Where it syncs |
|---|---|---|---|
| Interrupt page, tagged | default with `--host-exec` | BAR0 page 0 and the tree page 0xb81000 | the reactor scans the page and applies stores |
| Interrupt page, exact | tree page read-only (Windows launcher) | reads of the tree | each store traps, applied at once off the device lock |
| Display page | `CARAXES_DISPLAY_PAGE=1`, opt-in | reads of 0x611000 | read-only on Linux B: acks trap to a route off the lock |
| Display kick page | on with the display page | window `PUT` stores, 0x690000 to 0x6CFFFF | the reactor diffs `PUT` and runs the kick |
| BAR1 pool | default with `--host-exec`, 32 MiB (`CARAXES_BAR1_POOL_MIB`, 0: none) | guest BAR1 `[0, pool)` | table changes at trapped flushes and invalidates; replicas at engine starts |
| BAR2 memfd | `CARAXES_BAR2_MEMFD=1`, opt-in | BAR2 except the flush page 0xfea000 | page compares at engine starts, invalidates (tables), doorbells (the channel's pages) |
| Doorbell token pool | `CARAXES_DOORBELL_TOKENS=N`, opt-in, at most 253 | doorbells with a pool token | a datamatch ioeventfd per token; a channel's first doorbell traps |
| Guest tokens | automatic with `--boot-model` | `runlist << 16 \| chid` doorbells, ChID below 48 | datamatch ioeventfds fixed at realize |
| MSI re-arm ioeventfd | `CARAXES_MSI_REARM_IOEVENTFD=1`, opt-in | the `NV_XVE_CYA_2` write at 0x88704 | read by the token reactor |
| Posted doorbell | always on | the lock wait of a trapped doorbell | every later access rings the queue first |
| Fast reads | always on | the lock wait of `TRIGGER`, flush-page, PTIMER, `TIME` reads | state the lock holders publish |
| Held invalidate | always on | the lock hold at the `TRIGGER` write | `TRIGGER` reads busy until the worker applied the root's plans |

- The reactor reads the eventfds and scans the pages. `CARAXES_DOORBELL_POLL` is `hybrid` by
  default with `--host-exec` (spins 200 us after an event, then blocks, at most 1 ms with the
  interrupt page), `spin`, or `off` (refused with the interrupt page). On Linux B it took 23 %
  of a core on an idle desktop, 77 % under glmark2, 83 % in a CUDA loop.
- The re-arm ioeventfd needs the token pool or the guest tokens. One reply carries at most 253
  eventfds: Linux B has 64 pool and 144 guest tokens (runlists 0, 1, 7), Windows 240 guest
  tokens (0, 1, 7, 2, 3). A value outside the set traps and is posted.
- Built, opt-in, not GPU-validated: `CARAXES_RPC_IOEVENTFD=1` (the GSP queue head),
  `CARAXES_INVALIDATE_PAGE=1`, `CARAXES_USERMODE_PAGE=1` (gives up the doorbell ioeventfds),
  `CARAXES_BAR2_FLUSH_PAGE=memory`.
- In force: Linux B 615 runs every path but exact mode; nouveau the same without the display
  pages; Windows the exact page, the pool and the guest tokens; nova runs pure emulation,
  which keeps the trapped paths. Mechanisms: [interrupts](interrupts.md),
  [bar1-vram-window](bar1-vram-window.md), [bar2-page-tables-and-userd](bar2-page-tables-and-userd.md).

## Traps per second

| Linux B 615, 2026-09-30 binary | traps/s | vCPU % | top sources, /s |
|---|---:|---:|---|
| idle GNOME desktop | 421 | 1.6 | `TIME` 360, display acks 60 |
| glmark2, score 4410 | 3206 | 13.9 | flush read 1407, `TIME` 907, invalidate 704 |
| vkmark, score 3922 | 3061 | 14.2 | flush read 1385, `TIME` 852, invalidate 626 |
| Moonlight stream, vkcube and NVENC | 4300 | 23.2 | flush read 1620, invalidate 1323, `TIME` 1177 |
| CUDA loop | 474 | 2.2 | the desktop's; 6087 rings/s, all on the token pool |
| SotTR benchmark, 2026-09-29 binaries | 1670 to 2788 | 19 to 81 | invalidate 699 to 1214, `TIME` 532 to 588 |

| Windows 617.14 | traps/s, profiled + routed | vCPU % | top sources, /s |
|---|---:|---:|---|
| Heaven 4.0, 133 fps | 3290 + 5330 | 9.9 + 14 | tree stores 5330, BAR2 writes 1228, PTIMER 896, doorbell 497 |
| Superposition 1080p Medium | 2577 + 4780 | 8.1 + 13 | tree stores 4780, PTIMER 882, BAR2 719, doorbell 326 |
| Sunshine stream, NVENC / x264 | 478 + 2380 / 2217 + 750 | 1.6 + 6 / 6.4 + 2 | tree stores; x264: BAR2 writes 2133 |
| no desktop session | 720 + 650 | 1.8 + 2 | PTIMER 360, `TIME` 360 |
| desktop start, 47 s | 21380 | 65 | BAR2 writes 20567 |
| one boot | about 1 M | about 27 s | BAR2 0.83 to 1.0 M, PROM 80948, SEC2 25259 |

- vCPU share is modelled as rate x (round trip + device time); routed counts are estimates.
  Windows makes about 9 tree stores per interrupt and reads the clock as hi-lo-hi triples.
- Without the interrupt page an interrupt costs about 70 trapped accesses; Mutter went from 662
  to 230 trapped accesses per frame with it, 139 with the display page; the PCI capability walk
  reads up to 290 times per second ([research: windows-and-traps](../research/windows-and-traps.md)).

## Ranked sources and the plan

| # | Source | Linux, /s | Windows, /s | Spec-true removal | Built |
|---|---|---|---|---|---|
| 1 | tree stores, exact mode | 0 | 650 to 5330 | the tagged page on Windows | Linux only; TDRs under QEMU, cause open |
| 2 | MMU invalidate, 5 accesses | 404 to 1323; 3120 in a SotTR stall | 5 to 271 | the invalidate page, then a cheaper sync | opt-in |
| 3 | BAR2 writes, no memfd | 0 | 83 to 2133; 20567 at desktop start | the BAR2 memfd | Windows A/B open |
| 4 | PTIMER and `TIME` | 360 to 1177 | 720 to 1200 | the usermode and PTIMER (0x9000) pages read-only | half |
| 5 | BAR2 flush read | 185 to 1620 | 2 to 64 | the flush page as memory, after a cheap sync | opt-in |
| 6 | trapped doorbells | about 0 | 170 to 497 | every guest token an ioeventfd | 240 tokens |
| 7 | Windows display USERD | kick page | about 330 | the kick page, ARMED as memory | half |
| 8 | GSP RPC, 4 traps each | 5 to 40 RPC | 45 RPC in a stream | the queue head as an ioeventfd | opt-in |
| 9 | Linux display acks | 60 to 180 | - | - | routed |
| 10 | boot: PROM, SEC2, PRAMIN, config | about 75 k per boot | about 160 k per boot | the PROM window read-only, posted loads | no |

- Order, gain per effort: posted MMIO writes in Cloud Hypervisor's vfio-user client (vfio-user
  `No_reply`; a trapped write 25 to 30 us to about 6 us), then 3, 1, 4, 6 and 8, 2 and 5, 7,
  10; last, busy-polling the socket on both ends. Expected: Windows Heaven about 8600 to 250
  traps/s, Linux glmark2 and the stream to about 190, Linux SotTR to 200 to 400.
