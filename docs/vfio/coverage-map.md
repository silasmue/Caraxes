<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Coverage map: which device paths real workloads reach, and what they cost

Written 2026-09-25 for the operator's question "how do we find functions that are not easy to
test, which we could optimise?". The answer is a method with three parts:

1. an instrument that is cheap enough to leave on for any workload: the device's **profile**
   (`CARAXES_PROFILE=1`, §1);
2. a **map** of the device's paths against real workloads, one profile per workload, with
   LLVM function coverage beside it (§2);
3. **probes** for what no workload reaches (§3).

§4 ranks the optimisation candidates found this way, and §5 is the recipe for the next workload.

Records: `caraxes/runs/coverage-2026-09-25/` (README there). Tools: `caraxes/tools/profile-rank.py`
(new), `covwork.c`, `covtorch.py`, `ctlsweep.c` and `ctlsweep-gen.py` (new).

## Summary

- **The instrument.** The profile costs 84 ns per trapped access, 26 ns per control and 38 ns
  per window hit on this host (`examples/profile-cost.rs`). A trap costs the guest 23 to 35 us
  (23 to 27 us on a quiet host, mapping.md section 6 `bench-trap.c`; 28 to 35 us as the per-class
  medians under the other VMs' load, optimization-backlog.md section 4),
  so the profile adds about 0.3 %. With the profile off, a hook is one relaxed load (0.3 ns).
  short-auto with and without the profile is within the noise that the other VMs' GR work
  causes (§1.3).
- **The map: 29 workload windows on vm2** (25 in `map/`, 4 in `extra/`), the gates and benchmarks included, and a guest control sweep.
  - **Controls:** 208 of the 610 guest's 829 GSP-routed controls were reached, 97 by the
    workloads and 111 more by the sweep.
  - **Host-forwarded controls:** all 70 were reached.
  - **Untouched controls the device answers:** 42 synthesized, 59 guest-derived, 48 echo and
    427 refused.
  - **Device functions:** 931 of 1464 functions ran under a real workload, 392 run only in
    unit tests, and 141 run nowhere (§2.2).
- **The cost the benchmarks do not show.** Every CUDA process pays about 0.55 s of device time
  that no benchmark row names:
  - 512 BAR1 pool pages moved in and 512 moved out (363 ms median per process);
  - 834 BAR2 memfd syncs (180 ms).

  Outside the benchmarks:
  - host TSG preempts, mostly from the pool's warm-move barrier: 1112 calls, 3.9 s. At a
    kill -9 with a kernel running, one preempt took 3.2 s and the host logged Xid 109;
  - nvidia-smi's GSS-legacy PERF control takes 21 to 678 ms per call;
  - idle interrupt forwarding runs at 331 MSIs per second.

  §4 has the top ten.
- **Findings.**
  - `cuMemAllocAsync` (the stream-ordered allocator) is refused with 801, like
    `cuMemAllocManaged`.
  - A kernel storing to an unmapped address gives the guest `CUDA_ERROR_LAUNCH_FAILED` (719),
    not `ILLEGAL_ADDRESS` (700). The device posts no RC event; the host logs Xid 31 on the
    device's pid.
  - kill -9 of a process with a running kernel costs 3.2 s of host preempts, most of it in one
    call, and a host Xid 109 (CTX SWITCH TIMEOUT) on the device's pid. The next process works in both cases.

## Status

Follow-up of 2026-09-25, records `caraxes/runs/coverage-followup-2026-09-25/`:

- **§3, the 21 unanswered sweep getters: done.** Each has an answer source
  ([gsp-surface.md](gsp-surface.md), "After the sweep getters of the coverage map"):
  - 7 go to the host as plain forwards (`FORWARD`);
  - 5 come from the chip table (`chips/tu106/controls.toml`);
  - 9 are refused as absent features.

  The sweep on vm2 again: no control is `unanswered`, and 39 NV_OK answers against 31 before.
  Routed controls unhandled: 66 to 45 (610) and 67 to 46 (615).
- **§4 item 8, a cache for static forwarded answers: done, and smaller than hoped.**
  - `src/fwdcache.rs` keeps the first host answer of 22 static per-card getters (identity,
    capabilities, floorsweeping) for the device's lifetime; `CARAXES_FORWARD_CACHE=0` turns
    it off.
  - It removes their repeat host calls: 93 to 12 in 10 x `nvidia-smi -q`, 160 to 0 in 20 x
    `--query-gpu`, 10 to 0 in dmon.
  - That is 2.8 ms of about 83 ms of device time per `nvidia-smi -q` on a quiet host.
  - The GSS-legacy PERF 0x79 and the other perf and clock commands that dominate stay live:
    their layouts are not in the open headers, and 0x79 moves between runs.

    Their cost follows the host's load (perf 0x79 624 ms on the first call on a loaded host,
    at most 23 ms on a quieter one). A short TTL for them is still open.
