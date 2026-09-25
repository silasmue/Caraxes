<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Trap census and optimisation backlog

Written 2026-09-25 for the operator's direction "test and optimise; everything that can go
through the shared-memory zero-copy zero-trap window should". First the census: every CUDA
driver-API operation alone, with the device trace on, counted by BAR, page and register, with the
device's RPC and host RM work and the guest's wall time against native. Then the backlog that
follows from it, ranked by expected saving, the ReBAR note, and the order for the next three days.

Runs, scripts and raw tables: `caraxes/runs/trap-census-2026-09-25/` (README there). Tools:
`caraxes/tools/census-cu.c` (new, the probe), `caraxes/tools/census-table.py` (new, the per-operation
table), `caraxes/tools/rpc-cost.py` and `trap-table.py` (existing).

## Summary

1. **The zero-trap windows work for the steady state of CUDA.** With the interrupt page, the BAR1
   pool and the token pool on (mode `off` below), a submission, a copy, an event and a
   synchronise trap 0 times in the census (net of background), and PPO submits 28000 times per
   second with no trap per submission (`workload-rates.md`). What CUDA workloads still trap is the
   context lifetime and the host memory registrations, which are BAR2 page-table writes, and one
   MSI re-arm write per interrupt.
2. **BAR2 memfd and the BAR1 pool do not work together yet.** With both on (the brief's `on`
   configuration), the first `cuCtxCreate` hangs. It hangs with `CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1`
   too. The shadow's first walk of the context's root finds 15617 leaves where the pool-only
   configuration finds 16128, in both hung runs, and no later plan adds the missing 511. The BAR2
   memfd without the pool (`on-nopool`) works, and the vm2 series found the same pairing
   independently. This is backlog item 1.
3. **The BAR2 memfd is the largest single saving that is already built.** Measured without the
   pool: cuInit 177 to 20 ms (native 94 ms), cuCtxCreate 4.1 s to 0.87 s (native 0.15 s),
   cuCtxDestroy 1.24 s to 0.18 s, cuMemAllocHost 1 MiB 24.8 to 1.65 ms, cuMemHostRegister 1 MiB
   38.9 to 1.73 ms, cuMemAlloc 64 MiB 4.4 to 0.86 ms. It takes about 100000 trapped BAR2 writes out
   of every CUDA process, 3.4 to 4.0 s of trap time (`off/*-rpc-cost.md`).
4. **After the memfd, the context cost is host RM work, not traps.** cuCtxCreate with the memfd
   traps 1884 times (about 175 ms) but takes 874 ms. The shadow worker makes about 6000 host RM
   map calls per context, 761 ms of apply time. The guest waits for them at the first doorbell.
5. **Several remaining costs are not traps at all.** In the pool configuration, these operations
   have no trap in their window, yet each is several times native:
   - a single launch after 30 ms idle: 680 us against 155-177 us native;
   - D2D 16 MiB: 3.85 ms against 0.25 ms;
   - H2D 16 MiB: 1.9 to 5.2 ms against 1.3 ms, depending on the pass.

   When another VM's work holds the GR engine, every GR operation takes 10 to 30 ms (pass 1 of
   `off`). The next measurement for these is device-side timestamps, not trap windows.
6. **About 140 to 160 interrupts per second reach the guest in every CUDA workload**, and about 55
   per second at idle while other VMs use the card. Each one costs a trapped MSI re-arm write
   (`NV_XVE_CYA_2`, 0x88704, value ignored). The idle part is other clients' work, forwarded by the
   host notifier (mapping.md:192-196).

## 1. Method

**Probe.** `census-cu` (C against the driver API, built in the guest like `bench-cu`, the same
binary on the host) runs one operation at a time. It brackets every repetition with two writes to
`NV_PBUS_SW_SCRATCH(30)` (BAR0 0x1478), a latched register that nothing else uses. The value is
`0xce000000 | op << 12 | rep << 1 | end`. So the device trace shows exactly what falls inside the
operation: the trapped accesses of every vCPU (interrupt servicing included), the GSP RPCs, the
shadow plans and the doorbells. Between repetitions it sleeps 30 ms, outside the window. The
marker writes need root in the guest (`resource0`) and are outside the timed interval. Natively
the markers are off and only the times count.

