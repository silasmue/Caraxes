<!-- SPDX-License-Identifier: MIT -->
# Execution on the physical GPU (E5b part 2)

Written 2026-09-23. Driver citations are relative to `vendor/open-gpu-kernel-modules/`
(tag 610.57.04); `D/` = `crates/vfio-user-nvgpu/src/`. This is the device side of
[host-exec.md](host-exec.md) §2 (the wiring spec) and [cuda-path.md](cuda-path.md) §4
(option (a) + (c1)). Part 1 ([memory-model.md](memory-model.md)) made every guest address
final while the device still executed pushes in software; part 2 backs the framebuffer
with real VRAM and hands the guest's channels to the physical GPU.

Turned on with `--host-exec` (launcher `CARAXES_HOST_EXEC=1`), which needs
`--reserve-vram`, `--register-guest-ram` and `--host-rm` (the guest's FB and sysmem
addresses must be the host's before anything runs on the card). Evidence is in
`caraxes/runs/e5b2-2026-09-23/`.

## 1. What runs where

| Piece | Where it runs now |
|---|---|
| Guest FB (PRAMIN, BAR1, BAR3, the software executor's own reads/writes) | the reserved VRAM, through host BAR1 windows (`D/fb.rs` `VramStore`) |
| CUDA's copy-engine channels (COPY2-4 TSGs) | host `TURING_CHANNEL_GPFIFO_A` mirror channels on the physical CEs (`D/mirror.rs`) |
| GR channels (a `TURING_COMPUTE_A` is on them), nvidia-uvm's own channels (a `GP100_UVM_SW` is on them) | the software executor of `D/channel.rs`, over the VRAM store |
| GR0 / CE2-CE4 non-stall completions | host OS events → guest interrupt vectors + MSI (`D/mirror.rs`, `D/intr.rs`) |
| The guest kernel's register TLB invalidates, and the software executor's `MEM_OP` invalidates | a `MEM_OP MMU_TLB_INVALIDATE` of the guest PDB from a device-owned **privileged** channel (`D/invalidate.rs`); `DMA_INVALIDATE_TLB` on a mirror VAS is a no-op |

Host channel execution is gated behind `CARAXES_MIRROR_CHANNELS=1` (default off) while the
two execution walls below stand: with it off, `--host-exec` is the VRAM store plus the
mirror scaffolding (VA spaces, events, doorbell) and channel *work* stays in software, which
is a clean, no-Xid state that passes cuda-init/context/core/hostreg on real VRAM. With it on,
CUDA's CE channels run on the physical copy engines (measured, no Xid) up to the first-launch
stall (§6).

## 2. FB backing on real VRAM (option C)

`D/fb.rs` `VramStore` implements the `FbStore` interface behind `FakeFb` (part 1 kept it as
`HostRamStore`). Every guest FB access inside the reservation `[base, end)` goes to real VRAM
through a window of the reservation mapped into the device process over the host's BAR1
(`RM_MAP_MEMORY` on a fresh `/dev/nvidia0` registered to the client, then `mmap`, exactly
host-exec.rs `map_window` / `nvrm-client` `map_doorbell`).

- **Windows.** 64 MiB each (the largest RM granted in host-exec.md §1), halved on
  `NV_ERR_NO_MEMORY` down to 2 MiB; up to 3 held at once, LRU-evicted. A window comes back
  with its data intact because it is the same VRAM.
- **Bounds.** Addresses outside the reservation (the guest's fake WPR above `end`, which no
  engine ever sees) fall back to host RAM and are counted, as before.
- **Write filter.** The store sits behind a no-op `WriteFilter` hook (`FakeFb::set_write_filter`)
  where the page-table write filter of [dma-routes.md](dma-routes.md) §4 will sit; not
  implemented now, as the design requires.
- **Measured** (a full cuda gate window): ~1.2M reads / 9.4 MB and ~212k writes / 1.6 MB
  through 2 windows, 0 refused, 0 evictions (the guest's working set fits in the two windows).

**Write-combining (a finding).** BAR1 mappings are write-combined. The device writes the
guest's GPFIFO entries, USERD and page tables through a window and then tells the physical
GPU to read them (the host doorbell, a TLB invalidate). If the WC buffers have not drained,
the GPU reads stale VRAM: the PBDMA fetch of `GPFIFO[0]` faults with **PBENTRY** (`NV_PPBDMA_INTR_0`
bit 18) microseconds after the doorbell (measured: Xid 32, intr0 `0x00040000`). The fix is
`Window::flush` (`sfence` + a read-back that forces the WC buffer to evict), called before
every host doorbell and every TLB invalidate (`D/ctrl.rs`, `D/boot.rs`). With it, the same
tables the software executor walked in E5c resolve on the physical GPU (that is why the
software path never needed it: no BAR1 in its way).

## 3. Mirror objects

`D/mirror.rs` holds, on the device's own host RM client (the `HostMemory` client of part 1,
which also owns the reservation and the OS descriptors), one **mirror** of each guest RM
object the physical GPU needs. They are created lazily at the first doorbell of a channel,
when the guest has done what the host needs first (host-exec.md §1 ordering rules). Every
guest FB address handed to the host is unchanged (a real VRAM address by identity); every
sysmem address is the host IOVA the guest already writes (D13); each is checked to lie inside
the reservation before it is used.

| Guest event | Host RM call on the mirror | Addresses | Status |
|---|---|---|---|
| `GSP_RM_ALLOC` `FERMI_VASPACE_A` + `COPY_SERVER_RESERVED_PDES` | host `FERMI_VASPACE_A` (externally owned) + `DMA_SET_PAGE_DIRECTORY` (PRIVILEGED, scoped CAP_SYS_ADMIN) to the guest's root — a real VRAM address | PDB passes through | **NV_OK** (measured) |
| `GSP_RM_ALLOC` of a TSG (`KEPLER_CHANNEL_GROUP_A`) | host `KEPLER_CHANNEL_GROUP_A` on the mirror VAS, the guest's engineType | none | **NV_OK** |
| `GSP_RM_ALLOC` of a channel (`TURING_CHANNEL_GPFIFO_A`) | host channel on the mirror VAS; GPFIFO = the guest VA; USERD = the reservation object at `PA-base` for a vidmem USERD, an OS descriptor for a sysmem USERD; error notifier = the guest's page | GPFIFO/USERD FB pass through; sysmem USERD is a host IOVA | **NV_OK**; the host RM clears the USERD at alloc, so the device restores the guest's GPGet/GPPut afterward (a finding) |
| `GSP_RM_ALLOC` of a CE object (`TURING_DMA_COPY_A`) | host `TURING_DMA_COPY_A` with the guest's own parameters (the CE caps were forwarded, so the guest's engineType is the host's) | none | **NV_OK** |
| `GSP_RM_ALLOC` of a compute object (`TURING_COMPUTE_A`) | (would be) host `TURING_COMPUTE_A` on a GR channel | none | the GR channel stays in software (§6) |
| guest `GPFIFO_SCHEDULE` | host `GPFIFO_SCHEDULE` on the mirror channel, after an engine object exists | none | **NV_OK** |
| guest doorbell (BAR0 `0xBB0090`) | decode the guest token (ChID 11:0, runlist 22:16), look up the host token, flush the WC windows, write the host doorbell page (`VOLTA_USERMODE_A`, `nvrm-client` `map_doorbell`). The GPFIFO is never interpreted for a mirrored channel | forward only the doorbell | fire-and-forget |
| guest `GPU_PROMOTE_CTX` | **recorded** in `Registry::promoted` (variant c1); not replayed | promote-entry FB addresses are real | echo (NV_OK) |
| register / `MEM_OP` TLB invalidate | flush the WC windows, `DMA_INVALIDATE_TLB` on every mirror VAS rooted at that PDB | PDB passes through | **NV_OK** |
| non-stall event registration | host OS events with `NV01_EVENT_NONSTALL_INTR` on the GR0/CE2-CE4 notifiers, armed REPEAT; on each host event set the guest LEAF bit and raise MSI | none | forwarded as the guest's vector + MSI (measured) |
| guest free / device exit | channels, then TSGs, then VA spaces, then descriptors, then the reservation | — | every free NV_OK (§5) |

**The replayed (shared-management) VA space.** A bare externally-owned VAS can never schedule
a GR channel from user space: `kchannelIsSchedulable_TU102` refuses a GR channel on an
externally-owned VAS whose context is not bound (`kernel_channel.c:2130`; `bIsContextBound` is
set only by the kernel-internal `nvGpuOpsBindChannelResources`, `nv_gpu_ops.c:11140`), giving
`GPFIFO_SCHEDULE` → `NV_ERR_INVALID_STATE` (0x40, measured). The device therefore also
implements the shared-management form the guest kernel uses on GSP: at the guest's
`COPY_SERVER_RESERVED_PDES` it builds the mirror VAS with the guest's own alloc parameters
plus `VA_INTERNAL_LIMIT` (the guest's server-reserved range as the RM-internal range) and
replays `COPY_SERVER_RESERVED_PDES` (PRIVILEGED) with the guest's root/PD2/PD1 pages, so the
host RM owns the server-reserved range inside the guest's page-directory chain. This makes GR
channels schedulable, but the GR context then executes and faults (§6), so it is off by
default; the code stays for the GR work.

## 4. Doorbell and interrupt paths

**Doorbell.** `D/ctrl.rs` `Dispatcher::doorbell` routes the guest's `0xBB0090` write: a
channel the mirror takes runs on the physical GPU (its host mirror is built on first use),
every other channel on the software executor. The guest token's ChID selects the channel;
the device holds a guest-token → host-token map and writes the host token to the host
doorbell page. A trapped value is resolved guest ChID first (`route_token`): a host token is
only looked up for a value that names no guest channel, because host and guest ChIDs overlap
(doorbell-token-pool.md (e) rules 9-10). The GPFIFO is never interpreted for a mirrored channel.

**Completion (blocking sync and polling).** `D/intr.rs` `IntrShared` is shared between the
server thread and a host-event thread (`D/mirror.rs` `event_loop`). The event thread `poll`s
the OS-event ctl fds; on a GR0/CE non-stall event it sets the guest vector's LEAF bit
(`chips/tu106/interrupts.toml` now carries GR0=vector 4, CE2-CE4=5-7 in the notification
subtree) and writes the guest's MSI eventfd; the server thread folds the pending bits into
the interrupt tree at the guest's next access. Polling completion (semaphores, GPGet) lands
in guest memory directly by identity + the IOVA table.

**RC.** A host RC on a mirror channel arrives as a host OS event on the RC notifier (measured:
exceptType `0x20` PBDMA / `0x26`), which the device counts and logs; the guest also learns of
it through the mirrored error notifier (its own page) and, for a mirrored channel, RC-recovers
cleanly (UVM fails registration with `NV_ERR_RC_ERROR`, CUDA returns 719, no host effect).

**The E5c NV_OK-acknowledged controls.** `PREEMPT`, `STOP_CHANNEL`, `SET_TIMESLICE`,
`GPFIFO_SCHEDULE` on a channel/TSG that has a mirror are forwarded to the host object
(`Mirror::forward_control`); the rest (`PERF_BOOST`, exception mask, `MC_SERVICE_INTERRUPTS`,
…) stay acknowledged as in E5c, with the comment that part 2 maps them where the host needs
them.

## 5. Teardown

`HostMemory::shutdown` frees the mirror objects first (channels, newest first, so RM preempts
and idles each before its TSG and VA space go), then the descriptors (unpinning guest RAM),
then the reservation, then the client — the order of memory-model.md §4. A guest free of a
channel/TSG/VAS/client frees its mirrors first (`Dispatcher::free` → `Mirror::free_object`),
while the registry still knows what the handle stood for. Measured on `down`: 24 mirror
channels created and 24 freed (0 live), the reservation released, guest RAM unpinned
(`foll_pin` balanced), the host RM client freed, host `nvidia-smi` VRAM back to its prior
value, no leak.

## 6. The two execution walls (closed 2026-09-23)

Two problems were resolved on 2026-09-23 (experiments.md "E5b part 2", runs d-ce-*):
- **The host work-submit token.** The host RM writes the *host* token into the channel's
  notifier page (the guest's own page, `kchannelNotifyWorkSubmitToken`), and libcuda reads it
  back and rings it from the second submission on. The device maps a rung host token to its
  guest channel (`mirror.rs` `chid_of_host_token`); before that fix the guest's later
  doorbells hit "no such channel" and the CE work stopped after the first push.
- **TLB invalidation of the guest PDB.** `DMA_INVALIDATE_TLB` on a mirror VA space never
  invalidates the guest's tables (it hits the RM walker's own root; an externally-owned VAS
  has none). The device now replays the guest's register `MMU_INVALIDATE` as a
  `MEM_OP MMU_TLB_INVALIDATE` of the guest PDB from a **privileged** device-owned host channel
  in the reservation carve-out (`invalidate.rs`, allocated under CAP_SYS_ADMIN so the method
  is accepted, then a polled semaphore for the ack). Without it a fresh guest mapping faulted
  FAULT_PDE on the physical GPU (Xid 31). With it, guest CE copies read and write freshly
  mapped pages correctly. `CARAXES_MEMOP_INVALIDATE=0` restores the old (no-op) behaviour.

The CE copies themselves now run end to end: the physical GPU fetches each guest CE channel's
own GPFIFO entry, walks the guest page tables, does the H2D copies and lands the four-word
semaphore (payload + GPU timestamp) in guest sysmem (measured, d-ce-diag / d-ce-token).

1. **CE first-launch stall** (host-exec.md open item 3, still open). The *first* CE
   `LAUNCH_DMA` on a freshly scheduled host channel does not complete and the watchdog RCs it
   (a contained Xid 32, HCE `intr1` bit 3 or PBENTRY `intr0` bit 18, on the four-word
   channel-init semaphore release; measured d-ce-inval2). host-exec absorbed the same stall
   with a warm-up push on a channel it controlled; the mirror needs a warm-up issued on the
   channel before the guest's first doorbell (a device-owned GPFIFO slot at GPGet=GPPut=0,
   built when the channel is scheduled rather than at the first doorbell). This is the only
   wall between here and CUDA's copies running unattended on the physical CE.
2. **GR context on a host channel** (cuda-path.md §4.1, exec-formats.md §4.5). With the
   replayed VAS (§3) a GR channel schedules and the host RM binds its own golden GR context
   (variant c1); the guest's first compute push faults (**Xid 38**), because the host-built
   context lives in the host kernel's own page tables, which the channel's PDB (the guest
   root) does not map. Variant c2 was completed on 2026-09-23 (`d-gr-c2`): a privileged
   `GPU_PROMOTE_CTX` of the guest's own recorded context buffers (MAIN/PATCH with the
   reservation PAs and the initialize flag, FECS_EVENT/PRIV_ACCESS_MAP VAs) onto the host GR
   channel is **accepted (NV_OK)**, but the channel still Xid 38s: the host kernel already
   allocated and promoted *its own* GR context when `TURING_COMPUTE_A` was allocated on the
   replayed VAS, and that is what FECS loads (exec-formats.md §4.2, §4.4), so the guest's
   promoted buffers are unused. The only user-space configuration in which the guest's buffers
   would be the ones FECS loads is an externally-owned VAS where the host allocates no GR
   context — but such a VAS cannot schedule a GR channel (`kchannelIsSchedulable`). Resolving
   this needs the host to build the context reachable through the guest root, i.e. the shadow
   page table of decision D17 (route 4), not a user-space RM sequence. Until then GR channels
   stay in software (the kernel launch is refused there, as in E5c, and never RCs the card).
   This is the wall on the kernel launch itself.

   **c1s progress (2026-09-23, `shadow.rs`, `CARAXES_MIRROR_GR_SHADOW=1`).** The GR channel
   now runs on a **host-managed shadow VAS**: the host RM builds its own GR context (set B)
   there, and the guest's set-A (GPFIFO, pushbuffers, QMDs, shader/constant/local memory,
   semaphores) is mirrored into the same VAS at the guest's own VAs by fixed-VA
   `RM_MAP_MEMORY_DMA` into per-run `NV50_MEMORY_VIRTUAL` objects (D17 route 4 stage 1),
   with the reservation/registered-sysmem + non-compressible filter, the guest's
   promoted-context VAs excluded (the host owns set B). Measured: 15372 set-A leaves map
   NV_OK (0 collisions after enlarging the VAS to 1<<47 and making set-B exclusion
   cumulative); the GR channel schedules with **no Xid 38** and, once the guest's GPGet/GPPut
   are restored after the host RM clears the USERD, the **guest's first compute push executes
   on the physical GPU** (GPGet advances). It then MMU-faults (Xid 31 FAULT_PDE, GRAPHICS/FE,
   VA 0x100_04e00000) on a set-A page not yet mirrored. The remaining work is robust dynamic
   set-A mirroring: resync at every guest `MMU_INVALIDATE` is wired, but a VA remapped to a
   new PA needs unmap-before-remap (NVOS47), and a coalesced run that grows must free and
   re-allocate its virtual object. That is the bounded next step to the first landed launch.

   **c1s closed (2026-09-23 23:40-23:59, `caraxes/runs/e5b2-2026-09-24/`).** The kernel-launch wall is
   down: with `CARAXES_MIRROR_CHANNELS=1 CARAXES_MIRROR_GR_SHADOW=1`, nvprobe 3 (vecAdd, result
   correct), nvprobe 4 (100 launches, **18.2 ms/launch** for a 30M-cycle spin kernel) and the
   cuda gate's cuda-jit, cuda-launch, **cuda-torch** (cuDNN convolution) and **rlprobe** pass on the
   physical GPU, 0 Xid, 0 RC; nvprobe 2 (CE path) stays green. Three changes did it:
   - *Dynamic set-A mirror* (`shadow.rs`): each resync diffs the guest root's leaves against a
     VA -> (target, size) map; a vanished or changed leaf is unmapped (`NV_ESC_RM_UNMAP_MEMORY_DMA`,
     NVOS47) before any new map; mapped VAs live in one fixed `NV50_MEMORY_VIRTUAL` per 2 MiB
     region and page class (never grown; freed when empty or on a class change), or in a
     per-leaf object where a region touches a set-B range or is refused. The D16/D17 filter
     needs the whole leaf inside the reservation or a registered guest-RAM range; every
     refused leaf is counted and logged once with VA, PA, aperture and reason. Resync runs at
     the channel build, before every doorbell of a shadow GR channel and at every guest
     `MMU_INVALIDATE`.
   - *One host TSG per guest TSG* (`mirror.rs` `gr_shadow_doorbell`): CUDA's GR chids 2-9 (one
     guest TSG, one context share) now share one host TSG and one host GR context.
   - *Resync cost*: BAR1 window reads were byte-wise uncached bus reads (a full walk ~250 ms);
     qword reads and whole-table block reads bring it to ~30 ms, and the walk is skipped
     altogether when no write reached a page-table page of the previous walk (`fb.rs`
     `enumerate_leaves_watched`; BAR1 is fully trapped, so the FB store sees every guest PTE
     write; a table in sysmem disables the skip; `CARAXES_SHADOW_ALWAYS_RESYNC=1` forces the
     walk). Without the skip nvprobe 4 measured 68.4 ms/launch. The resync still runs
     synchronously on the server thread (inside the doorbell and the `MMU_INVALIDATE` register
     write); a walk that does run costs the guest ~30-60 ms of MMIO latency at that point.
     Moving it off the server thread is later work.
   - *One shadow VAS per guest root* (2026-09-24 00:18): every CUDA context has its own guest
     root with the same VA layout, so a single shadow VAS mirrored the wrong context for the
     second one (gpu gate robustness: 17 contained Xid 31 on the device's GR channels, FAULT_PDE
     at the second context's GPFIFO). `mirror.rs` keeps a `ShadowSpace` per guest root (the VAS,
     its set-B exclusions, its chids and TSGs), freed when its last channel and TSG are freed;
     the cuda gate's robustness stage passes after the change, 0 Xid.
   - *Set-B release waits for the host TSG* (2026-09-25, vm4-615,
     `caraxes/runs/nvenc-2026-09-25/vm4-615-bisect/NOTES.md`):
     - Set B follows the guest's live channels (`exclusions_of`, 4a6ba83), so a freed GR
       channel gave its promoted VAs back to set A at the next invalidate.
     - The host's GR context at those VAs belongs to the host TSG, which is freed only at the
       guest's TSG FREE, later. The re-sync in between mapped the guest's pages at 0x120000000
       and up; the host RM refused 23 fixed mappings (`virtmemAllocResources ... NO_MEMORY`,
       18:12:42 and 20:21:50).
     - Now: at a guest channel FREE, the channel's promoted ranges are taken while the
       registry still has them (`held_at_free`). If its host TSG still stands, they are held on
       the shadow root under the TSG key (`ShadowSpace::held`); set B is the live channels'
       ranges plus the held ones (`set_b_with_held`).
     - At the host TSG's free (`free_tsg`, after the free returns OK) the held ranges go back
       to set A. The FREE handler then re-syncs those roots (`take_released_roots`,
       `mirror_resync_released`), so the pages get mapped right away.
     - A refused host TSG free keeps them (fail closed). A channel whose host TSG is already
       gone, or never built, holds nothing: released at once, as before.
     - Test: `set_b_is_released_only_after_the_host_tsg_free`.
   - *L2 cache operations* (2026-09-24, `boot.rs` `l2_op`, `caraxes/runs/e5b2-2026-09-24/d-uflush/`).
     After a few CUDA processes every `cuCtxCreate` took ~68 s (bench `v2-hang`): the guest RM
     logged `kmemsysDoCacheOp_GM107: timeout error waiting for reg 0x70010 update` and `... 0x70004`
     twice per context. A GPU-cached (vol=0) sysmem mapping makes the RM evict system memory from
     L2 (`kmemsysNeedInvalidateGpuCacheOnMap_GV100`, `kmemsysCacheOp_GM200`). It writes
     `PENDING_BUSY` to `NV_UFLUSH_L2_FLUSH_DIRTY` 0x70010, then to `L2_SYSMEM_INVALIDATE`, and polls
     `PENDING|OUTSTANDING` (bits 1:0, gm200/dev_flush.h). On Turing, `L2_SYSMEM_INVALIDATE` is the
     VF-window alias 0xB80F00 (`kmemsysWriteL2SysmemInvalidateReg_TU102`); the log prints the
     `reg` argument 0x70004. One 30 s timeout covers both writes. The BAR0 default latched the
     written 1, so the poll never ended. The latch lives in the device process, so it survives FLR
     and rmmod. Why the first contexts escape it is not established; the device trace of a
     reproduction would show the first cached sysmem map (evidence `source-trace.txt`).
     The boot model now owns the block (chip.toml `l2_flush_dirty`, `l2_sysmem_invalidate`,
     `l2_ops_local`): a write with PENDING starts the operation and both busy bits read back
     clear. With host execution on, the guest's sysmem lines really sit in the physical L2, so
     FLUSH_DIRTY and SYSMEM_INVALIDATE first run on the physical GPU as
     `NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE` on the device's own subdevice (full cache, sysmem
     aperture; FLUSH_DIRTY sends write-back, SYSMEM_INVALIDATE sends invalidate, which the host RM
     turns into write-back + invalidate). The control is exported NON_PRIVILEGED (flags 0x10118),
     so the unprivileged client may call it. A host failure is logged and the register still reads
     complete: the guest RM could only time out and continue. FB_FLUSH, PEERMEM_INVALIDATE and
     CLEAN_COMPTAGS complete with no host operation: there are no peers, and under D16 nothing
     comptag-related reaches the host. On vm2 the bench `v2-hang` sequence
     (ten CUDA processes) then takes 8.8-10.2 s per process instead of ~69 s: each context's two
     evicts are polled once and read complete, the guest logs no cache-op timeout, and the host
     logs no Xid.


