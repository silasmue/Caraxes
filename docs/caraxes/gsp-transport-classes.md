<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# GSP controls by transport class: what can go zero-trap, and what cannot

Written 2026-09-25 for the operator's direction: classify the control list by the structure of
each command (call structure, data structure, pointers, handles, side effects) and by whether it
could use a zero-trap or DMA path, "instead of blindly optimising for workloads", with security
and isolation as key factors. Research only; nothing under `crates/` changes.

- Table: `caraxes/abi/gsp-615.71.09/transport.tsv`, one row per GSP-routed control of 615.71.09
  plus the GSS-legacy and BINAPI ids the profiles saw (881 rows).
- Tool: `caraxes/tools/gsp-transport.py` (generates the table; `--summary` prints the tables
  below; `--rpc-traps TRACE` the per-RPC trap timing of §2).
- 615 tree paths are relative to the open-gpu-kernel-modules 615.71.09 root (as in
  [gsp-transport-615.md](gsp-transport-615.md)).

## Summary

- **Verdicts (881 rows):** ZT-PAGE 101, ZT-QUEUE 155, HOST 103, TRAP 32, REFUSE 490. 184 rows
  were seen in a profile or device log; no record reached the other 697.
- **Controls are lifecycle traffic, not frame or submission traffic.** 0 RPCs in Weston's 10 s
  per-frame window, 1 (`BUS_FLUSH_WITH_SYSMEMBAR`) in Mutter's, 0 in 300 s idle. The cost sits in
  process start and stop, modesets and queries: 187 RPCs per CUDA process (45 of them controls),
  101 per `nvidia-smi -q`, 1352 per GNOME start, 272 per 615 boot (§4).
- **Per RPC, the transport costs more than the device.** A 615 RPC is four trapped accesses on
  BAR0 page 0x110000, 108 us median from the `QUEUE_HEAD` write to the fourth access, about
  140 us with that access's return. The device's own work is 3 us median. So for the 256
  device-answered verdicts (ZT-PAGE, ZT-QUEUE) the trap protocol is over 95 % of the cost; for
  HOST the host RM call (0.2 to 80 ms) dominates (§2).
- **The largest saving is not a transport change but the HOST class** (§6): over the profiles,
  GSS-legacy forwards cost 1852 ms, static host getters 375 ms, telemetry 373 ms. A stock 615
  guest caches a GSS-legacy answer when the reply's `rmctrlFlags` says CACHEABLE
  (`src/nvidia/src/kernel/vgpu/rpc.c:10899-10905`); the device zeroes that word today
  (`crates/vfio-user-nvgpu/src/boot.rs:1524`).
- **The polled queue needs the GSP register page, not the queue memory.** In 615 the command
  queue's write pointer is a register (`NV_PGSP_QUEUE_HEAD(0)`), not a word in shared RAM
  (gsp-transport-615.md §2.2), so P24's premise "already in shared guest RAM" (README.md:256)
  no longer holds for GSP. Design (b): page 0x110000 as a read-only memory area plus write
  ioeventfds; about 130 us saved per RPC: 24 ms per CUDA process, 13 ms per `nvidia-smi -q`,
  about 175 ms per GNOME start (§7).
- **An answer page does not pay for a driver change.** A stock 615 driver reads no such page;
  it caches only controls whose compiled-in flags say CACHEABLE, and GSS-legacy answers by the
  device's reply. The ZT-PAGE controls cost about 1.5 ms per CUDA process (§5).
- **What cannot be optimised:** 490 REFUSE (379 absent on TU106, 111 isolation) and 32 TRAP
  (addresses, handles, window order, host engine state; §8, §9), plus the transport's own
  lifecycle (boot, unload, reload, reset), which stays trapped.

## 1. Method

Per routed control (`controls.tsv` route `gsp`, 844) and per id the profiles saw outside the
export tables (37 GSS-legacy and BINAPI ids):

