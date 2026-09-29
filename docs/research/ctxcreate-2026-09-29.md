<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# cuCtxCreate, cuMemAlloc and cuMemFree on today's code (2026-09-29)

Scope: where a CUDA context's creation and destruction and a device allocation's alloc and free spend their time on the production candidate (gfxp-2026-09-29, 139662fa = P43), against native; the fixes ranked by gain per effort; the two implemented on branch `ctxcreate-2026-09-29`. `R` = `caraxes/runs/ctxcreate-2026-09-29/`.

## Method

- Lean compute VM of the night of 2026-09-28 (vm2-615, 615 guest, 2 GiB, slot 0, `matrix-2026-09-25/env.sh wl615`), one frozen binary per session [R/run.sh].
- `run.sh census`: `census-cu` init ×3, ctx ×5, ops ×10 with `NV_PBUS_SW_SCRATCH(30)` markers, the device trace on, `CARAXES_PROFILE=1`; then `bench-cu` init ×5, churn 20 ×3 as the user. `census-table.py`, `rpc-cost.py`, `profile-rank.py` on the records.
  - The markers need `resource0`, so `census` runs `census-cu` under sudo; `census-user` makes the resource world-writable and runs it as the user.
- `run.sh bench`: trace off (the clean times), `bench-cu` init ×10, churn 20 ×3, `census-cu` ctx 5 and ops 10 unmarked.
- `run.sh native LABEL`: the same programs on the host (615.71.09).
- `census-cu` is built on the host from `caraxes/tools/census-cu.c` (GLIBC 2.34 at most) and copied into the guest; `bench-cu` is the guest's own.

## Facts

### Numbers (baseline 139662fa, as the user, trace off)

| Operation | native | guest | ratio | source |
|---|---|---|---|---|
| cuInit, ms | 29.4 | 13.9 | 0.47 | bench-cu init ×10 [R/native, R/base-bench2] |
| cuCtxCreate, ms | 129 | 289 [224-346] | 2.2 | bench-cu init ×10 |
| cuCtxDestroy, ms | 71 | 135 | 1.9 | bench-cu init ×10 |
| cuMemAlloc 1 MiB, us | 42 | 280 [148-289] | 6.7 | churn, median of 3 runs |
| cuMemFree 1 MiB, us | 31 | 456 | 15 | churn |
| cuMemAlloc 64 MiB, us | 82 | 2456 | 30 | churn |
| cuMemFree 64 MiB, us | 103 | 2434 | 24 | churn |
| cuMemHostAlloc 1 MiB, us | 717 | 1686 | 2.4 | census-cu ops, unmarked |
| cuMemHostRegister 1 MiB, us | 348 | 3441 | 9.9 | census-cu ops, unmarked |
| d2h pinned 16 MiB, us | 1312 | 1354 | 1.03 | census-cu ops, unmarked |

- As root (base-census, `census-cu` under sudo, trace on) the copies are 11-42× native: h2d 16 MiB 14.8 ms, d2h 16 MiB 55.6 ms [R/base-census/results.txt].

### cuCtxCreate as a user process (~290 ms)

- Timeline of one `bench-cu init` process from its root client's allocation [R/base-census/census-trace.txt.xz, the bench part after the last marker]:
  - 0-14 ms cuInit; 14 and 23 ms two `FERMI_VASPACE_A`; 50 ms the GR TSG and its context share; 98-122 ms 20 channels with their objects;
  - 132 ms the first doorbell (CE chid 18) arrives, its build ends at 235 ms;
  - 235-280 ms the other 19 builds: GR 3.3-3.6 ms, CE 1.2-1.4 ms each;
  - after them about 70 ms of held invalidates: 30 invalidates, 283 busy trigger reads (39 ms).
- The first build waits for the context's first plan [same trace, the first `shadow: sync` line]:
  - 16128 leaves walked, 15876 mapped in 35 RM calls; apply 97.8 ms;
  - of it 30 gather descriptors, 91.3 ms (3.0 ms each), for libcuda's scattered 4 KiB sysmem.
