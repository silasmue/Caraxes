<!-- SPDX-License-Identifier: MIT -->
# Guest drivers and ABIs

One device binary serves four guest drivers: the Linux 615.71.09 open kernel modules, the
Windows 11 KMD of driver 617.14, nova-core with the GSP-RM 570.144 firmware, and nouveau's
r570 path with Mesa NVK. All four are GSP clients, but they speak two transport generations
with their own control and allocation layouts. The device reads the generation from the
guest's GSP boot, before any queue links, and answers with that generation's layouts
([research: one-abi-binary](../research/one-abi-binary-2026-09-28.md)).

## The four guests

| Guest | Driver and firmware | Transport | What reaches the device |
|---|---|---|---|
| Linux 615 | open kernel modules 615.71.09 with their `gsp_tu10x.bin` | message queue v2, GMCAPI `GSP_INIT` | the 14 RPC functions of a 615 client, 272 RPCs per boot |
| Windows 617 | the closed 617.14 KMD in GSP-RM mode | message queue v2, GMCAPI `GSP_INIT`, served as 615 | the same RPC functions, plus controls outside the 615 export tables |
| nova-core | Linux 7.2.7, GSP-RM 570.144 | message queue v0, pre-610 element | the boot RPCs only: no allocation, no control |
| nouveau with NVK | Linux 7.2.7 nouveau (r570 path), GSP-RM 570.144, Mesa NVK | message queue v0, pre-610 element | the boot RPCs, then RM allocations and controls in 570 layouts |

## How the device picks the ABI

- `BootModel::detect_guest` (`crates/vfio-user-nvgpu/src/boot.rs`) decides before any queue
  link, at the first doorbell or at Booter Load, from RMARGS, which it finds through the libos
  arguments in the falcon mailboxes ([gsp-boot-exchange](gsp-boot-exchange.md)).
- RMARGS (`GSP_ARGUMENTS_CACHED`) opening with the magic "GSP " means 615: message queue v2
  and the GMCAPI init (Linux 615, Windows 617). A 610 RMARGS starts with the low half of a
  page-aligned address, which never equals the magic
  ([vfio/gsp-transport-615.md](../vfio/gsp-transport-615.md) §8).
- Without the magic the queue is v0. At the link the first command element decides
  (`ver::element_frame`): the "VRPC" v3.0 header at offset 0x10 is 610, at 0x30 pre-610,
  served as 570. RMARGS not yet mapped: the next doorbell or Booter Load decides.
- `--guest-abi 570|595|610|615` pins the ABI, and 595 exists only as a pin. A guest that
  shows another gets a `guest ABI: ERROR` note and no link. `CARAXES_GUEST_ABI` is gone since
  2026-09-30; the device log names the choice (`grep 'guest ABI' gpu.log`).
- The generation is read, not the version: 570 and 595 look alike, and 617 is not named.

## Layouts per version

- Host layouts follow the running host driver (`hostabi::select`), guest layouts the
  detection (`ver::guest()`, one guest per device process). They meet in the PES map and the
  SM order, translated field by field (`hostabi::translate_sm_order`).
- `ver::GuestAbi` (570, 595, 610, 615) selects a `GuestLayouts` table (`L610`, `L615`) that
  generic code reads through `ver::layouts()`. 45 items differ between 610 and 615, among them
  the element header, the RMARGS queue offsets, `FIFO_GET_DEVICE_INFO_TABLE` (3212 or 3340
  bytes), the TSG allocation (20 or 28 bytes) and id 0x20800A36 (chip info in 610, fabric-probe
  sync in 615). `src/ver/common.rs` hides each of them behind a private stand-in.
- 570 (`src/ver/v570.rs`) is 610 plus the pre-610 element, the sequencer event, the 570 static
  information, the control frame with parameters at offset 24 instead of 40
  (`ver::RmControlFrame`), and the channel and VA-space allocations translated at the RPC
  ([research: open-render](../research/open-render-2026-09-29.md)); new versions:
  [gsp-abi-615](gsp-abi-615.md).

## Linux 615 kernel RM

- Sends one `GMCAPI_CMD_GSP_INIT` with 142 NVKV keys, 44 of them registry entries, and sets
  both unstable-interface bits ([research: gsp-transport](../research/gsp-transport.md)).
- A boot is 272 RPCs, 189 of them controls ([gsp-rpc-and-events](gsp-rpc-and-events.md)).
- Carries `NVreg_GpuInitOnProbe=1` and three registry keys in its module options
  ([kernel-rm-hal-and-registry](kernel-rm-hal-and-registry.md)).

## Windows 617 KMD

- Runs GSP-RM: its driver key carries `EnableGpuFirmware` and `RMEnableGpuFirmware` = 1
  ([research: windows-findings](../research/windows-findings.md), Cloud Hypervisor step 1).
