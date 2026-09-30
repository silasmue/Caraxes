<!-- SPDX-License-Identifier: MIT -->
# P20: two Leandro VFIO VMs on one RTX 2070 at the same time

Written 2026-09-24 for the operator's order "alles nacheinander und dann brauchen wir den
Parallelitaets-Proof" (README P20). Evidence is in `caraxes/runs/parallel-2026-09-24/`, with one
directory per experiment. The device build is `target/p20/release/vfio-user-nvgpu`, built with
`CARGO_TARGET_DIR=target/p20` from main 05cd1f8 plus the changes listed in §7. Each run directory
records the sha256 of the device it used. The tools are:

- `caraxes/tools/p20.sh`: per-VM environment, `up`, `down`, the host `nvidia-smi` snapshot, and the
  host log check (Xid lines plus the count of refused host BAR1 mappings).
- `caraxes/tools/p20-cuda-guest.sh`: nvprobe 2, 3 and 4 with wall times.
- `caraxes/tools/p20-patprobe.c`: the guest pattern filler and scanner.
- `caraxes/tools/p20-check.py`: the isolation checker.

The configuration is the green one of testing.md §7b:

```
CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
```

Each VM uses a 2 GiB profile (`CARAXES_FB_MIB=2048`, guest RAM 4 GiB) and its own slot: vm1 has
`CARAXES_RESERVE_SLOT=0`, vm2 has 1 and vm3 has 2. The display VM vm0 ran throughout and was
never touched. It holds no reservation and does not execute on the GPU.

## Summary

The result, in the operator's terms:

- **Proven.** Two stock guest drivers run CUDA at the same time on the one card that also drives
  the host desktop.
  - Each VM holds its own fixed VRAM slot.
  - The scored cuda gate passes on both VMs at once, with the same result as a single VM.
  - Neither VM can see the other's VRAM.
  - The host scrubs a slot before it is handed out again.
  - A failed teardown keeps everything the GPU could still reach.
  - A third VM that would push the desktop below 1 GiB of free VRAM is refused, and the two
    running VMs are not affected.
- **Not proven.** Two findings in the sysmem and mapping path came out of the run. Both are
  guest-triggerable, and both need fixes that are named in §8 and not built today:
  - An unscored stress test that maps half a million pages made the host RM stall for minutes.
  - A guest DMA buffer that crosses a 1 GiB chunk boundary produced page-table entries that
    point past the VM's registered guest RAM. The copy engines walk guest tables unfiltered, so
    the card read from IOVAs the VM does not own. The IOMMU blocked those reads only because
    nothing was mapped there.
- **VRAM isolation held throughout.** Across every run, no VRAM address outside the two slots
  was touched. The comptag audit found no compressible kind and no comptag line.

## 1. Two reserving VMs at once

`caraxes/runs/parallel-2026-09-24/e1-two-vms/`.

| | vm1 (slot 0) | vm2 (slot 1) |
|---|---|---|
| Reservation (device log) | `[0x169000000, 0x1e9000000)`, 2048 MiB, contiguous, 0 ms | `[0xe9000000, 0x169000000)`, 2048 MiB, contiguous, 0 ms |
| Free host VRAM at the request (min of HEAP_FREE and nvidia-smi) | 6624 MiB | 4576 MiB |
| Guest RAM IOVA chunks | 0x7fff80000000, 0x7ffec0000000, 0x7ffe80000000, 0x7ffe00000000 (1 GiB each) | 0x7ffd80000000, 0x7ffd40000000, 0x7ffd00000000, 0x7ffcc0000000 |
| Guest `nvidia-smi` | `Leandro RTX 2070-2G`, 0 MiB / 2048 MiB | the same |
| nvml gate | pass, 11/11 | pass, 11/11 |

Host `nvidia-smi` used memory was 1148 MiB before, 3195 MiB with vm1, and 5243 MiB with both:
exactly two profiles more. The slots follow `hostmem.rs` `slot_range`: slot n lies n profile
sizes below the 2 MiB-aligned end of the host FB heap region `[0x3600000, 0x1e9190000)`. The
host's largest free region started at 0x6c800000, so both slots fit above the desktop's own
allocations. The host logged no Xid.

## 2. Concurrent CUDA

### 2.1 Probes (nvprobe 2, 3, 4)

`e2d-concurrent-fixed/`, final build. The first measurement, in `e2-concurrent/`, used the build
before the BAR1 fixes and gave the same picture.

| | vm1 ms/launch | vm2 ms/launch |
|---|---|---|
| alone (one VM at a time) | 18.45 | 18.22 |
| concurrent, rep 1 | 24.71 | 25.04 |
| concurrent, rep 2 | 16.57 | 18.32 |
| concurrent, rep 3 | 16.47 | 17.63 |