| Column | Source |
|---|---|
| caller kind | observed windows first (boot, per CUDA process, NVML/`nvidia-smi` query, Vulkan, display, per-frame window, idle, error); else the function that sends it, from gsp-surface.py's call-site scan (`parse_call_sites`, `enclosing_function`) matched against name rules; else the display/UVM reference, `d-controls.tsv`'s caller, or the name; `never-tu106` for an E family absent on TU106 |
| frequency | `per_boot` (boot windows), `max_per_s` (window mean over windows of 1 s or more), `seen_total`, `per_frame_window` |
| sizes | params size (`controls.tsv`); the reply carries the same params (boot.rs resizes the answer); `queue_slots` with 96 bytes of framing (MCTP 24, RPC header 32, control header 40) |
| embedded | the params struct parsed from the 615 SDK headers, three levels deep: `NvP64` and pointer fields, `NvHandle` fields, address fields (`*Addr*`, `*Address*`, `*VA`, `*PA`, `*Base`, PDB/PDE) |
| answer source | the device's profile label, else provenance and `answered_by` |
| guest cache | `rmapi-cache` when flags carry `RMCTRL_FLAGS_CACHEABLE`(`_BY_INPUT`) and no access right (`rmapi_cache.c:152-172`); `possible` for GSS-legacy ids |
| isolation | what an unvalidated zero-trap version would allow, and the mitigation; `d-controls.tsv` risk text where it exists |

Reason codes:

| Verdict | Reason | Rule |
|---|---|---|
| ZT-PAGE | STATIC-CARD, STATIC-VM | getter answered from chip data, costume or the display configuration; not computed from the request |
| ZT-QUEUE | ECHO-ACK, DEVICE-STATE-GET, DEVICE-STATE-SET, DEVICE-COMPUTE | device-only: acknowledged without effect, read or set this VM's device state, or computed from the request (AUX, mode checks) |
| HOST | HOST-STATIC, HOST-TELEMETRY, HOST-GSS-LEGACY | host RM round trip; `host_cache` yes, ttl, or ttl-or-guest-cache |
| TRAP | ADDR-VALIDATE, HANDLE-STATE, WINDOW-ORDER, HOST-STATE | a state change that carries addresses or handles, touches a zero-trap window, or acts on a host object |
| REFUSE | ABSENT, ISOLATION | E family or D REFUSE; D DENY, d-controls risk "high", or a public-table refusal |
| any | PLANNED-... | the 67 unhandled controls, by their gsp-surface category (A page, B host, C/D device) |

Observations: the 610-guest profiles of `caraxes/runs/coverage-2026-09-25/{map,extra}` (the only
per-control profiles; the routed set is the same in 615 except the seven changed structs and
`GPU_GET_VMMU_SEGMENT_SIZE`, kernel-side in 615) and the 615-guest device logs of
`transport-615-2026-09-24` (boot, two reloads) and `display-615-2026-09-24/s4-series` (display
bring-up, three GNOME starts, hotplug, Weston and Mutter with their `trap-table.txt` windows).

## 2. One RPC on 615, measured

`gsp-transport.py --rpc-traps` on `display-615-2026-09-24/s4-series/traps-weston/device-trace.txt.xz`
(748 RPCs):

| Step | Access | Median us | p90 |
|---|---|---:|---:|
| the kernel writes the element, fences, writes the pointer (`msgq.c:352-377`) | W 0x110C00 `QUEUE_HEAD` | | |
| device receives the element | | 4 | 8 |
| device work, reply element written (inside the trapped write) | | 3 | 4 |
| `kgspRecvPoll` drains: reads the status-queue pointer (`msgq.c:408`) | R 0x110C80 `MSGQ_HEAD` | 33 | 64 |
| marks consumed, writes its read pointer (`msgq.c:479`) | W 0x110C84 `MSGQ_TAIL` | 34 | 61 |
| health check (`kernel_gsp.c:2070`), the crashcat put pointer `NV_PFALCON_FALCON_DEBUGINFO` (`kernel_crashcat_engine_tu102.c:57`, `dev_falcon_v4.h:45`) | R 0x110094 | 30 | 56 |
| head write to the fourth access | | **108** | 220 |

- Plus `QUEUE_TAIL` (0x110C04) reads when the cached free count runs out: 12 in 748.
- The 610 guest also traps four times per RPC: the head write and three falcon reads
  (profile rows 0x110C00 10119, 0x110094, 0x110804 and 0x110808 10118 each).
- The coverage map's 364 us per head-write trap (coverage-map.md:299) is the 610 mean including
  host-forwarded work; the device's mean control time there is 690 us, skewed by one 80 ms
  GSS-legacy id.
- The kernel spins until the reply (`kernel_gsp.c:3374-3459`, `osSpinLoop`). Every RPC but two
  is synchronous: `ECC_NOTIFIER_WRITE_ACK` (`rpc.c:11180`, no ECC on TU106) and GMCAPI
  fire-and-forget (`gmcapi_impl.c:124`, `GSP_SUSPEND` at unload).

