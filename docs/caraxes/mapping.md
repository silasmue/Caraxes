<!-- SPDX-License-Identifier: MIT -->
# Mapping package: incremental shadow, coalesced maps, worker, isolation fixes, doorbell study

Written 2026-09-24 for the mapping package. The package came out of the benchmark (benchmark.md
§5 and §7) and the parallelism proof (parallel.md §8). Evidence is in
`caraxes/runs/mapping-2026-09-24/`, one directory per step. `commands.txt` there lists every
command in order. The code is in `crates/vfio-user-nvgpu` (`ptcache.rs`, `shadow.rs`,
`mirror.rs`, `fb.rs`, `hostmem.rs`, `trace.rs`), `tools/caraxes-vm.sh` and
`caraxes/tools/bench-hostwatch.sh` and `bench-trap.c`. The checkpoint is commit `b58cb12`.

All runs used vm2 alone at reservation slot 0 with a 2 GiB profile and 4 GiB of guest RAM,
in the green configuration of testing.md §7b (`caraxes/runs/mapping-2026-09-24/env.sh`). The
display agent held slot 1 on vm0 the whole time. The baseline for the step-by-step numbers
is the device at `718a311` on the same configuration (`a0-baseline`). Those steps are single
runs (n=1) on a host that also carried vm0 and later vm3, so small differences are noise.
The n=5 numbers are in benchmark.md §8.

## Summary

| What | 718a311 (a0) | now | Where |
|---|---|---|---|
| cuMemAlloc 1 MiB / cuMemFree 1 MiB | 18.9 / 20.8 ms | 0.90-1.2 / 0.90-1.2 ms | a4, d3 |
| cuMemAlloc 64 MiB / cuMemFree 64 MiB | 61 / 54 ms | 6.4-10 / 4.6-7.3 ms | d2, d3 |
| device CPU for the churn run | 80.1 s | 6-7.4 s | a0, d2, d3 |
| matmul N=2048 | 781 GFLOP/s | 2440-2520 GFLOP/s | a1, d2 |
| short-kernel launch (nvprobe 4, 1000 cycles) | 0.457 ms | 0.12-0.19 ms | c0, d3 |
| cuCtxCreate / cuCtxDestroy | 5.4 / 3.4 s | 2.7-4.0 / 0.9-1.7 s | a4, d2, d3 |
| server-thread time per resync, p50 / p90 / p99 | 1.7 / 14.6 / 506 ms (walk plus RM calls, d1) | 0.04 / 0.18 / 2.4 ms (walk only, d2) | d1, d2 |
| RM mappings for the P20 async_streams suite | 555k (one per leaf) | 30k for 556k leaves, 18.9k live | b1 |
| sysmem leaves refused as chunk-crossing (async_streams) | 128 | 0 | b1, d1 |
| channels walking unfiltered guest tables | every CE channel | none | d1 |
| two VMs at once: cuda gate, p20-check, pattern test | P20: scored tests pass, suites stall the host RM | scored tests pass, async_streams passes on the 2 GiB VM, check PASS, 0 Xid | f1 |

What did not reach the operator's targets, and why:

- The submission cost is about 4 trapped MMIO writes of about 25 us each.
- A context creation is about 96k trapped 8-byte BAR2 writes by the guest RM.
- Neither is device work: the device's share of a trap is 1-2 us.

§3 and §6 give the accounting and the paths that remain: P21, and a Cloud Hypervisor change,
patch 0005.

## 1. Incremental shadow updates

**Before.** Every resync whose guest root had a written table page walked the whole tree
through the FB store. Every table read was an uncached BAR1 read of about 0.8 ms per 4 KiB page.
Then the whole leaf map was diffed. A CUDA context of 15k leaves cost about 30 ms per walk, and
the async_streams suite (545k leaves) about 0.5-0.8 s.

**Now** (`ptcache.rs`, `FakeFb::walk_update`). Each guest root has a `WalkCache` holding:

- a copy of every directory table it read;
- the leaves of every 2 MiB unit (one PD0 entry);
- for every table page in VRAM, the nodes that read it.

`FakeFb::write` sees every guest write to VRAM, because BAR1, BAR2 and PRAMIN are trapped. When
a write lands in a watched page, it patches that page's copy and marks the page dirty. An update
then works from the dirty pages only:

- A changed directory entry drops the old subtree and walks the new one.
- A changed page table recomputes its unit.
- Only pages the cache has never seen are read from VRAM.

The result is the list of units whose leaves may have changed. The mirror diffs only those
units.

**Correctness.** The cache relies on the same assumption as the earlier walk skip: every write
to a table page goes through the FB store. A GPU engine never writes guest page tables here. The
UVM and kernel channels that could run in software, and CUDA's user channels cannot reach
tables.

Two settings give references:

- `CARAXES_SHADOW_REREAD=1` drops a dirty page's copy and reads it again from VRAM.
- `CARAXES_SHADOW_ALWAYS_RESYNC=1` clears all copies and walks and diffs the whole tree at every
  request.

A tree with a table in guest sysmem cannot be watched, so it is walked in full every time, as
before. The cache holds at most 4M leaves per root and marks itself truncated beyond that. Unit
tests compare the incremental result with a full walk after table changes
(`ptcache::tests`, `fb::tests::enumerate_leaves_sees_every_page_size_once`).

**Bulk BAR1 reads.** `Window::read` now uses 16-byte streaming loads (`movntdqa`) for blocks of
64 bytes and more. The walk's p99 on the server thread dropped from 34 ms (d1) to 2.4 ms (d2).
Most of that is the first read of freshly allocated table pages.

**Measured.** With coalescing off (`a1`, `a2`), 1 MiB alloc/free went from 18.9/20.8 ms to 1.30/1.40
ms. With the write-through copies it went to 0.92/0.97 ms. The churn run's device CPU went from
80 s to 8.8 s. Matmul N=2048 went from 22 ms to 7 ms per product (781 to 2461 GFLOP/s): torch's
steady state kept changing mappings, and each change had cost a whole walk.

## 2. Coalesced maps, the mapping budget, incremental teardown

**Coalescing** (`shadow.rs` `coalesce`). New leaves become one `RM_MAP_MEMORY_DMA` when all of
these hold:

- contiguous in VA;
- the same page size and kind;
- the same backing handle and snoop;
- adjacent offsets in the backing;
- inside one 2 MiB region object.