Every stage passes in every rep: nvprobe 2 memory, nvprobe 3 vecAdd correct, and nvprobe 4 with
100 launches. The per-stage start times show the two VMs' stages overlap within 1-3 s. The first
concurrent rep is slower. After that, concurrency costs little, because a launch's 18 ms is
dominated by the device path (doorbell, shadow sync, completion interrupt), not by the 30M-cycle
kernel on the SMs. The runlists are shared and the host time-slices the two contexts like any two
host processes. There is no QoS. The earlier build measured 17.9 and 18.4 alone, and 28.3/27.2,
22.6/20.7 and 22.6/23.5 concurrent.

### 2.2 The cuda gate on both VMs at the same time

Scored tests, final build (`e2d-concurrent-fixed/gate-vm{1,2}/transcript.log`):

| Test | vm1 | vm2 |
|---|---|---|
| cuda-init, cuda-context, cuda-core, cuda-jit, cuda-launch | pass | pass |
| cuda-managed | fail (`AllocManaged 801`, the known guest-side gap, the same as single VM) | fail (same) |
| cuda-hostreg | pass | pass |
| cuda-torch (cuDNN convolution) | pass | pass |
| cuda-gdb | skip (no cuda-gdb) | skip |
| rlprobe | pass (mean10=56.9) | pass |
| convburn | pass (acc=3.041451216e+00, the same on both) | pass |
| robustness | pass | pass |

This is the single-VM green result of testing.md §7b, on both VMs at once, with 0 Xid and 0
refused host BAR1 mappings. The earlier build had one cuda-hostreg roundtrip fail on vm2 (1 of 2).
The guest log of that test was lost when the run was stopped, and it did not recur.

The gate's JSON verdict reads "exited before the verdict" in both run directories. The gate
continues with the unscored `probe/suites` after the scored tests, and both runs were stopped
there (§2.4 and §8.1).

### 2.3 Finding: host BAR1 is a shared resource the devices had exhausted

The first concurrent gate (`e2-concurrent/`) deadlocked both device server threads at 06:57, and
the guests' vCPUs stalled on MMIO (RCU stalls, soft lockups). There were two causes:

1. **A BAR1 leak.** `hostmem.rs` `Window::drop` unmapped the CPU mapping and closed the mapping
   FD, but never issued `RM_UNMAP_MEMORY`. The RM keeps the BAR1 mapping until the client dies.
   A single VM's working set fits in its windows, so no window was ever evicted and the leak
   never showed. With two VMs, evictions started and every eviction leaked BAR1. The card has
   256 MiB of BAR1, which cannot grow past 256 MiB even with resizable BAR (host-facts H2), shared by the desktop and every device.
   The host RM logged 732088 refused BAR1 mappings (`reusemappingdbMap` and
   `_reusemappingdbAddMappingCallback` NV_ERR_NO_MEMORY assert lines) between 06:53:47 and
   06:57:12. This is a host-side impact: the desktop's own BAR1 mappings were competing for the
   same exhausted space. The desktop clients stayed intact, and nothing crashed. Today's log has
   no such line before P20.
2. **A self-deadlock.** When no window was left, `VramStore` mapped a new one through
   `WindowSource::map`, which locked `HostMemory`. The shadow sync (`mirror.rs` `shadow_resync`,
   line 300) already holds that lock while it walks the guest's tables through the same FB store.

A further latent integrity bug sat on the same path. When the host granted no window at all,
`VramStore` returned zeros for reads and **dropped guest writes silently** (`Piece::Unmapped`).

Fixes, all in the device:

- `Window` now carries a duplicate of the client's ctl FD and releases its RM mapping with
  `RM_UNMAP_MEMORY` on drop. Measured (`e2c-bar1-unmap/`): 256 windows mapped and released, 0
  refused, 0 host asserts.
- `VramStore` maps through a `ReservationMapper` that holds its own FD duplicates, so it never
  takes the `HostMemory` lock.
- A refused map first evicts the device's own least recently used window and retries at the same
  size. A device therefore never grows its BAR1 share under pressure and never loops on refused
  maps.
- When no window can be mapped even at 2 MiB with none held, the access stalls and is retried,
  with an error line every 1000 stalls. It is never dropped.
- The per-device budget is a launcher option: `CARAXES_BAR1_WINDOWS` (default 2) times
  `CARAXES_BAR1_WINDOW_MIB` (default 32). The device logs it at startup (`budget 2 x 32 MiB = 64
  MiB of the host's shared BAR1`). Unit tests cover the eviction under a host that has room for
  one window, and the stall.

The arithmetic: the host reported about 232 MiB of BAR1 free with the desktop up (host-facts H2).
The old default of 3 x 64 MiB per device needs 384 MiB for two devices, more than the whole BAR.
The new default of 2 x 32 MiB needs 64 MiB per device, so three devices take 192 MiB and leave
the desktop about 40 MiB. The gate needed no more: the final concurrent run shows 0 refused maps.

