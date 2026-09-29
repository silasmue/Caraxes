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

### F2b built: the host fence (2026-09-28 night)

- **`CARAXES_SHADOW_FENCE=1`** (off by default; the Windows launcher sets it from P24).
  - GR and CE shadow VA spaces of the default layout hold [0x1_2000_0000, 0x1_4000_0000) with one
    host placeholder `NV50_MEMORY_VIRTUAL`, allocated right after the VAS and before any host
    buffer. The host's own set B lands first fit above it.
  - A guest leaf inside the fence carves its 2 MiB region out, under the same host lock: the
    placeholder is freed, the rest put back around the region, then the region object is
    allocated. An emptied region goes back into the fence. The teardown frees the fence.
  - Set-B exclusions inside the fence are dropped (`unfenced`): the host's copies are above
    it, so the guest's copies at its promoted VAs are plain set A.
  - A refused placeholder piece is a WARN ("a hole in the fence").
- **Linux:** unchanged (switch off). The video layout gets no fence.
- **Open:**
  - Host buffers above the fence can still meet guest VAs there. Windows' user allocations seen
    at 0x2_2000_0000; none in [0x1_4000_0000, 0x2_0000_0000) in P18-P23.
  - The fence size (512 MiB) comes from P18-P23 (guest RM range up to 0x1_21e4_0000).
- Depends on: L (a new part of the shadow layout), AB (set B moves), K (the compressed slice's
  kinds map through the same region objects; the fence only reserves VA).

## P24 (2026-09-28 21:29, P24-fence, slot 4 beside Linux B; fence on, pool off)

- **The fence works:** 8 fenced shadow VASes, 0 holes.
- **The first F2b build dropped set B wrongly** (commit fb78a283).
  - It dropped every set-B range inside the fence. The promoted buffers that carry their own
    PA (PATCH at 0x1200e4000, size 0x5000) are mapped by the host at the guest's VA (the
    deferred PROMOTE); only the size-0 global buffers are the host's own, placed first fit
    above the fence.
  - Dropping PATCH made the shadow map the guest's PATCH pages as set A: refused 0x1a (25 host
    0x51).
  - The per-region failure limit then left the guest's semaphore page 0x1_200ec000 unmapped:
    FE FAULT_PTE write, 6x from 21:30:32. Stopped at 21:31. D3D11 and CUDA not reached.
- **Correction (this commit):**
  - Only size-0 globals inside the fence lose their exclusion; PA-promoted buffers stay set B.
  - The fence is carved exactly: set-B ranges and per-leaf objects get their exact range, the
    rest of a 2 MiB region stays fenced, and a released range is fenced again.
  - Depends on: L, AB.

## P25 (2026-09-28 21:40, P25-fence-exact, slot 4, 1 GiB; fence on, pool off)

- **Boot clean:** 16 fenced shadow VASes, 3 fence holes (host 0x51 at 0x1_2010_4000 and
  0x1_200e_4000, the released PATCH still held by the host when re-fenced; retry open), no 0x1a.
- **D3D11 PASS**, own name in nvidia-smi ("Leandro VFIO RTX 2070-1G"); Device Manager shows the
  INF name (user decision).
- **CUDA:** cuInit and deviceQuery pass (driver CUDA 13.4, 36 SMs, sm_75);
  `cuDevicePrimaryCtxRetain` returned 999. Cause: the compute BIND refused a TSG with more than
  one hosted channel. Fixed in e705186b (the BIND goes to the whole host TSG).
- **Smoke:** vulkaninfo exit 1, D3D12 gate timeout (90 s), 5 nvlddmkm events (ids 14, 153).
- **Host Xid 31 at 21:43:33:** HOST_CPU FAULT_PTE read at 0x0477_4000, the teardown class of
  P21 (see P26).
- **The fence should become default-on for every guest** once proven (the user's
  4-5-knob goal); Linux B has not run with it yet. Depends on: L, AB.