- Gates after both: cuda 10 of 11 (cuda-managed 801), nvml power N/A only (the 610 guest);
  0 host Xid.

## 1. The instrument: the profile

### 1.1 What it counts

`CARAXES_PROFILE=1` makes the device aggregate in memory what the trace writes line by line
(crates/vfio-user-nvgpu/src/profile.rs:1-30). Each row has:
- a count;
- the device's wall time;
- the largest single time;
- a status histogram (at most 8 statuses, the rest in one bucket);
- the answer's label.

| Kind | Key | Where it is counted |
|---|---|---|
| access | region, R/W, 4 KiB page, dword register | `SharedDevice::region_read/write`, device.rs:255-282. Two times: the device's time, lock wait included, and the guest-seen time. The guest-seen time is rpc-cost.py's attribution: the time since the previous trapped access ended, when under 2 ms (profile.rs:264-320). BAR1 and BAR2 are keyed by page only; a BAR0 page folds into a page row after 64 registers (the PROM window at boot) |
| rpc | GSP RPC function, from receipt to reply | boot.rs:1844 |
| control | `GSP_RM_CONTROL` command, with status and label | boot.rs:1497 |
| alloc | `GSP_RM_ALLOC` class, with status and label | boot.rs:1579 |
| event | unsolicited event or GMCAPI request | boot.rs:323 |
| host | every host RM ioctl of the process: escape, control command or class, RM status, time | nvrm-abi's observer in `NvDevice::ioctl_raw` (crates/nvrm-abi/src/lib.rs); profile.rs:670 |
| shadow | the shadow's host RM calls by rpc-cost.py's kinds (map, unmap, object alloc and free, invalidate, gather alloc and free) | shadow.rs `RmCalls::add` |
| window | token pool rings, doorbell and re-arm eventfd wakeups, BAR2 memfd syncs and walks, BAR1 pool moves and barriers, interrupt raises and MSIs; the interrupt and display pages' scans and applied stores, read from their own counters | device.rs, bar2area.rs, bar1pool.rs, intr.rs; main.rs registers the page counters |

**Scopes.** There are two:
- the device lifetime;
- each guest RM client, from its root allocation (`GSP_RM_ALLOC` of `NV01_ROOT`) to its free
  (`FREE` of the client handle), boot.rs:1588 and :1633.

In the stock driver a client scope is one per process that opens the GPU. A client's rows are
the device rows at its free minus those at its allocation. So a client scope holds everything the
device did while that client lived, including other clients' work.

**Dumps.**
- SIGUSR2 writes `<prefix>.<n>.tsv`, and exit writes `<prefix>.exit.tsv`.
- The prefix is `CARAXES_PROFILE_OUT`, else `gpu-profile` beside the trace.
- SIGUSR2 is blocked in every thread before the first one starts, and a dump thread `sigwait`s
  for it (main.rs `serve`, profile.rs:512-600).
- A workload's profile is the dump after it minus the dump before it.
- The trace is unchanged and independent.