- Every other context needs the gathers: the first plans of base-census-user map their sysmem with 1, 30, 30 and 3 gather descriptors (1 ms against 97-149 ms) [R/base-census-user/census-trace.txt.xz `shadow: sync ... [full walk]`]; `bench-cu init` alternates 180-220 and 270-310 ms in every session of either binary [R/*/results.txt].
  - The guest's page allocator hands the same pages back to the next context, alternately in and against address order (inferred from the alternation; the page lists are not logged).
- Cost ranking of one `bench-cu init` process (cuInit + create + destroy, 440 ms), profile client scope [R/base-bench2/profile.1.tsv, client 0xc1d0000e]:

| Row | count | ms | note |
|---|---|---|---|
| shadow gather alloc | 35 | 129 | worker; 3.7 ms each |
| GSP queue head writes (RPCs) | 189 | 67 | guest-seen; the FREEs of the destroy carry host frees |
| BAR2 memfd syncs | 356 | 64 | 179 us each |
| host RM_FREE | 194 | 50 | one of 34 ms |
| MMU invalidate trigger writes / reads | 163 / 467 | 43 / 31 | the held invalidates |
| BAR2 flush reads | 472 | 39 | guest-seen, the syncs inside |
| host channel allocs | 20 | 30 | 1.5 ms each; compute objects 8 × 1.4 ms, TSGs 4 × 1.2 ms |
| BAR1 pool moves in and out | 1024 pages | 13 | at the process's start and end |
| NV50_MEMORY_VIRTUAL / RM_MAP_MEMORY_DMA | 86 / 98 | 9.5 / 4.8 | region objects and maps |
| preemption (late SET: idle, BIND, GETs) | 1 | 1.8 | plus 0.2 ms GET at the build |

- Host-side micro-benchmark of a gather's window (512 scattered pages of a 256 MiB memfd): mremap 1.6-2.1 ms, munmap 0.4-0.5 ms; `mmap(MAP_FIXED)` per page instead is no faster (1.4-1.8 ms) [R/micro/mr.c, mr.txt].
- Not costs here:
  - GSP RPCs: 85 per create, 331 us device time, 2 with a host call [R/base-census/census-table.md op 3];
  - the VRAM store's BAR1 windows: 135 window maps in the whole session [R/base-census/gpu.log `vram store`];
  - UVM: off for Linux, no `MEM_OP` holds;
  - the preemption bind: about 2 ms per context (20 of 20 GR TSGs got CUDA's CILP SET after the build) [R/base-bench2/gpu.log `preemption, host GR TSGs`].

### cuCtxCreate as root (388 ms, census)

- A root process is an admin client: the guest kernel sets `PRIVILEGED_CHANNEL` on its channels and passes `internalFlags` PRIVILEGE ADMIN (1) [615 tree: src/nvidia/src/kernel/gpu/fifo/kernel_channel.c:288-297, 2767-2770].
- Its twelve copy channels (flags 0x2012x0 with bit 5; as a user 0x2012x0 without) stayed in the software executor ("channel may issue guest physical-mode work") [R/base-census/census-trace.txt.xz].
- Each software push costs about 1.1 ms of device time, 16-byte memsets included: 62 memsets in one doorbell took 69 ms, one context about 100 ms [same trace, chid 10; the semaphore timestamps against the trace times].

### cuCtxDestroy (135 ms)

- 65 FREE RPCs, 27 with a host free (20 host channels at about 1.5 ms, 4 TSGs, the shadow's teardown), 169 BAR2 flush reads [R/base-census/census-table.md op 4].
- The BAR1 pool moves the process's 512 pool pages back (6-16 ms per process end) [census trace `bar1 pool: the poll ... held the device`].

### cuMemAlloc and cuMemFree

- 1 MiB (census op 10, 11): one held invalidate (plan, then the worker's map or unmap; the guest polls the trigger's busy bit) and 1-1.5 BAR2 flush reads per operation; half the allocations need a new 2 MiB region map.
- 64 MiB (op 12, 13): 29 RM map calls per allocation (worker apply 1.5 ms), 32 unmaps per free (1.8 ms): one `NV50_MEMORY_VIRTUAL` region object per 2 MiB, so a contiguous 64 MiB run is 32 map calls.
- A BAR2 memfd sync compares every mapped memfd page with its snapshot: 128 pages per sync, 14841 syncs, 146 us mean, 12.7 ms max in base-bench2's session [R/base-bench2/gpu.log `bar2 memfd`].
  - The new profile rows (new-bench, 14700 syncs): the scan is 132 us of it (1.03 us per page compared, max 21.9 ms), the BAR2 table walk 1 us, a drift check 52 us every 50 ms [R/new-bench/profile.1.tsv `bar2 memfd: sync`].
  - The same scan code takes 22 us for 135 clean pages in a release micro-benchmark (`bar2area::tests::bench_scan_of_135_clean_pages`, ignored), C 12-24 us [R/micro/sc.c, sc.txt]: in the device it is 6× slower; not explained yet (cache lines the guest's vCPUs own, preemption; `perf` on the device thread is the next step).
- A gather of 2 MiB of scattered guest sysmem, the new profile rows (new-bench, 464 gathers of 508 pages, 395 runs each): window of mremaps 1.41 ms, the host RM's descriptor 1.00 ms, the window's munmap 0.70 ms [R/new-bench/profile.1.tsv `gather:`].

## Consequence for Caraxes

### Ranked by gain per effort

| # | Fix | Gain (est.) | Effort | Status |
|---|---|---|---|---|
| 1 | Prefetch: a user context's shadow built and planned in the background from its TSG on | measured 303 -> 263 ms (census-user median); the first build's worker wait ~98 -> ~35 ms | ~300 lines | implemented (83bfe733, 8fa5af25) |
| 2 | Root processes' copy channels mirrored (ADMIN level) | measured: root cuCtxCreate 373 -> 271 ms, copies to native | ~40 lines | implemented (83bfe733) |
| 2b | Freed gather descriptors cached for the same pages | the slow half of the contexts (~90 ms each) | ~120 lines | implemented (0fd0f3df), A/B pending |
| 3 | BAR2 memfd syncs: split the cost (rows added), then fewer sync points or O(changed) | up to ~64 ms per process, ~100-150 us per small alloc and free | research | rows implemented |
| 4 | Unmap-only plans not held (09-25 part 1) | ~100-200 us per free, 1.8 ms per 64 MiB free | ~30 lines | needs a decision: a freed VA stays mapped on the host until the worker unmaps it |
| 5 | Region objects spanning a contiguous run (64 MiB = 1 map call) | ~1.4 ms per 64 MiB alloc | ~200 lines in shadow.rs | open |
| 6 | Cheaper gathers (a dma-buf of the pages instead of 512 mremaps) | up to 3 ms per 2 MiB of scattered sysmem, also in held invalidates | research (udmabuf, host RM dma-buf descriptors) | open |
| 7 | BAR1 pool moves at a process's start and end | ~20 ms per process | research | open |
| 8 | Software executor 1.1 ms per push (guest kernel channels) | per kernel CE push | research | open |
| - | Channel builds, host frees | none: host RM work native pays too, serial under the RM lock | - | kept |
| - | Preemption GET/BIND/GET, late-SET idle | ~2 ms per context | - | kept (gr-preemption rules) |

### Implemented (branch ctxcreate-2026-09-29; binaries `/mnt/vmstore/caraxes/vfio-user-nvgpu-ctx-<sha>`)

- 83bfe733 the prefetch, the admin copy channels, the timing rows; 8fa5af25 the pool barrier; 0fd0f3df the gather cache.

- **Prefetch** (mirror.rs `prefetch`, Unix guests only):
  - candidates: the root of a VA space a user process's non-video TSG, context share (of a client with a TSG) or TSG channel runs on (`Registry::prefetch_roots`; user clients are handles 0xC1D00000-0xC1DFFFFF [615 tree: src/nvidia/generated/g_resserv_nvoc.h:173-190]);
  - at the guest's invalidate of such a root the shadow VAS is built and the root planned without a wait, VAs from 8 GiB up only (`PREFETCH_VA_FLOOR`): below, the guest RM's own buffers and set B share 2 MiB regions (CUDA's set B 0x120000000-0x120120000, promoted only at the compute object);
  - the first channel's build takes the shadow over, plans the rest with its set B and waits for the worker as before (`wait_all` before the host TSG);
  - it drops the shadow and builds anew when the channel wants another layout (video) or a planned region lies in set B (`prefetch_discard`);
  - a shadow whose root no TSG or channel names any more is reaped after the FREE (`reap_stale_prefetched`); a device reset drops them.
  - the BAR1 pool's barrier (`Mirror::shadow_barrier`, under the device lock at every page move) waits per root for every shadow but a prefetched one (`barrier_roots`); before (83bfe733), the pool's poll at the process's first BAR1 mappings waited for the prefetch's gathers and the setup grew by ~20 ms per context.
- **Gather cache** (hostmem.rs `GatherCache`, 64 descriptors, oldest first): a gather the shadow frees is kept; a gather of exactly the same pages in the same order (registered descriptor and offset; host handles are never reused) takes it back. Freed for good at eviction, before a guest-RAM mapping change or release (`check_gpa_mapping_update`, `release_region`) and at the teardown (`free_mirrors`); counted in the mirror's exit line.
- **Admin copy channels** (mirror.rs `admin_application_channel`, Unix guests only): a privileged non-GR channel with internalFlags PRIVILEGE ADMIN is mirrored; KERNEL (the guest RM's scrubs and page-table writes, UVM) and USER-with-bit-5 stay in software.
- **Timing rows**: shadow channel builds and their worker wait in the mirror's exit line; `gather:` and `bar2 memfd: sync` phase rows in the profile.
- Tests: `channel::tests::prefetch_roots_name_the_vas_a_tsg_or_channel_runs_on`, `the_channel_privilege_level_is_decoded`; `mirror::tests::an_admin_copy_channel_of_a_unix_guest_is_mirrored`, `a_prefetched_shadow_plans_below_the_floor_only_at_the_build`, `the_pool_barrier_skips_a_shadow_built_ahead_of_its_channel`; `hostmem::gather_cache_tests::a_freed_gather_is_taken_back_only_for_the_same_pages_in_the_same_order`; `bar2area::tests::bench_scan_of_135_clean_pages` (ignored). `cargo test -p vfio-user-nvgpu`: 731 passed, 2 ignored; clippy clean.

### Rules kept

- No host channel starts on a mirror older than the guest's tables: the build still plans and waits for every job before the host TSG; a prefetched shadow has no channel.
- D16/D17: every prefetch plan is filtered like any other (reservation, registered guest RAM, kinds, private pages).
- Untrapped GPPut and `MEM_OP` holds (shadow-fault-race-2026-09-28.md): unchanged; a prefetched root is among the `MEM_OP` hold roots.
- GR preemption (gr-preemption-2026-09-29.md): the compute bind stays at the build, before the first schedule.
- Windows (windows-findings.md): nothing changes (`HOST_OS_TYPE` 2); its KMD channels are KERNEL privileged, the application-channel rule of P29 stays.
- A mirrored admin channel is unprivileged on the host (`DENY_PHYSICAL_MODE_CE`): a physical-mode method faults that channel only.
- The pool barrier still waits for every shadow a host channel may run on before a page moves; the build of a prefetched root waits for every job.
- A cached gather maps only pages of this VM's registered guest RAM that the filter resolved when it was made; it is taken back only for the same registered pages.
- The leak watch: the cache keeps up to 64 descriptors after their contexts ended, up to 32768 page pins (128 MiB of guest RAM, already pinned by its registration) in the host's `foll_pin` count until eviction or the device's exit; bounded, not a leak.

## A/B

Slot 5, 1 GiB, lean VM; census trace on (5 cuCtxCreate per session), bench trace off (10 processes; mean, and the medians of the fast and the slow five, the contexts alternate). Session-to-session spread of one binary: ±25 ms.

| Binary | cuCtxCreate census-user | setup / first build (ms, 5 reps) | bench create mean (fast / slow) | root census cuCtxCreate | root d2h 16 MiB | Xid |
|---|---|---|---|---|---|---|
| native | 139 | - | 130 (126 / 132) | - | 1.31 ms | - |
| 139662fa base | 303 | 73-100 / 17-171 | 251 (206 / 294) | 373 | 52.3 ms | 0 |
| 83bfe733 r1 | 297 | 92-185 / 6-76 | 274 (191 / 285) | 271 | 1.34 ms | 0 |
| 8fa5af25 r2 | 263 | 79-99 / 6-75 | 233 (191 / 271), 258 (216 / 300) | - | - | 0 |
| 0fd0f3df r3 | pending | pending | pending | - | - | - |

- Records: base-census-user, base-bench5, base-census5; new-census-user, new-bench, new-census; new2-census-user, new2-bench, new2-bench-b, new2-root-smoke [R].
- Host load: base-census-user, new-census-user, new-bench and new-census ran on an otherwise idle card; base-bench5, base-census5 and every new2-* ran next to the SotTR VM desktop-b-dmem (up from about 07:26; new2-root-smoke's root `bench-cu init` took 1.35-1.5 s per context under it). r3 and a paired baseline are to run on an idle card (new3b-*, base-bench5b).
- r1: the prefetch took over 15 of 15 contexts (0 dropped, 0 reaped) and halved the first build's worker wait (987 ms over 20 contexts in new-bench against ~98 ms per context before), but the guest's setup grew: the pool barrier (fixed in r2).
- Root processes (r1 against base-census5): cuCtxCreate 373 -> 271 ms, cuCtxDestroy 96 -> 137 ms (twelve more host channels to free, native 74), h2d 16 MiB 14.1 -> 1.32 ms, d2h 16 MiB 52.3 -> 1.34 ms.
- new2-root-smoke (8fa5af25, root and user, next to SotTR): 0 Xid; root copies at native (h2d/d2h 16 MiB 1.31/1.30 ms, 256 MiB 13.0 GB/s, d2d 16 MiB 173 GB/s); root transformer 74.7k tokens/s, root ResNet-18 342 images/s (user 372); llama 1B does not load in the 1 GiB profile (as a user too: the model needs the 2 GiB profile) [R/new2-root-smoke/guest.txt].
- cuMemAlloc and cuMemFree are not touched by these changes; their spread between sessions of one binary is up to 2× (base-bench2 against base-bench5, 64 MiB alloc 2.5 against 5.1 ms).

## Open

- The A/B above; the workload series with the new binary (llama, transformer, ResNet-18, NVENC) against series-p43.
- Fixes 3-8 of the table.