## P26 (2026-09-28 22:00, P26-workload, slot 0, 4 GiB FB, 8 GiB RAM; fence on, pool off)

- **Setup:** Linux B and the lean VM down.
  - QEMU logged `vfio_wait_reqs - timed out` once at boot: registering 6 GiB of high RAM takes
    6 x ~650 ms in one DMA_MAP. Harmless here; an 8 GiB guest needs the map split or answered
    early.
- **D3D11 PASS** (create 14.7 s), nvidia-smi "Leandro VFIO RTX 2070-4G" 4096 MiB, WDDM, CUDA
  UMD 13.4.
- **Workload install (tools/windows/workload.ps1):** llama.cpp b10173 (= e9fa0781f) CUDA 13.3
  with cudart, Python 3.12.10, torch 2.13.0+cu130, both models, all sha256-checked; torch and
  llama.cpp need the VC++ 2015-2022 runtime. `ggml_cuda_init` finds the device (4093 MiB).
- **CUDA hangs in context creation** (cuda-probe killed at 300 s, no Xid).
  - CUDA's TSG (grpID 22) has eight GR channels, chids 24-31, one per subcontext. The guest
    promotes the context once, on chid 24, and sends one CILP BIND per channel allocation;
    each bound the host TSG with only chid 24 hosted.
  - Chids 25-31 were held forever by the diagnostic's first-use gate, which wanted a bind of
    their own; the deferred-context gate behind it (`target_submission_blocked`) wanted a
    promotion of their own (code reading; not reached in the run).
  - Fix (this commit): a member of a TSG whose context is promoted and live, or whose host TSG
    is set up (`Mirror::tsg_context_ready`), submits and joins the host TSG at its first
    doorbell, as on Linux (CUDA's 8 channels, one context share).
  - Likely the same cause: Vulkan and D3D12 also build multi-channel TSGs; the P21-P26 Xid
    below fits a hosted channel that waits on a semaphore released by a held member.
  - Depends on: none (gating only); the joined members use the host TSG's context (R, AB as
    before).
- **Host Xid 31 at 22:17:45 (smoke, after the D3D11 gate):** HOST_CPU FAULT_PTE read at
  0x0477_4000, the same VA as P21, P22 and P25.
  - Order in the trace: a teardown sync unmapped 809 leaves of root 0xff45_2000 at 1073.014 s
    while its GR channels (chids 24 and 28) were still hosted; the RC event came at 1073.067 s;
    the guest idled (chid 29 "GPGet 0 GPPut 30", a held member) and freed the channels after.
  - The guest invalidated 0x4773000, 0x4774000 and 0x4777000 repeatedly before: a small page
    set the KMD rewrites, likely a fence/semaphore page that HOST polls.
  - Stopped by the rule. Next: P27 with the member fix; if the Xid remains, the unmap rule
    (P21 section) with semaphore pages in the held set.
- **Not reached:** short kernel, llama, PyTorch, NVENC/NVDEC.

## P27 attempt (2026-09-28 22:28, P27-workload): the guest powered off before the driver loaded

- QEMU exited cleanly 66 s after the start (22:29:20): no QEMU error, no OOM, no segfault, no
  Xid. The device saw only firmware DMA maps and INTx setup, then the connection close.
- **Cause, from the guest's System log (read offline, read-only):** Windows Update installed the
  2026-09 security update KB5129195 (26200.9457) during P26 (event 43 at 22:10:45, while the
  workload stages ran); at P26's `Stop-Computer` it reported install failure 0xc1800109
  (event 20, 22:18:41), and `Component Based Servicing\PackagesPending` (an LCU merge) is still
  set.
