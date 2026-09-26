<!-- SPDX-License-Identifier: MIT -->
# vm4-615: TURING_COMPUTE_A refused while a CUDA context lives

Bug (vm4-615-poollock/NOTES.md, isolation table): while any CUDA context is alive in the guest,
`RMALLOC class=0xc5c0` on a Vulkan client's GR channel returns 0x57 (NV_ERR_OBJECT_NOT_FOUND)
and no `GSP_RM_ALLOC` for it reaches the device. Source: 615.71.09 open modules
(`/mnt/vmstore/caraxes/ogkm-615.71.09`, paths below relative to `src/nvidia/src/kernel/`, `src/nvidia/src/libraries/` spelled out).

## Part A: source analysis

### The failing pattern in the two captures

- Both failures (`vm4-615-vulkan/vkcube-rmlog.txt:465`, `vm4-615-poollock/run2-pin0/vkcube-rmlog-full.txt:606`)
  are in a TSG with **two GR channels**:
  - main channel (ctxshare #1, `flags=0`): 902d, c5b5, a140, c597 OK, then c5c0 refused;
  - sibling (ctxshare #2, `flags=0x10000010` = `GROUP_CHANNEL_RUNQUEUE_ONE | GROUP_CHANNEL_THREAD_ONE`,
    the async-compute channel): its c5c0 is allocated **first** and succeeds.
- Both GR channels have `hVASpace=0`; the VAS comes from the ctxshares (device trace: both on
  VAS 0xbeef0300).
- In both failures the log shows `NV_SEMAPHORE_SURFACE_CTRL_CMD_UNBIND_CHANNEL` (0x00da0006) for the
  main channel **before** the failing c5c0, then a ZBC read, then the c5c0. In the successful
  single-GR-channel TSGs of the same runs there is no unbind before c5c0. The logger prints after
  the ioctl returns and does not log `hRoot` (the client), so the order across threads and the
  client of the c5c0 call are not known from this log.
- Device trace (810 s): one `GPU_PROMOTE_CTX` for the main channel before its 902d, none for either
  c5c0; the sibling's c5c0 arrives as `GSP_RM_ALLOC`; after the zcull bind and preemption-mode
  controls nothing more for this TSG.

### Every error in the GR object path prints at LEVEL_ERROR

- `nvCheckOkFailedNoLog` / `nvAssertOkFailedNoLog` (src/nvidia/src/libraries/utils/nvassert.c:180-257) print through
  `NV_PRINTF`; the release gate is `debuglevel_min = LEVEL_ERROR` (kernel/diagnostics/nvlog_printf.c:153-157).
  The guest logs show these lines normally (e.g. `gate-cuda-2026-09-24-4/guest/*.dmesg`).
- `kgrobjConstruct_IMPL` (gpu/gr/kernel_graphics_object.c:286) wraps every step:
  - `kgrmgrCtrlRouteKGRWithDevice` in `NV_ASSERT_OK_OR_CAPTURE_FIRST_ERROR` (:341);
  - `_kgrAlloc` (:173): `kgrobjSetComputeMmio_HAL`, `kgrctxAllocCtxBuffers`, `kgrctxMapCtxBuffers`,
    `kgraphicsAllocKgraphicsBuffers_HAL`, `kgrobjPromoteContext` in `NV_CHECK_OK_OR_RETURN(LEVEL_ERROR, ...)`,
    `kgraphicsInitializeDeferredStaticData` in `NV_ASSERT_OK_OR_RETURN`.
  - The callees (gpu/gr/kernel_graphics_context.c:1379-1990, gpu/gr/kernel_graphics.c:630-2022) use
    `NV_ASSERT_OK_OR_RETURN` / `NV_CHECK_OK_OR_RETURN(LEVEL_ERROR)` on every propagated error.
  - `rpcRmApiAlloc_GSP` (vgpu/rpc.c:10941) prints "GspRmAlloc failed" on an RPC error.
  - So a 0x57 that leaves `kgrobjConstruct` or the RPC would leave a `Check failed` / `Assertion failed`
    line with file:line.
- **The earlier "no dmesg line" is not evidence.** The guest runs `NVreg_ResmanDebugLevel=0`
  (`sottr-2026-09-24/guest/sottr-prepare.sh:50`), which logs every ioctl; the saved guest logs are
  the tail of a rotated ring (run6 `session/guest-dmesg.txt` covers uptime 771-910 s only, the
  failure was earlier). Part B records the kernel log continuously instead.

### Paths that return 0x57 without an RPC and without a LEVEL_ERROR print

| Site | When | In the c5c0 path? |
|---|---|---|
| rmapi/alloc_free.c:748 `clientGetResourceRef(pClient, hParent)` in `serverAllocResourceUnderLock` | `hParent` is not a handle of the client named by `hRoot` (wrong client, or the channel already freed) | yes, before any constructor; the only silent 0x57 that fails the alloc |
| gpu/gr/kernel_graphics_context.c:81 `kgrctxFromKernelChannel` | neither the channel nor its TSG has `hKernelGraphicsContext` | yes, but kgrobjConstruct ignores the result (:318); a NULL context then asserts later (prints) |
| gpu/gr/kernel_graphics.c:1941 `vaListFindVa` in `kgraphicsMapCtxBuffer` | this VAS has no mapping yet | yes, recovered: it creates the mapping |
| gpu/gr/kernel_graphics_context.c:1968 `vaListGetRefCount` in `kgrctxPreparePromoteCtxBuffer` | buffer not mapped in this VAS | yes, returns NV_OK without promotion |
| src/nvidia/src/libraries/resserv/src/rs_resource.c:484/501 `refFindAncestorOfType` (Device/Subdevice, or ancestor `bInvalidated`) | via `gpuGetByRef` in `gpuresConstruct` | yes, but `gpuresConstruct` returns NV_OK regardless (gpu/gpu_resource.c:90-94) |
| gpu/fifo/channel_descendant.c `chandesConstruct` | class/engine lookups | returns INVALID_CLASS / INVALID_OBJECT_PARENT, not 0x57 |

- Conclusion of part A: in the stock code, a silent 0x57 for this alloc can only come from the
  resource-server parent lookup (alloc_free.c:748). Any 0x57 from inside the GR object path would
  print. Part B decides between the two with a continuous log.

### What a live CUDA context shares with a second client's compute object

- Per GPU, shared across clients: `kgraphicsGetStaticInfo` (static, loaded once), the deferred
  context-buffer sizes (`pContextBuffersInfo`, once), the global ctx buffers per gfid
  (`globalCtxBuffersInfo.pGlobalCtxBuffers[gfid]`: memdescs and `bInitialized[]`), the FECS event
  buffer and its mapping state, the ctx-buffer pool.
- Per TSG (`KernelGraphicsContextUnicast`), not shared: main/patch/PM ctx buffers, the local
  ctx buffers (VPR only), every VA list (`globalCtxBufferVaList[]`, `ctxPatchBuffer.vAddrList`,
  `pmCtxswBuffer.vAddrList`, `ppEngCtxDesc[]->vaList`), the VEID heap (kernel_ctxshare.c).
- `kgrctxShouldPreAllocPmBuffer` pre-allocates the PM buffer only for externally owned (UVM) VA
  spaces or SR-IOV profiling; CUDA's own TSG is the UVM one, the Vulkan TSG is not.
- None of the per-GPU items is looked up by a key that a second client's object could miss: they
  are indexed by gfid (always PF here) and allocated once. So a CUDA context cannot make a
  per-GPU lookup in `_kgrAlloc` miss; a miss must come from per-TSG/per-client state.

### What the device answers into these paths (crates/vfio-user-nvgpu/src)

- `GPU_PROMOTE_CTX`: recorded per (hChanClient, hObject), answered NV_OK always (cuda.rs:91-98,
  channel.rs:478-531). A host-side refusal keeps the channel on the software executor, never
  reaches the guest (mirror.rs:2160-2171, 3386-3395).
- Internal static KGR controls: no client/channel argument; forwarded fixed controls or constants
  (kgr.rs:315-641). `FIFO_GET_ENGINE_CONTEXT_PROPERTIES` forwarded (ctrl.rs:1339-1345). Device-info
  table from the chip data (ctrl.rs:1138-1163).
- `GR_SET_CTXSW_PREEMPTION_MODE` and `GR_CTXSW_ZCULL_BIND`: echo NV_OK (cuda.rs:47-50,
  ctrl.rs:1347-1360). ctxshare allocs record hVASpace only; no VEID state (channel.rs:574-577).
- RC events go only to mirrored channels whose own error notifier the host wrote (boot.rs:601-641,
  events.rs:139-157), keyed by (hClient, hObject).
- Nothing here depends on another client existing. The stage runs with `CARAXES_MIRROR_CHANNELS=1
  CARAXES_MIRROR_GR_SHADOW=1` (sottr-2026-09-24/env.sh:21), so the CUDA compute channel and the
  Vulkan GR channels are host-mirrored; a host-side effect (RC notifier, semaphore, a stalled
  host channel) can still reach the guest through the channel's own notifier or through GPU work
  that never completes.

## Part B: instrumented guest (2026-09-26 01:43-02:40)

### What was done

- The Ubuntu DKMS package ships the RM core prebuilt (`/usr/src/nvidia-615.71.09/nvidia/nv-kernel.o_binary`),
  so the GR sources cannot be rebuilt from it. Instead the GitHub 615.71.09 tree with `cxdbg.patch` (the complete diff: `patch.py`'s
  wrappers plus the promote-entry, subdevice-lookup and map-detail prints added by hand) was built in the guest (`make -j3 modules`, gcc 13.3, 6.8.0-136 headers, ~9 min)
  and its four modules replaced the DKMS ones in `/lib/modules/6.8.0-136-generic/updates/dkms/`
  (originals kept in `~/ko-orig/` in the guest).
- The vm4-615 overlay persists across `up-615.sh` (`sottr-2026-09-24/up.sh:65` creates it only when
  missing), so the stock `.ko.zst` files were put back and `depmod -a` run before the last
  `down-615.sh`. The overlay now holds the stock modules again (checked: no `CXDBG` string in
  `nvidia.ko.zst`). The build tree stays in `~/cxbuild/`, the run folders in `~/cxrun/`.
- Kernel log recorded continuously (`dmesg -w` per run), ioctls with `rmioctl-log2` (the repo
  logger plus client handle and thread id; for r4 also RM_FREE and ioctls slower than 0.2 s).
  Sunshine stopped before every run. Script: `repro.sh` in the guest's `~/nvenc`
  (orders: CUDA NVENC probe then `vkcube-wayland --c 30`; a long vkcube, then the probe, then a
  second vkcube; create/destroy cycles).
- Boots: 1) poollock-304012cc (as asked; stock baseline + first instrumented runs), then
  2)-4) **invhold-3bad9810** (main b715c019 with the exclusion-release fix plus the invalidate
  gate, on the coordinator's request); all with `SOTTR_NAT=0` (no host sudo), boots 3-4 with
  `SOTTR_SUN_ENCODER=software` so no CUDA context exists at boot.

| Run (binary) | Result |
|---|---|
| stock-a (poollock, stock module) | vkcube before OK; **during the CUDA probe: hung 66 s** (c5c0 on both GR channels OK, then no progress); after OK. No RM check line. |
| dbg-a (poollock, instrumented, after an in-place module reload) | every vkcube hung 60 s (also before CUDA); discarded |
| inv-a (invhold) | before OK; during: host watch SIGTERM to the device (host `nvidia-smi` 3.29 s), guest then Xid 119 (RPC timeout) |
| r1 CUDA then Vulkan (invhold) | all three vkcube OK (during: 18 s) |
| **r2 Vulkan, then CUDA, then Vulkan (invhold)** | **second vkcube: c5c0 0x57, abort in `demo_create_device`**; the CUDA probe gave no output in 60 s |
| r3 cycles (invhold) | host watch SIGTERM again (02:21:40, `nvidia-smi` 3.24 s) |
| r4 = r2 again, timed logger (invhold) | second vkcube OK (23 s); the CUDA probe again silent for 60 s |

### The site

`src/nvidia/src/kernel/rmapi/alloc_free.c:748`, `serverAllocResourceUnderLock`:

```
status = clientGetResourceRef(pClient, hParent, &pParentRef);
if (status != NV_OK)
    goto done;
```

`r2-vk-then-cuda/kmsg-cxdbg.txt:297-299`:

```
[  427.412569] NVRM: serverAllocResourceUnderLock__cxinner: CXDBG alloc: parent 0xbeee0100 not in client 0xc1d00087 (class 0xc5c0 hObject 0xbeee90c0) -> 0x57
[  427.412574] NVRM: serverAllocResourceUnderLock: CXDBG serverAllocResourceUnderLock -> 0x57 class 0xc5c0 hClient 0xc1d00087 hParent 0xbeee0100 hObject 0xbeee90c0
[  427.412585] NVRM: rmapiAllocWithSecInfo: CXDBG rmapiAllocWithSecInfo -> 0x57
```

- No `CXDBG kgrobj:` line exists for this c5c0: the GR object constructor never ran. The
  kgrobj/kgrctx/kgraphics/ctxshare path is not involved; nothing in it missed.
- **What misses is the client's own handle table: the GR channel 0xbeee0100 had already been
  freed by the same client.** Device trace (`r2-vk-then-cuda/device-trace-excerpt.txt`):
  0xbeee0100 created at 423.29 s (chid 48, TSG 0xcef80002, ctxshare 0xcef00080), objects
  902d/c5b5/a140/c597 at 423.46-423.51 s, then **11 s with no doorbell on chid 48, 54 or 55** (the new
  client's GR and two CE channels), then at 434.63 s `FREE` of c5b5, c597, 902d, a140 and the channel
  0xbeee0100 (hRoot 0xc1d00087), then the c5c0 attempt, then the CE channels and the ctxshare.
- The guest side agrees: c597 at 416.12 s, the 0x57 at 427.41 s (11.3 s later); the ioctl log shows
  `NV_SEMAPHORE_SURFACE_CTRL_CMD_UNBIND_CHANNEL` (0x00da0006) for 0xbeee0100 right before the c5c0 -
  the start of the client's teardown of that channel, the same signature as in both earlier captures.
- So the Vulkan driver (userspace, one thread, tid 5886) gives up after ~11 s, tears its main GR
  channel down, and its error path still allocates the compute object on the freed handle. The 0x57
  is the symptom of that teardown, not a GR lookup.

### What the device must do differently

- The primary failure is the **stall**: while another GR/compute client is active, a newly created
  client's initialisation makes no progress (no doorbell for ~11 s, then the driver's own timeout).
  The same stall shows the other way round: with a vkcube running, the CUDA NVENC probe prints nothing
  in 60 s (r2, r4; the earlier note "the probe hangs in context creation"). In stock-a the stall
  appears as a vkcube that hangs after its c5c0 instead of timing out.
- Not the cause: the device's GR answers. `GPU_PROMOTE_CTX` is echoed NV_OK, the static KGR
  controls have no client argument, preemption mode and zcull bind are echoed (part A). The
  instrumented runs show every propagated GR status 0x0 up to the teardown.
- Proposed device work, in order:
  1. Find what the new client waits on during the 11 s. Its channels got no doorbell, so it waits on a
     CPU-visible condition, not on its own GPU work: a semaphore/notifier written by another channel
     (the semaphore-surface object 0xa, class 0x00da, which it unbinds first), an event that should
     reach it through the device's event path, or a host-side step that serialises a second
     GR TSG behind the first. `rmioctl-log2.c` (here) now records RM_FREE and every ioctl slower than
     0.2 s; run r2 with it until it reproduces, together with the device's event/notifier lines for
     the second client.
  2. Deliver notifier/event writes per client lifetime, not per numeric handle: handles repeat across
     clients and TSGs (every vkcube reuses 0xbeef0100, 0xcef80000...; the promote map and channel
     registry are keyed by (hClient, hObject), which is correct, but host mirror objects and shadow
     exclusions must not outlive their guest owner; the invhold binary logs `ERROR freeing host TSG
     0x80001ad6: 0xfffffffe` for guest TSG 0xcef80003 at teardown).
- A guest-kernel issue triggered by an answer of ours: no. The guest kernel behaves correctly (the
  parent is gone); the userspace driver's error path allocates on a freed channel, and it gets there
  because its initialisation stalls on the device.

### Also found

- Host watch: two of four boots lost the device to the stage's own guard (SIGTERM when host
  `nvidia-smi` takes > 3 s) while CUDA and Vulkan ran together in the guest; the guest then
  logs Xid 119 (RPC timeout, FREE) and Xid 175. Host Xid 0 in all sessions.
