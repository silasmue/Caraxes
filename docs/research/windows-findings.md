<!-- SPDX-License-Identifier: MIT -->
# Windows findings

Running list for the Windows 11 guest (stock 617.14) on the synthetic TU106. One entry per
finding: evidence, fix, and the parts of the memory model it depends on, so each one can be
re-checked on the per-VM compressed VRAM slice (D16b).

Memory-model parts: **R** reservation (today one uncompressed host object), **L** shadow VAS
layout (vaBase, VAS start, internal range), **K** kinds and comptags, **Z** ZBC, **AB** set A/B
(guest leaves vs the host's own buffers), **P** BAR1 pool.
Runs: `/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/<run>/`.

## P17 (2026-09-28 08:15, P17-owned-compute-cta)

- **A lower-level COPY_SERVER_RESERVED_PDES was installed as the VAS root.**
  - Windows sends the control a second time with one PD1-level table (shift 29, range 1..0).
  - The shadow then walked a PD1 as the PD3, the GPFIFOs were never mapped, and the host took
    Xid 31 FAULT_PDE.
  - Fix 1, 4fac4c61: a call whose level 0 is not a root keeps the root. P18 confirms: 8 calls kept.
  - Depends on: L (the root the shadow walks), AB.
- **Nothing noticed when the shadow walked a root the guest no longer uses.**
  - Fix 2, 11de8415 (pdbwatch.rs): the guest's TLB invalidates name the real root; a VAS walked
    from a former root holds its host doorbells.
  - Replaying P17's trace flags all four wrong roots. P18: silent, no false positive.
  - Depends on: L.
- **MEM_OP_C TLB_INVALIDATE_PDB 0:0 was decoded inverted** (ONE is 0, clc46f.h:134).
  - Before, a single-PDB invalidate counted as ALL, and an ALL invalidate was dropped as PDB 0.
  - Fixed in 11de8415. Forwarding still invalidates every mirror VA space per MEM_OP; forwarding
    only the named root is open (needs a run).
  - Depends on: L.
- **Malformed table entries were followed** (a wrong root yields garbage 2 MiB leaves at 2^40+).
  - Fix 3, 7c1ce283 (ptcache): not followed; a root with ≥ 16 of them maps nothing. P18: none seen.
  - Depends on: K (kind check), AB.

## P18 (2026-09-28 19:34, P18-root-kept, beside Linux B)

- **Guest VAs below 1 MiB could not be mirrored.**
  - The shadow VAS had base 0 without ALLOW_ZERO_ADDRESS, so the RM starts it at 1 MiB
    (gpu_vaspace.c:1154, g_gpu_vaspace_nvoc.h:789) and refuses lower fixed VAs (0x1f).
  - Windows maps pushbuffers there (0x10000-0x13000, 0xbf000-0xcf000). First Xid: PBDMA
    FAULT_PDE at 0x13000; the FE fault at 0x10000 is the same class.
  - F1, 8f7a7f38: the default layout sets ALLOW_ZERO_ADDRESS. P19: every shadow VAS starts at 0,
    no refused leaf below 1 MiB, no low-VA fault.
  - Depends on: L.
- **The host's own buffers collide with Windows kernel data at 0x1_2000_0000 and up.**
  - Both RMs place kernel buffers at the lowest free VA ≥ 4 GiB outside the server-RM range
    [4 GiB, 4.5 GiB) (virt_mem_allocator_gm107.c:2771-2775), so the host's set B and the guest's
    kernel data both start at 0x1_2000_0000.
  - 190 guest leaves were refused there with 0x51 (host already owns the VA). FE FAULT_PTE at
    0x1_200ec000 (inside the guest's BUNDLE_CB) and 0x1_2037f000.
  - Open. VA_INTERNAL_LIMIT does not help on a GSP-client host: RM forces the internal range to
    [4 GiB, 4.5 GiB) (vaspace_api.c:374-379), which the host reserves for its own GSP
    (gpu_vaspace.c:410-417, `_gvaspaceReserveVaForClientRm`).
  - Candidate: a host-owned fence over the guest's kernel range at VAS creation, with guest
    leaves there mapped into the fence (touches the shadow's region objects).
  - Depends on: L, AB.
- **Fault attribution needed a guest-table dump.** F3, 8f7a7f38: each MMU-fault RC of a shadow
  channel logs the fault page's guest leaf, shadow leaf and set-B/private hits
  ("shadow fault diagnostic"). Depends on: L, AB.
- **The host RCs now reach the guest at once** (4d6ab0ee host notifier page): 4 in P18 instead of
  fence timeouts. Depends on: none.
- **The bar1 pool refuses remaps of guest page-table pages** (P17 and P18 gpu.log ERROR lines).
  Not yet attributed to a failure. Depends on: P.

## P19 (2026-09-28 19:55, P19-zero-base, beside Linux B)

- **F1 holds:** 6 shadow VASes start at 0, no refused leaf below 1 MiB, no low-VA fault.
- **The set-B exclusion of a size-0 global buffer swallows guest pages.**
  - The first Xid (19:56:10) was FE FAULT_PTE write at 0x1_2010c000. Then 0x1_200ec000: 3x
    FAULT_PTE and 1x FAULT_PRIV_VIOLATION. Stopped at 19:57.
  - Both VAs are exactly BUNDLE_CB + 0x3000: the promote entry 3 (size 0) is at 0x1_20109000,
    resp. 0x1_200e9000.
  - F3 on roots 0x3d485000 and 0x3d471000: the guest maps a 4 KiB sysmem page there (kind 0);
    the shadow has a gap; "set B true".
  - `promoted_ranges` sizes a size-0 entry by the host's engine context size rounded up to 64 KiB
    (mirror.rs `global_ctx_size`). The guest packs its next allocation (semaphores; P18 had CE
    semaphores at +0x5000) right after its own reservation, so the exclusion overshoots into
    guest data.
  - The PRIV_VIOLATION shows the write is non-privileged (pushbuffer-driven), and that the host
    placed a privileged buffer of its own in that excluded hole in one VAS.
  - Sizes (2026-09-28, from P19's recorded static info: the host's own
    GR_GET_ENGINE_CONTEXT_PROPERTIES answer, forwarded unchanged as
    KGR_GET_CONTEXT_BUFFERS_INFO):
    - The host allocates its BUNDLE_CB from that table: 0x3000, alignment 0x100
      (kgraphics_gm200.c:71-74).
    - RM maps an internal vidmem buffer below 64 KiB with 4 KiB pages
      (mem_mgr.c `_memmgrPickDefaultGpuPageSize`), so the host's VA footprint is 0x3000.
    - The guest was told 0x3000 and reserved exactly that span: [0x1200e9000, 0x1200ec000), its
      next page at +0x3000; F3 shows nothing mapped below.
    - Host size <= guest span: no real overlap.
  - Other size-0 globals (host size = footprint = the 64 KiB rounding): PAGEPOOL 0x20000,
    RTV_CB 0x80000, FECS_EVENT 0x10000, PRIV_ACCESS_MAP 0x80000, ATTRIBUTE_CB 0x3f3a00 (2 MiB
    pages, 0x400000). Only BUNDLE_CB and GFXP_CTRL_BLK (0x464) have a footprint below their
    64 KiB extent.
  - F2a (this branch): exclusion = [base, first guest-mapped page), never below the host's
    footprint and never above the 64 KiB extent.
    - A cut logs one WARN per clip ("set-B exclusion cut short").
    - Pages the guest maps inside the footprint are its own copy of the buffer (P20) and never
      shrink it.
    - Each FIFO_GET_ENGINE_CONTEXT_PROPERTIES answer is now logged unrounded ("global context
      size"), so P20 confirms the BUNDLE_CB number.
  - Linux: unchanged unless the guest maps inside the old 64 KiB overshoot; the host footprint
    is excluded as before.
  - Validated in P20 (below). Depends on: AB, L; the host footprint rule on RM's page-size choice.
- **Host-owned VAs in the guest's kernel range remain:** 64 host VA allocations refused with 0x51 in
  0x1_2000_0000..0x1_2200_0000 (P18: 192). No Xid attributed to them in P19. Depends on: L, AB.

### F2 designs

- **F2a, exclusion extent: built** (see the P19 entry above). Depends on: AB.
- **F2b, host fence (parked; larger, touches shadow.rs `apply`, workstream A's neighbourhood):**
  - A host-owned `NV50_MEMORY_VIRTUAL` over [0x1_2000_0000, 0x1_2000_0000 + F) is created with the
    shadow VAS.
  - The host's first fit then lands above it. Guest leaves inside it are mapped into the fence
    (hDma = fence, fixed offsets) instead of per-region objects; the fence is never freed while the
    VAS lives.
  - Removes the 0x51 collisions and the host's buffers in guest holes.
  - Guest user VAs above 0x1_2000_0000 + F can still meet the host.
  - Depends on: L, AB; the compressed slice (D16b) must keep kinds/comptags per mapped leaf inside
    the fence (K).

## P20 (2026-09-28 20:16, P20-global-clip, beside Linux B)

- **First D3D11 rendering on the device.**
  - Triangle/clear/triangle readback passed, 3 frames: D3D11CreateDevice after 13.1 s, frames
    4.7 s (frame 0 readback 4.3 s).
  - The smoke stage passed it a second time: create 6.8 s, frames 1.3 s.
  - 0 host Xid until 20:21:36.
  - F2a: 72a8034c; F1 still holds (22 shadow VASes start at 0).
- **The host's global-buffer sizes, read by the device:** FIFO_GET_ENGINE_CONTEXT_PROPERTIES
  engine 0x11 (BUNDLE_CB) = 0x3000, alignment 0x100. All six size-0 globals match the static
  table. Depends on: none.
- **The guest maps its own copy of each global buffer at the promoted VA** (kernel roots
  0x3d598000, 0x3d499000). Expected: the host's copy replaces it in the shadow. The first F2a
  binary logged these as overlaps; fixed in the WARN text and check-p20. Depends on: AB.
- **Names.**
  - nvidia-smi: "Leandro VFIO RTX 2070-1G", KMD 617.14, CUDA UMD 13.4, VBIOS 90.06.3d.00.76,
    1024 MiB, WDDM.
  - Win32_VideoController.VideoProcessor and the registry AdapterString/ChipType carry the
    Leandro name.
  - Device Manager shows "NVIDIA GeForce RTX 2070": the INF DriverDesc for 10de:1f02, subsystem
    1458:4011.
  - Open (user decision): the Device Manager name. A stock driver takes it from the INF; a
    FriendlyName override is a guest change.
  - Depends on: none.
- **Host set B at guest VAs now costs Xids (the F2b class).**
  - During the D3D12 gate the GR FE took FAULT_PRIV_VIOLATION writes at 0x1_200fc000 and
    0x1_2011c000 (4x).
  - F3: the guest maps a sysmem page there; the shadow refused it with 0x1a ("VA collision with
    set B"); the page is not in the guest's set B. The host's own privileged buffer sits at a
    VA where this process's VAS holds guest data.
  - 197 such refusals in the run.
  - Windows processes lay out 0x1_2000_0000.. differently, and the host's first fit does not
    know it.
  - Open: F2b or a host-only area for set B. Depends on: L, AB.
- **Vulkan device creation hits a host BAR1 CPU fault.**
  - vulkaninfo: vkCreateDevice ERROR_DEVICE_LOST in nvoglv64.dll.
  - Same moment (20:21:36): host Xid 31, ENGINE HOST0 HUBCLIENT_HOST_CPU FAULT_PTE read at host
    BAR1 VA 0x04774000, pid of the Windows device.
  - A CPU access through a BAR1 PTE that is gone: the FB store's windows (2 x 16 MiB) or the
    BAR1 pool (32 MiB).
  - Not attributed; the trace has no window or pool move at that point. Depends on: P, R.
- **D3D12 hung** (gate timeout after 90 s), with the FE faults above. Not attributed separately.
- **Test-pack bugs**, fixed on this branch:
  - A `""` inside an expandable string broke smoke.ps1's parse.
  - Start-Process `ExitCode` is `$null` without a cached handle: every "exit" was empty and read
    as FAIL.
- **User decision, 2026-09-28 ("ja Puffer in den SLICE der VM das ist sauberer"): the owned
  GFXP buffers go into the VM's own VRAM slice.**
  - Today each GR context gets 6.47 MiB (6778880 bytes) of ordinary-GFXP buffers as separate
    host objects outside the reservation. P20's nvidia-smi showed the device at 1049 MiB = the
    1024 MiB reservation + about 4 live GFXP sets.
  - Target: allocate them from the VM's slice, so everything a VM uses is one contiguous piece.
  - Open. Not before the new memory architecture: hostmem.rs belongs to workstream A (the
    compressed per-VM slice, D16b).
  - Depends on: R, and the slice allocator.

## P21 (2026-09-28 20:33, P21-pool-off, beside Linux B; BAR1 pool off, compression off)

- **D3D11 renders without the BAR1 pool too.**
  - Probe: create 14.2 s, frames 1.5 s.
  - Smoke: create 4.9 s, frames 0.37 s.
  - nvidia-smi -L and -q pass (exit codes now read correctly).
  - Depends on: P (the pool is not needed for rendering).
- **The Vulkan-time Xid is not the pool.**
  - HOST0 HUBCLIENT_HOST_CPU FAULT_PTE read at VA 0x04774000, identical in P20 (pool 32 MiB) and
    P21 (pool off).
  - 0x04774000 is a guest VA, not a host BAR1 offset: the guest's paging channel mapped it at
    148.5 s (MEM_OP targeted invalidate, A 0x4774001, PDB 0x3f45f000, shadow root of GR chids 24
    and 28).
  - The fault comes at the teardown of the failed vulkaninfo process: a register invalidate of
    that root at 151.83 s re-synced the shadow, which unmapped 619 leaves, before the guest freed
    the channels, TSG and client.
  - A host channel on that root still read the page. HOST_CPU is likely the host scheduler
    re-polling a pending semaphore acquire.
  - Open: a shadow must not unmap pages its live host channels can still reach. Options: disable
    or preempt them first, or hold the unmaps until those channels' host free, as set B is held
    (`held_at_free`).
  - Depends on: AB, L.
- **D3D12 and Vulkan device creation wait forever on the P17 diagnostic.**
  - GR chids 29 and 30 were never bound: 9 GFXP BINDs refused ("P17 accepts only captured
    graphics pool binding shape", 0x56).
  - Their work stays held ("held until owned GFXP is bound", 2717 and 1201 replays), and device
    creation times out: the D3D12 gate hit its 90 s timeout, and vkCreateDevice returned
    DEVICE_LOST.
  - The refused payloads are not logged yet. Next: dump them, then translate that shape
    (likely a combined GFXP + compute-CTA request).
  - The pool bookkeeping stays unimplemented (P12: the driver needs real pool support).
  - Depends on: R (owned GFXP buffers, see the slice decision above), AB.
- **Held-doorbell replay spams the trace.** replay_held re-runs every P17-held channel at each scan
  (thousands of "enabled again ... held" pairs). Keep them held without replay until they are
  bound. Depends on: none.
- **Host set B at guest VAs:** 190 refusals 0x1a (F2b class), no Xid attributed in P21. Depends
  on: L, AB.
- **Teardown:** exit 3 "owned GFXP resources remain attached to a host context" (as in P17).
  Host FB/BAR1 returned to baseline. Depends on: R.

### The shadow must not unmap under a live host channel (P21; a Linux correctness item too)

- **Rule:** a guest leaf that a live host channel of the root can still reach is not unmapped from
  the shadow until that channel is idle or freed on the host. The guest's own order (invalidate,
  then FREE of channel/TSG/client) leaves a window in which the host channel still runs or
  re-polls on pages the shadow already unmapped: P21's HOST_CPU FAULT_PTE at 0x04774000.
- **Options:**
  - **(a) Hold unmaps per root while host channels live.** An unmap planned on a root with live
    host channels is deferred; the leaves stay mapped (read-only is not available per leaf) until
    the host free of the last channel on the root, or until a guest remap of the same VA replaces
    them.
    - Mirrors set B's `held_at_free`.
    - Cost: stale guest pages stay reachable for the dying channel only. Isolation holds: they
      are the guest's own pages, and the registered-range check was done at map time.
  - **(b) Preempt or disable first.** Before applying a plan with unmaps on a root, disable and
    preempt its host channels (TSG preempt, GPFIFO_SCHEDULE off), then unmap, then re-enable.
    - Exact, but costs a host control round trip per invalidate with unmaps. That hurts the hot
      path on Linux (UVM unmaps often) and on Windows (paging channel).
  - **(c) Hold only for channels the guest already disabled or stopped.** The teardown case of
    P21: the guest's STOP_CHANNEL/disable comes before the invalidate. Narrow and cheap, but misses
    unmaps while a channel still runs normally (a guest bug that real hardware would also fault
    on).
- **Proposal:** (a), bounded to unmaps whose VA lies inside a live channel's recently used set:
  GPFIFO, USERD and pushbuffer segments of the last entries. The cheap first cut is the GPFIFO and
  pushbuffer VAs the device already decodes. (c) comes as the teardown special case.
  - Linux keeps its behaviour for every unmap outside those ranges.
- **Tests:** a unit test with a root that has a live host channel, a guest invalidate that drops
  its GPFIFO leaf, and the channel's host free afterwards (the unmap applied only then).
- Implemented after P22. Depends on: AB, L (the shadow's apply path, shared with the compressed
  slice's kind handling).

## P22 (2026-09-28 20:45, P22-bind-payloads, beside Linux B; pool off, compression off)

- **D3D11 PASS again** (create 14.0 s, 3 frames 5.2 s).
- **CUDA reaches the device, not a context.**
  - deviceQuery OK: "Leandro VFIO RTX 2070-1G", 1021 MiB, sm_75, 36 SMs, 1620/7001 MHz, 256-bit,
    L2 4 MiB.
  - cuDevicePrimaryCtxRetain returns 999 CUDA_ERROR_UNKNOWN.
  - NVENC and NVDEC (ffmpeg) fail at their own cuCtxCreate, the same cause.
- **The GFXP BIND shapes** (payload log; `NV2080_CTRL_GR_CTXSW_PREEMPTION_BIND_PARAMS`, ctrl2080gr.h:917):

  | flags | gfxp | cilp | buffers | Sender | P17 diagnostic | P23 decision |
  |---|---|---|---|---|---|---|
  | 2 GFXP_SET | 2 GFXP_POOL | 0 | pool VAs + CPU ptr | D3D11 contexts | owned GFXP mode 1 | unchanged |
  | 1 CILP_SET | 0 | 1 CTA | none | D3D11 contexts | host CTA bind | unchanged (`bind_owned_compute`) |
  | 1 CILP_SET | 0 | 2 CILP | none | CUDA contexts (3 of them) | refused | host CILP bind + GET, else NOT_SUPPORTED; never downgraded to CTA |
  | 2 GFXP_SET | 0 WFI | 0 | none | 9 other D3D contexts | refused | host GET; OK only if the host context is in WFI |

  - User decision via main: no silent CILP-to-CTA downgrade.
  - A context whose request was carried out without an owned GFXP set is marked
    `preemption_ready`; the first-use gate opens for it.
  - Depends on: R (owned GFXP buffers; into the VM's slice later), AB.
- **The P21 teardown fault repeats** (HOST_CPU FAULT_PTE at 0x04774000, 20:50:40, during smoke).
  Fix: the unmap rule above (after P23). Depends on: AB, L.
- **Held-doorbell replay:**
  - P22's quiet replay worked: P17-gated replays fell from thousands in P21 (chid 29: 2717,
    chid 30: 1201) to 8.
  - The remaining 10112 "enabled again" lines are chid 15, the software paging CE channel
    re-polling its blocked semaphore acquires. That is by design (`blocked_software_channels`);
    only the per-replay trace line is noise.
  - Next (small): trace that replay only when the acquire is satisfied.
  - Depends on: none.
- **Linux B was relaunched by the main session at 20:48:57, during P22** (new pids
  708175/708207). Its "untrapped GPPut ... ran ahead of its doorbell" ERRORs are its own
  channels' evidence counters, not caused by the Windows device.

## P23 (2026-09-28 21:15, P23-cilp-wfi, slot 4 beside Linux B slot 0 with 4 GiB; pool off)

- **D3D11 PASS** (create 5.8 s, 3 frames 0.38 s).
- **Stopped at 21:18 by the Linux-B rule, before CUDA ran.**
  - A host Xid 31 named Linux B's device at 21:16:24: GR GPC1 FAULT_PDE write at
    0x7287_aa001000, during main's CUDA tests in Linux B.
  - Linux B's trace RC'd chids 21 and 22 on shadow root 0xef201000. Its F3 line read an empty
    fault record: `GET_MMU_FAULT_INFO` of those channels is zero when the fault belongs to another
    channel of the TSG.
  - Causality from Windows not shown: separate reservations, VASes and host clients.
  - Open: F3 should read the fault record of every channel of the TSG. Depends on: none.
- **Windows Xid at boot (21:16:51):** HOST0 HUBCLIENT_HOST FAULT_PDE read at 0x1_20223000, a
  PBDMA fetch.
  - The host refused guest leaves at 0x120220000-0x12022e000 with 0x51: host set B holds those
    VAs in that process's shadow VAS.
  - The F2b class now blocks at boot, and the stop rule then ends every run before CUDA.
  - Priority: F2b first, then the unmap rule. Depends on: L, AB.