- **Correction (M27-servicing, 22:38, boot without the GPU device):** the boot stops at the
  Windows recovery screen "code integrity could not be initialized, \\WINDOWS\\system32\\winload.efi,
  0xc0430001" and powers off about 60 s later (P27 did the same).
  - Offline, read-only: winload.efi, ci.dll, ntoskrnl.exe, VbsSiPolicy.p7b and
    driversipolicy.p7b were replaced at 22:09 (the LCU's staging), `WinSxS\\pending.xml` is
    there, the ESP's bootmgfw.efi is still the one of 2026-09-25. A half-applied LCU.
  - Not the device: it happens without the synthetic GPU.
  - Repair: WinRE (F1 at that screen) "Uninstall latest quality update", or DISM
    `/RevertPendingActions` from its prompt; fallback a fresh overlay on the base chain.
- Guest networking (QEMU user NAT, needed for the workload install) also lets Windows Update in,
  and Windows Update can deliver NVIDIA drivers: a risk for the stock 617.14.
- **Plan:** a maintenance boot without the synthetic GPU (`run27.py maint start`, WIN_GPU=0) to
  let the servicing finish, then `quiet.ps1 -Apply` (updates paused 7 days, no Windows Update
  drivers), then P27 again. Depends on: none.
- **Repaired (M28-repair, 23:05-23:20, boot without the GPU device):** WinRE (F1 at the error
  screen) refused "uninstall latest quality update" because of the pending actions;
  `dism /image:C:\ /scratchdir:C:\dismscratch /cleanup-image /revertpendingactions` from the WinRE
  prompt succeeded, and the next boot came up at build 26200.8037 (the pre-LCU build), CBS
  pending cleared, the workload install intact.
  - `quiet.ps1 -Apply`: updates paused until 2026-10-05, no drivers from Windows Update.
  - The DriverStore also holds nv_dispwi 32.0.16.1692 and nv_dispig 32.0.15.9186 besides the
    stock nv_dispi 32.0.16.1714; smoke's driver line shows which one binds.

## P28 (2026-09-28 23:21, P28-uvm1, merged with night-2026-09-28; slot 0, 4 GiB; fence on)

- **D3D11 PASS** (create 12.7 s). Stopped at the D3D12 stage by the Windows-device rule.
- **7 host Xid 31, 23:25:32-23:27:13:** GRAPHICS HUBCLIENT_FE FAULT_PRIV_VIOLATION writes at
  0x1_200f_c000, 0x1_2011_c000, 0x1_200f_f000, 0x1_2024_f000, 0x1_2038_f000, all inside the fence.
  - F3: chid 18, the guest maps 0x1_2011_c000 (a sysmem page), the shadow has a gap there, not
    set B: a leaf refused because a host page sits at that VA.
  - Checks: 46 guest leaves refused 0x1a, 90 host 0x51 inside the fence, 43 fence holes (P26: 3,
    no 0x1a).
- **Cause: my merge.** planfast moved the set-B computation into `GlobalExtents::set_b/refresh`,
  which size global buffers directly; the fence check stayed in `Mirror::global_extent` only. So
  size-0 globals inside the fence were set B again, the fence was carved around them, and the
  host's own buffers fell first fit into the holes, at the guest's VAs.
- **Fix 5738dc81:** `set_b` takes the root's fence (the shadow's layout, or `Mirror::fence_of`
  before the build); a regression test drives the `GlobalExtents` path (fails without the fix).
  Depends on: L, AB.

## P29 / P29b (2026-09-28 23:34-00:03, merged binary, fence fix; slot 0, 4 GiB, 8 GiB RAM)

- **No host Xid at all.** D3D11 PASS, **D3D12 PASS** (first time: fence round trip 2 us,
  dispatch verified), **Vulkan enumerates** (vulkaninfo exit 0, 617.14, API 1.4.351).
- **CUDA runs** (first time): context, vadd, short kernel median 72-76 us (native 6, Linux B
  13-20), launch + sync 199-512 us, HtoD 0.20 GB/s, DtoH 0.07 GB/s.
- **Managed memory with GSP_GET_FEATURES bit 0 = 1:** managedMemory 1,
  concurrentManagedAccess 0, 64 MiB CPU write + kernel increment + CPU check OK. The bit-0 half
  of the A/B is still to run.