This is an isolation and QoS finding. BAR1 is one host-wide resource with no per-client quota,
so any device that maps guest VRAM through BAR1 must budget it. For the later BAR1
direct-mapping package, note that this card's BAR1 is only 256 MiB. A direct mapping of guest
VRAM must stay windowed, or needs a card whose resizable BAR reaches the VRAM size.

### 2.4 Finding: mapping scale in the unscored suites stalls the host RM

With the BAR1 fixes in place, the rerun (`e2d-concurrent-fixed/`) passed the scored tests and
wedged again in the first unscored suite, `suite-test_async_streams`. That suite had never run
with host execution before. Measured from the device traces (`hang-vm{1,2}/events-tail.txt`):

- The test makes the GR shadow VAS mirror about 545000 leaves per device: vm1 walked 545251
  leaves, of which 544706 were set-A; 555359 fixed-VA `RM_MAP_MEMORY_DMA` calls were made in
  total and 10653 unmaps; the mirror used 1942 region objects. Each walk took about 0.8 s.
- When vm2's guest freed that context, its device thread spent minutes in the host kernel (state
  R, system time only). The most likely place is the RM freeing a VA space with half a million
  mappings, under the GPU lock.
- vm1's device thread sat in D state, and host `nvidia-smi` timed out after 20 s.
- The desktop's VS Code GPU process (electron) restarted at 08:04:34: pid 2131 went away and pid
  1893793 took its place, most likely from its own GPU watchdog.
- After both devices were killed at 08:07, `nvidia-smi` answered in 0.04 s with 889 MiB used, the
  other desktop clients were present, and the host logged 0 Xid.

This is a guest-triggerable host impact: any guest can map that many pages. The fix is named in
§8.1. The suites were not run again.

## 3. Isolation

### 3a. Each device maps only its own addresses

`p20-check.py` reads each device's `gpu.log` (reservation, registered chunks, exit summary) and
the `shadow:` lines of its trace. Every shadow sync line now carries the extent of every physical
address mapped so far (`shadow.rs` `ShadowStats::extents`). The checker verifies:

- every mapped vidmem PA lies in the device's own reservation and none in the other's;
- every sysmem chunk mapped from is one of its own registered chunks and none of the other's;
- every refused leaf lies outside its own ranges, and where it lies instead;
- the device's refused DMA addresses;
- the FB store's accesses outside the reservation;
- the comptag audit;
- Xid containment.

| Run | vm1 mapped vidmem | vm2 mapped vidmem | Sysmem chunks mapped from | Refused leaves | Verdict |
|---|---|---|---|---|---|
| `e2-concurrent` (concurrent gate, 873 and 914 syncs) | `[0x169200000, 0x1e8bb0000)` | `[0xe9200000, 0x168bb0000)` | only each VM's own 4 chunks | vm1: `Fb(0x2000/0x3000/0x16000)` kind 0x0f, and `Sys(0x7fff00000000..0x7fff00017000)`; vm2: the same Fb leaves, and `Sys(0xf0041b3000, 0xf0041d1000)` | PASS |
| `e3-isolation` (pattern tests, clean teardown) | `[0x169200000, 0x1e8bb0000)` | `[0xe9200000, 0x168bb0000)` | own chunks only | only the kind-0x0f Fb leaves | PASS |

No refused leaf lies in the other VM's reservation or registered RAM. The three kinds of refusal
are:

- `Fb(0x2000)`, `Fb(0x3000)`, `Fb(0x16000)`, kind 0x0F (`SMSKED_MESSAGE`). These are the
  compute object's SKED/MMIO mapping, whose address field "is completely ignored" and carries
  `ChID * 4 KiB` for debugging (`kernel_graphics_object.c:442-452`). The host builds its own for
  the host GR object, so refusing the guest's copy is harmless.
- `Sys(0xf0041b3000, ...)`: the guest's swiotlb pool alias below 2^40 (memory-model.md §2). It
  is not in the device's table by design.
- `Sys(0x7fff00000000 + n * 4 KiB)`, n = 0..0x17: the crossing leaves of §8.2.

The FB store saw 0 accesses outside the reservation on both devices.

### 3b. Comptag audit (D16)

The exit summaries of the clean runs (`e3-isolation/dev-vm{1,2}/gpu.log`):

- vm1: `0x00:0x0 x505531, 0x06:0x0 x104344`;
- vm2: `0x00:0x0 x246904, 0x06:0x0 x67958`.

Both end with "all uncompressed (kind 0x06/0x00, comptagline 0): comptag hold satisfied". The
devices that had to be killed (`e2`, `e2d`) wrote no exit summary. Their shadow filters refused
no compressible kind.