The operations are:
- `init`: cuInit, one process per repetition, 3 processes;
- `ctx 3`: cuCtxCreate and cuCtxDestroy, 3 each;
- `ops 10`: in one context, twice per mode:
  - 100 ms idle windows (the background);
  - cuMemAlloc and cuMemFree of 1 MiB (20 each) and 64 MiB (10);
  - cuMemHostAlloc and cuMemFreeHost of 1 MiB, cuMemHostRegister and cuMemHostUnregister of
    1 MiB (10 each);
  - a launch plus `cuCtxSynchronize` of nvprobe's 1000-cycle spin kernel, 20 times after 30 ms idle
    and 1000 times back to back (short-auto);
  - synchronous pinned copies of 1 and 16 MiB in each direction and D2D, and pageable 1 MiB
    (10 each);
  - `cuStreamSynchronize` of an idle stream, and `cuEventRecord` plus `cuEventSynchronize`
    (20 each);
  - `cuModuleLoad` of a 4.6 KiB cubin (linked from nvprobe's PTX by `census-cu ptx2cubin`) and
    `cuModuleUnload` (10 each).

**Table.** `census-table.py` splits the trace at the markers. For each operation, divided by the
repetitions, it gives:
- the traps by class (rpc-cost.py's classes, with the guest time each took);
- the traps by BAR, page and register;
- the traps on pages an existing window serves;
- the GSP RPCs, with device time and host RM calls;
- the shadow plans, with RM map calls and worker apply time;
- the doorbells and the MSIs.

`summarize.py` pools the passes and subtracts the background: the idle windows' rate times the
window length. `workload-rates.py` counts the same classes over rlprobe (50 episodes), PPO (30 s)
and short-auto, each traced as a whole process.

**Environment.** vm3 only, as `caraxes/runs/gsp-rpc-2026-09-24/run.sh gate610`:
- 610 guest, 1 GiB at reservation slot 5;
- host RM, host execution, mirror channels, the GR shadow, `CARAXES_DMA=register`;
- Cloud Hypervisor with patches 0001-0006.

Frozen device `/mnt/vmstore/caraxes/vfio-user-nvgpu-main-1ea2529`. It is the newest main binary,
built 00:20, and it replaced `-main-bc9aaf8` before the first run. The modes:

| Mode | Windows | Passes |
|---|---|---|
| `bare` | none: trapped doorbell, trapped BAR0 pages, trapped BAR1 and BAR2 | 2 |
| `off` | interrupt page, BAR1 pool (32 MiB), token pool (64), hybrid reactor | 2 |
| `on` | `off` plus `CARAXES_BAR2_MEMFD=1` (the brief's on configuration) | 1: cuInit, then a hung cuCtxCreate |
| `on-syncdb` | `on` plus `CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1` (bar2-memfd.md §7) | 1: the same hang |
| `on-nopool` | interrupt page, token pool, hybrid reactor, BAR2 memfd, no BAR1 pool | 1 |
| native | the same probe on the host, as a client of the host RM (615.71.09) | 2 |

The BAR1 pool ran at 32 MiB, not the default 64: vm1 and vm2 were up with their own BAR1 windows.
parallel.md asks for 32 MiB with two VMs up, and bar1-pool.md:62 bounds pool plus windows to
192 MiB of the host's 256 MiB.

**Conditions.**
- vm1 (costume ladder), vm2 (the alloc and memfd series) and later vm5-615 were up, and some of
  them were active. The host load average was 7 to 9.
- `census-others.txt` records the other devices' CPU during each census.
- The native runs share the card with the same VMs. The closing series' native column
  (benchmark.md:719-741) is the quiet-host reference.
- 0 host Xid in every part (`*/host-xid.txt`), and no host watch alert.
- The read-only host state reads were:
  - `nvidia-smi --query-gpu` (host watch and bench-run conditions) and one `nvidia-smi -q -d MEMORY`
    for the BAR1 headroom;
  - `/proc/driver/nvidia/version`;
  - `journalctl -k -g Xid`.
- Nothing on the host was changed.

## 2. Census table

Traps per operation, gross, with the net of the background in brackets. Wall times are guest
medians, microseconds. Background rates in the idle windows: `bare` 2.47 traps/ms (206.9 of 247.3
per 100 ms are interrupt-tree accesses), `off` 0.36/ms, `on-nopool` 0.66/ms. "Covered" splits the
`bare` traps by the window that takes them:
- intr: the interrupt page, BAR0 page 0 and 0xb81000;
- memfd: BAR2 except the flush page;
- tokens: the usermode doorbell;
- pool: BAR1 below the pool size.

The `off` wall times of the GR operations come from pass 2 only; pass 1 ran while another VM held
the GR engine (§3.4). Full tables: `summary.md`, and `*/census-table.md` per run with registers.

| Operation | traps `bare` | traps `off` | traps `on-nopool` | covered in `bare`: intr / memfd / tokens / pool | wall `bare` | wall `off` | wall `on-nopool` | native | `off` / native | `on-nopool` / native |
|---|---|---|---|---|---|---|---|---|---|---|
| cuInit | 4235 (3770) | 3290 (3226) | 174 (161) | 945 / 3116 / 0 / 0 | 188264 | 176517 | 20411 | 93697 | 1.9 | 0.22 |
| cuCtxCreate | 87141 (77440) | 75070 (73607) | 1884 (1305) | 11154 / 72736 / 214 / 642 | 3928484 | 4086274 | 874002 | 146463 | 28 | 6.0 |
| cuCtxDestroy | 27718 (24246) | 21507 (21064) | 924 (803) | 6256 / 20360 / 0 / 0 | 1405691 | 1237149 | 182667 | 86190 | 14 | 2.1 |
| cuMemAlloc 1 MiB | 36.0 (33.6) | 5.6 (5.4) | 3.6 (3.4) | 30.2 / 2.0 / 0 / 0 | 952 | 292 | 306 | 65 | 4.5 | 4.7 |
| cuMemFree 1 MiB | 42.3 (39.5) | 9.4 (9.3) | 6.6 (6.3) | 29.9 / 1.0 / 0 / 0 | 990 | 343 | 453 | 51 | 6.7 | 8.9 |
| cuMemAlloc 64 MiB | 266 (248) | 188 (187) | 8.0 (7.3) | 77.2 / 179.6 / 0 / 0 | 7301 | 4445 | 858 | 132 | 34 | 6.5 |
| cuMemFree 64 MiB | 143 (132) | 77.6 (76.6) | 13.2 (12.3) | 65.2 / 64.1 / 0 / 0 | 4897 | 2800 | 1428 | 172 | 16 | 8.3 |
| cuMemAllocHost 1 MiB | 905 (844) | 807 (799) | 8.2 (7.1) | 95.5 / 771 / 0 / 0 | 25197 | 24796 | 1651 | 758 | 33 | 2.2 |
| cuMemFreeHost 1 MiB | 317 (294) | 265 (262) | 7.1 (6.4) | 51.4 / 257 / 0 / 0 | 8782 | 8064 | 1105 | 323 | 25 | 3.4 |
| cuMemHostRegister 1 MiB | 1209 (1110) | 1049 (1035) | 15.2 (14.0) | 144.6 / 1030 / 0 / 0 | 39807 | 38913 | 1727 | 284 | 137 | 6.1 |
| cuMemHostUnregister 1 MiB | 358 (329) | 274 (270) | 14.2 (13.1) | 83.7 / 258 / 0 / 0 | 10971 | 11727 | 1651 | 176 | 66 | 9.4 |
| launch + sync, single, after 30 ms idle | 10.0 (7.8) | 2.4 (0.0) | 3.1 (2.6) | 3.8 / 0 / 1.0 / 3.0 | 804 | 680 | 772 | 163 | 4.2 | 4.7 |
| launch + sync, 1000 back to back, per launch | 4.5 (3.9) | 1.0 (0.0) | 3.6 (3.3) | 0.5 / 0 / 1.0 / 3.0 | 127 | 19.6 | 130 | 6.3 | 3.1 | 21 |
| H2D 1 MiB pinned | 5.0 (3.6) | 0.0 (0.0) | 3.0 (2.6) | 0.9 / 0 / 1.0 / 3.0 | 502 | 366 | 502 | 106 | 3.5 | 4.7 |
| D2H 1 MiB pinned | 5.8 (4.5) | 0.5 (0.4) | 3.0 (2.6) | 1.7 / 0 / 1.0 / 3.0 | 476 | 269 | 480 | 109 | 2.5 | 4.4 |
| D2D 1 MiB | 6.6 (4.2) | 0.4 (0.0) | 3.4 (2.7) | 2.4 / 0 / 1.0 / 3.0 | 967 | 875 | 936 | 176 | 5.0 | 5.3 |
| H2D 16 MiB pinned | 12.9 (0.0) | 0.5 (0.0) | 4.0 (0.5) | 8.4 / 0 / 1.0 / 3.0 | 5268 | 1880 / 5198 (two passes) | 5284 | 1305 | 1.4 to 4.0 | 4.0 |
| D2H 16 MiB pinned | 20.3 (6.9) | 1.3 (0.1) | 3.6 (0.1) | 10.8 / 0 / 1.0 / 3.0 | 5344 | 3243 | 5269 | 1293 | 2.5 | 4.1 |
| D2D 16 MiB | 6.9 (0.0) | 1.4 (0.0) | 3.2 (0.6) | 2.7 / 0 / 1.0 / 3.0 | 3829 | 3850 | 3934 | 251 | 15 | 16 |
| H2D 1 MiB pageable | 5.5 (3.8) | 0.1 (0.0) | 3.1 (2.7) | 1.4 / 0 / 1.0 / 3.0 | 623 | 324 | 614 | 198 | 1.6 | 3.1 |
| D2H 1 MiB pageable | 5.1 (3.4) | 0.1 (0.0) | 3.1 (2.7) | 1.0 / 0 / 1.0 / 3.0 | 622 | 383 | 623 | 191 | 2.0 | 3.3 |
| cuStreamSynchronize, idle stream | 0.1 (0.0) | 0.0 | 0.0 | 0 / 0 / 0 / 0 | 6.7 | 5.7 | 3.1 | 4.8 | 1.2 | 0.6 |
| cuEventRecord + cuEventSynchronize | 7.0 (4.9) | 0.4 (0.0) | 3.1 (2.6) | 2.9 / 0 / 1.0 / 3.0 | 775 | 660 | 762 | 152 | 4.3 | 5.0 |
| cuModuleLoad, 4.6 KiB cubin | 9.2 (6.7) | 4.8 (2.6) | 3.1 (2.6) | 3.9 / 0 / 1.0 / 3.0 | 976 | 792 | 879 | 231 | 3.4 | 3.8 |
| cuModuleUnload | 0.1 | 0.0 | 0.1 | 0 / 0 / 0 / 0 | 16.6 | 25.6 | 17.6 | 16.2 | 1.6 | 1.1 |
| idle, per 100 ms (background) | 247 | 35.9 | 66.4 | 207 / 0 / 0 / 0 | - | - | - | - | - | - |

`on` (memfd with the pool): cuInit 174 traps, 20.6 to 51.4 ms (3 processes), then cuCtxCreate did
not return in 8 minutes (`on/`, `on-syncdb/`, §3.1).

### 2.1 What stays trapped, by page and register

In `off` (the working stack with the pool), per operation:

| Operation | What traps, per operation | Page / register |
|---|---|---|
| cuInit | 3116 BAR2 writes, 82 GSP falcon reads, 40 MMU invalidate, 27 GSP queue doorbells, 24 BAR2 flush reads | BAR2 page-table and instance pages; BAR0 0x110000 (`NV_PGSP` 0x110094, 0x110804, 0x110808; `QUEUE_HEAD` 0x110c00); 0xb83000 (`PRIV_MMU_INVALIDATE` 0xb830a0/a4/b0); BAR2 0xfea000 |
| cuCtxCreate | 72736 BAR2 writes, 753 PCI config reads, 488 MMU invalidate, 391 MSI re-arm, 273 flush reads, 255 GSP falcon, 85 GSP queue, 60 usermode page, 20 first doorbells | as above; PCI config 0x06, 0x34, 0x60, 0x68, 0x78 (a capability walk); BAR0 0x88704 (`NV_XVE_CYA_2` through the `NV_PCFG` mirror); 0xbb0000 (`TIME_0/1` and the first ring of each channel before it has a pool token) |
| cuCtxDestroy | 20360 BAR2 writes, 280 invalidate, 257 PCI config, 207 GSP falcon, 169 flush reads, 166 re-arm, 69 GSP queue | the same pages |
| cuMemAlloc / cuMemFree 1 MiB | 2 / 1 BAR2 writes, 2.5 / 5 invalidate, 1 / 1.5 flush reads | 0xb83000, BAR2 0xfea000 |
| cuMemAlloc 64 MiB | 180 BAR2 writes (PD0 entries), 5.5 invalidate, 2.3 flush reads | BAR2, 0xb83000 |
| cuMemAllocHost / cuMemHostRegister 1 MiB | 771 / 1030 BAR2 writes (256 4 KiB PTEs, each written as two dwords twice), 5 / 10 invalidate, 2.5 / 5 flush reads | BAR2, 0xb83000 |
| submission, copy, event, module load | 0 (net) | the MSI re-arm and the capability walk of the background only |

In `on-nopool` the BAR2 writes are gone. What remains of cuCtxCreate:
- 642 BAR1 writes (no pool);
- 488 invalidate accesses;
- 273 flush reads, 71 ms: each carries a memfd sync;
- 255 GSP falcon and 85 GSP queue accesses;
- 53 re-arm writes;
- 20 first doorbells.

The allocations keep only the invalidate (2.5 to 10) and the flush reads (1 to 5). These now cost
about 200 us each, because the memfd sync runs inside them: 8151 syncs, 1.03 s in total, 6.8 ms
at most (`on-nopool/device-summary.txt`).

### 2.2 Device-side RPC and host RM work

Per operation, from the same windows (`*/census-table.md`, "GSP RPCs", "shadow plans", "events
naming a host RM call"):

| Operation | GSP RPCs (device time) | with a host RM call | Shadow plans: RM map calls, worker apply | Other host work |
|---|---|---|---|---|
| cuInit | 27 (105 us) | 12 forwarded controls (FB info, GSS legacy, BINAPI, BUS_GET_INFO, ECC) | none | - |
| cuCtxCreate | 85 (381-428 us): 20 channels 0xc46f, 20 CE 0xc5b5, 8 compute 0xc5c0, 8 OS events, 4 TSGs, controls | 1 | 20 plans: 6000-6900 RM map calls, apply 286 ms (`off`), 761-861 ms (`on-nopool`, `bare`) | 20 host GR channels, 16 TSG joins, 28 engine objects, 1 host VAS at the first doorbells |
| cuCtxDestroy | 69 (290 us): 65 FREE | 26 (the mirror's channel and TSG frees) | 15-16 plans unmapping about 12500 leaves: apply 40 ms (`on-nopool`) to 556-581 ms (`off`, `bare`) | - |
| cuMemAlloc 1 MiB | 0 | - | 0.5 plans, 0.5 map calls, 63-91 us apply | - |
| cuMemAlloc 64 MiB | 0 | - | 0.8-0.9 plans, 24-29 map calls, 1.5-4.4 ms apply | - |
| cuMemHostAlloc / Register 1 MiB | 0 | - | 0.5-1 plans, 15-49 map calls, 1.7-3.6 ms apply | - |
| launch, copy, event | 0 | - | 0 | 1 host doorbell write per submission |

No allocation sends an RPC, which confirms alloc-path.md §1.2. The host RM work of an allocation
is the worker's apply, off the guest's critical path until the next doorbell. The context
creation waits for its plans at the first doorbell of each channel.

## 3. Findings the census adds

### 3.1 BAR2 memfd with the BAR1 pool: the first context creation hangs

`on/` and `on-syncdb/` behave the same way:
1. cuInit passes, with 174 traps.
2. In cuCtxCreate the 20 channels are created and rung.
3. The shadow's full walk of the context's root maps 15617 leaves. In the pool-only runs the same
   walk maps 16128 leaves, a second plan unmaps 252, and a third maps 1.
4. With the memfd, the second plan unmaps the same 252, and no third plan follows.
5. The guest then sits in `NV_ESC_RM_CONTROL` once per second at 100 % CPU, and the host notifier
   interrupts it about 70000 times.
6. The run was stopped by hand: `teardown.sh`, then the device summary. That summary shows
   `alias conflict(s)` 0 and `inconsistencies: none` for the interrupt page.

The vm2 series of the same night found `on` and `on-syncdb` hung, and the memfd without the pool
working (`bar2-memfd-2026-09-24/night/` in that agent's worktree, not yet merged).

Where the two meet: `FakeFb::read` (crates/vfio-user-nvgpu/src/fb.rs:1102-1120). The pool's remap
turns the guest FB address into the pool address first. Then `read_through` asks the memfd for
that pool address. The memfd's remap table is keyed by the guest FB pages that guest BAR2 maps
(bar2-memfd.md §1). A page that is both pooled and behind BAR2 would then read the pool's copy
from VRAM, not the guest's newer bytes in the memfd, and a sync's write-back would land at the
unpooled address. That fits a deterministic walk that misses one table's worth of leaves. It is a
hypothesis from reading the code, not verified.

### 3.2 What the memfd buys, measured

The `on-nopool` column against `off` (§2). The price moved into the flush reads and the syncs:
- 1026 ms of sync time over 8151 syncs in the `on-nopool` run;
- a flush read now costs about 200 us instead of 30 us;
- a sync takes 126 us on average, comparing the live pages. The live set was 118 to 137 pages, not
  the 77 of the design's estimate (bar2-memfd.md §2).

For cuMemFree 1 MiB this makes the memfd configuration slower than `off`: 453 against 343 us. KVM
dirty logging, or a sync limited to the pages that the invalidated root can reach, is the remedy
(bar2-memfd.md:203, backlog item 3).

### 3.3 Background interrupts

With nothing running in the guest, the idle windows still trap:
- `bare`: 207 interrupt-tree accesses per 100 ms;
- `off`: 5.5 MSI re-arm writes (`NV_PCFG` 0x88704) and 25.6 PCI config reads per 100 ms.

In the workloads the re-arm runs at 140 to 160 per second in every mode (`workload-rates.md`). The
host notifier forwards every client's GR non-stall event to each guest (mapping.md:192-196), so
another VM's work costs this guest interrupts.

The PCI config reads are a capability walk. The same eight offsets appear each time: 0x06, 0x34,
0x60/0x61, 0x68/0x69, 0x78/0x79. The rate is 0 to 280 per second, intermittent (PPO pass 2: 0).
Their guest caller is not identified. Neither the nvidia module's capability lookups
(`nv-pci.c:2709`, used only by procfs) nor the RM's bif code matched at first reading.

### 3.4 Costs that are not traps

These rows have no net trap in `off` and are still far from native:

| Row | `off` | native | Trace in the window |
|---|---|---|---|
| launch after 30 ms idle | 680 us | 155-177 us | eventfd ring, host doorbell, end: the GPU work plus the path back |
| D2D 16 MiB | 3.85 ms (4.4 GB/s) | 0.25 ms | host doorbell at +0.13 ms, one interrupt at +0.53 ms, end at +3.7 ms (`off-2`); `bare` spins with no interrupt, end at +3.85 ms |
| H2D 16 MiB pinned | 1.88 ms in pass 1, 5.20 ms in pass 2 | 1.30 ms | host doorbell, then 5 ms of spinning, no interrupt |
| every GR operation in pass 1 of `off` | 10 to 30 ms (launch, D2D, event, module load) | - | host doorbell at +1.7 ms, end at +29 ms, nothing between: another VM's work held the GR engine by timeslice |

The copy engine's bandwidth into guest RAM varies with the other VMs' activity (8.9 against
3.2 GB/s in two passes of the same configuration). The interrupt page series on vm0 had 11.9 GB/s
(benchmark.md:818). D2D runs on the compute channel through the shadow VA space
(benchmark.md:296-300), and its 16 MiB case sits at 4.4 GB/s in every mode, with or without
traps. The closing series had the same band ("16 to 167", benchmark.md:738). Neither is a trap
question. Both need the host-side timeline of the host doorbell write, the host channel's GPGet and
semaphore, and the completion notifier.

### 3.5 Workloads

Whole-process windows, rlprobe 50 episodes and PPO 30 s (`workload-rates.md`, `*/rl-rpc-cost.md`,
`*/ppo-rpc-cost.md`):

| | `bare` | `off` | `on-nopool` | native (closing series) |
|---|---|---|---|---|
| rlprobe stage 4 | 17.7-17.8 s | 2.1-2.3 s | 14.9 s | 1.20 s |
| rlprobe transfers/s | 162 | 1234-1344 | 193 | 2350 |
| rlprobe process wall | 32.6-32.8 s | 18.9-19.6 s | 26.1 s | 4.64 s |
| PPO steps/s | 6883-7119 | 61123-63310 | 8497 | 107663-112259 |
| short kernel, ms/launch | 0.151-0.197 | 0.020-0.022 | 0.274 | 0.006 |
| submissions per second, PPO | 3500 (each traps its doorbell and 3 BAR1 writes) | 28000 (eventfd, 0 traps) | 4460 (eventfd doorbell, 3 trapped BAR1 writes each) | - |
| traps per second, PPO steady state (without context setup) | about 16500 (BAR1 10350, doorbells 3450, interrupt pages 2310) | about 400 (re-arm 148, PCI config 0-234, invalidate 25) | about 13900 (BAR1 13500) | - |
| BAR2 writes per process (context create and destroy) | 100700 | 100700 | 0 | - |

In `off` the rlprobe process spends about 4.0 s of its 19 s on the context's BAR2 writes (rpc-cost.py, `off/rl-rpc-cost.md`), and PPO
steady state is no longer bound by traps. The remaining gap to native (0.56x steps/s) is per
submission device work (eventfd wake-up, the host doorbell write, 3.6 us from ring to host
doorbell, benchmark.md:699) and GR sharing, not exits.

### 3.6 Display and game, cited

The display VM was not borrowed (brief). From intr-page.md §10.6 (intr-page.md:617-618), per
presented frame with the interrupt page and the display page:
- Mutter traps 139.86 times (59.86 Hz);
- Weston traps 31.08 times (57.41 Hz).

What Mutter still traps per frame (intr-page.md §9 B3 and §6a, intr-page.md:293):
- 71 BAR2 PTE writes and 23 flush-page reads (the BAR2 writes per Mutter frame differ by series, not by method: 71 (70.85) is series F on the 610 guest without the interrupt and display pages (`caraxes/runs/display-f-2026-09-24/f3b-traps-mutter-fresh/trap-table.txt`, "BAR2 writes, all"), 73.5 is run B3 of intr-page.md section 9, and 73.63 the 615 guest with both pages in display-615.md section 5);
- 11.8 usermode `TIME_0/1` reads;
- 9.6 `NV_PCFG` re-arm writes;
- 10 MMU invalidate accesses;
- 2 to 3 GPPut writes.

SotTR has no trap table yet: the game VM ran on the operator's session only (sottr-stage.md). Its
frame is at least the compositor's plus the game's own submissions, which are 0 traps with the
pool and the token pool, and its fences, one re-arm each. The unknown part is how many of its
host-visible Vulkan buffers the guest RM maps through guest BAR1 outside the pool (§5, item 9).

## 4. Ranked backlog

Cost per trap for the estimates: 28 to 35 us, the per-class medians of rpc-cost.py in these runs
(mapping.md:308-313 measured 23 to 27 us on a quiet host). "Rate" is how often the source occurs.

| # | Remaining source | Mechanism: which page or register, which window | Expected saving | Risk | Estimate |
|---|---|---|---|---|---|
| 1 | BAR2 page-table writes, with the pool on: 72.7k per cuCtxCreate, 20.4k per cuCtxDestroy, 3116 per cuInit, 771-1030 per 1 MiB host allocation or registration, 180 per 64 MiB allocation | Make the existing BAR2 memfd work beside the BAR1 pool. One order of translation for read, write, sync write-back and `watched_dword` (fb.rs:1102-1147, fb.rs:774): memfd keys and pool remaps must agree on whether they hold guest or pool addresses. A unit test with both on the fake FB, then `run.sh on` on vm3 | Per CUDA process about 100k traps and 3.4-4.0 s of trap time (rlprobe process 19 s to about 15 s). cuCtxCreate 4.1 s to 0.87 s, cuCtxDestroy 1.24 s to 0.18 s, cuInit 177 to 20 ms, host alloc/register 25-39 ms to 1.7 ms, 64 MiB alloc 4.4 to 0.86 ms (all measured in `on-nopool`). Display: Mutter's 71 BAR2 writes per frame, about 2 ms per frame | Ordering between pool moves (barrier, bar1-pool.md §4) and memfd syncs; a pooled page behind BAR2 must be written back to where the GPU reads it; fail closed as both designs do. Isolation unchanged: both map only the VM's reservation | 0.5-1 day |
| 2 | Host RM map calls of the shadow during cuCtxCreate: about 6000 per context, 286-861 ms of worker apply, the guest waits at the first doorbell | No trap; the device's host work. Region-object free list across contexts (alloc-path.md:158), larger coalesced runs, and planning the root while the channels are created (lazy plans, alloc-path.md §3) | cuCtxCreate with item 1: 874 ms to about 250-350 ms. cuCtxDestroy's 556-581 ms apply in `off` and `bare` goes with the same batching | The budget and the teardown order (alloc-path.md §2); a reused region object must not keep a stale leaf | 1-2 days |
| 3 | MMU invalidate (0xb83000: `_PDB`, `_UPPER_PDB`, trigger) and the flush read (BAR2 0xfea000), which with the memfd carries the sync: 2.5-10 invalidate and 1-5 flush traps per allocation, 488 + 273 per context, 10 per Mutter frame | alloc-path.md step 3 (alloc-path.md:157): page 0xb83000 and the flush page as memory, scanned by the reactor; the memfd sync moves to the doorbells and the GSP doorbell. The sync cost itself (126 us mean, 6.8 ms max): KVM dirty log or a sync limited to pages reachable from the invalidated root (bar2-memfd.md:203) | cuMemAlloc 1 MiB 292-306 us to about 60-80 us, cuMemFree 343-453 us to about 60-80 us, 64 MiB allocations to the worker's apply. Context creation: about 70-100 ms. Per Mutter frame 10 traps, about 0.3 ms | The fault-buffer registers on page 0xb83000 (0x3070-0x3094) need a model or must stay trapped; a trigger must never be lost (the RM polls bit 31); the doorbell barrier stays the only point where the GPU sees table changes | 2-3 days |
| 4 | MSI re-arm, `NV_XVE_CYA_2` BAR0 0x88704, value ignored (kernel-rm-hw-surface.md:119): one trap per interrupt, 140-160 per second in every CUDA workload, 55 per second idle, 9.6 per Mutter frame | A KVM ioeventfd without datamatch on 0x88704. Patch 0005's `VFIO_USER_DEVICE_GET_REGION_IO_FDS` path, as the doorbell uses it. The device only counts it. Alternatively page 0x88000 as memory, with the reactor applying the three mirrored writes; that is more work, since config mirror writes have effects | 30-50 us per interrupt (p90 250-570 us under load): 5-8 ms of vCPU per second in PPO and rlprobe, 0.3-0.5 ms per Mutter frame | Low: the write has no effect in the model. Check `bar0.rs` for the register's effect, and keep the other writes of the page trapped | 0.5 day |
| 5 | Host notifier events of other clients' work forwarded as MSIs: 55 per second idle in `off`, 207 interrupt-tree traps per 100 ms idle in `bare` | Progress-gated forwarding (mapping.md:192-196, mapping.md:404): raise the guest vector only when one of this VM's channels moved (its GPGet or semaphore), and keep the interrupt page's retrigger watchdog as the bound | Idle interrupts about 55/s to near 0. Each MSI saves the re-arm trap (item 4) and the guest's interrupt handler (1-2 us with the interrupt page, about 60 traps without it). Under multi-VM load the saving grows with the other VMs' work | A missed wake-up is a hang: the watchdog bound (10 ms) must cover every gated case; a completion that shows only in a semaphore the device does not watch must still wake the guest | 1-2 days |
| 6 | GSP queue doorbell (`NV_PGSP_QUEUE_HEAD` 0x110c00) and falcon status reads (0x110094, 0x110804, 0x110808): 27 + 82 per cuInit, 85 + 255 per cuCtxCreate, 69 + 207 per cuCtxDestroy | P24 poll-mode GSP (roadmap.md:110): the queue-head write as an ioeventfd (the value is the new head; one datamatch-free eventfd, the reactor reads the head from the command queue in guest memory), and page 0x110000 as memory if its read registers are constants of the boot model | With item 1 these are 63 % of cuInit's 174 traps: cuInit about 20 ms to about 12-15 ms; context lifecycle about 620 traps, 20-40 ms | The GSP doorbell is a memfd sync point (bar2-memfd.md §1) and the RPC order is strict (msgq.rs); the reactor must sync the memfd before it reads a command | 2 days |
| 7 | PCI config capability walk, 0-280 reads per second | First the guest caller (ftrace on `pci_read_config_*` in the guest). Then remove the cause, or answer the read-only config dwords from a cache in the VMM (a Cloud Hypervisor patch; vfio-user config is not mmap-able) | 0-8 ms of vCPU per second | Low | 0.5 day to identify |
| 8 | Usermode `TIME_0/1` reads, 11.8 per Mutter frame, and 60 usermode-page accesses per cuCtxCreate | A time page: page 0xbb0000 cannot be memory (the doorbell ioeventfd sits there, intr-page.md §4). The TIME registers would need the guest to read another page, which the stock driver does not. Only a guest-side change or a paravirtual clock helps | 0.35-0.4 ms per Mutter frame | Monotonicity and the doorbell page's fail-closed rules | not before items 1-6 |
| 9 | Guest BAR1 CPU access outside the pool: Vulkan host-visible device-local memory, the console mapping, display imports; CUDA has none in the census (0 BAR1 traps in `off`) | Size the pool to what the game maps (bar1-pool.md; bounded by the host's 256 MiB BAR1, bar1-pool.md:62); on a ReBAR card see §5 | Unknown until SotTR's trap table exists; every trapped BAR1 access is 30 us | The pool's warm-move barrier (bar1-pool.md §4) under a game's mapping churn | measure first: 0.5 day on the game VM |
| 10 | Non-trap stalls (§3.4): D2D 16 MiB 3.85 ms against 0.25, H2D 16 MiB 1.9-5.2 ms against 1.3, single launch after idle 680 against 160 us, 10-30 ms under GR sharing | A device-side timeline: host doorbell write, host GPGet and semaphore, host notifier, guest wake-up, each timestamped in the trace; then the host TSG timeslice and priority for mirrored TSGs | Up to 3.6 ms per D2D 16 MiB and 3.9 ms per H2D 16 MiB; the multi-VM GR case is 10-30 ms per operation | None for the measurement; timeslice changes touch host scheduling of other clients (the desktop) | 1 day to measure |
| 11 | Replica merges of BAR1 area pages the pool refuses (guest table pages and pages mapped twice, bar1-pool.md §10): two 4 KiB reads per replica at every MMU invalidate, GSP doorbell and poll | Count the replicas and merges on the game VM first. If they are many: why the 615 RM maps its tables into BAR1 (its transfer path) and whether its old BAR1 mappings linger (the aliases); merge only replicas whose area page the guest wrote (a dirty bit would need the area as a memfd) | Unknown; 25 aliases and 2 table pages in the black session | Correctness first: data written only through an aliased mapping reaches the GPU at the next flush or invalidate, not at a doorbell | measure first: part of the game-VM run |

**Status of item 2 (2026-09-25, done; memory-model.md §7, runs
`caraxes/runs/opt-shadow-maps-2026-09-25/`).** Measured on vm0 with the memfd and without the pool:
- **Calls.** Host RM calls per cuCtxCreate went from 13171 to 228, and the worker's apply time
  from 770 to 67 ms.
- **Wall time.** cuCtxCreate went from 815 to 309 ms (native 146). cuCtxDestroy went from 170 to
  107 ms, with 1145 to 79 calls.
- **The cause** was the guest's scattered sysmem pages: about 15000 one-page maps and later
  unmaps per context (29 and 81 us each). A queued destroy delayed the next creation by up to
  1.4 s.
- **The fix.** Gather descriptors map a scattered stretch of up to 512 pages with one call, and
  an emptied region is freed with one call. Deferred TLB invalidation was measured without gain
  and stays off.
- **bench-cu init** cuCtxCreate went from 1719 to 353 ms; churn is unchanged.
- **Gates.** The cuda gate gave 11 of 12 (cuda-managed), with 0 host Xid.

What is left of the creation is the GPU's first work on each of the 20 channels, not host RM
calls.

**Top five by expected saving in the target workloads:**
1. BAR2 memfd with the pool (item 1): about 100k traps and 3.4-4.0 s per CUDA process; context 4.1 s to
   0.9 s; host registrations 20x faster; 71 traps per Mutter frame.
2. Shadow host map work at context creation (item 2): about 0.5-0.6 s per context after item 1.
3. Invalidate and flush page as memory, and a cheaper memfd sync (item 3): allocations 300-450 us
   to under 100 us; 10 traps per Mutter frame.
4. MSI re-arm through an ioeventfd (item 4): 140-160 traps per second in every workload, 9.6 per
   Mutter frame.
5. Progress-gated interrupt forwarding (item 5): about 55 interrupts per second at idle and the
   other VMs' share under load, with each interrupt's handler.

**Status (2026-09-25):**
- **Item 1: done** (commit bb52a46, bar2-memfd.md §8, `caraxes/runs/opt-memfd-pool-2026-09-25/`).
  The hypothesis of §3.1 was a real fault but not the cause of the hang.
  - The cause: the pool learns about BAR1 table changes only from trapped writes, and with the
    memfd the guest RM writes those tables through BAR2. `FakeFb::note_pages_changed` did not
    tell the pool, so the pool never moved a page. The hung runs show 0 pages moved in, and the
    shadow's 2 MiB leaf over the pooled page stayed whole (the 511 leaves).
  - Both are fixed, with two unit tests that fail without the fix.
  - With memfd and pool on (`on`), cuCtxCreate takes 0.61 s with 1228 traps (`on-nopool` 0.87 s,
    `off` 4.09 s). cuCtxDestroy takes 0.20 s, cuInit 28 ms, cuMemAllocHost 1.72 ms and
    cuMemHostRegister 2.29 ms.
  - The steady-state rows keep 0 traps. rlprobe's process wall is 15.5 s against 21.3 s in the
    same session.
  - The cuda gate is 10 of 12 (the gate script's count: the declared-unsupported cuda-gdb row is among the 12; documents that count only the 11 scored rows write the same result as 10 of 11, testing.md section 3, cuda), as before, and there was 0 host Xid.
  - Copy rows: the pool's rows do not regress over three A/B passes. The 256 MiB rows and the d2d
    rows are unchanged or better. The pinned 1 and 16 MiB rows switch between about 6.5 and
    12.9 GB/s with the card's PCIe link state (gen 1 or 2 at P8 or P5, gen 3 at P0) in both
    configurations.
  - The device's CPU per copy process falls from 11 s to 3.5 s.
  - This is now the recommended configuration (bench-run.sh header).
- **Item 4: done** (commit 0c2cb79, `CARAXES_MSI_REARM_IOEVENTFD=1`, intr-page.md §4,
  `caraxes/runs/opt-msi-ioeventfd-2026-09-25/`).
  - The re-arm write is an ioeventfd without datamatch, read by the token pool's reactor, which
    does what the trapped write did.
  - Re-arm traps per second: rlprobe 141 to 0, PPO 153 to 0, idle 102 to 0 per second (10.2 per
    100 ms window). Against the 55 per second of items 4 and 5 and the 331 of coverage-map.md: the idle rate depends on what else runs on the card: 55 per second is the census's idle windows in `off` (section 3), 102 per second the idle census window of the ioeventfd run (`caraxes/runs/opt-msi-ioeventfd-2026-09-25/`, 10.2 per 100 ms), 331 per second the coverage map's 300 s idle while other VMs worked (coverage-map.md section 4, row 9); the MSIs are other clients' work forwarded, so the rate follows the other VMs' load.
  - All traps per second: rlprobe 508 to 456, PPO 423 to 314.
  - The cuda gate is 10 of 12, and there was 0 host Xid. Mutter was not measured: expected 9.6
    traps per frame fewer.

**Coverage map (2026-09-25).** [coverage-map.md](coverage-map.md) extends this census from
single operations to whole workloads with the device's profile (`CARAXES_PROFILE=1`). Its §4
ranks ten costs the census could not see, because they are not traps or they lie outside the
benchmarks:
- the BAR1 pool's page moves: 1024 per CUDA process, 363 ms;
- the pool barrier's host TSG preempts;
- the device's BAR1 window remaps;
- nvidia-smi's GSS-legacy forwards;
- idle interrupt forwarding at 331 per second.

Items 3, 5 and 6 above get per-workload numbers there.

### By transport class

[gsp-transport-classes.md](gsp-transport-classes.md) replaces workload guessing for the GSP
controls with a classification of every routed 615 control by its structure
(`caraxes/abi/gsp-615.71.09/transport.tsv`): ZT-PAGE 101, ZT-QUEUE 155, HOST 103, TRAP 32,
REFUSE 490. Controls are lifecycle traffic (187 RPCs per CUDA process, 101 per
`nvidia-smi -q`, 0 per frame), and one 615 RPC costs four traps (108 us median) against 3 us of
device work. The list in its order (§10 there):

| # | Item | Rows | Expected saving |
|---|---|---:|---|
| T1 | Static GSS-legacy answers cached by the guest (reply `rmctrlFlags` CACHEABLE, proven static first), TTL for the rest | 33 | up to 1.85 s over the profiles; `nvidia-smi -q` 56 ms to a few ms |
| T2 | Device cache of static host getters (after the public filter) | 31 | 375 ms; 2-4 ms per CUDA process |
| T3 | TTL cache for host telemetry, rate limit per VM | 39 | 373 ms; dmon 203 ms per 31 s |
| T4 | Polled queue: page 0x110000 read-only, `QUEUE_HEAD`/`MSGQ_TAIL` as ioeventfds (VMM patch) | all RPCs | about 130 us per RPC: 24 ms per CUDA process, 175 ms per GNOME start |
| T5 | First step of T4: `QUEUE_HEAD` ioeventfd only (backlog item 6, narrowed) | all RPCs | about 30 us per RPC |
| T6 | FREE answered before the mirror's host teardown | RPC | 0.83 ms x 71 per CUDA process |

Not optimisable: the 490 refused and 32 TRAP controls (addresses, handles, window order, host
TSG state), the transport lifecycle, and an answer page for a stock driver (no read path, about
1.5 ms per CUDA process). Item 6 above ("the reactor reads the head from the command queue in
guest memory") does not hold for 615: the head is a register there.

### Host resources (2026-09-25)

Left open by [host-resources.md](host-resources.md) (§7 there):
- RC the guest channel for every refused shadow build, not only for memory refusals.
- A guest-visible error instead of all-ones reads when the VRAM store has no BAR1 window.
- Free a TSG made for a channel whose build failed, before the guest frees it.
- A per-device budget of host VRAM outside the reservation (the floor is host-wide).
- Attribute the 270 MiB creep of the 2026-09-25 game session with a soak run.
- The dynamic BAR1 gate (host-resources.md §6) has not run on the GPU. Also open: a guest
  error path instead of all-ones reads when the window budget is refused, and a sub-second
  BAR1 sampler (the 1 Hz watch misses transients).

## 5. ReBAR

The operator's framing: on Turing, BAR1 is capped at 256 MiB, so the pool multiplexes it; on a
resizable-BAR card each VM gets its own window, and the pool's remap copies disappear.

Today on this card:
- **BAR1 is 256 MiB and cannot grow.** The resizable-BAR capability offers only 64, 128 and
  256 MiB (host-facts.md:37, parallel.md:122).
- **The desktop and every device share it.** The pool plus the device's own BAR1 windows must stay
  under 192 MiB (bar1-pool.md:62), so the census ran the pool at 32 MiB beside two other VMs.
- **The pool has to copy.** It gets a fixed piece of the reservation, so every page the guest maps
  into the area is copied in, and back out at release (bar1-pool.md:106).
- **The windows miss.** The device reaches the rest of the reservation through two 32 MiB BAR1
  windows at 1.6 us per uncached read and 2.5 ms per window miss (benchmark.md:617-618).

On a card whose BAR1 covers its VRAM, the list changes as follows:

| Backlog item | With ReBAR |
|---|---|
| 9, guest BAR1 outside the pool | The pool can cover all of guest BAR1, with no 192 MiB budget and no refusal for lack of host BAR1. SotTR's host-visible heap fits whatever it maps |
| Pool copies (bar1-pool.md §3 rule 4) | They go only if a remap becomes a mapping change instead of a copy: the area page `g` would map the guest's page `X` directly. That needs the whole reservation in host BAR1, which ReBAR allows, and a way to re-point one 4 KiB page of the VMM's mapping of the area. Cloud Hypervisor maps sparse areas once (bar2-memfd.md §2), so this needs a VMM patch, for example a per-page remap message or a device-controlled mapping. The warm-move barrier of bar1-pool.md §4 stays, because the GPU must not see the page move under it |
| The device's own BAR1 windows | One mapping of the whole reservation: no window misses in the walks, the FB store, the BAR TLB and the memfd syncs, whose loads read VRAM through the windows today. This shortens item 2's walks and item 1's page loads |
| 1, 3, 4, 5, 6 | Unchanged: BAR2, BAR0 and interrupts do not depend on BAR1's size |
| Multi-VM | Each VM's area and windows no longer compete with the desktop's BAR1. The P20 BAR1 exhaustion (parallel.md:115-127) cannot recur |

Today's code needs none of this. ReBAR changes item 9 from "size the pool within 192 MiB" to "make
the pool zero-copy", which is a VMM change and not a device one.

## 6. The next three days

| Day | Work | Measured by |
|---|---|---|
| 1 | Item 1: memfd with the pool, a unit test on the fake FB with both translations, then `caraxes/runs/trap-census-2026-09-25/run.sh on` on vm3. Item 4 alongside: the re-arm ioeventfd | `on` passes the census. cuCtxCreate at or below `on-nopool`'s 874 ms, with the traps of `on-nopool` minus its 642 BAR1 writes, about 1250. rlprobe stage 4 at or below 2.3 s, and its process wall about 15 s. PPO steps/s at or above 61000. The re-arm class at 0 in every census window. cuda gate green, 0 host Xid |
| 2 | Item 2: the shadow's host map work at context creation, with the region free list and batched maps; start item 3 with the sync cost (a sync limited to reachable pages) | cuCtxCreate below 400 ms in the census, worker apply per context in `census-table.md` below 150 ms, RM map calls per context below 1000; cuMemFree 1 MiB below `off`'s 343 us with the memfd on |
| 3 | Item 3: page 0xb83000 and the flush page as memory. Item 10: the device-side timeline for D2D and H2D 16 MiB, one census pass with host timestamps. If the game VM is free: SotTR's trap table for item 9 | cuMemAlloc and cuMemFree 1 MiB below 100 us with 0 invalidate and flush traps in their windows; the D2D and H2D 16 MiB stalls attributed to a named step; SotTR per-frame traps by page from `trap-table.py --pages` |

Every step is measured with the same probe and table: `run.sh MODE` on vm3, `summarize.py`, and
`workload-rates.py` for rlprobe and PPO. Numbers only count with 0 host Xid on the device's pid
and the other VMs' activity recorded (`census-others.txt`).