- The poollock binary showed the old "VA Space alloc failed" bursts on the host during boot 1
  (coordinator: 01:48, 01:57, 01:58); the invhold runs are the ones to cite.
- Promote bookkeeping (coordinator's question): `record_promote_ctx` (channel.rs:478-513) never wipes a
  PA with a VA-only entry (`if pa != 0`). But the guest initialises the global buffers (FECS event,
  priv access maps) once per GPU (`bInitialized[]` per gfid), and the PA lands only in the map of
  the channel that promoted first (e.g. `CXDBG promote ... chan 0xc1d00012/0xbeef0100 ... init 1`);
  `record_free` (channel.rs:754) drops that map with the channel. A later mirrored channel's replay
  (mirror.rs:3350-3395) then has VA-only entries for those buffers. Worth a per-GPU store for the
  global buffers' PA/size.
- The nvidia-drm `__nv_drm_handle_flip_event` warnings (nvidia-drm-crtc.h:368) appear in every run,
  with and without CUDA.

### Files

- `repro.sh` (the guest run script)
- `NOTES.md` (this), `cxdbg.patch`, `patch.py` (instrumentation), `rmioctl-log2.c` (logger variant)
- `<run>/summary.txt`, `<run>/kmsg-cxdbg.txt` (CXDBG, check, assert and Xid lines of the continuous log)
- `r2-vk-then-cuda/vk-during2.rmlog` (the failing vkcube), `r2-vk-then-cuda/device-trace-excerpt.txt`
- `stock-a/vk-during.rmlog` (the hung vkcube on the stock module)
- The full guest logs stay in the guest (`~/cxrun/`) and in the scratch copy of this session.

## Follow-up: the 11 s stall (2026-09-26, analysis without a VM)

Verdict so far: the 11 s are spent **in user mode** in the Vulkan driver's main thread, after its
last GPU-memory setup ioctl and before the unbind/free of its first GR channel. No ioctl of that
thread is pending during the wait. The UMD then frees the whole channel tree with one RM_FREE and
continues on its error path (the c5c0 on the freed handle). vkcube retries device creation, and
each attempt stalls the same way.

### 1. Control 0xd01

- `NV0000_CTRL_CMD_CLIENT_GET_ADDR_SPACE_TYPE` (ctrl0000client.h:56-66): `{hObject, mapFlags,
  addrSpaceType out}`. The 12-byte params are handle, map flags and the answer, not a count.
  - `mapFlags 0x8000` = `NVOS33_FLAGS_MAPPING_DIRECT` (nvos.h:1758-1761).
  - `addrSpaceType 1` = SYSMEM.
  - The handles (0x11/0x12, 0x2b/0x2c, 0x39/0x3a) are memory objects from RM_ALLOC_MEMORY /
    VID_HEAP_CONTROL, which the logger did not print.
- Implementation `cliresCtrlCmdClientGetAddrSpaceType_IMPL` (rmapi/client_resource.c:5050): a handle
  lookup plus `memGetMapAddrSpace`. No RPC, no wait. The UMD asks it before it CPU-maps each buffer.
- The extra `(0x3c, 0, 1)` / `(0x1c, 0, 1)` query is already on the error path. It comes after the
  unbind in r2 (vk-during2.rmlog:217-218) and belongs to the UMD's error reporting, together with
  `0x3d05`, `alloc 0x00fe` and `GPU_GET_NAME 0x20800110`.

### 2. Where the 11 s go (r2, from the continuous kernel log)

- With `ResmanDebugLevel=0` the guest kernel logs every escape with the caller's param pointer.
  The failing vkcube thread (tid 5886) uses stack pointers `0xcc6a....`. `r2-vk-then-cuda/kmsg.txt`,
  that thread only:
  - `416.1236`: c597 on 0xbeee0100.
  - `416.1259-416.1278`: 7 × control (906f0101 ×6, 0x801707).
  - `416.1287-416.2312`: VID_HEAP_CONTROL (0x4a) ×6, MAP_MEMORY_DMA (0x57) ×4, the zcull bind and
    preemption-mode controls, GET_ADDR_SPACE_TYPE ×2, MAP_MEMORY (0x4e) ×2: three buffers
    allocated, GPU-mapped and CPU-mapped (both are SYSMEM, per the 0xd01 answers).
  - **`416.2312` to `427.2444`: no escape from this thread (11.01 s).**
  - `427.2444`: `0xda0006` UNBIND of 0xbeee0100.
  - `427.2469`: one RM_FREE (0x29), the channel tree. The device got it at device time 434.633; the
    device/guest clock offset is 7.388 s, from the c597 pair.
  - `427.37-427.41`: ZBC reads. `427.4121`: c5c0 -> 0x57.
- The same thread issues the free. It is not the CUDA process and not a kernel thread.
- vkcube made three device creations in r2 (vk-during2.rmlog:205, 347, 539; 0xbeef0100 twice, then
  0xbeee0100). Each stops after the same step with an unbind about 11 s later. The first two end
  in a full teardown and a retry; the third reaches the stale c5c0.
- 11 s ≈ a 10 s UMD timeout plus teardown. That points at a bounded wait for GPU progress on the
  new channel, not at a lock.

### 3. What the UMD polls (candidates; to confirm with the gdb snapshot of `repro2.sh`)

- None of these reads traps, so none of them shows in the device trace:
  - **The three SYSMEM buffers mapped just before the wait** (0x4a/0x57/0x4e at 416.13-416.23),
    most likely the init pushbuffer and its completion semaphore. The device never backs guest
    sysmem: the host GPU or the software executor writes the semaphore into guest RAM through the
    IOMMU mapping/DMA table.
  - **USERD GPGet** of chid 48 (`userd Fb(0x12fa7f800)` in the r2 device trace, VRAM). The UMD reads it through its
    BAR1 mapping, which is backed by the device's BAR1 pool (the memfd over host VRAM). A mirrored
    channel's GPGet is advanced by the host PBDMA; for a software channel, by the executor.
  - The GPFIFO/pushbuffer in VRAM, same BAR1 path.
- The device only sees the UMD's doorbell (usermode region BAR0 0xbb0090, ioeventfd or trapped).
  The r2 excerpt counted only `guest chid N -> host token` lines, i.e. rings of already mirrored
  channels. A first ring that builds a GR shadow channel, a held ring (`doorbell chid N: scheduling
  disabled by the guest; held`, ctrl.rs:437-446) or a stale pool token (`not assigned to any
  channel (stale), dropped`, ctrl.rs:237-243) has other wording. The trace was truncated before it
  could be checked, so "no doorbell" is not established.

### 4. Device-side candidates (to be decided by the next run)

- The error is per channel: in the vulkan capture the sibling 0xbeed0104 got its c5c0 before the
  first channel's unbind. Only the channel whose init the UMD waits on is torn down.
- (a) **The first ring of the new GR channel builds a shadow channel** (`CARAXES_MIRROR_GR_SHADOW=1`,
  ctrl.rs:449 `gr_shadow_doorbell`). This needs host RM work: host TSG, channel, promote. The CUDA
  context's mirrored compute TSG shares the host GR engine and its subcontexts. A refused or delayed
  build leaves the guest's first submission unexecuted until the UMD times out.
- (b) **Doorbell token pool** (ctrl.rs:203-260, `hand_out_token`): after the first trapped ring the
  device writes a pool token into the channel's work-submit notifier, and the UMD rings that token
  from then on. If chid 48 (reused by every attempt) or a pool token still maps to the previous
  owner, the ring is dropped as stale or lands on another channel.
- (c) **Scheduling flags** (rc.rs:239-274): a software GR channel that is not enabled has its
  doorbell held. If the CUDA side's `FIFO_DISABLE_CHANNELS` / usermode toggle leaves a flag set, a
  new channel's rings are held. The flags are keyed by (hClient, hChannel), so a stale entry for a
  reused handle would hit exactly the second client.
- Next run (`repro2.sh` + `rmioctl-log3.c`, both here): per-thread timestamps, memory escapes with
  raw params, a 1 s state sampler, and gdb snapshots at +6 s and +9 s of the stalled vkcube (the
  polled address against `/proc/pid/maps`). On the device side, grep the new client's chid for
  every doorbell wording (`-> chid N$`, `held`, `stale`, `shadow: GR chid N`, `REFUSED`,
  `in software`).