## 3. Counts

| Verdict | Rows | Seen | Reasons |
|---|---:|---:|---|
| ZT-PAGE | 101 | 30 | STATIC-CARD 39, STATIC-VM 12, PLANNED-STATIC-CARD 50 |
| ZT-QUEUE | 155 | 57 | ECHO-ACK 63, DEVICE-STATE-GET 32, DEVICE-STATE-SET 24, DEVICE-COMPUTE 19, PLANNED-DEVICE 17 |
| HOST | 103 | 74 | HOST-TELEMETRY 39, HOST-GSS-LEGACY 33, HOST-STATIC 31 |
| TRAP | 32 | 11 | WINDOW-ORDER 13, ADDR-VALIDATE 12, HANDLE-STATE 5, HOST-STATE 2 |
| REFUSE | 490 | 12 | ABSENT 379, ISOLATION 111 |

| Caller kind | ZT-PAGE | ZT-QUEUE | HOST | TRAP | REFUSE |
|---|---:|---:|---:|---:|---:|
| boot | 28 | 21 | 13 | 4 | 5 |
| per-object (context, channel, modeset) | 29 | 107 | 22 | 21 | 125 |
| per-allocation | 3 | 0 | 1 | 4 | 2 |
| per-query (NVML, nvidia-smi) | 41 | 17 | 66 | 0 | 48 |
| on-error | 0 | 7 | 1 | 3 | 1 |
| periodic | 0 | 1 | 0 | 0 | 0 |
| never-tu106 / never-sent | 0 | 2 | 0 | 0 | 309 |

No control is per-submission or per-frame in any record: the submission path has no RPC
(optimization-backlog.md §2.2) and the display frames none (§4).

## 4. Where the RPCs are (windows)

| Window | RPCs | Controls | ZT-PAGE | ZT-QUEUE | HOST (n / device ms) | TRAP (n / ms) |
|---|---:|---:|---:|---:|---|---|
| 610 boot | 96 | 55 | 18 | 15 | 13 / 2 | 5 / 0 |
| one CUDA process (bench-init, graphs, oom, ...) | 187 | 45 | 11 | 8 | 12 / 2-5 | 14 / 1-2 (28 with a preempt) |
| torch-matmul | 194 | 52 | 11 | 7 | 22 / 45 | 12 / 1 |
| vulkaninfo | 391 | 277 | 57 | 150 | 50 / 38 | 20 / 0 |
| nvidia-smi -q x10 | 1010 | 910 | 40 | 0 | 830 / 716 | 0 |
| nvidia-smi dmon, 31 s | 568 | 544 | 27 | 0 | 514 / 573 | 0 |
| nvml gate | 145 | 125 | 8 | 0 | 111 / 737 | 0 |
| 615 boot | 272 | 189 | 31 | 18 | 126 | 5 |
| 615 GNOME start | 1352 | 944 | 50 | 724 | 42 | 127 |
| 615 hotplug | 181 | 181 | 2 | 178 | 0 | 0 |
| 615 Weston / Mutter per-frame window, 10 s | 0 / 1 | 0 / 1 | 0 | 0 / 1 | 0 | 0 |
| idle 300 s | 0 | 0 | | | | |

Device ms only in the 610 profiles. The rest of the 187 RPCs per CUDA process are 71
`GSP_RM_ALLOC` and 71 `FREE` (FREE 0.83 ms mean with the mirror teardown, coverage-map.md §4
row 10).

Top per class, by count x (device time + 138 us of traps), all records:

| Class | Control | Seen | Device us | Why it is there |
|---|---|---:|---:|---|
| ZT-PAGE | `CE_IS_DECOMP_LCE_ENABLED` 0x20802A12 | 350 | 1 | per CUDA/Vulkan/display client, not cacheable in the guest (flags 0x154) |
| ZT-PAGE | `CE_GET_FAULT_METHOD_BUFFER_SIZE` 0x20802A08 | 228 | 19 | per channel |
| ZT-PAGE | `INTERNAL_GPU_GET_SMC_MODE` 0x20800A4C | 125 | 4 | per client |
| ZT-QUEUE | `DP_AUXCH_CTRL` 0x00731341 | 1369 | 3 | AUX per modeset and hotplug |
| ZT-QUEUE | `GET_ZBC_CLEAR_TABLE_ENTRY` 0x90960107 | 450 | 33 | per client, this VM's ZBC table |
| ZT-QUEUE | `IS_MODE_POSSIBLE` 0xC3720101 | 356 | 3 | per modeset |
| HOST | GSS-legacy PERF 0x79 0x2080A079 | 12 | 80794 | NVML; 678 ms first call, 21 ms after (coverage-map.md:300) |
| HOST | GSS-legacy CLK 0x04 0x20809004 | 146 | 2259 | NVML, 14 per second in dmon |
| HOST | `PERF_GET_VID_ENG_PERFMON_SAMPLE` 0x20802087 | 128 | 1420 | NVML telemetry |
| HOST | `GPU_GET_INFO_V2` 0x20800102 | 78 | 1292 | per client; host answer with marked indices |
| TRAP | `NVA06C PREEMPT` 0xA06C0105 | 48 | 1504 | host TSG preempt of the mirror |
| TRAP | `INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER` 0x20800A58 | 357 | 3 | pushbuffer address per display channel |
| TRAP | `NVA06C GPFIFO_SCHEDULE` 0xA06C0101 | 180 | 5 | enables a channel the doorbell then rings |

## 5. Mechanism (a): the answer page