Every Xid seen while iterating was contained to the device process (its pid), on our own
mirror channels, with the cause identified; the host desktop was unaffected throughout. The
sequence of attempts is in experiments.md ("E5b part 2") and `caraxes/runs/e5b2-2026-09-23/`.

## 7. Isolation (P7) notes for later

- **The per-engine notifier is host-wide.** A host non-stall interrupt fires for *every*
  client's work on that engine, so the host desktop's own GR completions reach our subdevice
  event (measured: ~209k GR0 events raised in a three-minute window under desktop load). The
  device coalesces (one MSI per pending vector until the guest reads its leaf), which keeps
  the guest correct, but for real isolation the forwarding must be **progress-gated**: forward
  the guest's vector only when a mirrored channel on that engine has advanced (its USERD
  GPGet/GPPut, one BAR1 read each), and swallow the event otherwise, so an idle guest sees
  zero events. Designed, not yet implemented (it belongs after the launch lands).
- **No physical bounds.** Guest page tables can name any VRAM address; Turing has no
  per-channel physical bounds. Acceptable only for a trusted guest, as in v1. The write filter
  hook (§2) and the shadow-page-table route (dma-routes.md §4) are where isolation is added.
- `GP100_UVM_SW` is `RS_FLAGS_ALLOC_PRIVILEGED`: the unprivileged host client cannot allocate
  it (measured `NV_ERR_INSUFFICIENT_PERMISSIONS`), so nvidia-uvm's own channels cannot be
  mirrored as-is. Whether it can be allocated inside the scoped CAP_SYS_ADMIN window (like
  `DMA_SET_PAGE_DIRECTORY`) is open; today those channels run in software on the VRAM store.