### 3c. Xid containment

The host logged 0 Xid over the whole P20 window (06:41 to 08:25). Each run directory has
`host-xid.txt`, with the Xid lines and the count of refused BAR1 mappings. There was nothing to
contain.

### 3d. The direct test: a pattern in VM A is not visible from VM B

`e3-isolation/`, with `p20-patprobe` built in each guest against the guest's libcuda:

| Step | Result |
|---|---|
| vm1 fills a 1 GiB `cuMemAlloc` buffer with 16-byte records `{0xCA7A5E5A, 0x50A77E2D, 0xA1, index}` and holds it | `filled_mib=1024 tag=0xa1 verify_bad=0`; guest `nvidia-smi` 1123 / 2048 MiB |
| vm2 allocates 64 MiB chunks until the allocator refuses, copies each back and scans | 1728 MiB scanned (`CUDA_ERROR_OUT_OF_MEMORY` after that): **0 records, 0 non-zero words** |
| vm1, second process, scans its remaining 640 MiB while A1 is held | 0 records, 0 non-zero words |
| positive control in vm1: a process fills 256 MiB with tag 0xA0 and exits, the next process scans | 16777216 records of tag 0xA0 (all of it): the scanner sees stale data, and the guest RM does not scrub its own frees |
| vm2 fills 1 GiB with tag 0xB2 and holds it; vm1 scans | 640 MiB scanned: **0 records of 0xB2** (only its own stale 0xA0) |

Why this holds by construction:

1. The two reservations are disjoint host allocations at fixed slots. The device asks for
   exactly its slot and refuses anything else the RM hands back (`hostmem.rs` `reserve`); a VM
   that would not fit is refused (item 5).
2. Each guest RM allocates only from the FB heap it was told about, which is its own reservation
   (memory-model.md §1).
3. The FB store refuses and counts every access outside the reservation (0 here).
4. Every GR mapping passes the D16/D17 filter, which admits only the device's own reservation
   and registered RAM (§3a).
5. The fresh slot is scrubbed by the host (§4b), so a scan finds zeros and not the previous
   owner's data.

What the construction does not cover: the copy-engine channels run on the guest's own page
tables with no filter (execution.md §7). A stock guest never points a vidmem PTE outside its
heap. A malicious guest kernel could, and then only the missing CE filter would stop it (§8.2).
The intra-VM positive control shows the guest RM does not scrub freed memory. That is a guest
matter between processes of one VM, not a cross-VM one.

## 4. Teardown hygiene

### 4a. Fail-closed teardown

Before P20, `HostMemory::shutdown` logged a failed free and carried on. A failed mirror-channel
free was followed by unpinning guest RAM, which is the inverse of upstream's rule
(nova-vgpu-upstream.md §3.5 and §4 item 1). Now:

- `run_teardown` frees in order: mirror objects, descriptors, reservation, client. It stops at
  the first failure. No descriptor is freed after a failed mirror free, and the reservation is
  never freed after a failed unpin.
- Any host free that failed earlier in the device's life counts as well. `free_object` and
  `free_live` record every failure.
- `release_region`, the client's DMA_UNMAP, keeps the descriptors while mirror objects live, or
  after any failed free, and leaves them to the teardown.
- On a fail-closed teardown the device logs `FAIL-CLOSED teardown: ...` and exits with status 3,
  from both the normal path and the signal thread. The close of `/dev/nvidiactl` then lets the
  host RM free the client in its own dependency order.

The decision is a pure function over a `TeardownOps` trait, unit-tested with a scripted fake:

- a clean order;
- a failed mirror free keeps all four descriptors and the reservation;
- an earlier failure also fails closed;
- a failed unpin stops before the reservation;
- a failed reservation free keeps the client.

On hardware (`e4-failclosed/`), vm2 ran with `CARAXES_INJECT_TEARDOWN_FAILURE=1` (one counted
failure; nothing freed wrongly):

```
hostmem: earlier failed host free: injected by CARAXES_INJECT_TEARDOWN_FAILURE=1
hostmem: freed 60 of 60 host mirror object(s) before the descriptors
hostmem: FAIL-CLOSED teardown: 1 host object free(s) failed before the descriptors; kept 4 guest
  RAM descriptor(s) (4096 MiB pinned) and the reservation [0xe9000000, 0x169000000) (kept: true); ...
exit 3: teardown failed closed (1 host object free(s) failed before the descriptors)
```

After the process exit, host used memory fell by the 2048 MiB of the slot. The fd close freed
it, with 0 Xid.

### 4b. Scrubbing the reservation: the host RM does it

Source reading, 610.57.04 (`vendor/open-gpu-kernel-modules/src/nvidia/`):

- `mem_mgr.c:200-208` enables scrub on free by default ("for GK110+") unless the registry key
  `RmDisableScrubOnFree` is set.
