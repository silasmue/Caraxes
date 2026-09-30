<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# RM resource model

The NVIDIA Resource Manager (RM) organises everything a client can touch as a tree of
objects under a client handle: devices, subdevices, memory, address spaces, channels,
engine objects and events. Both Leandro architectures speak this model, VirtIO Leandro at
the ioctl layer and VFIO Leandro one layer below, inside the GSP RPCs. Driver references
name the open kernel modules (615.71.09 for cited lines).

## The tree

| Level | Class | Role |
|---|---|---|
| client | `NV01_ROOT` (0x0000, 0x0041 for a user client) | one per process and driver instance; owns handles |
| device | `NV01_DEVICE_0` (0x0080) | one physical GPU as the client sees it |
| subdevice | `NV20_SUBDEVICE_0` (0x2080) | the GPU's engines, memory, controls (`NV2080_CTRL_*`) |
| memory | `NV01_MEMORY_LOCAL_USER`, `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR`, `NV01_MEMORY_LIST_*` | VRAM allocations, pinned system memory, page lists |
| VA space | `FERMI_VASPACE_A` | a GPU virtual address space with its page tables |
| TSG and channel | `KEPLER_CHANNEL_GROUP_A` (0xA06C), `TURING_CHANNEL_GPFIFO_A` (0xC46F) | a time-slice group and a GPFIFO channel on a runlist |
| engine objects | `TURING_COMPUTE_A`, `TURING_A` (3D), `TURING_DMA_COPY_A` | what a channel's subchannels can run |
| deferred API | `NV50_DEFERRED_API_CLASS` (0x5080) | a software object under a channel that runs a registered control when the channel reaches it |
| events | `NV01_EVENT_*` | notifications bound to an object |

- A handle is a 32-bit number the client chooses, or 0 to let the RM assign one (nouveau
  allocates this way, [research: open-render](../research/open-render-2026-09-29.md)); the
  same number can exist under two clients. The server side keeps the
  tree in the resource server
  (`src/nvidia/src/libraries/resserv/src/rs_client.c`, `rs_server.c`); a free of a parent
  frees its children (615: `rs_client.c:1347`).
- Allocation: `NV_ESC_RM_ALLOC` with a class id and a parameter struct; refused with
  `NV_ERR_INVALID_CLASS` for an unknown class (615: `src/nvidia/src/kernel/rmapi/alloc_free.c:717-720`)
  and `NV_ERR_NOT_SUPPORTED` for a class outside the GPU's class database (615:
  `alloc_free.c:1393-1398`).
- Control: `NV_ESC_RM_CONTROL` with a command id and a parameter struct; see
  [rm-controls](rm-controls.md).
- Duplicate: `DUP_OBJECT` copies an object reference into another client; the source is
  looked up globally, which is why VirtIO Leandro guards foreign handles
  ([security.md](../virtio/security.md)).
- The list of every class with its routing flags: `src/nvidia/src/kernel/rmapi/resource_list.h`
  (224 `RS_ENTRY` rows in 615).

## The deferred API object

- A client registers a control under an API handle on the object
  (`NV5080_CTRL_CMD_DEFERRED_API` 0x50800101, `_V2` 0x50800103;
  `src/common/sdk/nvidia/inc/ctrl/ctrl5080.h`), and the software method 0x200 with that handle
  runs it in the channel's stream. The kernel RM forwards V2 to GSP-RM as `_INTERNAL`
  0x50800104 (`src/nvidia/src/kernel/gpu/deferred_api.c`, `defapiCtrlCmdDeferredApiV2`).
- The Windows KMD prepares GR contexts and moves channels to larger rings this way
  ([channel-migration-and-deferred-api](channel-migration-and-deferred-api.md)).

## Where each architecture cuts the tree

- VirtIO Leandro forwards the ioctls (`RmIoctl` in `src/nvidia/arch/nvalloc/unix/src/escape.c`
  on the host side) and keeps per guest process a session, mirrored file descriptors and
  handle bookkeeping ([architecture.md](../virtio/architecture.md)).
- VFIO Leandro's guest kernel RM keeps the whole tree itself; only GSP-bound allocations and
  routed controls travel as `GSP_RM_ALLOC` and `GSP_RM_CONTROL`
  (`rpcRmApiAlloc_GSP`, `rpcRmApiControl_GSP` in `src/nvidia/src/kernel/vgpu/rpc.c`). The
  device constructs the same objects under the guest's handles and mirrors channels and
  VA spaces on host objects of its own ([execution-mirror](execution-mirror.md)).
- A stock TU106 Linux stack allocates 36 of the 48 GSP-bound classes in the chip's class
  database ([gsp-rpc-and-events](gsp-rpc-and-events.md)). The four guest drivers differ in what
  they allocate ([guest-drivers-and-abis](guest-drivers-and-abis.md)).

## Depth

- [research/gsp-rpc-events-classes.md](../research/gsp-rpc-events-classes.md): the classes on
  the wire and the refusal rule.
- [caraxes/gsp-plan/classes.md](../vfio/gsp-plan/classes.md): every class that reaches GSP,
  its alloc parameters in 610 and 615, and the device's answer.
- [caraxes/leandro-reuse.md](../vfio/leandro-reuse.md): why the ioctl-shaped v1 translation
  and the inline GSP payloads differ.