A run never crosses its region object. When the guest later changes part of a run, only that
piece is unmapped (NVOS47 with the piece's range). The RM splits the mapping and keeps the rest
(`virtmemUnmapFrom_IMPL`, virtual_mem.c:1636). A kernel that is still using the rest of the run
never loses it. If the RM refuses a split, the whole run is unmapped, and its other leaves are
mapped again in the same pass. If the RM refuses a coalesced map, the leaves are mapped one by
one. `CARAXES_SHADOW_COALESCE=0` restores one mapping per leaf.

**How much it coalesces.** Vidmem allocations are contiguous inside the reservation and
coalesce to one call per region. The guest's pinned sysmem pages mostly do not:

- The guest driver takes them one 4 KiB page at a time from its own page pool
  (`nv_mem_pool ... order=0`), so neighbouring VAs get scattered GPAs.
- A context's 15k sysmem leaves take 1.9k-15k calls, depending on what the pool held.
- The async_streams suite took 30k calls for 556k leaves (b1), against 556k before: a factor of
  18.5, not orders of magnitude.

The rest would need a gather descriptor: one OS descriptor over a device-side VA range that
maps the scattered guest pages back to back, so that one call maps a whole unit. It is not
built.

**Budget.** `MapBudget` counts the device's live RM mappings over all its shadow VA spaces. A
run past the budget (`CARAXES_MAP_BUDGET`, default 32768) is refused and never mapped. The
guest's access then faults on its own channel. It is logged once as `ERROR shadow: the device's
mapping budget ... is reached`, and again after the budget has had room. The default fits the
worst workload measured, 18.9k live maps for async_streams, and bounds a teardown at the budget
to a few seconds of batched calls.

**Incremental teardown.** A shadow VA space is freed in steps:

1. unmap up to 64 runs;
2. free up to 64 virtual objects;
3. free the VAS.

The host RM lock is released between steps. Since item 4 the steps run in the worker's idle
time (§4).

**The P20 suite again** (`b1-async-streams`, items 1 and 2):

- It passed in 14.8 s.
- 544750 leaves were held in 18921 RM mappings.
- The teardown of 18914 mappings took 2.5 s in 324 batches.
- Host `nvidia-smi` took at most 0.056 s (50 samples, `bench-hostwatch.sh`), 0 Xid, 0 alerts.

In P20 the same suite made `nvidia-smi` time out after 20 s and restarted the desktop's
VS Code GPU process. The suite and the whole cuda gate passed again in d1 and d2 on the later
builds.

## 3. Context creation and destruction

**Profile** (`p3-profile/init-trace*.txt`, the device trace of one `bench-cu init`). A context
creation made:

- 96193 trapped 8-byte writes to the BAR2 aperture;
- about 4M uncached BAR1 reads.

The BAR1 reads came from the device's own BAR2 page-table walk at every trapped BAR access. The
writes are the guest RM's:

- 107 fresh page-table pages zeroed with 512 writes each (54.8k writes);
- about 19.8k PTEs, each written twice with the same value (39.5k writes);
- small pieces.

A destruction made about 20k PTE-clearing BAR2 writes.

**Device side, fixed.**

- *BAR aperture TLB* (`fb.rs` `bar_translate`). A trapped BAR access looks up
  `(root, va page) -> target`. A miss walks the tables and records the pages it read. A guest
  write to any of those pages flushes the TLB. After the whole gate, the d1 device counted
  4606539 hits against 16159 walks and 3483 flushes. `CARAXES_BAR_TLB=0` disables it.
- *The first doorbell of every shadow channel slept 5 ms* for a one-shot USERD diagnostic, about
  half a second per context. It now runs only with `CARAXES_MIRROR_DIAG=1`.
- *Coalescing, the worker and the background teardown* (§2, §4) took the context's RM calls
  off the critical path.

Measured: cuCtxCreate went from 5.4 s to 2.7 s (a4) and cuCtxDestroy from 3.4 s to 1.0 s. Later
single runs on a busier host gave 3.4-4.7 s and 0.9-1.7 s.

**The floor.** A trapped access costs 23-27 us at p50 (§6), and the device's share of it is
1-2 us. 96k traps are therefore 2.2-3.6 s, whatever the device does. The measured average was
37.7 us per BAR2 write in d2: the guest's interrupt handler traps from another vCPU on the same
single-threaded device.

cuCtxCreate under 500 ms, and cuCtxDestroy under 300 ms, need the BAR2 writes to stop trapping.
One way is a directly mapped BAR2 aperture: the P21 idea applied to BAR2. It must keep the
page-table writes visible to the device, for example by write-protecting only table pages. Until
then the device cannot reach those targets.

Startup (3.4 s to the first cuInit) was not profiled in this package.

**Interrupt noise.** The host's GR non-stall notifier fires for every client's work on the
engine, including the desktop and vm0, and each event interrupts the guest (execution.md §7).
In the init profile the guest's handler ran 487 times in 8.8 s, about 13 traps each.
Progress-gated forwarding would remove most of that; it is not built.

## 4. Resync off the server thread

**Before.** A guest `MMU_INVALIDATE` write, and every doorbell of a shadow channel, ran the
whole resync on the server thread: the walk and every RM map and unmap call. The guest's vCPU
waited on the MMIO write for all of it: 1.7 ms at the median, 14.6 ms at p90, 0.5 s at p99 (d1).
The display agent measured 26 ms walks at 60 Hz under Mutter and 33 of them per 120 Vulkan
frames.

**Now** (`shadow.rs` `ShadowFront::plan` and `ShadowVas::apply`, `mirror.rs` `ShadowWorker`).
The server thread only walks (the cached walk of §1) and filters, then hands a plan to the
worker. The plan is the list of changed units, each with the leaves it should hold and their
backing. The worker thread owns the host side of every shadow VA space and applies plans in
order. It takes the host RM lock per 2 MiB unit.

**Ordering guarantee.** Every job carries a sequence number. A doorbell of a shadow channel first
plans that root, then waits until the worker has finished every job submitted before it
(`wait_all`), and only then rings the host doorbell. So no host channel runs before the mirror
matches the guest's tables as of the guest's last invalidate or doorbell. A channel's creation
waits the same way before the host validates its GPFIFO VA against the shadow VAS. The guest's
invalidate itself returns right after the walk.

Unmaps are deferred as well. That is safe for the same reason as on native hardware: work the
guest submits after the invalidate waits behind the barrier, and the guest does not free memory
that work in flight still uses.

**Teardown.** A freed shadow VA space moves to a graveyard. The worker steps through it only when
no job is waiting, so a teardown never delays a plan. In d2, before the graveyard, plans had
waited up to 2 s behind a teardown. Device shutdown waits for the graveyard to empty before the
guest RAM descriptors are freed (`wait_idle`), so the fail-closed teardown order of parallel.md
§4a holds.

`CARAXES_SHADOW_WORKER=0` waits after every submission: the synchronous reference.

**Measured** (d2, the whole cuda gate): server-thread time per resync p50 42 us, p90 180 us, p99
2.4 ms, max 12.7 ms. The worker's apply time is p50 1.8 ms, p90 15.6 ms, p99 471 ms.

The gate is green on d2. d3 changes only when teardowns run; its quick metrics pass, and the final series of benchmark.md §8 exercises it. The display acceptance cases are pending: Mutter frame callbacks
at 60 Hz under table churn, and a 120-frame Vulkan present on Weston losing at most one frame to
shadow work. vm0 belongs to the display agent, which reruns them on this tree after the merge.

## 5. The two isolation fixes of P20 §8

**(a) IOVA = GPA + one offset** (`hostmem.rs` `register_linear`, launcher
`CARAXES_GUEST_RAM_BYTES`).

A descriptor gets one IOVA range, size-aligned and allocated top-down (sysmem-iova.md §1.4, §2).
So the device waits until the client has mapped every zone, then registers:

- the 1 GiB chunks from the highest GPA down;
- a filler descriptor for the [3, 4) GiB hole, so the zones on both sides stay one offset apart.

The filler is a 1 GiB VA range made of one device-owned 2 MiB memfd page mapped 512 times. It is
never in the guest's table or in the shadow filter's ranges.

Every placement is checked. When a piece lands out of line, because a host allocation sits in
the slot below, the placed pieces stay registered as blockers, the sequence restarts below them,
and the blockers are freed at the end.

On vm2 the first attempt found a host allocation in the way. After one restart:

- guest RAM got IOVA = GPA + 0x7ffac0000000;
- the guest's table shrank from 4 entries to 2 (`0:7ffac0000000:c0000000,100000000:7ffbc0000000:40000000`).

The guest module needed no change: its `ranges=` format already carries one entry per contiguous
stretch. `CARAXES_LINEAR_IOVA=0` keeps the old chunks. A failed linear registration falls back
to them with an `ERROR hostmem` line.

**(b) CE channels on the filtered shadow VAS** (`mirror.rs` `is_shadow`). Every channel the
mirror takes now runs on the shadow VAS of its root: GR as before, and CUDA's copy engines with a
host TSG of the guest TSG's own engine (COPY2 to COPY4). No host channel walks the guest's
unfiltered tables any more. `CARAXES_CE_SHADOW=0` restores the externally-owned VAS.

**Evidence.** `d1-isolation`: the whole cuda gate plus every unscored suite (async_streams,
cuda_graphs, high_freq_event_polling, multi_process_cuda, vram_churn). Results:

- The scored set is green as before (cuda-managed is the known gap).
- 400 channels ran on shadow VASes; 0 externally-owned VAS and 0 `DMA_SET_PAGE_DIRECTORY` were
  created.
- 0 sysmem leaves were refused. The async_streams suite had 128 refused chunk-crossing leaves at
  0x7ffdc0015000 and up in b1.
- The host logged no `IO_PAGE_FAULT` and 0 Xid.
- The only refusals left are the known kind-0x0F SKED leaves at FB 0x2000/0x3000/0x16000
  (parallel.md §3a).

`p20-check.py` against P20's slot-1 evidence as the other VM: every mapped vidmem PA inside the
own reservation, every sysmem chunk one of the own, comptag hold satisfied, 0 Xid
(`d1-isolation/check.txt`).

Its two overlap FAILs are an artefact of that run's log: the restart's blocker placements were
logged as registered chunks and overlap P20's old IOVAs. Such placements are now logged as
`placed` and only the final chunks as `registered`.

The concurrent check on two VMs at once passed on 0d00b26 (parallel.md §8.4): vm1 2 GiB at
slot 0 and vm2 1 GiB at slot 4, the cuda gate on both, p20-check PASS, the pattern test clean
in both directions, 0 Xid, and host `nvidia-smi` at most 0.087 s while vm1 ran async_streams.

Not built: an intentionally chunk-crossing buffer. The suite's natural crossing buffers are the
test, and they now map and pass. The CE filter also covers what a malicious guest kernel could
point a CE PTE at.

## 6. The doorbell path

**Accounting** (`c0-doorbell-study/short-trace.txt`). One short-kernel submission is 4 trapped
writes, about 25 us apart:

1. GPFIFO entry, low word;
2. GPFIFO entry, high word. libcuda places the GPFIFO in vidmem and writes it through guest BAR1.
3. GPPut, in USERD, also vidmem through BAR1;
4. the doorbell at BAR0 0xbb0090.

`caraxes/tools/bench-trap.c` times 20000 trapped BAR0 accesses that the model answers: p50 23-27
us per write or read. The device's share is 1-2 us. Turning off the trace lines changed nothing
(27 us). SCHED_IDLE spinners on every host core made it worse (37 us), so neither C-states nor
device work explain it. The rest is the KVM exit plus Cloud Hypervisor's synchronous vfio-user
round trip: the vfio_user 0.1.3 client does `write_vectored`, then a blocking `read_exact` of
the reply, for every region write.

**Route (a), an ioeventfd for the doorbell.** Cloud Hypervisor v53 has no ioeventfd or
`VFIO_USER_DEVICE_GET_REGION_IO_FDS` on its vfio-user path. The server crate vfio_user 0.1.6
answers `GetRegionIoFds` with `UnsupportedCommand`. Missing:

- the request in CH's vfio-user PCI device;
- a `KVM_IOEVENTFD` registration per returned fd, with a datamatch per token, because the
  written value matters;
- the server side.

**Route (b), the host usermode page as a sparse-mmap area of BAR0.** CH v53 does mmap sparse
areas from a region fd, at file offset `region_info.offset + sparse.offset`. The doorbell page
is at BAR0 + 0xbb0000. `nvidia_mmap_helper` refuses any VMA with `vm_pgoff != 0` and needs the
size to equal the RM mapping (nv-mmap.c). So the RM's usermode mapping cannot sit at a nonzero
BAR0 offset.

**What removing the doorbell trap alone buys.** One trap of four: about 100 us per launch
instead of 125. The 30 us target needs the GPFIFO and GPPut writes to stop trapping. That is
P21, BAR1 window passthrough. P21 plus the trapped doorbell comes to about one trap, 25-30 us.

**USERD in sysmem by configuration** (`c2-userd-sysmem`). The guest's RM decides where RM-owned
USERD lives from `RMInstLoc` bits 17:16 (`kernel_fifo_gm107.c`). That sets
`bUserdInSystemMemory` and the FIFO cap `USERD_IN_SYSMEM` that libcuda reads. With
`NVreg_RegistryDwords=...;RMInstLoc=0x10000` (USERD = COH) and sysmem USERD support for shadow
channels in the device, the guest RM never finished `RmInitAdapter`:

- its last RPC was `GSP_RM_CONTROL 0x20802a08` (CE fault method buffer size), answered with a
  synthesized NV_OK;
- after that it issued no RPC;
- the device then saw 13.1M reads of BOOT_0 (the RM's is-the-GPU-there loop), 0 channels and no
  USERD allocation.

The guest log was a loop of `nvAssertFailedNoLog` at `alloc_free.c:360` and `rs_server.c:1382`,
which had overwritten its own ring buffer. The configuration was reverted.

The device trace of that run is lost: `tools/caraxes-vm.sh up` truncates `gpu-trace.log`, and
the VM was restarted before the trace was copied. The next experiment on this path must copy the
trace first. The variant stays parked. No public libcuda switch for the GPFIFO location was found
in the strings of libcuda.

**The path for the last trap, for the operator's decision on a patch 0005.** Two changes to
Cloud Hypervisor's vfio-user client, either of which works:

1. **Pipelined region writes.** Region writes are posted, so the client could send a write
   without waiting for its reply (vfio-user allows `NO_REPLY`, and the server crate already
   honours `header.no_reply()`), and wait only at the next read.
2. **`GET_REGION_IO_FDS` with KVM ioeventfds** for the doorbell. The device then reads the token
   from an eventfd, or from a datamatch per token.

Either removes the guest's wait on the doorbell. Neither removes the GPFIFO and GPPut traps,
which need P21.

**Poll mode (P24, design only).** Present the usermode doorbell page as a memfd-backed
sparse-mmap area of BAR0; a memfd may sit at any region offset, unlike the RM mapping. Put USERD
in sysmem, or have P21 map it. A poller thread on one host core then watches the mirrored
channels' GPPut words in guest RAM and rings the host doorbell on change.

The open issues are:

- the first doorbell of a channel, which builds the mirror and today needs the server thread;
- the USERD-in-sysmem failure above.

Expected: with USERD and GPFIFO in guest RAM and the doorbell page as memory, a submission makes
no exit at all. It costs the poller's reaction time plus the host doorbell, a few microseconds
at one busy host core per poller.

## 7. Switches (launcher environment)

| Variable | Default | Effect |
|---|---|---|
| `CARAXES_MAP_BUDGET` | 32768 | live RM mappings per device before guest leaves are refused |
| `CARAXES_SHADOW_ALWAYS_RESYNC` | 0 | 1: full walk and diff at every resync (reference) |
| `CARAXES_SHADOW_REREAD` | 0 | 1: re-read a dirty table page from VRAM instead of the write-through copy |
| `CARAXES_SHADOW_COALESCE` | 1 | 0: one RM mapping per leaf |
| `CARAXES_SHADOW_WORKER` | 1 | 0: wait for the worker after every submission (synchronous reference) |
| `CARAXES_BAR_TLB` | 1 | 0: walk the BAR tables at every trapped access |
| `CARAXES_CE_SHADOW` | 1 | 0: CE channels on the externally-owned VAS (the guest's own tables) |
| `CARAXES_LINEAR_IOVA` | 1 | 0: independent 1 GiB chunks instead of IOVA = GPA + one offset |
| `CARAXES_TRACE_ACCESSES` | 1 | 0: no trace line per region access (events still written) |
| `CARAXES_TRACE_FLUSH` | 1 | 0: flush the trace at events and every 100 ms instead of every line |
| `CARAXES_MIRROR_DIAG` | 0 | 1: the per-doorbell and first-doorbell USERD diagnostics |

## 8. Open

- The concurrency check is done (parallel.md §8.4). It left three small items: the BAR1
  windows are unmapped after the reservation is freed (0x33 at exit); vm1 refused 8 DMA
  addresses that look like GPAs, source open; and host BAR1 is near its limit with three
  devices (4 host refusals, handled by eviction).
- The display acceptance of item 4: Mutter at 60 Hz, and a Vulkan present on Weston.
- The gather descriptor for scattered sysmem leaves (§2).
- Progress-gated interrupt forwarding (§3).
- BAR2 writes without traps, the P21 idea for BAR2 (§3).
- The CH patch for the doorbell (§6).
- The guest RM's CeUtils channel: its VA space root and a path to execute it (§9).

## 9. The CeUtils experiment

**Question.** The guest boots with `RmDisableGlobalCeUtils=1` in `NVreg_RegistryDwords`
(`/etc/modprobe.d/caraxes.conf`). Without global CeUtils the guest RM zeroes every fresh
page-table page by CPU through BAR2: 54.8k of the 96k trapped writes of a cuCtxCreate (§3).
Does the guest run, and run faster, with the key dropped?

**Run** (`caraxes/runs/mapping-2026-09-24/e1-ceutils`, 2026-09-24 13:05-13:11, vm2 at slot 0,
2 GiB profile, device built from main `8cca8e6` into `target/mapping`, the mapping
environment of `env.sh`):

1. `caraxes.conf` backed up, `RmDisableGlobalCeUtils=1;` removed; the rest of the
   registry string (`RMDisableScrubOnFree=1;RmRcWatchdog=0`) kept (`guest-conf.txt`).
2. `bench-run.sh v2-restart`: a fresh device process, and the launcher's probe step unloads,
   resets and reloads the guest driver (`up.txt`, `guest-dmesg.txt`).

**Result: red at the driver load.** The guest RM failed `RmInitAdapter` (0x25:0x65:1310):

- `memmgrTestCeUtils` timed out in its CE memset (`memmgrMemSet ... TRANSFER_FLAGS_PREFER_CE`,
  mem_mgr.c:445, `NV_ERR_TIMEOUT`);
- `pCeUtils->lastCompletedPayload == lastSubmittedPayload` failed at ce_utils.c:350;
- `memmgrInitCeUtils` returned the timeout (mem_mgr.c:508) and `RmInitNvDevice` bailed out.

So there was no CUDA to measure: `bench-cu init`, `bench-cu churn 20` and the cuda gate were
not run, and there is no BAR2 write count, cuCtxCreate or cuCtxDestroy for this
configuration. Following the rule for a red result there was no second attempt.

**Which path the CeUtils channel took** (`ceutils-channel.txt`, the device trace events of the
guest RM's internal client 0xc1e00006):

- The guest RM allocated a bare `0xc46f` channel (chid 2, engine type 0xb, a GPFIFO of 4096
  entries at VA 0x120064000, USERD in vidmem) directly under its device, with no TSG and no
  VA space handle. For a physical CE channel on a GSP client without MIG, CeUtils names no VA
  space (channel_utils.c:86-93) and the channel runs on the device's default VA space. The
  one `0x90f1` object this client allocated was freed again before the channel was created.
- A `0xc5b5` (TURING_DMA_COPY_A) object was bound, the channel scheduled, and the doorbell
  rang once (token 0x10002).
- The device logged `chid 2: VAS root unknown, nothing run` and
  `mirror: chid 2 in software: bare channel (no TSG): stays with the software executor`.

The channel therefore took neither path that executes work. The mirror refused it as a bare
channel (`CARAXES_MIRROR_BARE_CE` is off in this configuration), and the software executor
found no VA space root and ran nothing, so the memset's semaphore never arrived. No host
channel was created for it, and it never walked guest tables on the GPU: the fail-safe held.
The host logged 0 Xid and 0 refused BAR1 mappings from 13:05 on (`host-xid.txt`).

**Restored.** The backup was copied back (`guest-conf-restored.txt`, the registry string as
before) and vm2 restarted: `RmInitAdapter succeeded`, and one `bench-cu init` gave cuInit
156 ms, cuCtxCreate 2697 ms and cuCtxDestroy 893 ms (`restored/`), inside the range of §3.

**What dropping the key would need.** Three things, in the device, before the experiment
can be repeated:

1. The default VA space of a device: a channel with no VA space handle must resolve to the
   root the guest RM uses for its device's default VAS. The three `0x90f1` objects of this
   run reported only their server-reserved range (0x100000000-0x11fffffff) and roots, and
   the device ties none of them to a channel without a handle. Until the device knows that
   root, no path can run the channel.
2. A path for the bare kernel CE channel. The software executor is the right one: its
   memsets and copies write through `FakeFb::write`, so the walk cache of §1 sees every
   page-table write. The host mirror is not: a copy engine on the host would zero guest
   page-table pages behind the FB store, and the cache would keep stale copies of pages it
   watched before (§1, "Correctness"). If the mirror ever takes this channel, it must be on
   the filtered shadow like every CE channel, and the cache must drop the pages a CE channel
   writes.
3. A measurement that shows the gain. The expected one is the 54.8k zeroing writes (107
   pages) replaced by 107 memsets of about 4 trapped writes each, that is 1.2-2 s off
   cuCtxCreate at 23-37 us per trap. The PTE writes (39.5k) and the destruction's 20k
   clearing writes stay, so this is not the 500 ms target of §3.

**For the operator.** The guest default should keep `RmDisableGlobalCeUtils=1`. With the
current device the guest driver does not load without it. Revisit after items 1 and 2 above,
with a green cuda gate as the condition for the switch.