- `mem_mgr_gm107.c:1535-1545` turns it off only for Windows WDDM, simulation, MODS, a vGPU host,
  a paravirtual guest, `RMCFG_FEATURE_PLATFORM_GSP` (the firmware build, not this host's kernel
  RM) and SLI. None applies here. `/proc/driver/nvidia/params` shows `RegistryDwords: ""`, so
  there is no override.
- `memmgrScrubHandlePostSchedulingEnable_GP100` (`mem_mgr_scrub_gp100.c:40-70`) builds the
  scrubber and registers it with PMA.
- The reservation is an `NV01_MEMORY_LOCAL_USER` with `FIXED_ADDRESS_ALLOCATE`, which is a PMA
  allocation with `PMA_ALLOCATE_SPECIFY_ADDRESS_RANGE` (`video_mem.c:236-249`). It is not
  allocated with `SKIP_SCRUB`, so it does not carry `PMA_ALLOCATE_NO_ZERO` (`video_mem.c:259-261`).
- Its free goes through `vidmemPmaFree` (`video_mem.c:367-410`) to `pmaFreePages`
  (`phys_mem_allocator.c:1390-1540`). With scrub on free and a valid scrubber, every freed frame
  becomes `ATTRIB_SCRUBBING`, and `scrubSubmitPages` queues a copy-engine memset. The free
  returns once the work is submitted.
- A later allocation of those frames waits for the scrubber (`scrubCheckAndWaitForSize`,
  `phys_mem_allocator.c:1057-1075`).

The device therefore adds no CE memset of its own. The proof (`e4-scrub/`) uses a sampled read
of the reservation through BAR1, done by the device itself with `CARAXES_P20_SCAN=1` (`hostmem.rs`
`scan_reservation`: every 16 MiB a 2 MiB window, 64 KiB read, pattern records and non-zero
dwords counted):

| Step | Result |
|---|---|
| vm1 (A) holds the 1 GiB pattern 0xA1; `down`; the device's scan **before the free** | 327680 pattern records, 1316332 non-zero dwords of 2097152 sampled (records at offsets 0x7000000..) |
| the free | `freed the VRAM reservation [0x169000000, 0x1e9000000) (2048 MiB) in 1 ms (host RM scrub on free)` |
| vm1 again as VM C, slot 0: the device's scan **after the reservation** | 0 pattern records, 0 non-zero dwords |
| VM C's guest scans everything it can allocate | 1728 MiB: 0 records, 0 non-zero words |

Every other fresh reservation in P20 scanned to 0 non-zero dwords as well, so no host data
reached a guest on the allocation side either.

The cost for 2 GiB (`e4-scrub/timing-*.log`, a standalone device X reserving slot 0 and exiting,
then Y reserving slot 0 at once, three reps): X's free returns in 1 ms, and Y's allocation of the
same range takes 38, 36 and 23 ms. That is the wait for the host's asynchronous scrub, done by
the host's CeUtils channel. The operator's budget question is answered by those tens of
milliseconds per 2 GiB, paid by the next owner of the range.

Upstream's two remaining directions are covered too:

- Guest data reaching the host desktop's next allocation: the same scrub on free.
- Host data reaching a new VM: every earlier owner freed through a scrubbing RM, and the scans
  above read zero.

A device-side memset stays unnecessary as long as the host keeps scrub on free. If a host sets
`RmDisableScrubOnFree`, the device should refuse to reserve. That check is not built yet: the
registry key is not visible to an unprivileged client, so today it is a documented precondition
of this host.

## 5. The refusal case

`e5-refusal/`: with vm1 and vm2 up and holding their patterns, vm3 (`CARAXES_RESERVE_SLOT=2`,
2 GiB) was started:

```
vfio-user-nvgpu: hostmem: refused: reserving 2048 MiB would leave the host 569 MiB of VRAM, below
the 1024 MiB headroom (free 2617 MiB = min(HEAP_FREE, nvidia-smi); shortfall 455 MiB); choose a
smaller profile or free host VRAM
ERROR: server did not create /mnt/vmstore/caraxes/vm3/gpu.sock
```

The device stopped before its socket existed, so Cloud Hypervisor never started, and host VRAM
stayed at 5154 MiB. Afterwards nvprobe 3 passed on vm1 and vm2, both pattern holders were still
alive, and the host logged 0 Xid and 0 refused BAR1 mappings.

## 6. What P20 proves and what it does not

It proves:

- Two stock guest drivers execute CUDA, including cuDNN, PyTorch and RL workloads, concurrently
  on the physical GPU that drives the desktop.
- VRAM is partitioned into fixed slots with the D11 refusal rule. A third VM is refused before it
  starts.
- VRAM does not leak between VMs, by construction and by test. The host scrubs a slot before it
  is reused (measured, not assumed).