- **Comptag safety hold (2026-09-23).** No mirrored CE or GR execution may run while the
  guest's vidmem PTEs carry a compressible KIND or a non-zero `COMPTAGLINE`: the `COMPTAGLINE`
  (PTE bits 55:36) indexes the host's *global* comptag store shared with the desktop's own
  compressed surfaces, so a copy through such a PTE that landed (rather than hung) would be
  global card corruption, not contained to our channels. The device now reports no FB
  compression so the guest picks uncompressible kinds (memory-model.md §6). Today's evidence:
  the CE channels that ran physically (WC-flush run) took exactly one doorbell each and then
  stalled with no GPGet advance, no Xid and no observed host effect — consistent with the
  comptag write hanging rather than landing, but the PTE kind/COMPTAGLINE of those submissions
  was not captured. Re-enabling the mirror requires first verifying, through BAR1, that fresh
  vidmem PTEs across a software-mode cuda-core run carry kind 0x06/0x00 with COMPTAGLINE 0.

## 8. The held invalidate without the device lock (2026-09-26)

Code: `D/invgate.rs`, `D/mirror.rs` `shadow_invalidated`, `invalidate_busy`, `ShadowWorker`,
`D/boot.rs` (the trigger read, `gp_put_barrier`). Evidence for the problem:
`caraxes/runs/nvenc-2026-09-25/vm4-615-latency/NOTES.md`.

