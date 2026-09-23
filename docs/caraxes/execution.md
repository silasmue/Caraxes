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
| The guest kernel's register TLB invalidates, and the software executor's `MEM_OP` invalidates | `DMA_INVALIDATE_TLB` on the mirror VA spaces |

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

## 6. The two execution walls (open)

1. **CE first-launch stall** (host-exec.md open item 3). With the WC flush (§2), CUDA's CE
   channels build, schedule and take their first doorbell on the physical GPU with **zero
   Xid**. But the first CE `LAUNCH_DMA` on a freshly scheduled host channel does not complete
   (host-exec.md measured the same: it stalls, the watchdog RC-recovers): the channel-init
   semaphore release never advances the guest's tracking semaphore, so the guest waits and
   cuda-core times out with no Xid and no RC. host-exec absorbed this with a warm-up copy on a
   channel it fully controlled; injecting a warm-up into the guest's own channel (a NOP
   `LAUNCH_DMA` or CE init before the guest's first doorbell) is the next step. This is the
   only wall between here and CUDA's copies running on the physical CE.
2. **GR context on a host channel** (cuda-path.md §4.1). With the replayed VAS (§3) a GR
   channel schedules and the host RM binds its own golden GR context (variant c1); the guest's
   first compute push then executes on the SMs and faults (**Xid 38**, a GR/engine exception),
   because the host-built context and the guest's method stream / promoted context do not
   agree. Variant c2 (promoting the guest's context buffers onto the host channel) is
   unestablished. Until this is resolved, GR channels stay in software (the kernel launch is
   refused there, as in E5c, and never RCs the card). This is the wall on the kernel launch
   itself.

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