Design, ten lines:
1. A per-VM 4 KiB page (more if needed) in the device's memfd, guest-readable, never writable.
2. Header: a version word (odd while the device writes, even when stable), the ABI tag (615.71.09), an entry count.
3. Entries: control id, params size, a flag for "by input" (index-keyed tables), offset of the answer bytes.
4. Filled once at device start from the VM's labelled answers (D9): chip tables, costume/profile (D6), display configuration; the host-static answers after one host read, rewritten to the profile.
5. Device-updated entries (this VM's ZBC table, display connector state) bump the version word; the reader retries on an odd or changed version.
6. No host telemetry, no per-handle state, nothing another VM or the desktop could be read through.
7. The reader: a 20-line hook in `rpcRmApiControl_GSP` (`rpc.c:10655`) before the RPC is built: look up (cmd, size) in the page, copy, return.
8. Fail closed: a missing entry, a size mismatch or an ABI tag mismatch falls back to the RPC.
9. The page is advertised as a sparse mmap area of a BAR the stock driver does not touch.
10. Isolation: read-only mapping, filled only from the VM's own view; the page can only be wrong, never a channel.

What a stock 615 driver can use without modification:
- **Its own control cache.** Controls with `RMCTRL_FLAGS_CACHEABLE` (or `_BY_INPUT`) and no
  access right are cached per GPU after the first reply (`rpc.c:10708`, `:10755`, `:10898`;
  key (GPU, cmd), `rmapi_cache.c:516`; on by default, `rmapi_cache.c:192`). Only 19 routed
  controls qualify (`guest_cache` column); the flags are compiled into the kernel's export tables,
  the device cannot add any.
- **GSS-legacy answers by the device's reply.** For ids outside the export tables the kernel
  caches the reply when its `rmctrlFlags` carry CACHEABLE and no access right (`rpc.c:10762-10770`
  read, `:10899-10905` write). The device sets 0 today (`boot.rs:1524`). This is the one lever a
  stock driver offers the device: 37 ids, the most expensive class (§6).
- **The static configuration of `GMCAPI_CMD_GSP_INIT`** (gsp-transport-615.md §5.3): read once
  at boot; the kernel reads what it reads, the device cannot extend it.
- **The reply side already is a page.** The status queue lives in guest memory
  (gsp-transport-615.md §2.1): the reply element costs no trap. Only the pointer registers do.

What it cannot use: a separate answer page. That needs the hook of step 7, a guest driver
change against D17's "stock driver, bare metal" line. Its gain: the seen ZT-PAGE rows are 11
calls per CUDA process (about 1.5 ms), 31 per 615 boot (4 ms), 50 per GNOME start (7 ms).
**Verdict: not worth a driver change.** The ZT-PAGE verdict still marks the controls whose
answer could be published; with the polled queue they cost about 10 us each.

## 6. HOST: cache, TTL or round trip

| Group | Rows | Seen calls / device ms (profiles) | What can be done | Isolation rule |
|---|---:|---|---|---|
| HOST-STATIC | 31 | 565 / 375 ms | device-side cache per card, filled at the first call and kept for the device's life, rewritten to the profile as today | public-control filter (public.rs) before the first host call; the cache holds only the filtered answer |
| HOST-GSS-LEGACY | 33 (+2 refused, +2 synthesized) | 1136 / 1852 ms | static tables: set CACHEABLE in the reply so the guest caches (stock); telemetry: device TTL cache per NVML burst | only for answers proven identical across calls; never for per-process or per-client data |
| HOST-TELEMETRY | 39 | 566 / 373 ms | TTL cache (100 ms to 1 s), shared by the burst of one `nvidia-smi` call | host-wide telemetry shows the desktop's and other VMs' load: rate limit per VM, TTL bounds the resolution a guest gets |

- **Batching:** the guest sends one RPC at a time under the GPU lock (`kgspRecvPoll`, no
  recursion, `kernel_gsp.c:3320`), so no transport batching. What batches is the host side:
  on the first call of an NVML burst the device fetches the static set once.
- **Round trips that stay:** fresh telemetry beyond the TTL; the per-handle HOST-STATE pair of
  §8 (preempt, timeslice).

## 7. Mechanism (b): the polled queue

Design, ten lines:
1. BAR0 page 0x110000 (the four queue pointers, the falcon registers) becomes a read-only memory area backed by the device's memfd: reads of `MSGQ_HEAD`, `QUEUE_TAIL`, `DEBUGINFO` and the other status words hit memory.
2. Writes still exit (a read-only memslot turns stores into MMIO exits); `QUEUE_HEAD` and `MSGQ_TAIL` get KVM ioeventfds, as the MSI re-arm write already has (optimization-backlog.md §4 status, item 4).
3. A `QUEUE_HEAD` write wakes the doorbell reactor (poll.rs, hybrid or spin; a `Hook` as the interrupt page's, poll.rs:709-714).
4. The eventfd counter is the number of submitted elements (one pointer write per `GspMsgQueueSendCommand`, `message_queue_cpu.c:597-607`, `msgq.c:352-377`); the reactor takes exactly that many elements, each sized by its own MCTP header.
5. Before the first element it runs the BAR2 memfd sync (the GSP doorbell is a sync point, bar2-memfd.md:155).
6. Device-only answers are served on the reactor; HOST and HOST-STATE work goes to a worker, the reply is posted when it returns.
7. Publish: reply element, store fence, then `MSGQ_HEAD` in the page; `QUEUE_TAIL` after consuming.
8. The guest's status-queue read pointer comes from the "SEEN" marks it writes on consumed replies (`message_queue_cpu.c:775`); unknown means not consumed, and replies are held as today.
9. Everything else on the page keeps the value the device maintains; every other write exits as today.
10. Cost per RPC: two KVM exits (1.2 to 1.4 us each, doorbell-poll.md:188), the reactor's reaction, the device's work.

Fail-closed rules:
- The switch needs a polling reactor and a VMM that maps a read-only area and registers
  ioeventfds on it; without either the device keeps the trapped page, never a half mode.
- A `QUEUE_HEAD` write of 0 (reload without reset, gsp-transport-615.md:175) must be seen:
  a datamatch-0 ioeventfd registered before the wildcard one; its signal drops the link.
- A register of the page whose read has a side effect (the falcon IMEM/DMEM data ports
  0x110184 and 0x1101C4, `dev_falcon_v4.h:104`, `:117`; written 128 and 21 times at boot in the
  610 profile, never read) must
  not be read by the guest while mapped; the boot model lists them, and the switch refuses if
  a read of one is ever needed.
- An element that fails the v2 checks (magic, sizes, `msgq/v2.rs`) fails the queue as today;
  the count from the eventfd is never rounded or guessed.
- FLR and reset restore the page's boot values and drop the link.

What it saves (about 130 us per RPC, all RPC functions, not only controls):

| Window | RPCs | Saved |
|---|---:|---:|
| one CUDA process | 187 | about 24 ms (cuCtxCreate is 0.61 s after backlog item 1) |
| `nvidia-smi -q` | 101 | about 13 ms (the host part is 72 ms, §6) |
| vulkaninfo | 391 | about 51 ms |
| 615 boot | 272 | about 35 ms |
| GNOME start | 1352 | about 175 ms |
| per frame, idle | 0 | 0 |

A smaller first step without a VMM change: only the `QUEUE_HEAD` write as an ioeventfd (patch
0005's path); the device serves the pending element inside the guest's trapped `MSGQ_HEAD` read.
One of four traps goes: about 30 us per RPC, 6 ms per CUDA process.

What stays trapped:
- the transport lifecycle: the boot writes to page 0x110000 until the link, `GSP_SUSPEND`, a
  reload, FLR;
- nothing per control. The poll is per queue: every RPC takes it. That is safe for the TRAP
  rows, because the guest waits for the reply either way (§2) and the device validates in order
  before it publishes.

Ordering hazards:
- **Head and tail.** Monotonic u32 counters; ring full at 63 (gsp-transport-615.md:100). The
  device publishes only after the fence, and the kernel reads after a load fence (`msgq.c:408-409`).
- **Interrupts.** The reply path has no interrupt in 615 (the kernel polls). Unsolicited events
  and the interrupt page are unchanged; the reactor runs the interrupt page's hook in the same
  loop.
- **Windows.** The memfd sync before an element (step 5); the doorbell token of a channel is
  assigned at its first doorbell, after `GPFIFO_SCHEDULE`'s reply (doorbell-token-pool.md).

## 8. TRAP: 32 controls, why each group stays synchronous

| Group | Rows | Examples | Reason |
|---|---:|---|---|
| ADDR-VALIDATE | 12 | `DMA_SET_PAGE_DIRECTORY`, `GPU_PROMOTE_CTX`, `INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER`, `INTERNAL_INIT_USER_SHARED_DATA`, `PROMOTE_FAULT_METHOD_BUFFER`, `VASPACE_COPY_SERVER_RESERVED_PDES`, fault-buffer and FBSR registration | guest physical or virtual addresses the device maps or writes: D17/D18 validation before use, in order after the memfd sync |
| WINDOW-ORDER | 13 | `GPFIFO_SCHEDULE` (0xA06C0101, 0xA06F0103), `NVA06F BIND`, `DMA_FLUSH`, `FLUSH_L2_ALL_RAMS`, `RESET_CHANNEL`, `IDLE_CHANNEL`, `PROGRAM_*_PROMOTE`, `DISABLE_USERMODE_CHANNELS` | change what a zero-trap window serves (doorbell tokens, BAR1 pool, BAR2 memfd, display channels) |
| HANDLE-STATE | 5 | `FIFO_IDLE_CHANNELS`, `FIFO_DISABLE_CHANNELS`, `ZCULL_BIND`, `PREEMPTION_BIND`, `DMA_UNSET_PAGE_DIRECTORY` | embedded handles that must resolve to this client's objects |
| HOST-STATE | 2 | `NVA06C PREEMPT`, `SET_TIMESLICE` | act on the VM's host TSG; resolved to the mirror before the host call (D10, D12) |

Never cached, batched, deferred or answered from a page. They ride the polled queue in order.

## 9. REFUSE: 490

- ABSENT 379: 295 E families (nvlink, vgpu, mig, cc, ...), 69 D REFUSE rows, the rest named
  refusals; 296 are never sent on TU106.
- ISOLATION 111: 71 D rows (DENY, or REFUSE with a "high" risk in `d-controls.tsv`), 40
  public-table refusals.
- 12 were sent in a record (for example three `INTERNAL_STATIC_KGR_GET_*` per boot); each costs
  one RPC and stays refused. Details: [gsp-plan/d-device-model.md](gsp-plan/d-device-model.md),
  [gsp-plan/e-families.md](gsp-plan/e-families.md).

## 10. What we can optimise, ranked

| # | What | Class, rows | Expected saving (profile numbers) | Risk |
|---|---|---|---|---|
| 1 | Guest cache for static GSS-legacy answers (reply `rmctrlFlags`), device TTL for the rest | HOST-GSS-LEGACY, 33 | up to 1.85 s over the profiles; PERF 0x79 889 ms in 11 calls; `nvidia-smi -q` 56 ms to a few ms | a frozen value if an id is not static: prove byte-identical answers first (open question 1) |
| 2 | Device cache of static host getters | HOST-STATIC, 31 | 375 ms; 2-4 ms per CUDA process, 31 ms per vulkaninfo, 38 ms per 10 `nvidia-smi -q` | stale only across a host driver change; keep the public filter before the fill |
| 3 | TTL cache for telemetry | HOST-TELEMETRY, 39 | 373 ms; dmon 203 ms in 31 s | resolution a guest sees; rate limit per VM |
| 4 | Polled queue, full (b) | all RPCs | about 130 us per RPC: 24 ms per CUDA process, 13 ms per `nvidia-smi -q`, 175 ms per GNOME start | VMM read-only area and ioeventfds; reactor core |
| 5 | Polled queue, head ioeventfd only | all RPCs | about 30 us per RPC: 6 ms per CUDA process | low; patch 0005 path exists |
| 6 | FREE answered before the mirror's host teardown | RPC `FREE`, not a control | 0.83 ms mean x 71 per CUDA process (coverage-map.md §4 row 10) | host object freed after the guest handle is gone; the device namespace must refuse reuse until then |

Top ten single controls by expected saving:

| Control | Mechanism | Saving over the records |
|---|---|---|
| GSS-legacy PERF 0x79 0x2080A079 | guest cache or TTL | about 800 ms (11 calls, first call kept) |
| GSS-legacy CLK 0x04 0x20809004 | TTL | up to 321 ms (142 calls) |
| `PERF_GET_VID_ENG_PERFMON_SAMPLE` 0x20802087 | TTL | up to 176 ms (124 calls) |
| GSS-legacy PERF 0x97 0x2080A097 | guest cache or TTL | up to 108 ms (81 calls) |
| `GPU_GET_INFO_V2` 0x20800102 | device cache | about 97 ms (75 calls) |
| `GPU_GET_BRAND_CAPS` 0x00800294 | device cache | about 77 ms (18 calls) |
| `CE_GET_ALL_PHYSICAL_CAPS` / `CE_GET_PHYSICAL_CAPS` 0x20802A0B / 0x20802A07 | device cache | about 68 ms (81 calls) |
| `FB_GET_INFO_V2` 0x20801303 | device cache | about 47 ms (96 calls) |
| `DP_AUXCH_CTRL` 0x00731341 | polled queue | about 178 ms of traps (1369 calls, display) |
| `CE_IS_DECOMP_LCE_ENABLED` 0x20802A12 | polled queue | about 46 ms of traps (350 calls) |

## 11. What we cannot

| What | Rows | Reason |
|---|---:|---|
| REFUSE | 490 | feature absent on TU106, or the effect would reach shared host hardware (D16, d-controls risk) |
| TRAP | 32 | addresses (D17/D18 validation), handles, window order, host TSG state (D10, D12): synchronous by necessity |
| HOST round trips | fresh telemetry, HOST-STATE | the value lives on the host; a cache only bounds how often |
| an answer page a stock driver reads | 101 ZT-PAGE | no read path without a driver change (D17); worth about 1.5 ms per CUDA process |
| a per-control trap/poll choice | all | the queue is one ring with one pointer: the transport is per queue |
| the transport lifecycle | boot, unload, reload, FLR | falcon data ports and the link protocol need every write served in order |
| the RPC count itself | 187 per CUDA process | the stock driver decides it; the device answers only faster |

## 12. Open questions

1. Which GSS-legacy answers are byte-identical across calls and processes? A profile run with
   reply hashes per id (NVML gate, `nvidia-smi -q` x10) decides row 1.
2. Does KVM deliver a store to a `KVM_MEM_READONLY` slot to an ioeventfd? The host's KVM sources
   are not here; extend `caraxes/tools/kvm-ioeventfd-probe.c` (doorbell-token-pool.md) to measure
   it before any VMM patch.
3. Does Cloud Hypervisor map a vfio-user sparse area read-only (a new patch after 0006)?
4. The 615 per-control profile: these frequencies come from the 610 guest for CUDA and NVML; one
   615 coverage run (coverage-map.md §5 recipe) replaces them.
5. The reactor's reaction time when blocked in hybrid mode (up to `sleep_ms`, intr_page.rs:68)
   against the ioeventfd wake-up: measure the RPC latency with the switch on.

## 13. Reproduce

```sh
python3 caraxes/tools/gsp-transport.py                    # writes caraxes/abi/gsp-615.71.09/transport.tsv
python3 caraxes/tools/gsp-transport.py --summary          # the tables of §3, §4 and the tops per class
python3 caraxes/tools/gsp-transport.py --rpc-traps \
    caraxes/runs/display-615-2026-09-24/s4-series/traps-weston/device-trace.txt.xz
```

The tool needs the 615.71.09 tree (`--ogkm`, `CARAXES_OGKM_615`, or the Leandro checkout beside
the repository, as `tools/check.sh` finds it). It runs in about 10 s, standard library only.