- **llama 1B (b10173 CUDA 13.3):** pp512 2947 ± 2344, tg128 33.7 t/s (native 7639/305, Linux B
  7084/242).
- **P29 device drop:** QEMU's vfio-user reply timeout (5 s) fired during a 64 MiB DtoH copy: the
  software CE read VRAM at 10 MB/s for 6.7 s inside the message loop. Every Windows channel is
  PRIVILEGED, so CUDA's copies ran in software.
- **TDR at every CUDA exit:** the KMD evicts the TSG's context through the last channel left;
  the context was tied to the first (freed) channel: Xid 45 RC on the management channel.
- **NVENC/NVDEC in software:** the video channels carry the KMD's 0x5080 object; refused.

### Changed defaults after P29b (one universal configuration, no switches)

- **The deferred context belongs to its TSG** (c5deec2a): it moves to a live member when its
  channel is freed. Depends on: none.
- **Video channels may carry 0x5080** (1dc497c5): it stays an inert host object; the deferred
  calls run only on the management channel in the device. Depends on: none.
- **Application copy channels are mirrored** (f8faee97): a privileged CE channel whose root is
  also that of a skip-context GR channel (an application context) runs on the host, unprivileged
  with DENY_PHYSICAL_MODE_CE; the KMD's paging and scrub channels (system VA space) stay in
  software. A physical-mode method on a mirrored channel faults that channel only (to be
  reported per case). Linux never sets SKIP_CTXBUFFER_ALLOC: unchanged. Depends on: AB (the
  channel's leaves are shadowed like the GR channels'), L (the same shadow VAS), K.
- **The software CE is sliced** (ca751140): at most 40 ms per call, continued by a 1 ms clock;
  copies up to the guest's memory. Also Linux (scrub memsets). Depends on: R (VRAM reads through
  the BAR1 windows set the chunk size), P.
- The launcher's WIN_VFIO_USER_TIMEOUT_MS (8d3a3045) stays as a stopgap; with slicing the 5 s
  default suffices.
- Teardown exit 3 ("owned GFXP resources remain attached") is the P17 diagnostic's owned GFXP
  only; a production build has no owned GFXP set and does not take that path.

## P30 (2026-09-29 00:27-00:54, P30-uvm1: one binary, fixes 1-3, A, B; Linux B ran vkmark on slot 4)