- The teardown fails closed.
- No compressible kind or comptag line was ever seen.
- No Xid was raised.

It does not prove, or it found the opposite:

- **No QoS.** The runlists are shared, the host time-slices contexts, and one VM's heavy work
  delays the other's and the desktop's. BAR1 was the first shared resource to run out (§2.3).
  Budgets now exist for BAR1, but not for time, channels or RM work.
- **Guest-triggerable host impact through mapping scale** (§2.4 and §8.1).
- **An isolation hole in the sysmem path** (§8.2). CE channels walk guest tables unfiltered. The
  IOMMU contained this instance only because the neighbouring IOVAs were unmapped.
- **The host desktop is on the same card.** Its allocations, BAR1 and GPU lock are shared. It
  stayed usable throughout, but one of its GPU processes restarted once (§2.4).
- **Interrupt forwarding is not yet progress-gated** (execution.md §7). An idle VM still receives
  the host's non-stall wakeups, which is a timing side channel.
- **Malicious guests are untested.** Everything here used the stock driver. The CE path would let
  a malicious guest kernel name any VRAM address in a PTE until the CE channels move to the
  filtered shadow VAS.

## 7. Code changes in this package

| File | Change |
|---|---|
| `crates/vfio-user-nvgpu/src/hostmem.rs` | fail-closed teardown (`TeardownOps`, `run_teardown`, `TeardownOutcome`, `fail_closed`, failure recording in `free_object` and `free_live`, `release_region` defers while mirrors live); `Window` releases its BAR1 mapping with `RM_UNMAP_MEMORY`; `ReservationMapper` (lock-free windows); `scan_reservation` and `count_pattern` (`CARAXES_P20_SCAN=1`); free timing in the log; unit tests |
| `crates/vfio-user-nvgpu/src/fb.rs` | `VramStore` maps through the mapper; budget `CARAXES_BAR1_WINDOWS` x `CARAXES_BAR1_WINDOW_MIB` (default 2 x 32 MiB); evict-own-and-retry on a refused map; stall instead of dropping writes; `stalls` counter; unit test under a host with room for one window |
| `crates/vfio-user-nvgpu/src/shadow.rs` | every sync line and the summary carry the extents of all mapped PAs (vidmem extent, maps per sysmem chunk) |
| `crates/vfio-user-nvgpu/src/boot.rs`, `main.rs` | mapper wiring; the budget in the startup line; `stalls` in the summary; exit status 3 after a fail-closed teardown |
| `crates/nvrm-abi/src/lib.rs` | `NvDevice::try_clone` (dup) |
| `tools/caraxes-vm.sh` | documents the BAR1 budget options |
| `caraxes/tools/p20.sh`, `p20-cuda-guest.sh`, `p20-patprobe.c`, `p20-check.py` | the P20 tools |

## 8. Findings to build on (not implemented today)

### 8.1 Mapping scale: a per-device mapping budget

- The measured numbers: 545k leaves and about 556k per-leaf fixed `RM_MAP_MEMORY_DMA` calls per
  device, 0.8 s per walk, and a VA-space teardown with half a million mappings that held the host
  GPU lock for minutes. Host `nvidia-smi` timed out after 20 s, and the desktop's electron GPU
  process restarted at 08:04:34.
- A per-device mapping budget: refuse (or stall) guest mappings beyond N mirrored leaves, and log
  it.
- One RM map per physically contiguous run of guest leaves instead of one per 4 KiB leaf. Guest
  allocations are mostly contiguous inside the reservation, so this should cut the count by
  orders of magnitude.
- An incremental teardown: unmap and free the shadow VAS in bounded batches while the guest is
  still alive, never one huge free at the end.

### 8.2 The sysmem crossing hole

- Guest RAM is registered in 1 GiB chunks whose IOVAs descend as the GPA rises: GPA 0 is at
  0x7fff80000000 and GPA 1 GiB is at 0x7ffec0000000 (D13, memory-model.md §2).
- The guest module `caraxes_dma_range` installs a `dma_range_map`, and Linux translates a
  mapping by its start address. A physically contiguous guest buffer that crosses a chunk
  boundary (here GPA 2 GiB) therefore gets one contiguous IOVA range, and its tail points past
  the chunk's end.
- The GR shadow filter refused exactly those leaves: vm1, `Sys(0x7fff00000000..0x7fff00017000)`,
  in both concurrent runs.
- The CE mirror channels walk the guest tables with no filter. The host IOMMU logged
  IO_PAGE_FAULTs for reads at 0x7fff00000000 + 0..0x5000 (domain 0x1e, the GPU) from 07:58:30 to
  08:00:41, with 190 lines logged plus about 300 suppressed every 6 s.
