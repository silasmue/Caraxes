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

## Part B

Pending (the VM is held by the latency experiment and the Windows runs; see the end of this file).
The instrumentation is prepared: `cxdbg.patch` (made by `patch.py` against the 615.71.09 tree):
- wrappers print `CXDBG <function> -> <status>` on any failure of: `serverAllocResourceUnderLock`,
  `rmapiAllocWithSecInfo`, `clientAllocResource`, `chandesConstruct`, `gpuresConstruct`,
  `kgrobjConstruct`, `_kgrAlloc`, `kgrobjSetComputeMmio`, `kgrobjPromoteContext`,
  `kgrmgrCtrlRouteKGRWithDevice`, `kgraphicsInitializeDeferredStaticData`,
  `kgraphicsAllocKgraphicsBuffers`, `kgraphicsMapCtxBuffer` (with VAS, VA list, map count),
  `kgrctxAllocCtxBuffers`, `kgrctxMapCtxBuffers`, `kgrctxMap(Global)CtxBuffer(s)`,
  `kgrctxAlloc{Main,Patch,Pm}CtxBuffer`, `kgrctxPrepare{Initialize,Promote}CtxBuffer`,
  `kctxshare(api)Construct`;
- entry lines `CXDBG kgrobj:` (class, client/parent/object, chid, runqueue, context lookup, the
  channel's and TSG's context handles, legacy mode, VAS) and `CXDBG _kgrAlloc` (status, context, VAS);
- `CXDBG alloc: parent ... not in client` at alloc_free.c:748 and `CXDBG alloc: RPC for class ...`
  before the alloc RPC.