- **Problem.** The untrapped-GPPut rule (bar1-pool.md §9) waited for the shadow worker inside
  the trapped trigger write, under the Device mutex. While Sunshine streams (a map and unmap of
  the scanout buffer per frame) that was 78-115 holds/s, 17-22 % of wall time, single holds up
  to 130 ms. Flip latch, vblank, doorbells, re-arms and every trapped access queued behind it.
- **What the guest does.** 610 and 615 alike: `kgmmuCommitTlbInvalidate_TU102` writes
  `NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE` with TRIGGER, then `kgmmuCheckPendingInvalidates_TU102`
  reads it until TRIGGER is FALSE (`osSpinLoop` between reads, `GPU_TIMEOUT_DEFAULT`, 4 s on the
  OS timer). The same poll runs before every invalidate. It is the only writer of the register.
- **Design (b), completion through the busy bit.**
  - The held trigger write plans as before (lazy flush plus the root's plan, on this thread),
    submits, arms `InvalidateGate` at the worker's last sequence number, and returns.
  - A trigger read reports TRIGGER TRUE until the worker's `Progress` reached that number. Each
    busy read waits at most 50 us for it under the lock; the lock is free between reads.
  - A read that finds the gate armed for 1 s or more waits to the end under the lock (the old
    hold), so the guest's 4 s timeout can never let it continue early.
  - Doorbells of a shadow channel and trapped GPPut stores wait only for their own root's jobs
    (`Seqs::last_of`, `wait_root`), not for every job in the worker.
  - `CARAXES_SHADOW_INVALIDATE_ASYNC=0` restores the hold under the lock (A/B only; a malformed
    value keeps the hold).
- **Why FAULT_PDE stays prevented.**
  - The hazard: a host channel of root r runs guest work that uses a mapping of an invalidate
    before the worker applied it (bar1-pool.md §9).
  - The guest's order is page tables, invalidate, poll until TRIGGER FALSE, return from the map
    call, then GPFIFO entry, GPPut (trapped or not), doorbell. Nothing that uses the new mapping
    can come before the map call returns.
  - TRIGGER reads FALSE only when `Progress::done >= until`. The worker finishes jobs in
    submission order, and `until` is the last job submitted by the invalidate, so the condition
    is exactly the one `wait_all` waited for inside the write. Every guest step after the poll
    therefore meets the applied mirror, as before.
  - Work not ordered after the invalidate (another guest thread on the same root) cannot use a
    VA whose map call has not returned; for an unmap the guest has idled the work first. This is
    the hardware contract, unchanged. As defence in depth a doorbell or trapped GPPut store of
    root r still waits for all of r's jobs, the gated plan included.
  - Per-root waits are enough: a channel of root r translates only through r's shadow VAS, and
    `last_of(r)` covers every Create, Apply and Free of r.
- **Unchanged sync points.** The pool's `refs` and warm-barrier `resync` (bar1-pool.md §4, §11,
  §12) still call `shadow_barrier` (wait for every job), so its cached walks equal the shadows.
  A pool poll that runs `refs` right after a trigger write can still hold the lock for the walk
  (rare since §11; the slow-poll trace line shows it). The set-B release (`take_released_roots`)
  submits without a wait as before; the root's next doorbell waits for it.