- The IOMMU contained it only because that IOVA range was unmapped. Had another VM's or the
  host's pages sat there, it would have been a cross-domain read.
- The fixes:
  - register guest RAM so that its IOVA is GPA plus one offset: a single contiguous registration
    if the RM accepts one, or chunks placed ascending and adjacent;
  - put the CE channels on the filtered shadow VAS like GR, so that no channel walks unfiltered
    guest tables (D17 stage 2).

These two holes are in the sysmem and mapping path. The D11 reservation isolation and the D16
comptag rule held throughout: no VRAM outside the two slots was ever touched, and no compressible
kind or comptag line appeared.

### 8.3 Closure (mapping package, 2026-09-24)

Both findings are closed in [mapping.md](mapping.md), first on vm2 alone at slot 0 (below),
then on two VMs at once (§8.4).

- **Mapping scale** (§8.1), run `caraxes/runs/mapping-2026-09-24/b1-async-streams`.
  - `suite-test_async_streams` passed in 14.8 s.
  - Its 544750 set-A leaves sat in 18921 RM mappings; 556k leaves were mapped by 30k calls in
    total, against 556k calls before. The walk only reads dirty table pages.
  - A per-device budget of 32768 live mappings refuses anything beyond it, with an error line.
  - The teardown of 18914 mappings took 2.5 s in bounded batches. Since the worker it runs in the
    worker's idle time.
  - Host `nvidia-smi` stayed under 0.06 s throughout, with 0 Xid.
- **Chunk-crossing hole** (§8.2), run `caraxes/runs/mapping-2026-09-24/d1-isolation`.
  - Guest RAM is registered with IOVA = GPA + 0x7ffac0000000 for the whole guest (chunks from the
    top down, a filler over the hole), so a crossing buffer keeps a valid IOVA.
  - The CE channels run on the filtered shadow VAS of their root, so no host channel walks
    unfiltered guest tables.
  - The whole cuda gate and every unscored suite passed with 0 refused sysmem leaves (128 before
    in b1), 0 IOMMU faults and 0 Xid.

### 8.4 The concurrency check on the mapping tree (2026-09-24)

Evidence: `caraxes/runs/mapping-2026-09-24/f1-concurrent/`. Both devices ran the main build
`0d00b26` (`target/mapping`, sha256 prefix 7b3c77fd2f92dd68), in the configuration of §1 with
linear IOVA and CE-on-shadow at their defaults. `CARAXES_BAR1_PASSTHROUGH` and
`CARAXES_DOORBELL_TOKENS` were unset.

**Placement.** The display agent's vm0 held 2 GiB at slot 1, `[0xe9000000, 0x169000000)`, so
the host had 4713 MiB free. Two 2 GiB profiles would have left about 620 MiB, below the D11
headroom of 1 GiB. The check therefore ran with two profile sizes, agreed with the
coordinator, which also exercises the slot arithmetic across sizes:

| | vm1 | vm2 |
|---|---|---|
| Profile, slot | 2 GiB, slot 0 | 1 GiB, slot 4 |
| Reservation | `[0x169000000, 0x1e9000000)` | `[0xa9000000, 0xe9000000)` |
| Guest RAM IOVA (one offset, `dma table`) | GPA + 0x7ffac0000000 | GPA + 0x7ff840000000 |
| Linear registration restarts (host allocations in the way) | 3 | 3 |

Host `nvidia-smi`: 3053 MiB used before, 6126 MiB with both VMs (exactly 3 GiB more), 1647
MiB free, 3076 MiB after the teardown. A first attempt to put vm2 at slot 2 with 2 GiB, made
while the coordinator's vm1 still held 1 GiB at slot 4, was refused by the host RM before the
device's socket existed (`NV_ERR_NO_MEMORY` for the fixed range, no floating fallback).

**The cuda gate on both at once** (`b-gate/`, started together at 13:36:51, 371 s and 354 s):

| Test | vm1 (2 GiB) | vm2 (1 GiB) |
|---|---|---|
| cuda-init, -context, -core, -jit, -launch, -hostreg, -torch | pass | pass |
| cuda-managed | fail (`AllocManaged 801`, the known gap) | fail (same) |
| cuda-gdb | skip | skip |
| rlprobe | pass (mean10=56.9) | pass (mean10=56.9) |
| convburn | pass (acc=3.041451216e+00) | pass (acc=3.041450977e+00) |
| robustness | pass | pass |
| suite-test_async_streams (unscored) | pass, 14.9 s | fail: torch `OutOfMemoryError` at 628 of 855 MiB (the 1 GiB profile is too small for the suite) |
| suite-test_cuda_graphs, high_freq_event_polling, multi_process_cuda, vram_churn | pass | pass |

The scored result is the single-VM green one on both VMs. The convburn sums differ in the
seventh digit on the 1 GiB profile. The gate accepts both. The likely cause is a different
cuDNN algorithm choice under less free memory, which was not checked.