- **Guest ABI 615 detected** from the RMARGS magic "GSP " (one binary, no feature flag).
- **No host Xid.** D3D11 PASS, D3D12 PASS, vulkaninfo exit 0.
- **CUDA:** copies on the host (rule A): HtoD 12.9 GB/s, DtoH 12.9 GB/s (P29b: 0.20/0.07). Short
  kernel median 157 us (Linux B's vkmark shared the GPU). No TDR at the CUDA exit (fix 1).
- **Managed memory, bit 0 = 1:** OK again (64 MiB CPU write, kernel increment, CPU check).
- **NVENC/NVDEC run** (fix 3): H.264 encode 14 fps, HEVC 5 fps, decode 23-35 fps (5 s clips of
  ffmpeg's generated 1080p60 source; the probe's frame-count parse of the encoded files failed).
- **Workloads** (00:44-00:53, beside Linux B's vkmark):

  | workload | native | Linux B | Windows P30 |
  |---|---|---|---|
  | llama 1B pp512 / tg128 | 7639 / 305 | 7084 / 242 | 4445 ± 2132 / 108 |
  | llama 7B -ngl 20 pp512 / tg128 | 1085 / 16.6 | 914 / 11.4 | 547 / 12.2 |
  | transformer tokens/s | 107485 | 105625 | 49552 (first step 22 s) |
  | ResNet-18 images/s | 532.5 | crashed | 190.2 (first step 121 s) |

- **TDRs left:** after a D3D12 or D3D11 process: the TSG's context moved to its last channel,
  which never rang (no host channel), and the eviction failed "host context is gone": Xid 45 on
  the management channel. Fixed in 6c566092 (such an eviction frees the host TSG). Smoke's D3D12
  gate timed out in that TDR.

## P31 (2026-09-29 00:58-01:26, UVM A/B; eviction fix 6c566092)

- **Smoke ALL PASS for the first time** (both halves): D3D11, D3D12, vulkaninfo, nvidia-smi,
  0 nvlddmkm events. No host Xid.
- **One TDR left (uvm1, 01:03:20):** the KMD's own NVENC channel (chid 8, boot-time, PRIVILEGED)
  ran in software. The RM sets PRIVILEGED on every kernel-allocated channel
  (kernel_channel.c 273-283), and video engines have no physical mode: fixed in bac4f5fa
  (privileged NVENC/NVDEC channels mirrored unprivileged). Depends on: none.
- **UVM bit A/B (GSP_GET_FEATURES bit 0):** Windows works the same with 0 (managed memory OK,
  concurrentManagedAccess 0 either way), but CUDA's pinned copies drop from 12.9 to 3.3 GB/s.
  Per-OS rule (Windows 1, Unix 0) or one value: a decision for the user.
- **Performance notes (after stability):** the first steps of the PyTorch workloads (17-121 s)
  are the KMD's paging channel in software: a candidate for a host CE path. NVENC 13-14 fps (boost
  off) against Linux 89-101 (165 with boost): P32 measures it with the boost on.
- **Decision (2026-09-29): the per-OS UVM rule stays** (Windows 1, Unix 0; the device decides
  from the guest's OS type at GSP_INIT, no switch). Reason, the A/B of P31:

  | | bit 0 = 1 (P31-uvm1) | bit 0 = 0 (P31-uvm0) |
  |---|---|---|
  | D3D11 / D3D12 / vulkaninfo | PASS / PASS / 0 | PASS / PASS / 0 |
  | managedMemory, concurrentManagedAccess | 1, 0 | 1, 0 |
  | managed 64 MiB CPU write, kernel +1, CPU check | ok | ok |
  | smoke | ALL PASS | ALL PASS |
  | pinned HtoD / DtoH (64 MiB x5) | 12.8 / 12.9 GB/s | 3.27 / 3.33 GB/s |
  | llama 1B pp512 / tg128 | 5857 / 197 (vkmark beside) | 5822 / 219 |
  | transformer tokens/s | 81776 (vkmark beside) | 84777 |

## P32 (2026-09-29 01:22-01:38, P32-boost: privileged video channels mirrored, host boost on)

- **No host Xid, no RC, no TDR, 0 nvlddmkm events; smoke ALL PASS.** The KMD's NVENC/NVDEC
  channels run on the host (8 build notes, no fault).
- **CUDA:** HtoD 12.9 GB/s, DtoH 13.0 GB/s; short kernel median 58.7 us, launch + sync 123 us;
  managed memory OK.
- **Media (boost on):** NVENC H.264 21 fps, HEVC 22 fps; NVDEC 30-40 fps (P31 boost off: 13/6,
  25-36). Still far below Linux (165/128 with the boost). The source is ffmpeg's testsrc2 generated
  on the guest CPU and uploaded through the driver: the next thing to measure (a probe with frames
  already in video memory, as Linux's nvenc-probe does).
- **Workloads, boost on (Linux B idle):**

  | workload | native | Linux B (cb4974fc) | Windows P32 |
  |---|---|---|---|
  | short kernel, us/launch | 6 | 13-20 | 58.7 (median) |
  | llama 1B pp512 / tg128 | 7639 / 305 | 7084 / 242 | 5931 ± 3172 / 176.7 |
  | llama 7B -ngl 20 pp512 / tg128 | 1085 / 16.6 | 914 / 11.4 | 542 ± 197 / 15.8 |
  | transformer tokens/s | 107485 | 105625 | 83769 |
  | ResNet-18 images/s | 532.5 | crashed | 341.7 (first step 98 s) |

- Teardown exit 3 at stop: P17 diagnostic only (owned GFXP), as noted.

## P34 (2026-09-29 02:35-02:57, P34-devce2: the device CE; slot 0, 4 GiB, 8 GiB RAM; fence on)

- First attempt (P34-devce): QEMU dropped the device at boot ("vfio_wait_reqs - timed out").
  - The 6 GiB DMA_MAP took 25 s to register under host memory pressure (1 GiB chunks 0.7 s
    rising to 7.5 s), beyond QEMU's 5 s `x-msg-timeout`.
  - Stopgap: run34.py sets a 60 s timeout. Fix: registration off the loop (below).
- Device CE at exit: 981 pushes, 981 ops, 29251 MiB, 1600 ms busy, all NV_OK; no fallback.
  P32 moved the same kind of memsets (19.7 GiB) on the CPU: 1804 slices, 75 s.
- Smoke all PASS, 0 TDR, no host Xid; D3D11 CreateDevice 7.4 s.
- Idle card (Linux B's vkmark overlapped a first round; those rates halved and are dropped):

  | | P34 | P32/P33 |
  |---|---|---|
  | transformer first step | 12.4 s | 17.7 s (P31) |
  | transformer tokens/s | 81689 | 83769 |
  | ResNet-18 first step | 61.8 s | 98-121 s |
  | ResNet-18 images/s | 350.6 | 341.7 |
  | CUDA short kernel median | 54.0 us | 58.7 us |
  | NVENC vram H.264 / HEVC fps | 106.1 / 117.0 | 123.8 / 133.6 (P33) |
  | NVENC session open / init | 6.0 / 5.1 s | 6.4 / 4.9 s (P33) |

- Host FB used: 5550 MiB at exit, 1007 MiB after the stop (Windows about 4.5 GiB: the 4 GiB
  reservation and about 450 MiB of host contexts).
- Open: NVENC 13 % below P33 (one sample each), with the session open and init times.

### Guest RAM registration off the loop (2026-09-29, registrar.rs)

- `DMA_MAP` is answered after the mmap and two quick checks: the mappings may still change,
  and the host RAM floor holds counting what is still queued.
- A worker registers the region in 1 GiB pieces; it faults each piece in before taking
  hostmem's lock (`MADV_POPULATE_WRITE`), so the loop's RM calls wait at most for one pin.
- Before a piece is registered:
  - the device's own accesses to it go through in GPA mode (`GuestMemory::translate`);
  - the shadow's descriptor snapshot waits for the worker (mirror.rs `descs`), and
    `host_iova` finds finished pieces the device has not applied yet: no guest access fails.
- The device applies finished pieces at its next access or on the 1 ms clock (main.rs).
- `DMA_UNMAP` drops the queued pieces of its region and waits for the one in flight.
- A failed piece leaves one ERROR line; DMA to it stays refused.
- Linux (CH, linear IOVAs): the same worker; the launcher already waits for the full table.
- Memory model: independent of the VRAM layout; depends on the DMA table (GPA mode).
- P35 (03:13-03:18, P35-registrar, QEMU's own 5 s timeout): boot clean.
  - 9 DMA_MAPs answered at once; OVMF's remaps unmapped 7 pieces still queued (dropped, never
    pinned); the 1 GiB chunks pinned in 55-130 ms after the pre-fault (P34: 0.7-7.5 s).
  - The shadow never waited for the registrar; 0 ERROR lines, no host Xid.
  - Smoke all PASS; CUDA short kernel 59.4 us median; transformer 78755 tokens/s, first step
    12.7 s; device CE 217 pushes, 2019 MiB, 104 ms.

## P36 (2026-09-29 03:28-03:31, P36-gate on 8e621e0a, profile on; Linux B on slot 4)

- Gate passed: smoke all PASS, no TDR, no host Xid, 0 ERROR lines, no guest access refused at
  the private pages, the shadow never waited for the registrar; device CE 114 pushes, 268 MiB.
- The CUDA probe's trap census (profile dumps around it): windows-traps-exit-2026-09-29.md.

## P37 (2026-09-29 03:47-04:26, the memory-window A/B under QEMU; slot 0, 4 GiB, Linux B off)

- Binary c6e651b1 (dev 8644a81c plus the launch-timing fix); each arm: smoke, cuda-probe,
  transformer and ResNet-18 60 s, NVENC vram; stopped cleanly, no host Xid in any arm.

  | | arm 0: as now | arm 1: BAR2 memfd | arm 2: + interrupt page |
  |---|---|---|---|
  | smoke | PASS (73 s) | PASS (36 s) | FAIL: 3 TDRs, Basic Display |
  | short kernel median | 75.6 us | 208.1 us | hung |
  | transformer first step / rate | 12.9 s / 82336 tok/s | 4.8 s / 50668 | - |
  | ResNet-18 first step / rate | 63.4 s / 371.8 img/s | 30.3 s / 207.1 | - |
  | NVENC open / init | 6.0 / 4.8 s | 1.3 / 0.6 s | - |
  | NVENC h264 / hevc | 119.8 / 138.4 fps | 61.5 / 68.2 | - |

- The launch-timing split works on Windows now (arm 0): a trapped doorbell to its host doorbell
  p50 11.9 us, 9.6 us of it the shadow USERD harvest (six uncached reads of the host USERD in
  vidmem through BAR1; fixed off the submission path in 3d5dd954); host doorbell to the next
  host event p50 78 us.
- Arm 1: 7450 syncs (476 ms in all), 67 KiB written back; the guest's time from a host doorbell
  to its next submission went from p50 7 to 50 us. Two explanations, tested in P38: the deferred
  flush (a submission waits for a sync owed until the next hand-over or 1 ms), or GPU writes to
  VRAM the KMD reads through BAR2, which the memfd shows stale (sampled drift check, 6d7db985).
- Arm 2 (the interrupt page needs the token pool and a polling reactor): nvlddmkm event 153
  three times, the KMD reset the GSP each time, Windows fell back to the Basic Display Adapter;
  the page's retrigger watchdog sent 3898 MSIs besides 5705 for raises.
- Open: the interrupt page on Windows. Hypothesis: the page shows `TAG` (bit 31) in every leaf
  and in `TOP_EN_SET` so the reactor can see guest stores (intr_page.rs); the Linux RM masks
  leaves with its enabled vectors, the 617 KMD may take bit 31 for a pending vector it can never
  clear (the watchdog MSIs fit). Isolation owed: arm 1 plus the token pool and the hybrid
  reactor without the page, and a trace of the leaf values and acks.
- Memory model: independent of the VRAM layout.

## P38 (2026-09-29 04:35-05:02, the harvest fix and the BAR2 memfd's loss; slot 0, 4 GiB)

- Binary 6d7db985 (P17 diagnostic applied). No host Xid in any arm, smoke PASS in all.

  | | a: as now | b: memfd, sync per flush read | c2: memfd, sync per doorbell |
  |---|---|---|---|
  | device side p50 (ring to host doorbell) | 7.0 us (P37: 11.9) | 8.2 us | 61.3 us |
  | short kernel median | 68.6 us | 919 us | 226 us |
  | transformer first / rate | 12.3 s / 86008 tok/s | 4.8 s / 53747 | 4.9 s / 45493 |
  | ResNet-18 first / rate | 62.3 s / 374.6 img/s | 30.2 s / 223.9 | 31.7 s / 219.8 |
  | NVENC h264 / hevc | 130.2 / 136.9 fps | 67.3 / 78.1 | 62.1 / 64.6 |
  | VRAM behind the memfd | - | 0 of 543 | 0 of 3865 |

- The harvest fix (3d5dd954): the USERD step 9.6 -> 4.5 us, device lock p99 120 -> 53 us,
  launch + synchronize 462 -> 348 us.
- The BAR2 memfd's steady-state loss is not the deferred flush (b), not GPU writes the memfd
  shows stale (drift 0), and not submission data written through BAR2 (c2: 278067 doorbell
  syncs found 1877 dirty pages in all). The device's share per ring is unchanged; the guest's
  time from a host doorbell to its next submission grows by about 40 us. Cause open; the memfd
  stays off for Windows.
- Memory model: independent of the VRAM layout.

## P39 (2026-09-29 05:03-05:06, P39-plain: the production binary, no diagnostic patch)

- Binary 888b17af (win-2026-09-29b on dev a48e54cf) built without diagnostic.patch: no P17 BIND
  handling, no first-use gate, no pool acknowledgement. Every gate since P17 carried that patch.
- Result: Windows does not start the driver: Device Manager Code 43, nvidia-smi exit 9, D3D11,
  D3D12, Vulkan and CUDA (`CUDA_ERROR_NO_DEVICE`) fail; no TDR, no host Xid, clean teardown.
- Cause in the trace: at 15.0 s the KMD asks `GR_GFX_POOL_QUERY_SIZE` (0x2080121f); dev refuses
  it (ctrl.rs `control_inner`: "host GFXP pool is not implemented", NOT_SUPPORTED); 0.4 s later
  the KMD unloads (GMCAPI `GSP_SUSPEND`, gsp reset 3). No host GR context was built, so the
  preemption-mode log had nothing to read.
- Consequence: the Windows path needs the pool bootstrap answered and the BIND handled (P17's
  owned GFXP today; the productization proposal carves the buffers from the VM's slice).
- Memory model: the GFXP buffers' placement depends on it (host vidmem today, the VM's slice
  proposed).

## Security fix: the device's private pages were reachable as guest memory (2026-09-29)

- **Before (all builds up to 70614b41's parent):** the top of the reservation holds device-owned
  pages: the invalidator's page tables, GPFIFO, pushbuffer and USERD (an ADMIN host channel, the
  top 256 KiB) and, since the device CE, its tables and ring (the 256 KiB below).
  - The software CE accepted any piece inside the reservation (`software_fb_range` = the whole
    reservation), so a guest push on a software channel could write those pages, physical or
    through a guest VA.
  - The shadow accepted any guest leaf inside the reservation (`resolve_leaf` up to its end), so
    a guest mapping of those pages ended up in a host channel's VA space.
  - With the invalidator's tables writable, a guest could redirect an ADMIN host channel's
    PTEs to any host VRAM address: an escape from the VM's reservation.
- **After (8dbce31a):** the top 512 KiB (devce.rs `PRIVATE_AT`) is no guest memory.
  - `software_fb_range` ends below it (ctrl.rs), so a software CE piece there is refused as
    outside VM backing;
  - the shadow's leaves end below it (mirror.rs `leaf_end`), so a guest leaf there is refused;
  - the device CE's window maps only the usable FB below the whole 2 MiB carve-out.
- **Closed next (BAR paths):** the guest RM builds its BAR tables itself and could point one at
  the private pages; the PRAMIN window (BAR0) reaches any FB page the guest names.
  - A trapped BAR1/BAR2 access whose target touches the private pages is refused: write dropped,
    read as zeros (fb.rs `bar_access`); a guest BAR table there is no table (`bar_translate`).
  - The BAR2 memfd never maps a private page nor reads one as a table (bar2area.rs).
  - A PRAMIN read or write there is refused the same way (boot.rs).
  - The first refusal leaves one line in gpu.log; the exit summary counts them all.
  - BAR1 passthrough and the BAR1 pool already stop below the 2 MiB carve-out.
- Depends on: R (the reservation and its carve-out).