- **Limits.** The plan (the walk of the guest's tables) still runs in the trapped write. The
  worker is FIFO: a doorbell of root B waits for A's walk if B has a job queued behind it.
- **Counters.** The mirror line's untrapped-GPPut part: invalidates held (now from the trigger
  write to the first read that finds them applied), trigger reads that were busy, reads that
  fell back to the full wait.
- **Tests** (`invgate::tests`): a 300 ms walk on root A leaves the lock free for a display tick
  and a doorbell of root B, the trigger reads busy until the walk is done and no read holds the
  lock for long; a doorbell of the gated root waits for the walk; past 1 s a read waits to the
  end; a second invalidate extends the gate.
- **GPU run must confirm.** Kick rate while streaming (cursor test 25-60 kicks/s instead of
  8-25), Sunshine frame latency, reactor lock-wait p99 back near the 13.6 ms of no stream, the
  busy-read and fallback counters, and no Xid 31 over five Moonlight connect cycles.

### 8.1 The invalidate's scope and an urgent-first worker (2026-09-29)

Evidence: `/mnt/vmstore/caraxes/runs-2026-09-29/sottr-stall/` (SotTR stuck on its loading screen
for 20+ minutes, P43, 4 GiB).

- **What the guest did.** 90 % of the game's samples in `osDevReadReg032` polling TRIGGER under
  `gvaspaceAlloc_IMPL` and `dmaUpdateVASpace_GF100`; VRAM allocations and DMA maps took 250-570 ms
  each, 3-4 per second (guest perf and bpftrace, `guest-perf-1228.txt`, `guest-ioctl-hist-1227.txt`).
- **What the device counted** (`gpu-exit.log`): 54213 held invalidates, 1850 s held in total
  (34 ms mean, 794 ms longest), 3.47 M busy trigger reads (64 per hold); 42066 invalidates of other
  roots deferred (lazy) and planned in 15762 batches. The per-shadow lines left at exit show only
  seconds of apply time, and those roots were created and freed many times (131 channels; the
  lines of those freed earlier went to the trace, which was off). The holds were most likely
  waiting, behind other roots' jobs in the FIFO worker (the lazy batches a hold
  flushed first, doorbell batches, new VASes' full syncs) and behind the device lock (pool polls
  over 2 ms: 5924; the BAR2 memfd's full scans: 134277 syncs, 55 s).
- **The card's rule.** An invalidate completes when the TLB holds no stale translation for its
  target. So TRIGGER reads busy until the worker applied the jobs of the invalidate's scope only:
  - the named root, or for `ALL_PDB` the roots whose GPPut store does not trap
    (`untrapped::held_roots`); every other root's channel runs after a doorbell whose barrier
    plans and waits for its root, so those follow the lazy rule (planned without a wait, or
    deferred);
  - the hold no longer flushes the other roots' deferred invalidates first.
- **Urgent first.** The worker keeps one queue per root (`JobQueues`); a root someone waits for
  (an armed gate, a doorbell or GPPut barrier, `Progress::wait`) has its jobs applied before older
  jobs of other roots. A root's jobs never pass each other (Create, Apply, Free, Coverage). Progress
  is per job (`Progress::begin`/`finish`), so `wait_all` (`Target::All`) still means every job up to
  a number, and `wait_root` (`Target::Roots`) exactly the root's jobs.
- **Kept.** Maps are applied before TRIGGER clears: a host channel whose GPPut store does not trap
  may fetch the next work at once, and a missing leaf there is an MMU fault (Xid 31), so the
  plan's unmaps and maps both belong to the hold. The MEM_OP hold (`memop_holds`) and the pool's
  `shadow_barrier` are unchanged. Not done: coalescing queued plans of one root (a later plan
  covers only its changed units) and parallel workers (the host RM serialises the calls anyway).
- **Counters.** The mirror line ends with `shadow worker: N plan(s) applied in X ms, queued Y ms in
  total (Z us max), P job(s) of a waited-for root applied before older ones`. With
  `CARAXES_PROFILE=1` the rows `held invalidate: armed to TRIGGER read done` and
  `shadow worker: plan queued before apply` give the distributions; the host RM rows give the
  worker's call times.