- Uses the RPC functions of the Linux client (run X0: 107 controls, 33 allocations, 36 frees,
  3 `ALLOC_MEMORY`; [research: astra-review](../research/astra-review-2026-09-26.md)).
- Allocates class 0xB297 (no public header) under its subdevice; refused, the start ended in
  Code 43, so the device records it (`WINDOWS_ONLY` in `classes.rs`). FIFO control 0x20801111,
  absent from the 615 export tables, carries two 0xB297 handles and an 8-page sysmem
  allocation; the device acknowledges it (`recorded.rs`). Its meaning is unknown; it comes
  at teardown and twice per frame in an NVENC stream
  (`caraxes/runs/win11-2026-09-25/night-2026-09-25/WINDOWS-LOOP.md` Y8 to Y10,
  `caraxes/runs/codec-2026-09-30/NOTE.md`).
- Needs the GR preemption pool controls (`GR_GFX_POOL_QUERY_SIZE` 0x2080121F and three more)
  answered; refused, the KMD sends `GSP_SUSPEND` and reports Code 43 (windows-findings P39).
  They are kernel-only controls in the RM; `recorded.rs` answers them.
- Uses the deferred API object (0x5080, [channel-migration-and-deferred-api](channel-migration-and-deferred-api.md)) for GR
  `INITIALIZE_CTX`, `PROMOTE_CTX`, `EVICT_CTX` and `FIFO_UPDATE_CHANNEL_INFO` 0x20801116, which
  moves a channel to a new GPFIFO and USERD (`crates/vfio-user-nvgpu/src/deferred.rs`;
  [research: windows-deferred-context](../research/windows-deferred-context-2026-09-28.md);
  windows-findings, Cloud Hypervisor step 23).
- Allocates its channels PRIVILEGED and application GR channels with `SKIP_CTXBUFFER_ALLOC`,
  which Linux never sets (windows-findings P29, P29b).
- Gets `GSP_GET_FEATURES` bit 0 (`UVM_ENABLED`) = 1, Unix guests 0; the device decides from
  the guest OS type in `GSP_INIT` (windows-findings P31).
- The device's Windows-only answers sit behind open switches (`CARAXES_WIN_ACKS`, `_RUSD`,
  `_LPWR_2806_OK`, `_DEFERRED_CONTEXT`) and the diagnostic `_ACK_UNANSWERED`
  ([device-configuration](device-configuration.md)).

## nova-core

- Sends `GSP_SET_SYSTEM_INFO`, `SET_REGISTRY` (`RMSecBusResetEnable`,
  `RMForcePcieConfigSave`, `RMDevidCheckIgnore` = 1), `GET_GSP_STATIC_INFO` and
  `UNLOADING_GUEST_DRIVER`; it waits for `GSP_RUN_CPU_SEQUENCER` and `GSP_INIT_DONE`
  ([vfio/nova-guest.md](../vfio/nova-guest.md) §2.1).
- Checksums every element, matches a reply by function only, and has no channel code.
- The device rejects a bad RPC header inside a valid frame and keeps GMCAPI sequences at 64
  bits ([research: nova-transport-compatibility](../research/nova-transport-compatibility-2026-09-27.md)).
- Gate of 2026-09-29, pure emulation: 5 probes, 4 unloads, 0 ERROR lines
  ([research: open-render](../research/open-render-2026-09-29.md)).

## nouveau with NVK

- Boots like nova, but queues its first RPCs and rings before the mailboxes carry the libos
  arguments; the device links at Booter Load when commands are queued.
- Allocates root, device, subdevice, `FERMI_VASPACE_A`, bare `TURING_CHANNEL_GPFIFO_A`
  channels (no TSG), `TURING_A`, `TURING_COMPUTE_A`, `KEPLER_INLINE_TO_MEMORY_B`,
  `FERMI_TWOD_A` and `TURING_DMA_COPY_A`, some with hObject 0; the device then assigns the
  handle as the RM does.
- Needs `GPU_GET_FERMI_GPC_INFO` and `_TPC_INFO`, `GR_GET_ZCULL_INFO`, 0x20800A46 acknowledged
  as 570's KGR workaround id, and interrupt vectors for NVDEC1 and NVDEC2.
- NVK's bare GR channel runs on a host GR channel in a host TSG of its own; the kernel's fence
  channel stays on the software executor ([execution-mirror](execution-mirror.md)).
- Keeps only FB regions marked compressible and ISO, so it runs with `CARAXES_COMPRESSION=slice`.
- NVK draws and reads back a triangle; the gate of 2026-09-29 passed R1 to R8
  ([research: open-render](../research/open-render-2026-09-29.md)).

## Depth

- [research/one-abi-binary-2026-09-28.md](../research/one-abi-binary-2026-09-28.md): the detection, the 45 items and the tests.
