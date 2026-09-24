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
doorbell page. The GPFIFO is never interpreted for a mirrored channel.

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
     comptag-related reaches the host. Unit tests pass. The hardware run on vm2 (the sequence of
     bench `v2-hang`) is pending: while vm1 held the one fixed reservation, vm2 could not take one.


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