**The mapping-scale finding (§8.1), on two VMs at once.**

- vm1's async_streams suite mapped 555907 leaves in 24186 RM mappings (240 syncs, 2.96 s in
  total).
- vm2's copy of the suite reached 543921 leaves in 12947 mappings before its OOM.
- Both teardowns ran in the worker's idle time.
- The host watch sampled host `nvidia-smi` 70 times at 5 s intervals: max 0.087 s, mean
  0.027 s, 0 alerts, 0 Xid (`b-gate/hostwatch.txt`). In P20 the same suite on one VM made
  `nvidia-smi` time out after 20 s.

**The chunk-crossing finding (§8.2), on two VMs at once.** `p20-check.py` over both devices
(`b-gate/check.txt`): **PASS**.

- Every mapped vidmem PA lies inside the device's own reservation, and none lies in the other's.
- Every sysmem leaf mapped comes from one of the device's own registered chunks, and none
  from the other's.
- Mapped per chunk: vm1 7921, 805, 563 and 18263 maps; vm2 13093, 2473, 4096 and 9398
  maps.
- The only refused leaves are the known kind-0x0F SKED leaves at FB 0x2000, 0x3000, 0x4000
  and 0x16000 (§3a), which lie in neither VM's range.
- Refused sysmem leaves: 0. FB store accesses outside the reservation: 0. No compressible
  kind was seen.
- Both devices mirrored 400 channels each, all 400 on host-managed shadow VASes. There were
  0 on an externally-owned VAS and 0 mirror refusals.
- The host logged 0 Xid and 0 `IO_PAGE_FAULT`.

**The pattern test** (`a-pattern/`, the same placement, earlier in the same window):

| Step | Result |
|---|---|
| vm1 fills 1024 MiB with tag 0xA1 and holds it | `verify_bad=0` |
| vm2 scans all it can allocate | 704 MiB: **0 records, 0 non-zero words** |
| vm1, second process, scans while A1 is held | 640 MiB: 0 records, 0 non-zero words |
| vm2 fills 512 MiB with tag 0xB2 and holds it; vm1 scans | 1728 MiB: **0 records of 0xB2**; 67108864 records of 0xA1, which is vm1's own freed A1 (the guest RM does not scrub its own frees, §3d) |

The a-pattern run's gate step did not run (a script error, then stale guest test material),
so its `check.txt` covers only the pattern workload. Its one host Xid (13:20:35, Xid 31, an MMU
fault on GRAPHICS) names pid 2731076, vm0's device. The checker counts it as "not
contained" because vm0 is not one of the two devices under test. It is outside this check.

**Teardown.**

- vm1, clean: 400 of 400 host mirror objects freed before the descriptors, then the
  reservation (2 ms, host scrub on free), then the 4 guest RAM descriptors.
- vm2 ran with `CARAXES_INJECT_TEARDOWN_FAILURE=1`: `FAIL-CLOSED teardown: 1 host object
  free(s) failed before the descriptors; kept 4 guest RAM descriptor(s) (4096 MiB pinned) and
  the reservation [0xa9000000, 0xe9000000)`, then exit 3.
- The fd close then let the host RM free the client. Host used memory went back to the
  pre-run value, with 0 Xid.

**Side observations, none isolation-relevant.**

- *Host BAR1 refusals.* The host RM logged 4 refused BAR1 mappings between 13:39:47 and 13:40:41
  (`reusemappingdbMap` `NV_ERR_NO_MEMORY`, 3 log lines each; `host-xid.txt` counts lines, so
  12).
  - The devices' VRAM stores counted 3 refused window maps (vm1 1, vm2 2), each followed by an
    eviction and a retry, with 0 stalls.
  - Host BAR1 (256 MiB) is shared by the desktop, vm0 and two stores of 2 x 32 MiB. It is
    near its limit with three devices, as in §2.3.
- *Unmap errors after the reservation free.* At vm1's clean exit the two BAR1 windows still
  open were unmapped after the reservation was freed. Both `RM_UNMAP_MEMORY` calls returned
  0x33 (`NV_ERR_INVALID_OBJECT_HANDLE`), because the RM had already dropped them with the
  memory. This is harmless, but the store should close its windows before the reservation
  goes.
- *DMA addresses refused by vm1.* vm1's device refused 8 DMA addresses outside its table in
  both runs, for example 0x109dfc000 and 0x10a9210a0. They lie in neither VM's IOVA ranges. They
  look like guest physical addresses, not IOVAs. vm2 refused none. The traces were overwritten
  before this was noticed, so the source is open.

**What stays open from P20.** The third-VM refusal case (§5) was not repeated: the check's
window ended with the gate. vm0 stayed up and untouched throughout.