`CARAXES_PROFILE_HOST_TIMELINE=1` additionally retains up to 8,192 completed
host A612/A613 ioctl intervals and writes `<prefix>.<tag>.host.tsv`. It needs
`CARAXES_PROFILE=1`. The buffer is absent by default. Snapshots overlap;
deduplicate by process instance and sequence, and collect both TSVs after the
dump completes. See [timing scope and calibration](../research/idle-and-host-timeline-2026-10-09.md#exact-host-ioctl-timeline).

### 1.2 The tool

`caraxes/tools/profile-rank.py DUMP [--base DUMP] [--scope device|clients|client:0x..]` ranks rows
by count times mean latency, that is by total time:
- the guest-seen time for trapped accesses;
- the device time for the rest.

Each row is joined with what explains it:
- controls with `controls.tsv`: name, category, provenance, `answered_by`, privilege and route;
- classes, RPCs and events with `classes.tsv`, `rpc.tsv` and `events.tsv`;
- accesses with rpc-cost.py's access class;
- the handler module, a file under `crates/vfio-user-nvgpu/src`.

Other options:
- `--seen-in` marks the controls that none of the given reference profiles sent;
- `--clients` lists the client lifetimes;
- `CARAXES_PROFILE_ABI=gsp-610.57.04` joins with the 610 guest's tables. This map does that; the
  default is 615.71.09.

### 1.3 Cost

| Measurement | Result |
|---|---|
| per hook, `examples/profile-cost.rs`, 2 million calls | trapped access 84 ns (0.34 % of a 25 us trap), control 26 ns, window hit 38 ns, profile off 0.3 ns |
| client scope open and close, 601 rows | 81 us and 32 us. The map's device scope had 3855 rows at exit, about 0.5 ms per snapshot, once per process |
| dump | 0.5 ms and 51 KB for 601 rows. The map's dumps grow with the closed client scopes, 0.5 to 1.5 MB |
| short-auto, profile off against on, vm2 in the map's configuration | ms per launch (100 launches of a 1000-cycle kernel), median (min) over 10 reps, plain release build: off 0.021 (0.017), on 0.022 (0.013) (`overhead-off-2/`, `overhead-on-2/`). The first pair, 5 reps each: off 0.020 (0.018), on 0.075 (0.021). Two of its reps per side were at 4.9-5.0 ms, while another VM held the GR engine. Within noise |

The first profile build keyed every BAR0 register. The boot's PROM reads made 33351 BAR0 read
rows and 51131 rows in all (`overhead-on/`). The per-page fold (profile.rs:274-297, with a unit
test) came from that run, and the `aborted-map/` run was stopped for it.

## 2. The map

### 2.1 Runs

The environment is `caraxes/runs/coverage-2026-09-25/run.sh`:
- vm2, 610 guest, 1 GiB at reservation slot 3;
- the night environment (`caraxes/runs/night-2026-09-24/env.sh`) plus the recommended
  configuration: interrupt page, token pool 64, hybrid reactor, BAR1 pool 32 MiB,
  `CARAXES_BAR2_MEMFD=1`, `CARAXES_MSI_REARM_IOEVENTFD=1`, `CARAXES_ALLOC_LAZY=1`;
- Cloud Hypervisor 0001-0006;
- the device built from this branch with `-C instrument-coverage` (target/cov-instr), so the LLVM
  counters run beside the profile. The map's times therefore include the counters' overhead.

The other VMs were up throughout: vm0, vm3, vm5 and vm7, with vm1's VMM. Their GR work shares the
engine by timeslice (optimization-backlog.md §3.4).

| Part | Workloads | Host Xid on the device's pid |
|---|---|---|
| `map/` | boot, gates nvml and cuda, benchmarks (bench-cu init, short-auto, rlprobe, PPO 30 s), nvidia-smi -q x10, dmon/pmon/query loops, CUDA graphs, streams, cudaMallocManaged, texture and surface objects, zero-copy host memory, device queries, torch matmul/conv/attention, two processes, OOM, NVRTC, Vulkan, idle 300 s, kill -9 mid-kernel, illegal address, a query after the errors | 2: Xid 109 at the kill -9, Xid 31 at the illegal store |
| `extra/` | what `map/` could not run: streams and two processes without the refused `cuMemAllocAsync`, NVRTC with its builtins on the library path, the control sweep (§3) | 0 |

Not run:
- **cuda-gdb, nvcc, ffmpeg (NVENC/NVDEC) and clinfo:** not in the image (`map/inventory.txt`).
  The NVENC and NVDEC capability controls still arrive, from Vulkan's device creation.
- **OpenCL:** no ICD loader in the image.
- **GNOME and vkcube:** vm2 has no display. Mutter's per-frame traps are in intr-page.md §9-10.

### 2.2 (a) Per workload

The full tables are in `combined-coverage-map.md`, from `coverage-map.py map extra` with the 610
tables: top rows per workload, controls sent in no gate's window, and all counts. The shape per
workload:

| Workload | Device cost, ms | What dominates | Controls no gate sends |
|---|---|---|---|
| boot | 13264 | host `RM_ALLOC_MEMORY` of the reservation (18 calls, 10.0 s), the SEC2 and GSP falcon boot registers | 39 (static info, internal init) |
| gate-nvml | 2361 | GSS-legacy PERF 0x79 forwarded to the host: one call, 678 ms | - |
| gate-cuda | 62009 | memfd syncs 9.5 s, flush reads 8.4 s, pool moves 12.5 s, BAR1 window remaps 3.6 s (11261 `RM_MAP_MEMORY`) | - |
| each CUDA process: bench-init, graphs, texsurf, zerocopy, oom, nvrtc, ... | 1200-1700 | pool moves 1024 pages (363 ms median), memfd syncs 834 (180 ms), flush reads (190 ms), mirror channel and object allocations | 0 |
| smi-q, 10 x nvidia-smi -q | 3051 | 910 controls through the GSP queue: 0.8 ms each; PERF 0x79 21 ms each | 0 |
| smi-dmon | 2252 | CLK 0x04 (2.3 ms, 98 calls), `PERF_GET_VID_ENG_PERFMON_SAMPLE` (1.4 ms, 80), PERF 0x97 (1.3 ms, 69) | 1: `PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2` |
| vulkan (vulkaninfo --summary) | 979 | memfd syncs and flush reads; 131 cold pool moves each way | 14: NVENC/NVDEC caps, ZBC table, CE caps, `GPFIFO_SCHEDULE`, BIND, ZCULL bind |
| idle, 300 s | 287 | 115318 host `RM_GET_EVENT_DATA` drains, 108611 raises, 99339 MSIs, 99022 guest re-arms through the eventfd: other VMs' work | 0 |
| kill9 | 7273 | 58 host TSG preempts (56 from 21 pool barriers, 2 forwarded from the guest): 3.2 s, one of them 3.2 s long; host Xid 109 | 0 |
| illegal | 1517 | as any process; guest status 719; host Xid 31 | 1: `NV83DE_..._DEBUG_READ_ALL_SM_ERROR_STATES`, refused |
| ctlsweep | 107 | 111 controls through the GSP queue | 111 |

### 2.3 (b) The union, and the untested surface

Controls of `caraxes/abi/gsp-610.57.04/controls.tsv` (the guest's version; 615 has the same
provenance columns) that reach GSP:

| Provenance | Answered by the device | Touched, map | Touched, map and sweep | Untouched |
|---|---|---|---|---|
| synthesized | yes | 26 | 38 | 42 |
| host-forwarded | yes | 40 | 70 | 0 |
| guest-derived | yes | 8 | 15 | 59 |
| echo | yes | 16 | 16 | 48 |
| refused | yes (a named refusal) | 7 | 48 | 427 |
| unhandled | no | 0 | 21 | 45 |

35 GSS-legacy and BINAPI ids outside the export tables were seen as well: thermal, clk, perf,
pmgr and the two cuInit 0x81 ids.

The per-id lists are the `untouched` field of `combined-coverage.json`.

Other surfaces:
- **RPC functions:** 10 were exercised: SET_GUEST_SYSTEM_INFO, FREE, DUP_OBJECT,
  GET_GSP_STATIC_INFO, UPDATE_BAR_PDE, GSP_SET_SYSTEM_INFO, SET_REGISTRY, GSP_RM_CONTROL,
  GSP_RM_ALLOC and INIT_GSP_TRACE_CRASH_BUFFER. Not exercised: IDLE_CHANNELS,
  GPU_EXEC_REG_OPS, DUMP_PROTOBUF_COMPONENT, ECC_NOTIFIER_WRITE_ACK, MAP/UNMAP_MEMORY_DMA and
  UNLOADING_GUEST_DRIVER (boot.rs:1678-1760).
- **Unsolicited events:** no event went through `post_unsolicited` (boot.rs:284) in any
  workload. GSP_INIT_DONE at boot is sent elsewhere (boot.rs:1877) and is not counted.
  POST_EVENT, RC_TRIGGERED and OS_ERROR_LOG (events.rs) did not fire, not even for the illegal
  address, and the GOE path of rc.rs was not reached.
- **Allocation classes:** 21 were exercised, among them the debugger `GT200_DEBUGGER`, INFOROM and 2D.
- **Host RM calls:** 112 kinds by escape and key; every shadow kind; 21 window counters.

The device's own code, by LLVM function coverage (`llvm-functions.md`, `.tsv`, from `llvm-cov.sh`):

| | Functions |
|---|---|
| run by a workload and by unit tests | 690 |
| run by a workload only | 241 (hostmem.rs 55, mirror.rs 49, shadow.rs 23, device.rs 18) |
| run by unit tests only | 392 (display 97 on a display-less vm2, 615 transport: msgq/v2 23, gmcapi 27, chip/boot_fsp 21, gmmu_v3 12) |
| run by neither | 141 |
| all | 1464, 53.7 % of the crate's lines ran under the workloads |

Notable functions no workload and no test ran (`llvm-functions.tsv`):
- **mirror.rs**, 17: `idle_channels`, `set_scheduling`, `channel_query`, `mmu_fault_info`,
  `on_page_directory`, `drop_vas`, `plans_barrier`, `promote_gr`;
- **public.rs**, 8: the mirror-forwarding objects `MirrorFwd::forward`, `idle`, `schedule` and
  `channel_query`;
- **invalidate.rs**, 6: the whole `Invalidator`;
- **hostmem.rs**, 12: `release_region`, `map_reservation`, `alloc_virtual`, the admin paths;
- **device.rs**, 14: `dma_unmap`, `reset`, `remove_guest_ram`, the token-less doorbell reader;
- **rc.rs**: `MirrorRef::forward`.

These are the paths that are "not easy to test": they need an event the workloads do not cause (a
reset, a DMA unmap, a host fault, a mirror query) and have no unit test.

### 2.4 (c) Candidates

§4.

## 3. Probes for the untested surface

**The control sweep** (`caraxes/tools/ctlsweep.c`, table from `ctlsweep-gen.py`):
- **The table.** It is generated from controls.tsv and restricted to controls that:
  - reach GSP;
  - user space may call: non-priv, or priv run as root;
  - are getters by name;
  - live on an object the probe makes: client, device, subdevice, DISPLAY_COMMON,
    SUBDEVICE_DIAG, I2C or ZBC_CLEAR;
  - `--only` limits it to the ids a coverage run left untouched.
- **What it does.** In the guest, through the stock RM ioctl path, it issues each control once
  with zeroed parameters of the control's size, on its object. The setup is Leandro-Test's
  `rmdirect.c`: version handshake, root client 0x41, attach, device and subdevice.
- **The run.** Generated from `map/`'s untouched list with `--priv`:
  - 172 controls; 119 issued (DISPLAY_COMMON is refused on vm2, 53 controls);
  - **111 reached the device**; the guest kernel answered 8 before GSP;
  - the device's answers: 28 refused as absent on the chip, 27 forwarded to the host, 21
    unanswered, the rest synthesized, chip data or guest state (`sweep-reach.md`).

  With the profile on, the sweep's window shows each one arriving as `GSP_RM_CONTROL`.
- **Walls found.** The 21 unanswered user-callable getters are the next walls, for example
  `GPU_GET_IP_VERSION`, `GET_P2P_CAPS`, `FB_GET_FS_INFO`, `GR_GET_GPC_TILE_MAP`,
  `FB_GET_COMPBIT_STORE_INFO` and `FLCN_GET_ENGINE_ARCH` (`extra/work/ctlsweep-table-out.txt`,
  provenance `unhandled`).

**What stays untouched, and how to reach it** (`sweep-reach.md`):

| Provenance | Untouched | User space, non-priv | As root (priv) | Needs an object the sweep lacks (channel, profiler, debugger, display) | Guest kernel only (internal, kernel) |
|---|---|---|---|---|---|
| synthesized | 42 | 15 | 12 | 7 | 8 |
| guest-derived | 59 | 11 | 7 | 18 | 23 |
| echo | 48 | 5 | 17 | 8 | 18 |
| refused | 427 | 51 | 96 | 73 | 207 |

- **User space.** The user-space rest are mostly setters, which the sweep leaves out because they
  change state, and the display getters (vm2 has no display engine). They need:
  - a setter sweep with read-back pairs;
  - the sweep on a display VM;
  - a channel and a profiler session in the probe, for the object column.
- **Guest kernel only.** The internal and kernel-privileged ones cannot come from user space:
  - 207 refused ones are the E families: nvlink, vgpu, notebook power, gsync, mig, cc;
  - 49 answered ones are display internals, `ctrl:D table` rows and echo acknowledgements.

  They belong in unit tests in `ctrl.rs`, `public.rs` and `display/mod.rs`, as the gsp-plan's
  per-family tests do.
- **Code no control reaches.** The mirror's query and scheduling paths, `Invalidator`,
  `dma_unmap` and `reset` need:
  - events: a guest reboot, a VM memory unplug, an IDLE_CHANNELS RPC (the guest sends it on
    channel teardown only with `RmRcWatchdog`), a host MMU fault on a UVM channel;
  - or unit tests against the fake host of `shadow.rs` and `bar1pool.rs`.

## 4. Candidates: high cost outside the benchmarks

Ranked by device time in the non-benchmark workloads (`combined-coverage-map.md` (c)), and by
whether the benchmarks show the row at all. Per process means per CUDA process: each of graphs,
texsurf, oom and the others is one.

| # | Path | Measured | In the benchmarks | Module | Direction |
|---|---|---|---|---|---|
| 1 | BAR1 pool page moves at every context: 512 pages in (cold) and 512 out (warm) per process | 21123 + 19456 moves, 20.2 s outside the benchmarks; 363 ms median per process; 318-570 us per 4 KiB page | yes, but no benchmark row names it | bar1pool.rs:828 and :854 through `FakeFb::copy_page_raw`, fb.rs:1664: uncached reads through the BAR1 windows | copy through the reservation's CPU mapping with write-combining, or by the copy engine; keep a context's pooled pages warm across processes instead of moving them out at teardown |
| 2 | BAR2 memfd syncs and flush reads | 48209 syncs, 14.1 s; 28524 flush reads, 12.5 s guest-seen; 834 syncs (180 ms) per process | 1.5 s | bar2area.rs:773-830, fb.rs | backlog item 3 (sync limited to reachable pages, the flush page as memory), measured per workload here |
| 3 | Host TSG preempts: the pool's warm-move barrier, and the guest's own TSG_PREEMPT forwarded to the mirror | 1112 `NVA06C PREEMPT`, 3.9 s, mean 3.6 ms. At the kill -9 there were 58 calls: 56 from 21 barriers and 2 forwarded (463 ms each). The longest single call took 3.2 s, the host's ctx-switch timeout, with host Xid 109 | 83 ms | bar1pool.rs:591, mirror.rs:733; ctrl.rs `MIRROR_FORWARDED` | no barrier for pages of a VA space being torn down; one preempt per TSG per batch |
| 4 | The shadow's gather allocations at context creation | 1194 calls, 4.5 s, mean 3.8 ms (5.3 ms at kill -9) | 0.31 s | shadow.rs, hostmem.rs:1473 | the region free list (alloc-path.md:158) for gathers too; gathers larger than 512 pages |
| 5 | The device's own BAR1 window remaps (window misses) | 11792 `RM_MAP_MEMORY` (324 us) + `REGISTER_FD` (81 us) + `RM_UNMAP_MEMORY`, 5.4 s, almost all in the cuda gate | 10 ms | hostmem.rs:189-215 (`map_window` opens a new fd per map) | keep the mapping fd, larger windows; ReBAR removes it (optimization-backlog.md §5) |
| 6 | Host channel and engine objects per context | CE 3.6 ms, GPFIFO 3.1 ms, COMPUTE 4.8 ms, TSG 2.4 ms each; 823 + 823 + 331 + 150 allocations, 7.3 s | 0.6 s | mirror.rs | allocate at first doorbell only for channels the context uses (20 channels are made, compute uses few); pool host channels across processes |
| 7 | GSP RPC service on the trapped queue-head write | 10132 traps, 3.7 s guest-seen, 364 us each; nvidia-smi's controls 0.5 to 0.8 ms each | 0.25 s | boot.rs `service_queues`, msgq.rs | backlog item 6 (poll-mode GSP); answer static controls without the host call |
| 8 | GSS-legacy PERF 0x79 and CLK 0x04 forwarded to the host | PERF 0x79: 678 ms on the first call, 21 ms after (11 calls); CLK 0x04 2.3 ms (142 calls); PERF 0x97 1.3 ms | 0 | ver/v610.rs:1033-1041, public.rs forward | cache the static tables for a short TTL: one nvidia-smi -q then costs about 1 ms instead of 21 |
| 9 | Idle interrupt forwarding | 99339 MSIs and 115318 host event drains in 300 s idle (331/s and 384/s, other VMs' work), 99022 guest re-arms | idle is not a benchmark | intr.rs:68-118, hostmem.rs:332-345 | backlog item 5 (progress-gated forwarding), now at 6x the census's 55/s; the idle rate follows the other VMs' load (55/s census, 102/s in the ioeventfd run of 2026-09-25, 331/s here), so the three are different conditions, not a regression |
| 10 | FREE RPC with the mirror's teardown | 3064 FREEs, 2.5 s, mean 0.83 ms; 10020 host `RM_FREE`, 3.3 s | 0.46 s | boot.rs:1625-1640, mirror.rs `free_object` (:2393) | free host objects in the worker after the reply, as the shadow's plans already are |

Items 1, 3, 5 and 8 are new: the census could not see them. They are not traps, and they sit in
teardown, the pool or nvidia-smi.

## 5. The recipe for the next workload

For SotTR, Steam or a browser:

1. Start the VM with the configuration under test plus `CARAXES_PROFILE=1`, and
   `CARAXES_PROFILE_OUT=<run>/profile/dev`.
2. `kill -USR2 $(cat $W/gpu.pid)` before the workload; wait for `dev.1.tsv`.
3. Run the workload.
4. `kill -USR2` again (`dev.2.tsv`).
5. `profile-rank.py dev.2.tsv --base dev.1.tsv --top 30`: the ranked rows with provenance and
   module.
6. Add `--seen-in <gate dumps>` to list the controls no gate sends. Those are the walls and the
   untested answers of this workload.
7. Add `--scope clients` or `--clients` to see per process: which process caused the cost.
8. For the untouched surface, compare with `combined-coverage.json`, and extend
   `ctlsweep-gen.py --only`.
9. For device code no path reaches, rebuild with `-C instrument-coverage`, set
   `LLVM_PROFILE_FILE`, and run `llvm-cov.sh report`.
10. Keep the dumps and a `marks.tsv` beside the host Xid lines of the device's pid. The
    run.sh of this series is the template.