- **Tests.** `invgate::tests` (out-of-order finish, urgency while waiting and while a gate is
  armed, a gate with nothing in flight), `untrapped::tests::a_held_invalidate_waits_for_its_scope_only`,
  `mirror::tests::the_worker_takes_a_waited_for_root_first_and_keeps_each_roots_order`.

## 9. Lock ordering and what may run under the device lock (2026-09-29)

The device lock (D, `Arc<Mutex<Device>>`, device.rs) serialises every trapped access, every
reactor wake-up and every clock. A long hold stops unrelated engines: Windows C61 timed out its
VSync behind a 495 ms mirror build and 500-650 ms control replies; SotTR on Linux waited up to
441 ms behind a pool poll. The card's rules bound what needs D:

- a doorbell only promises that the GPU fetches the new entries eventually;
- a control reply may take as long as it needs, but must not stop unrelated engines;
- the held invalidate's trigger read waits at most 50 us per read (§8), and the guest polls it.

**Order.** D first, then any of: the BAR1 pool's lock, the display engine's lock, the host RM
mutex (`hm`, `HostMemory`), the shadow worker's and the builder's internals (`invgate::Progress`,
job queues), the BAR2 flush state. A thread that holds any of these never takes D. The shadow
worker, a builder, the pool's mover and the display clock hand their results to D-holders through
queues or atomics (a completion drained at the next D hold, or a host doorbell write the worker
does itself).

**What runs under D.** Short state changes only: reading a guest structure, updating the
registry and the mirror's maps, writing a reply or a register, submitting a job. Not under D:

- waiting for the shadow worker, a builder or the pool (a doorbell holds its ring instead, §9.1);
- host RM calls that build objects (a channel's mirror build, §9.2) or serve a slow control
  (the RPC worker, §9.4);
- a shadow full walk (bounded resumable steps, §9.3).

The one wait that stays is the trigger read's `POLL_WAIT` (50 us). A wait under D that is not
bounded is a bug; the D hold histogram in the exit summary names the holder of every long hold.

§9.1-9.4 are the steps (branch lockfree-2026-09-29): the held ring, the build off D, bounded walks,
the RPC worker. The pool's barrier becomes a held ring as well (the pool rework); the display
clock's per-tick work takes the display engine's lock and never D.

### 9.1 The held ring (2026-09-29)

- A doorbell of a built shadow channel plans its root as before, then checks its target (the
  root's last job, `invgate::Target`) and the ring gate (`ringgate.rs`). Applied and not held: it
  rings at once. Otherwise it records a held ring for the channel (one per channel; later doorbells
  join it with the newer target) and returns.
- The shadow worker pokes the gate after every job. The completer thread (main.rs,
  `ring-completer`) waits on the gate without D while rings are held and takes D briefly to
  forward the ready ones (`Mirror::complete_held_rings`: FB flush, shadow USERD publish, the
  forwarded GPPut, the host doorbell, the harvest).
- Holds for other users (the BAR1 pool): `Mirror::hold_rings(roots) -> token`,
  `Mirror::release_rings(token)`, `Mirror::submit_call(root, call, Some(token))` runs a host RM
  call on the worker in order with the root's plans and releases the token after it. The gate is
  process-wide (`ringgate::global`), so a model rebuilt at a reset keeps it.
- A freed channel's held ring is dropped; a reset drops them all.
- Counters: the mirror line's `held rings: ...` part; the device-lock holds by holder
  (`dlock.rs`) at exit: count, total, p50/p99 bounds, max per holder, and the longest holds.
- Still waiting under D: the preparation path (`submit` false), the trapped GPPut store's barrier
  (`gp_put_barrier`), the software MEM_OP hold. The first doorbell's build moved off D in §9.2;
  the pool's barrier holds rings instead (§9.5, bar1-pool.md §15).
- Tests: `ringgate::tests` (a ring waits for its root's jobs and holds only; the completer wakes
  on a poke while rings are held), `dlock::tests`.

### 9.2 The mirror build off D (2026-09-29)

- A shadow channel's first doorbell takes the build's inputs under D and hands the host RM calls
  to the shadow worker as one job (`mirror.rs` `shadow_build`, `build_on_host`).
  - The inputs: the VAS (made under D if the root is new) and its set-A plan, the USERD plan, the
    error-notifier page, the engine objects, the preemption request, a reserved GFXP slot.
  - The job is queued after the root's plans, and a root's jobs never pass each other, so the
    set-A mirror is there before the host RM validates the GPFIFO VA. Nothing waits for it.
- The job makes the host TSG, the USERD and error-notifier objects, the channel and the engine
  objects, then binds GFXP and the compute preemption mode. It takes `hm` per RM call. It does not
  schedule: the channel cannot run before the commit.
- The builds of one guest TSG share a cell (`mbuild.rs` `TsgCell`): the first job the worker runs
  makes the host TSG, the later ones join it, and the preemption check is claimed once.
- The commit, under D briefly (`commit_build`), runs at the completer's next pass after the job
  (the worker pokes the gate) or at the channel's next doorbell. It:
  - takes the job's output into the maps;
  - schedules the channel and gets its token (the host RM writes the token into the guest's
    notifier; the dispatcher pins a pool token over it, `take_committed`);
  - adds the engine objects the guest allocated during the build;
  - writes back the GPPut the host RM cleared;
  - rings once the root's jobs up to the newest doorbell are applied, else holds the ring (§9.1).
- Doorbells during the build join it. The root is re-synced as for a built channel, the newest
  nonzero GPPut and the root's last job are noted, and the commit rings for all of them.
- The USERD clear: the host RM zeroes a sysmem USERD at the channel's allocation
  (`kfifoSetupUserD_HAL`, kernel_channel.c 2435-2449 of 615.71.09), now on the worker.
  - The commit writes the newest rung GPPut back only over a zero (`put_to_restore`).
  - A store of the guest after the clear stays. The synchronous build restored the first
    doorbell's GPPut over any other value, which also overwrote a newer store.
- Teardown and the free path:
  - A guest free of the channel, one of its engine objects, its TSG, its VAS or their client
    first waits for the build and commits it (boot.rs FREE, `settle_builds_for_free`), so the free
    finds a built channel.
  - The mirror's own teardowns (`free_channel`, `free_tsg`, `drop_vas`, reset) wait for a build
    in flight and put it in the maps unscheduled (token 0, context not ready) for the teardown to
    free (`settle_build`). Its doorbells go with the channel.
  - These are the only waits under D for a build; they are counted.
- A preparation (`submit` false, the deferred context) waits for its build under D: its caller
  needs the host context at once.
- The BAR1 pool sees a channel in flight as mirrored (`pool_channels`), so it does not move the
  USERD page the build took.
- Counters (exit summary, the mirror line):
  - `mirror builds (execution.md §9.2)`: started, committed, refused, dropped, in flight, joined
    doorbells, host time on the worker, commit time under D, first doorbell to commit, and the
    waits under D.
  - The device-lock work table (`dlock::work`: every GSP RPC by function and command or class,
    doorbells by software or host, the build's walk, snapshot and commit) and the tags a long hold
    keeps (`dlock::tag`: what the hold did).
- Still under D in a build: a new root's VAS allocation (one or two RM calls) and its full walk
  (§9.3).
- Tests: `mbuild::tests` (the TSG cell: the first build makes it, the next join, a refusal leaves
  it empty; the preemption check is claimed once; the GPPut write-back only over the clear; the
  counters), `dlock::tests` (tags of a long hold, the work table).

### 9.5 The BAR1 pool (2026-09-29, bar1-pool.md §13-§15)

- Under D:
  - the poll's decision;
  - the GPU check, with no worker drain;
  - the moves in slices of about 500 us behind the invalidate trigger;
  - the barrier's start (idle check, ring holds, preempt jobs queued) and its end (re-sync
    plans, ring releases queued).
- Waits for the worker: at most 50 us per poll (in-flight jobs, the barrier's preempts, its
  re-sync), except with the invalidate page (as its held invalidate). The worker releases the
  held rings itself.
- Still under D: each slice's copies.
- Next, within the order above (D, then the pool's lock, the display lock, `hm`, worker
  internals): the copies on a pool mover thread that never takes D.
  - The pool state goes behind its own lock.
  - A page in transit stays at home for every device path; a device write to it marks it for a
    second copy.
  - The mover commits the remap switch under the pool lock.
  - A commit that changes a record page the display core pinned tells the display core (pool
    lock, then display lock, never the reverse).
- `RingGate::clear_holds` at `Mirror::reset`: a reset drops the holds of a batch that went with
  the model.

### 9.6 The software executor's translations (step 5a, 2026-09-29)

- Windows C65: the software doorbells of chid 15 (the KMD's paging channel) and chid 4 held D
  for 200-540 ms each (357 doorbells, 21.8 s in total). Their copy-engine bytes were small, and the
  device CE's large transfers never wait under D.
- `GpuMem::pieces` translated every 4 KiB page separately: 4-5 uncached table words through the
  BAR1 windows per page, each of which could evict the window the previous one mapped. An 8.8 MB
  memset is about 2160 pages.
- `FakeFb::translate_pages` gives every page of a range exactly what `walk_in` gives it. It reads
  a 2 MiB unit's directory entries once and its page-table entries in one block per table.
  Test: `a_range_translates_as_its_pages_do` (small and big pages and their precedence, a sparse
  PTE, a 2 MiB page, a sysmem page table, a missing directory, partial pages, unit crossings).
- A software doorbell's time is split (`dlock::Spend`): translations and the software CE's CPU
  copies apart, in the long hold's tag and the work table (`software translate`,
  `software CPU copy`; the rest is the doorbell's total minus both).
- Next, if the split shows the copies or the rest dominate: the executor off D (step 5b), or the
  paging channel on a host CE.

### 9.7 The executor TLB and bounded software runs (step 5b, 2026-09-29)

- Windows C67 (5a): software doorbells of the KMD's channel still held D 516-562 ms, about half
  of it translating. A doorbell makes one or two translations per GPFIFO entry and per semaphore
  or CE op, each 5-7 table words through the two BAR1 windows; table pages, GPFIFO, pushbuffers
  and data sit in different 32 MiB regions and evict each other (C67: 38765 window maps).
- The executor TLB (`fb.rs` `ExecTlb`): `(root, VA page)` to the page's target, filled by
  `translate_pages` from walks that read only VRAM tables the device tracks (not sysmem, not the
  BAR1 pool, not the private pages), with the table pages recorded.
  - Flushed by a write to a recorded table page (the BAR TLB's three hooks: device writes,
    BAR2 memfd syncs, writes of unknown content), by every MMU invalidate (register or the
    invalidate page) and by every `MEM_OP` TLB invalidate the executor runs.
  - That is the card's own rule: an engine may use cached translations until the next
    invalidate, and a guest that changes a PTE invalidates before an engine uses it, so writers
    the device cannot see are covered too.
  - At most 2^18 entries. Counted: `executor tlb: page hits, unit walks, flushes`, and per long
    software doorbell in its tag (hits, walks, window maps).
- Bounded runs (`channel.rs` `RUN_BUDGET`, 2 ms): a software channel's run stops between two
  GPFIFO entries once its budget is spent; the slice clock (the `ce-slices` thread, 1 ms, D taken
  briefly) continues it at the same GPGet through the held replay, as a sliced CE transfer.
  The CE slice is 4 ms (40 ms before).
  - Order: the channel's own entries, semaphore releases and non-stall events stay in its order;
    other channels, host or software, run in between, as on the GPU (a doorbell only promises
    that the entries run eventually). Acquires go through the held replay as before.
  - A hold of a software run is bounded by the budget plus one entry (a CE chunk at most).
- Tests: `the_executor_tlb_holds_until_a_table_write_or_an_invalidate`,
  `a_run_past_its_budget_stops_between_entries_and_continues_in_order`,
  `a_range_translates_as_its_pages_do`.
- Not done: the executor on its own thread without D. It reads and writes the FB through FakeFb
  (VRAM windows, walk-cache hooks, BAR2 memfd merges, victim patches, the pool's remap), all
  owned under D; only if the hold histogram still shows long software holds.

### 9.8 The invalidate page, held off the lock (2026-09-29)

- Target: about 100 us per guest MMU invalidate end to end. Run D spent, per invalidate, three
  trapped writes (19, 12 and 1180 us of device time), about 2.9 trapped TRIGGER polls (53 us
  each) and 445 us from arming to applied, plus the vCPU exits of five traps.
- The invalidate page (`CARAXES_INVALIDATE_PAGE=1`, inv_page.rs, intr-page.md §11.3) makes the
  PDB, UPPER and TRIGGER stores and the TRIGGER polls memory. Its reactor used to wait out a
  held invalidate under D before it cleared TRIGGER.
- Now the reactor runs the invalidate under D and takes the gate's hold (`InvalidateGate::
  hand_off`: the roots and their last jobs, still urgent for the worker). The page's
  completion thread (`inv-complete`) waits for the worker's progress without D and only then
  clears TRIGGER; until then the reactor takes no new trigger (the guest stores none while
  TRIGGER shows). The card's rule holds: TRIGGER reads busy until every root the invalidate
  names (with the untrapped scope rules, untrapped.rs) has its jobs applied.
- One completion thread in the guest's order: the guest RM waits for TRIGGER before its next
  invalidate. Counted: the page line's "held until the worker applied their roots" with the
  armed-to-done mean and max, and the profile's "held invalidate: armed to TRIGGER read done".
- Still under D in the page path: the BAR1 pool's poll after the trigger (at `Pace::ALL`, it may
  wait for the worker; the pool's own rework).
- Tests: `a_held_trigger_completes_when_the_worker_applied_its_root` (busy until the root's job
  is applied, no second trigger meanwhile, urgent while held), the page's existing tests.

### 9.9 The posted doorbell (2026-09-29)

- A trapped write of the usermode doorbell (`NOTIFY_CHANNEL_PENDING`) no longer takes D. The
  vfio-user loop queues it in the ring gate (`RingGate::post`, one entry per token: a second write
  of a queued token joins it, since a doorbell only makes the engine look at GPPut) and answers
  at once. A trapped doorbell costs the guest one BAR0 exit, whatever the client does with its
  token (a pool token the scan missed, a cached token, Windows).
- The completer thread rings the queue under D, each write exactly as its trapped write was
  handled (`Device::ring_posted`: `region_write` on BAR0, i.e. the trace, the BAR2 sync before
  the ring, the ring, the steps after a BAR0 write).
- Order: every other guest access rings the queue first, before its own work: every trapped
  access (`SharedDevice::lock`), the eventfd readers, the page reactors (invalidate, usermode,
  display kick), and a display channel's PUT that the routes serve without D (its USERD page,
  `display::USERD_RANGE`: a flip's semaphore acquire may wait for work a posted doorbell
  starts). No access passes a doorbell the guest wrote before it, as on PCIe, where a read
  or write does not pass a posted write. The card promises no more: a doorbell only makes the GPU
  fetch the new entries eventually.
- Counted: the exit summary's `posted doorbells` line (writes, joined, rung by the completer or by
  a later access, queue time mean and max, deepest queue); the profile's "posted doorbell: posted
  to rung" and "posted doorbell: rung (device time)"; the D holders "posted doorbells" and the
  work key "posted doorbell rings".
- Tests: `a_trapped_doorbell_is_posted_and_rung_before_the_next_access` (answered while D is
  held elsewhere, rung by the next access first, joined writes, other registers not posted),
  `a_posted_doorbell_wakes_the_completer_and_keeps_the_order`.

- Target: about 100 us per guest MMU invalidate end to end. Run D spent, per invalidate, three
  trapped writes (19, 12 and 1180 us of device time), about 2.9 trapped TRIGGER polls (53 us
  each) and 445 us from arming to applied, plus the vCPU exits of five traps.
- The invalidate page (`CARAXES_INVALIDATE_PAGE=1`, inv_page.rs, intr-page.md §11.3) makes the
  PDB, UPPER and TRIGGER stores and the TRIGGER polls memory. Its reactor used to wait out a
  held invalidate under D before it cleared TRIGGER.
- Now the reactor runs the invalidate under D and takes the gate's hold (`InvalidateGate::
  hand_off`: the roots and their last jobs, still urgent for the worker). The page's
  completion thread (`inv-complete`) waits for the worker's progress without D and only then
  clears TRIGGER; until then the reactor takes no new trigger (the guest stores none while
  TRIGGER shows). The card's rule holds: TRIGGER reads busy until every root the invalidate
  names (with the untrapped scope rules, untrapped.rs) has its jobs applied.
- One completion thread in the guest's order: the guest RM waits for TRIGGER before its next
  invalidate. Counted: the page line's "held until the worker applied their roots" with the
  armed-to-done mean and max, and the profile's "held invalidate: armed to TRIGGER read done".
- Still under D in the page path: the BAR1 pool's poll after the trigger (at `Pace::ALL`, it may
  wait for the worker; the pool's own rework).
- Tests: `a_held_trigger_completes_when_the_worker_applied_its_root` (busy until the root's job
  is applied, no second trigger meanwhile, urgent while held), the page's existing tests.

### 9.10 Status reads without the device lock (2026-09-29)

- SotTR run E, benchmark window: 507 TRIGGER reads/s at 152 us and 587 flush-page reads/s at 95 us,
  about 133 ms per second, mostly spent waiting for D. On the card both are plain status reads.
- The vfio-user server answers them before it takes D (device.rs `SharedDevice::region_read`,
  fastread.rs), from state that D-holders publish:
  - **TRIGGER** (`NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE`, bit 31): the trapped trigger write
    hands its held invalidate off (`Mirror::hand_off_invalidate`, as the invalidate page does).
    A read returns the latched word, with TRIGGER set while the hold's jobs are not applied. It
    waits at most `POLL_WAIT` for the worker, and past `FALLBACK` to the end. The first read that
    finds a hold done finishes it: its roots stop being urgent, and it is counted.
  - **The BAR2 flush page** (`pReadToFlush`, the RM ignores the value): a read marks the sync
    owed (`DeferredFlush::mark`, atomic) and returns the page's bytes as the last locked read
    published them. The copy goes on a device write to its FB page, on a BAR2 walk that
    re-points pages, and at a reset.
  - **The time** (`NV_PTIMER_TIME_0/1`, the usermode `TIME_0/1`), Windows line (2026-09-29):
    the hottest trapped read of a Windows guest (C67: 119k). Answered from the device clock
    (`channel::device_now_ns`, as the locked path). The PTIMER torn-read guard (the high word
    of the last TIME_1 read, boot.rs `ptimer_word`) is one atomic in fastread.rs, shared by
    both paths. On this line nothing else a guest sees is ordered by a time read (the
    time-read display catch-up b4d8975d was reverted in 336caf59; the scan position is computed
    from the time at its own read). Should it come back, the gate needs the next tick of each
    active head (from `Head::t0_ns`, `ticks` and `period_ns`), published after every catch-up:
    a read at `now` below it needs no catch-up.
- These reads take D:
  - TRIGGER while a BAR1 pool batch runs (its slices continue at those reads);
  - a register the trace writes a line for (`CARAXES_TRACE_ACCESSES`, per kind: TRIGGER, the
    flush page, PTIMER, the usermode time; a BAR0 range without them keeps them off D);
  - reads narrower than a dword;
  - the first read of the flush page after its copy went (it publishes a new one);
  - with the flush reads syncing at once (`CARAXES_BAR2_MEMFD_DEFER_FLUSH=0`).
- Card semantics: TRIGGER reads pending until the invalidate's roots have their jobs applied,
  then idle; the flush read orders the guest's BAR2 writes (the owed sync, settled at the next
  engine start or by the flush clock) and returns the page's content.
- Order: the state's mutexes are leaves. Nothing on this path takes D, and D-holders only
  publish. The posted doorbells (device agent) are not rung first by these reads: neither
  result depends on a doorbell.
- Counted at exit: `reads without the device lock: N TRIGGER read(s) (B busy), F flush-page
  read(s); under the lock ...; H held invalidate(s) completed by a read (mean, max)`. Held
  invalidates completed this way are counted there, not in the mirror line's hold count.
- Tests: `fastread::tests` (busy until the hold's job is applied; a pool batch or the access
  trace sends the read to the lock; the flush copy marks the sync owed and goes on a write to
  its page), `boot::tests::the_flush_page_and_trigger_are_published_for_reads_without_the_lock`.
