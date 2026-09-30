<!-- SPDX-License-Identifier: MIT -->
<!-- Written 2026-09-23 from vendor/open-gpu-kernel-modules at tag 610.57.04 (commit e4a5faa).
     Section (a) is generated output of caraxes/tools/gsp-abi-sizes.sh; do not edit it by hand, re-run the tool. -->
# The GSP message ABI of driver 610.57.04, compiled

This note answers open question Q4 ([open-questions.md](caraxes-open-questions-2026-09-23.md)). It gives the exact sizes and offsets of the structures a synthetic GSP has to speak for driver 610.57.04, measured by a compiler rather than derived by hand. It also lists what the CPU-side kernel RM does with each field.

Unless a path says otherwise, it is relative to `vendor/open-gpu-kernel-modules/`. Every claim cites file:line in that tree. Offsets are for x86-64 (LP64), the ABI the kernel module is built for.

## Key numbers

| structure | sizeof | notes |
|---|---:|---|
| `msgqTxHeader` | 32 | one per queue, at offset 0 of the queue |
| `msgqRxHeader` | 4 | at `rxHdrOff` = 32 |
| `GSP_MSG_QUEUE_ELEMENT` header | 16 | `mctpHeader`, `nvdmHeader`, `checkSum`, `seqNum`; the payload starts at 16 |
| `GSP_MSG_QUEUE_ENCRYPTION_TAG` | 40 | only with Confidential Compute; not on Turing |
| `rpc_message_header_v` | 32 (align 8) | `rpc_message_data` starts at 32 |
| `GspStaticConfigInfo` | **1600** (0x640) | GET_GSP_STATIC_INFO reply payload |
| `GspSystemInfo` | **952** (0x3b8) | GSP_SET_SYSTEM_INFO payload |
| `rpc_set_guest_system_info_v03_00` | 792 | SET_GUEST_SYSTEM_INFO request and reply payload |
| `GSP_ARGUMENTS_CACHED` | 152 | the `RMARGS` region |
| `MESSAGE_QUEUE_INIT_ARGUMENTS` | 64 | first member of `GSP_ARGUMENTS_CACHED` |
| `LibosMemoryRegionInitArgument` | 32 | array element of the libos init-args page |
| `GspFwWprMeta` | 256 | matches `ct_assert(sizeof(*pWprMeta) == 256)` at kernel_gsp_tu102.c:772 |

Q4 asked whether the two big payloads fit in one message. They do, with room to spare. The largest RPC is `maxRpcSize` = 65520 bytes, so at most 65488 bytes follow the RPC header. A GET_GSP_STATIC_INFO reply is 16 + 32 + 1600 = 1648 bytes and a GSP_SET_SYSTEM_INFO message is 16 + 32 + 952 = 1000 bytes. Each fits in a single 4 KiB queue element.

### Corrections to earlier notes, found while doing this

- **The GSP static info has no GR info in this version.** The brief expected `NV2080_CTRL_INTERNAL_STATIC_GR_INFO` inside `GspStaticConfigInfo`. In 610.57.04 the struct holds 49 top-level members and none of them is GR info (gsp_static_config.h:80-167). Chip info and the device-info table are fetched later through internal RM controls: `gpuGetChipInfo` (gpu_gspclient.c:218, :325, :354) and `NV2080_CTRL_CMD_INTERNAL_GET_DEVICE_INFO_TABLE` (gpu_gspclient.c:241-246). On the wire those are GSP_RM_CONTROL RPCs, not static info.
- **SET_REGISTRY has no struct.** `rpc_set_registry_v17_00` exists only as a print descriptor (generated/g_rpc-structures.h:7900-7912). The payload is a `PACKED_REGISTRY_TABLE` (generated/g_os_nvoc.h:222-235) placed directly at `rpc_message_data` (rpc.c:10522-10526). The tool measures that struct instead.
- **The MCTP check covers every message.** [other-architectures.md](../vfio/other-architectures.md) §6(c) describes it as belonging to the CrashCat log path, which is wrong. `GspMsgQueueReceiveStatus` checks the MCTP version and the vendor ID on every status-queue element it reads (message_queue_cpu.c:737-759).
- **SET_GUEST_SYSTEM_INFO_EXT is not sent on bare metal.** Its macro sends only when `GPU_GET_VGPU_RPC` is non-NULL (vgpu/rpc_vgpu.h:71-78). That is only the case under `IS_VIRTUAL` (gpu.c:6357-6365). A GSP-client boot therefore sends SET_GUEST_SYSTEM_INFO alone.
- **`GSP_FMC_INIT_PARAMS` is Hopper-only.** It is used only inside `GSP_FMC_BOOT_PARAMS`, and only in kernel_gsp_gh100.c:173, :191 and :430. Turing never builds it. It is measured here for completeness.

## (a) Compiled sizes and offsets

### How the numbers are produced

`caraxes/tools/gsp-abi-sizes.sh` compiles `caraxes/tools/gsp-abi-sizes.c` against the vendored tree and runs it. The C file includes the headers in the same order and with the same `RPC_*` guard macros as kernel_gsp.c:60-99, which is the translation unit that uses these structs. It prints `sizeof`, `_Alignof` and `offsetof` for every field. Nested structs are expanded as dotted paths. An array of structs is listed as one row, and its element type gets its own table.

The field lists were generated once from clang's `-fdump-record-layouts` over the same translation unit and then written out by hand. If a field disappears or is renamed in a later driver, the file stops compiling. If a field is added, the size line changes.

**Why the ioctl flags are not enough.** `crates/nvrm-sys/wrapper.h` needs only SDK headers. xtask passes it five `-I` roots plus `-DNV_LINUX -D__linux__ -std=gnu11` (crates/xtask/src/abi/mod.rs:22-28, :312-317). The GSP structs, however, live in RM-internal headers that pull in NVOC classes: gsp_static_config.h:40 includes `gpu/gpu.h`, and message_queue_priv.h:31-33 pulls in more. These need the RM's own include roots and the nvport configuration.

**How the minimal flag set was found.** The starting point was the driver's full set (src/nvidia/Makefile:28-58 for the includes, :106-138 for the defines). Flags were removed one at a time. A flag stayed out only if the program still compiled and printed byte-identical output. What remained:

- 14 include roots: `src/nvidia/{interface, inc, inc/kernel, inc/libraries, generated, arch/nvalloc/common/inc, arch/nvalloc/common/inc/gsp, arch/nvalloc/unix/include}` and `src/common/{sdk/nvidia/inc, shared/inc, inc, inc/swref, uproc/os/common/include, nvlink/inband/interface}`.
- 7 defines: `-DNVRM -DPORT_IS_KERNEL_BUILD=1 -DPORT_IS_CHECKED_BUILD=0 -DPORT_MODULE_atomic=1 -DPORT_MODULE_debug=1 -DPORT_MODULE_memory=1 -DPORT_MODULE_thread=1`.

The header closure is 398 files and 7.5 MB (`clang -MM`). The ioctl closure in `vendor/nvidia-rm-headers/610.57.04` is 102 files.

**Verification.** `--check` compiles three ways and fails if any two outputs differ: the minimal set, the full driver set under GCC, and the minimal set under the other compiler. The run below passed all three: GCC 16.2.1 with both sets, and clang 22.1.8. The full driver set compiles only under GCC. Under clang, nvport selects `inline/atomic_c11.h`, which includes a `nvport/clang/stdatomic.h` that the open tree does not ship (nvport/atomic.h:396-397, nvport/inline/atomic_c11.h:36-40). The tool therefore always builds that set with GCC.

**Every requested struct compiled. None needed a best-effort estimate.** One layout has no C struct: the libos log-buffer header. It is an `NvU64` array, and its table below is assembled from the code that indexes it.

### Reproduce

```sh
caraxes/tools/gsp-abi-sizes.sh --check            # minimal flags, cross-checked
caraxes/tools/gsp-abi-sizes.sh --driver-flags     # the driver's full CFLAGS (GCC)
caraxes/tools/gsp-abi-sizes.sh /path/to/other/open-gpu-kernel-modules
```

### Output (610.57.04)

<!-- check: identical with clang, minimal flag set -->
<!-- check: identical with the driver flag set -->
<!-- compiler: cc (GCC) 16.2.1 20260810; flags: minimal; tree: vendor/open-gpu-kernel-modules -->
<!-- generated by caraxes/tools/gsp-abi-sizes.sh; driver 610.57.04 -->

### Message queue transport (msgq library, one header per queue)

#### `msgqTxHeader`: sizeof 32 (0x20), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `version` |
| 4 | 0x0004 | 4 | `size` |
| 8 | 0x0008 | 4 | `msgSize` |
| 12 | 0x000c | 4 | `msgCount` |
| 16 | 0x0010 | 4 | `writePtr` |
| 20 | 0x0014 | 4 | `flags` |
| 24 | 0x0018 | 4 | `rxHdrOff` |
| 28 | 0x001c | 4 | `entryOff` |

#### `msgqRxHeader`: sizeof 4 (0x4), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `readPtr` |

### Queue element and RPC header

#### `GSP_MSG_QUEUE_ELEMENT`: sizeof 16 (0x10), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `mctpHeader` |
| 4 | 0x0004 | 4 | `nvdmHeader` |
| 8 | 0x0008 | 4 | `checkSum` |
| 12 | 0x000c | 4 | `seqNum` |
| 16 | 0x0010 | flex | `payload` (flexible array) |

#### `GSP_MSG_QUEUE_ENCRYPTION_TAG`: sizeof 40 (0x28), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `encryptedSize` |
| 4 | 0x0004 | 4 | `reserved` |
| 8 | 0x0008 | 16 | `authTagBuffer` |
| 24 | 0x0018 | 16 | `aadBuffer` |

#### `rpc_message_header_v`: sizeof 32 (0x20), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `header_version` |
| 4 | 0x0004 | 4 | `signature` |
| 8 | 0x0008 | 4 | `length` |
| 12 | 0x000c | 4 | `function` |
| 16 | 0x0010 | 4 | `rpc_result` |
| 20 | 0x0014 | 4 | `rpc_result_private` |
| 24 | 0x0018 | 4 | `sequence` |
| 28 | 0x001c | 4 | `u` |
| 28 | 0x001c | 4 | `u.spare` |
| 28 | 0x001c | 4 | `u.cpuRmGfid` |
| 32 | 0x0020 | flex | `rpc_message_data` (flexible array) |

### Init RPCs: CPU to GSP and replies

#### `rpc_set_guest_system_info_v03_00`: sizeof 792 (0x318), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `vgxVersionMajorNum` |
| 4 | 0x0004 | 4 | `vgxVersionMinorNum` |
| 8 | 0x0008 | 4 | `guestDriverVersionBufferLength` |
| 12 | 0x000c | 4 | `guestVersionBufferLength` |
| 16 | 0x0010 | 4 | `guestTitleBufferLength` |
| 20 | 0x0014 | 4 | `guestClNum` |
| 24 | 0x0018 | 256 | `guestDriverVersion` |
| 280 | 0x0118 | 256 | `guestVersion` |
| 536 | 0x0218 | 256 | `guestTitle` |

#### `rpc_set_guest_system_info_ext_v25_1B`: sizeof 268 (0x10c), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 256 | `guestDriverBranch` |
| 256 | 0x0100 | 4 | `domain` |
| 260 | 0x0104 | 2 | `bus` |
| 262 | 0x0106 | 2 | `device` |
| 264 | 0x0108 | 4 | `gridBuildCsp` |

#### `rpc_gsp_set_system_info_v17_00`: sizeof 4 (0x4), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `data` |

#### `PACKED_REGISTRY_TABLE`: sizeof 8 (0x8), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `size` |
| 4 | 0x0004 | 4 | `numEntries` |
| 8 | 0x0008 | 0 | `entries` |

#### `PACKED_REGISTRY_ENTRY`: sizeof 16 (0x10), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `nameOffset` |
| 4 | 0x0004 | 1 | `type` |
| 8 | 0x0008 | 4 | `data` |
| 12 | 0x000c | 4 | `length` |

#### `rpc_get_gsp_static_info_v14_00`: sizeof 4 (0x4), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `data` |

#### `rpc_init_done_v17_00`: sizeof 1 (0x1), align 1

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 1 | `bIsD3Hot` |

### RM API passthrough RPCs

#### `rpc_gsp_rm_alloc_v03_00`: sizeof 32 (0x20), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `hClient` |
| 4 | 0x0004 | 4 | `hParent` |
| 8 | 0x0008 | 4 | `hObject` |
| 12 | 0x000c | 4 | `hClass` |
| 16 | 0x0010 | 4 | `status` |
| 20 | 0x0014 | 4 | `paramsSize` |
| 24 | 0x0018 | 4 | `flags` |
| 28 | 0x001c | 4 | `reserved` |
| 32 | 0x0020 | flex | `params` (flexible array) |

#### `rpc_gsp_rm_control_v03_00`: sizeof 40 (0x28), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `hClient` |
| 4 | 0x0004 | 4 | `hObject` |
| 8 | 0x0008 | 4 | `cmd` |
| 12 | 0x000c | 4 | `status` |
| 16 | 0x0010 | 4 | `paramsSize` |
| 20 | 0x0014 | 4 | `rmapiRpcFlags` |
| 24 | 0x0018 | 4 | `rmctrlFlags` |
| 28 | 0x001c | 4 | `rmctrlAccessRight` |
| 32 | 0x0020 | 8 | `reserved0` |
| 40 | 0x0028 | flex | `params` (flexible array) |

#### `rpc_free_v03_00`: sizeof 16 (0x10), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 16 | `params` |
| 0 | 0x0000 | 4 | `params.hRoot` |
| 4 | 0x0004 | 4 | `params.hObjectParent` |
| 8 | 0x0008 | 4 | `params.hObjectOld` |
| 12 | 0x000c | 4 | `params.status` |

#### `rpc_dup_object_v03_00`: sizeof 28 (0x1c), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 28 | `params` |
| 0 | 0x0000 | 4 | `params.hClient` |
| 4 | 0x0004 | 4 | `params.hParent` |
| 8 | 0x0008 | 4 | `params.hObject` |
| 12 | 0x000c | 4 | `params.hClientSrc` |
| 16 | 0x0010 | 4 | `params.hObjectSrc` |
| 20 | 0x0014 | 4 | `params.flags` |
| 24 | 0x0018 | 4 | `params.status` |

### Events GSP to CPU that the boot path accepts

#### `rpc_os_error_log_v17_00`: sizeof 272 (0x110), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `exceptType` |
| 4 | 0x0004 | 4 | `preemptiveRemovalPreviousXid` |
| 8 | 0x0008 | 4 | `runlistId` |
| 12 | 0x000c | 4 | `chid` |
| 16 | 0x0010 | 256 | `errString` |

#### `rpc_post_event_v17_00`: sizeof 32 (0x20), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `hClient` |
| 4 | 0x0004 | 4 | `hEvent` |
| 8 | 0x0008 | 4 | `notifyIndex` |
| 12 | 0x000c | 4 | `data` |
| 16 | 0x0010 | 2 | `info16` |
| 20 | 0x0014 | 4 | `status` |
| 24 | 0x0018 | 4 | `eventDataSize` |
| 28 | 0x001c | 1 | `bNotifyList` |
| 29 | 0x001d | flex | `eventData` (flexible array) |

#### `rpc_ucode_libos_print_v1E_08`: sizeof 8 (0x8), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `ucodeEngDesc` |
| 4 | 0x0004 | 4 | `libosPrintBufSize` |
| 8 | 0x0008 | flex | `libosPrintBuf` (flexible array) |

#### `rpc_gsp_lockdown_notice_v17_00`: sizeof 1 (0x1), align 1

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 1 | `bLockdownEngaging` |

### GET_GSP_STATIC_INFO reply payload

#### `GspStaticConfigInfo`: sizeof 1600 (0x640), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 268 | `gidInfo` |
| 0 | 0x0000 | 4 | `gidInfo.index` |
| 4 | 0x0004 | 4 | `gidInfo.flags` |
| 8 | 0x0008 | 4 | `gidInfo.length` |
| 12 | 0x000c | 256 | `gidInfo.data` |
| 268 | 0x010c | 48 | `SKUInfo` |
| 268 | 0x010c | 4 | `SKUInfo.BoardID` |
| 272 | 0x0110 | 9 | `SKUInfo.chipSKU` |
| 281 | 0x0119 | 5 | `SKUInfo.chipSKUMod` |
| 288 | 0x0120 | 4 | `SKUInfo.skuConfigVersion` |
| 292 | 0x0124 | 5 | `SKUInfo.project` |
| 297 | 0x0129 | 5 | `SKUInfo.projectSKU` |
| 302 | 0x012e | 6 | `SKUInfo.CDP` |
| 308 | 0x0134 | 2 | `SKUInfo.projectSKUMod` |
| 312 | 0x0138 | 4 | `SKUInfo.businessCycle` |
| 320 | 0x0140 | 904 | `fbRegionInfoParams` |
| 320 | 0x0140 | 4 | `fbRegionInfoParams.numFBRegions` |
| 328 | 0x0148 | 896 | `fbRegionInfoParams.fbRegion` |
| 1224 | 0x04c8 | 1 | `bPdiValid` |
| 1232 | 0x04d0 | 8 | `pdi` |
| 1240 | 0x04d8 | 80 | `sriovCaps` |
| 1240 | 0x04d8 | 4 | `sriovCaps.totalVFs` |
| 1244 | 0x04dc | 4 | `sriovCaps.firstVfOffset` |
| 1248 | 0x04e0 | 4 | `sriovCaps.vfFeatureMask` |
| 1256 | 0x04e8 | 8 | `sriovCaps.FirstVFBar0Address` |
| 1264 | 0x04f0 | 8 | `sriovCaps.FirstVFBar1Address` |
| 1272 | 0x04f8 | 8 | `sriovCaps.FirstVFBar2Address` |
| 1280 | 0x0500 | 8 | `sriovCaps.bar0Size` |
| 1288 | 0x0508 | 8 | `sriovCaps.bar1Size` |
| 1296 | 0x0510 | 8 | `sriovCaps.bar2Size` |
| 1304 | 0x0518 | 1 | `sriovCaps.b64bitBar0` |
| 1305 | 0x0519 | 1 | `sriovCaps.b64bitBar1` |
| 1306 | 0x051a | 1 | `sriovCaps.b64bitBar2` |
| 1307 | 0x051b | 1 | `sriovCaps.bSriovEnabled` |
| 1308 | 0x051c | 1 | `sriovCaps.bSriovHeavyEnabled` |
| 1309 | 0x051d | 1 | `sriovCaps.bEmulateVFBar0TlbInvalidationRegister` |
| 1310 | 0x051e | 1 | `sriovCaps.bClientRmAllocatedCtxBuffer` |
| 1311 | 0x051f | 1 | `sriovCaps.bNonPowerOf2ChannelCountSupported` |
| 1312 | 0x0520 | 1 | `sriovCaps.bVfResizableBAR1Supported` |
| 1320 | 0x0528 | 4 | `sriovMaxGfid` |
| 1324 | 0x052c | 12 | `engineCaps` |
| 1336 | 0x0538 | 1 | `poisonFuseEnabled` |
| 1344 | 0x0540 | 8 | `fb_length` |
| 1352 | 0x0548 | 64 | `gpuNameString` |
| 1416 | 0x0588 | 64 | `gpuShortNameString` |
| 1480 | 0x05c8 | 1 | `bGpuInternalSku` |
| 1481 | 0x05c9 | 1 | `bIsQuadroGeneric` |
| 1482 | 0x05ca | 1 | `bIsQuadroAd` |
| 1483 | 0x05cb | 1 | `bIsNvidiaNvs` |
| 1484 | 0x05cc | 1 | `bIsVgx` |
| 1485 | 0x05cd | 1 | `bGeforceSmb` |
| 1486 | 0x05ce | 1 | `bIsTitan` |
| 1487 | 0x05cf | 1 | `bIsTesla` |
| 1488 | 0x05d0 | 1 | `bIsMobile` |
| 1489 | 0x05d1 | 1 | `bIsGc6Rtd3Allowed` |
| 1490 | 0x05d2 | 1 | `bIsGc8Rtd3Allowed` |
| 1491 | 0x05d3 | 1 | `bIsGcOffRtd3Allowed` |
| 1492 | 0x05d4 | 1 | `bIsGcoffLegacyAllowed` |
| 1493 | 0x05d5 | 1 | `bIsMigSupported` |
| 1494 | 0x05d6 | 2 | `RTD3GC6TotalBoardPower` |
| 1496 | 0x05d8 | 2 | `RTD3GC6PerstDelay` |
| 1504 | 0x05e0 | 8 | `bar1PdeBase` |
| 1512 | 0x05e8 | 8 | `bar2PdeBase` |
| 1520 | 0x05f0 | 1 | `bVbiosValid` |
| 1524 | 0x05f4 | 4 | `vbiosSubVendor` |
| 1528 | 0x05f8 | 4 | `vbiosSubDevice` |
| 1532 | 0x05fc | 4 | `vbiosRevision` |
| 1536 | 0x0600 | 4 | `vbiosOemRevision` |
| 1540 | 0x0604 | 1 | `bPageRetirementSupported` |
| 1541 | 0x0605 | 1 | `bSplitVasBetweenServerClientRm` |
| 1542 | 0x0606 | 1 | `bClRootportNeedsNosnoopWAR` |
| 1544 | 0x0608 | 4 | `hInternalClient` |
| 1548 | 0x060c | 4 | `hInternalDevice` |
| 1552 | 0x0610 | 4 | `hInternalSubdevice` |
| 1556 | 0x0614 | 1 | `bSelfHostedMode` |
| 1557 | 0x0615 | 1 | `bAtsSupported` |
| 1558 | 0x0616 | 1 | `bSysL2CacheCoherentMode` |
| 1559 | 0x0617 | 1 | `bIsGpuUefi` |
| 1560 | 0x0618 | 1 | `bIsEfiInit` |
| 1564 | 0x061c | 12 | `ecidInfo` |
| 1564 | 0x061c | 4 | `ecidInfo.ecidLow` |
| 1568 | 0x0620 | 4 | `ecidInfo.ecidHigh` |
| 1572 | 0x0624 | 4 | `ecidInfo.ecidExtended` |
| 1576 | 0x0628 | 16 | `fwWprLayoutOffset` |
| 1576 | 0x0628 | 8 | `fwWprLayoutOffset.nonWprHeapOffset` |
| 1584 | 0x0630 | 8 | `fwWprLayoutOffset.frtsOffset` |
| 1592 | 0x0638 | 1 | `bBusResetRequired` |

#### `NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO`: sizeof 56 (0x38), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | `base` |
| 8 | 0x0008 | 8 | `limit` |
| 16 | 0x0010 | 8 | `reserved` |
| 24 | 0x0018 | 4 | `performance` |
| 28 | 0x001c | 1 | `supportCompressed` |
| 29 | 0x001d | 1 | `supportISO` |
| 30 | 0x001e | 1 | `bProtected` |
| 31 | 0x001f | 18 | `blackList` |
| 52 | 0x0034 | 4 | `regionTag` |

### GSP_SET_SYSTEM_INFO payload

#### `GspSystemInfo`: sizeof 952 (0x3b8), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | `gpuPhysAddr` |
| 8 | 0x0008 | 8 | `gpuPhysFbAddr` |
| 16 | 0x0010 | 8 | `gpuPhysInstAddr` |
| 24 | 0x0018 | 8 | `gpuPhysIoAddr` |
| 32 | 0x0020 | 8 | `nvDomainBusDeviceFunc` |
| 40 | 0x0028 | 8 | `simAccessBufPhysAddr` |
| 48 | 0x0030 | 8 | `notifyOpSharedSurfacePhysAddr` |
| 56 | 0x0038 | 8 | `pcieAtomicsOpMask` |
| 64 | 0x0040 | 8 | `consoleMemSize` |
| 72 | 0x0048 | 8 | `maxUserVa` |
| 80 | 0x0050 | 4 | `pciConfigMirrorBase` |
| 84 | 0x0054 | 4 | `pciConfigMirrorSize` |
| 88 | 0x0058 | 4 | `PCIDeviceID` |
| 92 | 0x005c | 4 | `PCISubDeviceID` |
| 96 | 0x0060 | 4 | `PCIRevisionID` |
| 100 | 0x0064 | 4 | `pcieAtomicsCplDeviceCapMask` |
| 104 | 0x0068 | 1 | `oorArch` |
| 112 | 0x0070 | 8 | `clPdbProperties` |
| 120 | 0x0078 | 4 | `Chipset` |
| 124 | 0x007c | 1 | `bGpuBehindBridge` |
| 125 | 0x007d | 1 | `bFlrSupported` |
| 126 | 0x007e | 1 | `b64bBar0Supported` |
| 127 | 0x007f | 1 | `bMnocAvailable` |
| 128 | 0x0080 | 4 | `chipsetL1ssEnable` |
| 132 | 0x0084 | 1 | `bUpstreamL0sUnsupported` |
| 133 | 0x0085 | 1 | `bUpstreamL1Unsupported` |
| 134 | 0x0086 | 1 | `bUpstreamL1PorSupported` |
| 135 | 0x0087 | 1 | `bUpstreamL1PorMobileOnly` |
| 136 | 0x0088 | 1 | `bSystemHasMux` |
| 137 | 0x0089 | 1 | `upstreamAddressValid` |
| 138 | 0x008a | 10 | `FHBBusInfo` |
| 138 | 0x008a | 2 | `FHBBusInfo.deviceID` |
| 140 | 0x008c | 2 | `FHBBusInfo.vendorID` |
| 142 | 0x008e | 2 | `FHBBusInfo.subdeviceID` |
| 144 | 0x0090 | 2 | `FHBBusInfo.subvendorID` |
| 146 | 0x0092 | 1 | `FHBBusInfo.revisionID` |
| 148 | 0x0094 | 10 | `chipsetIDInfo` |
| 148 | 0x0094 | 2 | `chipsetIDInfo.deviceID` |
| 150 | 0x0096 | 2 | `chipsetIDInfo.vendorID` |
| 152 | 0x0098 | 2 | `chipsetIDInfo.subdeviceID` |
| 154 | 0x009a | 2 | `chipsetIDInfo.subvendorID` |
| 156 | 0x009c | 1 | `chipsetIDInfo.revisionID` |
| 160 | 0x00a0 | 676 | `acpiMethodData` |
| 160 | 0x00a0 | 1 | `acpiMethodData.bValid` |
| 164 | 0x00a4 | 72 | `acpiMethodData.dodMethodData` |
| 164 | 0x00a4 | 4 | `acpiMethodData.dodMethodData.status` |
| 168 | 0x00a8 | 4 | `acpiMethodData.dodMethodData.acpiIdListLen` |
| 172 | 0x00ac | 64 | `acpiMethodData.dodMethodData.acpiIdList` |
| 236 | 0x00ec | 12 | `acpiMethodData.jtMethodData` |
| 236 | 0x00ec | 4 | `acpiMethodData.jtMethodData.status` |
| 240 | 0x00f0 | 4 | `acpiMethodData.jtMethodData.jtCaps` |
| 244 | 0x00f4 | 2 | `acpiMethodData.jtMethodData.jtRevId` |
| 246 | 0x00f6 | 1 | `acpiMethodData.jtMethodData.bSBIOSCaps` |
| 248 | 0x00f8 | 580 | `acpiMethodData.muxMethodData` |
| 248 | 0x00f8 | 4 | `acpiMethodData.muxMethodData.tableLen` |
| 252 | 0x00fc | 192 | `acpiMethodData.muxMethodData.acpiIdMuxModeTable` |
| 444 | 0x01bc | 192 | `acpiMethodData.muxMethodData.acpiIdMuxPartTable` |
| 636 | 0x027c | 192 | `acpiMethodData.muxMethodData.acpiIdMuxStateTable` |
| 828 | 0x033c | 8 | `acpiMethodData.capsMethodData` |
| 828 | 0x033c | 4 | `acpiMethodData.capsMethodData.status` |
| 832 | 0x0340 | 4 | `acpiMethodData.capsMethodData.optimusCaps` |
| 836 | 0x0344 | 4 | `hypervisorType` |
| 840 | 0x0348 | 2 | `virtualConfigBits` |
| 842 | 0x034a | 1 | `bIsPassthru` |
| 848 | 0x0350 | 8 | `sysTimerOffsetNs` |
| 856 | 0x0358 | 40 | `gspVFInfo` |
| 856 | 0x0358 | 4 | `gspVFInfo.totalVFs` |
| 860 | 0x035c | 4 | `gspVFInfo.firstVFOffset` |
| 864 | 0x0360 | 8 | `gspVFInfo.FirstVFBar0Address` |
| 872 | 0x0368 | 8 | `gspVFInfo.FirstVFBar1Address` |
| 880 | 0x0370 | 8 | `gspVFInfo.FirstVFBar2Address` |
| 888 | 0x0378 | 1 | `gspVFInfo.b64bitBar0` |
| 889 | 0x0379 | 1 | `gspVFInfo.b64bitBar1` |
| 890 | 0x037a | 1 | `gspVFInfo.b64bitBar2` |
| 896 | 0x0380 | 1 | `bIsPrimary` |
| 897 | 0x0381 | 1 | `bIsUnixHdmiFrlComplianceEnabled` |
| 898 | 0x0382 | 1 | `isGridBuild` |
| 900 | 0x0384 | 4 | `pcieConfigReg` |
| 900 | 0x0384 | 4 | `pcieConfigReg.linkCap` |
| 904 | 0x0388 | 4 | `gridBuildCsp` |
| 908 | 0x038c | 1 | `bPreserveVideoMemoryAllocations` |
| 909 | 0x038d | 1 | `bTdrEventSupported` |
| 910 | 0x038e | 1 | `bFeatureStretchVblankCapable` |
| 911 | 0x038f | 1 | `bEnableDynamicGranularityPageArrays` |
| 912 | 0x0390 | 1 | `bClockBoostSupported` |
| 920 | 0x0398 | 8 | `hostPageSize` |
| 928 | 0x03a0 | 1 | `bIsCmcBasedHws` |
| 929 | 0x03a1 | 1 | `bGspNocatEnabled` |
| 930 | 0x03a2 | 1 | `bS0ixSupport` |
| 931 | 0x03a3 | 1 | `bWindowChannelAlwaysMapped` |
| 932 | 0x03a4 | 4 | `pciePowerControlValue` |
| 936 | 0x03a8 | 1 | `bPciePowerControlPresent` |
| 940 | 0x03ac | 4 | `pf0DeviceControl2Reg` |
| 944 | 0x03b0 | 1 | `bIsCxlDevice` |

#### `MUX_METHOD_DATA_ELEMENT`: sizeof 12 (0xc), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `acpiId` |
| 4 | 0x0004 | 4 | `mode` |
| 8 | 0x0008 | 4 | `status` |

### Boot arguments (RMARGS region, libos init args, WPR meta)

#### `GSP_ARGUMENTS_CACHED`: sizeof 152 (0x98), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 64 | `messageQueueInitArguments` |
| 0 | 0x0000 | 8 | `messageQueueInitArguments.sharedMemPhysAddr` |
| 8 | 0x0008 | 4 | `messageQueueInitArguments.pageTableEntryCount` |
| 16 | 0x0010 | 8 | `messageQueueInitArguments.cmdQueueOffset` |
| 24 | 0x0018 | 8 | `messageQueueInitArguments.statQueueOffset` |
| 32 | 0x0020 | 8 | `messageQueueInitArguments.queueElementHdrSize` |
| 40 | 0x0028 | 8 | `messageQueueInitArguments.queueElementSizeMin` |
| 48 | 0x0030 | 8 | `messageQueueInitArguments.queueElementSizeMax` |
| 56 | 0x0038 | 4 | `messageQueueInitArguments.queueHeaderAlign` |
| 60 | 0x003c | 4 | `messageQueueInitArguments.queueElementAlign` |
| 64 | 0x0040 | 12 | `srInitArguments` |
| 64 | 0x0040 | 4 | `srInitArguments.oldLevel` |
| 68 | 0x0044 | 4 | `srInitArguments.flags` |
| 72 | 0x0048 | 1 | `srInitArguments.bInPMTransition` |
| 76 | 0x004c | 4 | `gpuInstance` |
| 80 | 0x0050 | 1 | `bDmemStack` |
| 88 | 0x0058 | 16 | `profilerArgs` |
| 88 | 0x0058 | 8 | `profilerArgs.pa` |
| 96 | 0x0060 | 8 | `profilerArgs.size` |
| 104 | 0x0068 | 16 | `sysmemHeapArgs` |
| 104 | 0x0068 | 8 | `sysmemHeapArgs.pa` |
| 112 | 0x0070 | 8 | `sysmemHeapArgs.size` |
| 120 | 0x0078 | 16 | `rmStateMonitorBufferArgs` |
| 120 | 0x0078 | 8 | `rmStateMonitorBufferArgs.pa` |
| 128 | 0x0080 | 8 | `rmStateMonitorBufferArgs.size` |
| 136 | 0x0088 | 16 | `bindataArgs` |
| 136 | 0x0088 | 8 | `bindataArgs.radix3` |
| 144 | 0x0090 | 8 | `bindataArgs.size` |

#### `MESSAGE_QUEUE_INIT_ARGUMENTS`: sizeof 64 (0x40), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | `sharedMemPhysAddr` |
| 8 | 0x0008 | 4 | `pageTableEntryCount` |
| 16 | 0x0010 | 8 | `cmdQueueOffset` |
| 24 | 0x0018 | 8 | `statQueueOffset` |
| 32 | 0x0020 | 8 | `queueElementHdrSize` |
| 40 | 0x0028 | 8 | `queueElementSizeMin` |
| 48 | 0x0030 | 8 | `queueElementSizeMax` |
| 56 | 0x0038 | 4 | `queueHeaderAlign` |
| 60 | 0x003c | 4 | `queueElementAlign` |

#### `GSP_SR_INIT_ARGUMENTS`: sizeof 12 (0xc), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `oldLevel` |
| 4 | 0x0004 | 4 | `flags` |
| 8 | 0x0008 | 1 | `bInPMTransition` |

#### `LibosMemoryRegionInitArgument`: sizeof 32 (0x20), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | `id8` |
| 8 | 0x0008 | 8 | `pa` |
| 16 | 0x0010 | 8 | `size` |
| 24 | 0x0018 | 1 | `kind` |
| 25 | 0x0019 | 1 | `loc` |

#### `GspFwWprMeta`: sizeof 256 (0x100), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | `magic` |
| 8 | 0x0008 | 8 | `revision` |
| 16 | 0x0010 | 8 | `sysmemAddrOfRadix3Elf` |
| 24 | 0x0018 | 8 | `sizeOfRadix3Elf` |
| 32 | 0x0020 | 8 | `sysmemAddrOfBootloader` |
| 40 | 0x0028 | 8 | `sizeOfBootloader` |
| 48 | 0x0030 | 8 | `bootloaderCodeOffset` |
| 56 | 0x0038 | 8 | `bootloaderDataOffset` |
| 64 | 0x0040 | 8 | `bootloaderManifestOffset` |
| 72 | 0x0048 | 8 | `sysmemAddrOfSignature` |
| 80 | 0x0050 | 8 | `sizeOfSignature` |
| 72 | 0x0048 | 4 | `gspFwHeapFreeListWprOffset` |
| 76 | 0x004c | 4 | `unused0` |
| 80 | 0x0050 | 8 | `unused1` |
| 88 | 0x0058 | 8 | `gspFwRsvdStart` |
| 96 | 0x0060 | 8 | `nonWprHeapOffset` |
| 104 | 0x0068 | 8 | `nonWprHeapSize` |
| 112 | 0x0070 | 8 | `gspFwWprStart` |
| 120 | 0x0078 | 8 | `gspFwHeapOffset` |
| 128 | 0x0080 | 8 | `gspFwHeapSize` |
| 136 | 0x0088 | 8 | `gspFwOffset` |
| 144 | 0x0090 | 8 | `bootBinOffset` |
| 152 | 0x0098 | 8 | `frtsOffset` |
| 160 | 0x00a0 | 8 | `frtsSize` |
| 168 | 0x00a8 | 8 | `gspFwWprEnd` |
| 176 | 0x00b0 | 8 | `fbSize` |
| 184 | 0x00b8 | 8 | `vgaWorkspaceOffset` |
| 192 | 0x00c0 | 8 | `vgaWorkspaceSize` |
| 200 | 0x00c8 | 8 | `bootCount` |
| 208 | 0x00d0 | 8 | `partitionRpcAddr` |
| 216 | 0x00d8 | 2 | `partitionRpcRequestOffset` |
| 218 | 0x00da | 2 | `partitionRpcReplyOffset` |
| 220 | 0x00dc | 4 | `elfCodeOffset` |
| 224 | 0x00e0 | 4 | `elfDataOffset` |
| 228 | 0x00e4 | 4 | `elfCodeSize` |
| 232 | 0x00e8 | 4 | `elfDataSize` |
| 236 | 0x00ec | 4 | `lsUcodeVersion` |
| 208 | 0x00d0 | 16 | `partitionRpcPadding` |
| 224 | 0x00e0 | 8 | `sysmemAddrOfCrashReportQueue` |
| 232 | 0x00e8 | 4 | `sizeOfCrashReportQueue` |
| 236 | 0x00ec | 4 | `lsUcodeVersionPadding` |
| 240 | 0x00f0 | 1 | `gspFwHeapVfPartitionCount` |
| 241 | 0x00f1 | 1 | `flags` |
| 242 | 0x00f2 | 2 | `padding` |
| 244 | 0x00f4 | 4 | `pmuReservedSize` |
| 248 | 0x00f8 | 8 | `verified` |

### GSP-FMC boot parameters (Hopper and later only, not used on Turing)

#### `GSP_FMC_INIT_PARAMS`: sizeof 4 (0x4), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `regkeys` |

#### `GSP_FMC_BOOT_PARAMS`: sizeof 88 (0x58), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `initParams` |
| 0 | 0x0000 | 4 | `initParams.regkeys` |
| 8 | 0x0008 | 32 | `bootGspRmParams` |
| 8 | 0x0008 | 4 | `bootGspRmParams.target` |
| 12 | 0x000c | 4 | `bootGspRmParams.gspRmDescSize` |
| 16 | 0x0010 | 8 | `bootGspRmParams.gspRmDescOffset` |
| 24 | 0x0018 | 8 | `bootGspRmParams.wprCarveoutOffset` |
| 32 | 0x0020 | 4 | `bootGspRmParams.wprCarveoutSize` |
| 36 | 0x0024 | 1 | `bootGspRmParams.bIsGspRmBoot` |
| 37 | 0x0025 | 1 | `bootGspRmParams.bInstInSysMode` |
| 40 | 0x0028 | 16 | `gspRmParams` |
| 40 | 0x0028 | 4 | `gspRmParams.target` |
| 48 | 0x0030 | 8 | `gspRmParams.bootArgsOffset` |
| 56 | 0x0038 | 24 | `gspSpdmParams` |
| 56 | 0x0038 | 4 | `gspSpdmParams.target` |
| 64 | 0x0040 | 8 | `gspSpdmParams.payloadBufferOffset` |
| 72 | 0x0048 | 4 | `gspSpdmParams.payloadBufferSize` |
| 80 | 0x0050 | 8 | `gspRmMemParams` |
| 80 | 0x0050 | 4 | `gspRmMemParams.flushSysmemAddrValLo` |
| 84 | 0x0054 | 4 | `gspRmMemParams.flushSysmemAddrValHi` |

### LIBOS log buffer header (no struct; NvU64 words)

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | put pointer, in NvU64 entries (liblogdecode.h:156, liblogdecode.c:2543) |
| 8 | 0x0008 | 8 | buffer PA, written by the CPU before boot (kernel_gsp.c:3649); also ring slot 0 |
| 8 | 0x0008 | ... | ring of `size/8 - 1` NvU64 slots starting at word 1 (liblogdecode.c:1231,1279) |

### Derived constants (computed by the compiler from the same headers)

| name | decimal | hex |
|---|---:|---:|
| `queueElementHdrSize (no CC)` | 16 | 0x10 |
| `queueElementHdrSize (CC)` | 56 | 0x38 |
| `queueElementSizeMin` | 4096 | 0x1000 |
| `queueElementSizeMax` | 65536 | 0x10000 |
| `maxRpcSize = SizeMax - HdrSize` | 65520 | 0xfff0 |
| `max payload after rpc_message_header_v` | 65488 | 0xffd0 |
| `hdr + GspStaticConfigInfo (must be <= maxRpcSize)` | 1632 | 0x660 |
| `hdr + GspSystemInfo (must be <= maxRpcSize)` | 984 | 0x3d8 |
| `msgq rxHdrOff (hdrAlign 4)` | 32 | 0x20 |
| `msgq entryOff (entryAlign 12)` | 4096 | 0x1000 |
| `msgq msgCount for a 0x40000 queue` | 63 | 0x3f |
| `mctpHeader = mctpCreateTransportHeader(1,1,0,0,0)` | 3221225473 | 0xc0000001 |
| `nvdmHeader = mctpCreateNvdmHeader(NVDM_TYPE_RM_RPC)` | 621862526 | 0x2510de7e |
| `MCTP_MSG_HEADER_VENDOR_ID_NV` | 4318 | 0x10de |
| `MCTP_MSG_HEADER_TYPE_VENDOR_PCI` | 126 | 0x7e |
| `NVDM_TYPE_RM_RPC` | 37 | 0x25 |
| `MSGQ_VERSION` | 0 | 0x0 |
| `MSGQ_FLAGS_SWAP_RX` | 1 | 0x1 |
| `MSGQ_MSG_SIZE_MIN` | 16 | 0x10 |
| `header_version (rpc_common.c:155-156)` | 50331648 | 0x3000000 |
| `NV_VGPU_MSG_SIGNATURE_VALID` | 1129337430 | 0x43505256 |
| `NV_VGPU_MSG_RESULT_SUCCESS` | 0 | 0x0 |
| `NV_VGPU_MSG_RESULT_RPC_PENDING` | 4294967295 | 0xffffffff |
| `NV_VGPU_MSG_UNION_INIT` | 0 | 0x0 |
| `DRF_BASE(NV_VGPU_MSG_RESULT__VMIOP)` | 4278190080 | 0xff000000 |
| `VGX_MAJOR_VERSION_NUMBER` | 46 | 0x2e |
| `VGX_MINOR_VERSION_NUMBER` | 13 | 0xd |
| `NV_VGPU_MSG_FUNCTION_FREE` | 10 | 0xa |
| `NV_VGPU_MSG_FUNCTION_SET_GUEST_SYSTEM_INFO` | 1 | 0x1 |
| `NV_VGPU_MSG_FUNCTION_DUP_OBJECT` | 21 | 0x15 |
| `NV_VGPU_MSG_FUNCTION_SET_GUEST_SYSTEM_INFO_EXT` | 64 | 0x40 |
| `NV_VGPU_MSG_FUNCTION_GET_GSP_STATIC_INFO` | 65 | 0x41 |
| `NV_VGPU_MSG_FUNCTION_GSP_SET_SYSTEM_INFO` | 72 | 0x48 |
| `NV_VGPU_MSG_FUNCTION_SET_REGISTRY` | 73 | 0x49 |
| `NV_VGPU_MSG_FUNCTION_GSP_RM_CONTROL` | 76 | 0x4c |
| `NV_VGPU_MSG_FUNCTION_GSP_RM_ALLOC` | 103 | 0x67 |
| `NV_VGPU_MSG_EVENT_GSP_INIT_DONE` | 4097 | 0x1001 |
| `NV_VGPU_MSG_EVENT_OS_ERROR_LOG` | 4102 | 0x1006 |
| `NV_VGPU_MSG_EVENT_POST_EVENT` | 4099 | 0x1003 |
| `NV_VGPU_MSG_EVENT_UCODE_LIBOS_PRINT` | 4108 | 0x100c |
| `NV_VGPU_MSG_EVENT_GSP_LOCKDOWN_NOTICE` | 4124 | 0x101c |
| `GSP_FW_WPR_META_MAGIC` | 15869187694674993331 | 0xdc3aae21371a60b3 |
| `GSP_FW_WPR_META_REVISION` | 1 | 0x1 |
| `GSP_FW_WPR_META_VERIFIED` | 11574427654092267680 | 0xa0a0a0a0a0a0a0a0 |
| `LIBOS_MEMORY_REGION_INIT_ARGUMENTS_MAX` | 4096 | 0x1000 |
| `LIBOS_MEMORY_REGION_CONTIGUOUS` | 1 | 0x1 |
| `LIBOS_MEMORY_REGION_LOC_SYSMEM` | 1 | 0x1 |
| `id8("LOGINIT")` | 21479265821149524 | 0x4c4f47494e4954 |
| `id8("LOGRM")` | 327747588685 | 0x4c4f47524d |
| `id8("RMARGS")` | 90491761870675 | 0x524d41524753 |

## (b) `GspStaticConfigInfo`: what the kernel RM reads

### Where the reply goes

`kgspInitRm_IMPL` sends SET_GUEST_SYSTEM_INFO and then GET_GSP_STATIC_INFO (kernel_gsp.c:5153, :5160). `rpcGetGspStaticInfo_v14_00` handles the second call (rpc.c:9669-9707):

1. It writes a header whose length is 32 + 1600 (rpc.c:9693-9695).
2. It waits synchronously (rpc.c:9699-9700).
3. It copies `sizeof(GspStaticConfigInfo)` = 1600 bytes from `rpc_message_data` of the reply into `pKernelGsp->gspStaticInfo` (rpc.c:9703).

The copy happens whatever `length` the reply declares. That target is the object that `GPU_GET_GSP_STATIC_INFO` returns (gpu.c:6332-6339; generated/g_kernel_gsp_nvoc.h:583). It was zeroed before boot (kernel_gsp.c:4362-4364).

Right after the copy, `_kgspInitGpuProperties` (kernel_gsp.c:5170) and `_kgspSetFwWprLayoutOffset` (kernel_gsp.c:5173) consume some fields. Everything else is read later, through the `*_FWCLIENT`/`*_GSPCLIENT` HAL variants. For TU10x the HAL tables select those variants. For example, generated/g_gpu_nvoc.c picks `_FWCLIENT` for `gpuInitSriov` (:1844), `gpuInitBranding` (:931), `gpuGetNameString` (:896), `gpuGenGidData` (:1434) and `gpuGetRtd3GC6Data` (:949). generated/g_kern_bus_nvoc.c picks `_GSPCLIENT` for `kbusPatchBar1Pdb` and `kbusPatchBar2Pdb` (:636, :646).

To find every consumer, I grepped `GPU_GET_GSP_STATIC_INFO`, `GspStaticConfigInfo` and `gspStaticInfo` over `src/nvidia`. The hits are in gpu_gspclient.c, gpu.c, subdevice_ctrl_gpu_kernel.c, kernel_gsp.c, mem_mgr_gsp_client.c, kern_gpu_gh100.c, kern_bus.c, kern_mem_sys.c, gpu_boost_mgr.c and chipset_pcie.c. Each file was then read at the hit.

**Classes:**

- **identity**: describes which card this is (name, FB size, GID, SKU, PCI).
- **structural**: shapes the driver's object model (engines, handles, PDE bases, BAR sizes, platform and power flags).
- **unread**: nothing reads it on a TU106 GSP-client boot.

**When:**

- **init**: read unconditionally while the GPU is brought up.
- **demand**: read only when a client, the error path or a feature asks.

| field | offset | class | consumer (file:line) | when | what the device must put there |
|---|---:|---|---|---|---|
| `gidInfo.data[0..15]` | 0x000c | identity (GID) | gpu_gspclient.c:182-188 in `gpuGenGidData_FWCLIENT`, called from `gpuGetGidInfo` (gpu.c:5356) | demand, on the first UUID query | The card's 16-byte SHA-1 GID (`RM_SHA1_GID_SIZE` = 16, gpu_uuid.h:34). If it is all zero, the call fails with `NV_ERR_INVALID_STATE` (:182-186). `index`, `flags` and `length` (0x0, 0x4, 0x8) are never read. |
| `SKUInfo` (all 48 bytes) | 0x010c | identity (SKU) | subdevice_ctrl_gpu_kernel.c:3747-3750 (BIOS_GET_SKU_INFO control); `project` and `projectSKU` at gpu_boost_mgr.c:714-715, :744-745 | demand | The real card's values. No checks. |
| `fbRegionInfoParams` | 0x0140 | structural, also carries FB size | mem_mgr_gsp_client.c:59-120, from `memmgrInitBaseFbRegions_HAL` (mem_mgr.c:3423) | init | `numFBRegions` must be 1..16: 0 fails (:60-66) and more than `MAX_FB_REGIONS` = 16 fails (:68-75; generated/g_mem_mgr_nvoc.h:432). Per region the code reads `base`, `limit` (inclusive), `reserved`, `performance`, `supportCompressed`, `supportISO`, `bProtected` and `regionTag` (:84-92); `blackList` is not read. A region with a non-zero `reserved` counts as reserved, every other region as usable (:96-106). The last region's `limit` sets `fbAddrSpaceSizeMb` (:119-120), which should be at least `fbTotalMemSizeMb` (`NV_ASSERT` only, :122). |
| `bPdiValid`, `pdi` | 0x04c8, 0x04d0 | identity | gpu_gspclient.c:153-155 (`gpuGetPdi` maps to `_FWCLIENT`, generated/g_gpu_nvoc.h:3061); on Turing the only caller is the CPER record (gpu.c:2585) | demand (error report) | The card's PDI with `bPdiValid` = 1. With 0 on silicon the call returns an error and the caller ignores it. |
| `sriovCaps.bar0Size`, `.bar1Size`, `.bar2Size` | 0x0500, 0x0508, 0x0510 | structural (BAR sizes) | gpu_gspclient.c:65-67, from `gpuInitSriov_HAL` (gpu.c:2694) | init | VF BAR sizes; 0 on a card without SR-IOV. No other `sriovCaps` member is read. |
| `sriovMaxGfid` | 0x0528 | structural | gpu_gspclient.c:69, overwritten at :92 with the CPU's own `totalVFs` | init | Ignored in effect; 0. |
| `engineCaps[3]` | 0x052c | structural (engine caps) | gpu.c:6840-6843 in `gpuCheckEngineWithOrderList_KERNEL`, called for every engine while the engine list is built (gpu.c:1832, :2100) | init | `NV2080_ENGINE_TYPE` bitmask, converted by `gpuGetRmEngineTypeCapMask` (gpu.c:6846-6850). A missing engine is removed from the class database, so this must list every TU106 engine the guest should see. |
| `poisonFuseEnabled` | 0x0538 | structural | gpu_gspclient.c:199, via `gpuIsAmpereErrorContainmentXidEnabled_KERNEL` (gpu.c:6722-6733) | demand | 0 |
| `fb_length` | 0x0540 | identity (FB size) | mem_mgr_gsp_client.c:118 sets `fbTotalMemSizeMb` = `fb_length` rounded up to MiB | init | FB size in bytes. Should not exceed the last region's `limit` + 1 (:122). |
| `gpuNameString` | 0x0548 | identity (name) | gpu_gspclient.c:296-297; read by the NAME_STRING control (subdevice_ctrl_gpu_kernel.c:3628), CPER (gpu.c:2550) and nvlink (kernel_nvlinkcorelib.c:307) | demand | ASCII, NUL-terminated, 64 bytes (`NV2080_GPU_MAX_NAME_STRING_LENGTH`, ctrl2080gpu.h:327). |
| `gpuShortNameString` | 0x0588 | identity (name) | gpu_gspclient.c:311-312; SHORT_NAME_STRING control (subdevice_ctrl_gpu_kernel.c:3648) | demand | As above. |
| `bGpuInternalSku` | 0x05c8 | unread | `gpuIsInternalSku_FWCLIENT` (gpu_gspclient.c:46-54) has no caller under `src/` | none | 0 |
| `bIsQuadroGeneric`, `bIsQuadroAd`, `bIsNvidiaNvs`, `bIsVgx`, `bGeforceSmb`, `bIsTitan`, `bIsTesla` | 0x05c9 to 0x05cf | identity (branding) | gpu_gspclient.c:132-139, from `gpuStatePreInit` (gpu.c:2162) | init | GeForce: all 0, which gives `bIsGeforce` = 1 (:139). |
| `bIsMobile`, `bIsGc6Rtd3Allowed`, `bIsGc8Rtd3Allowed`, `bIsGcOffRtd3Allowed`, `bIsGcoffLegacyAllowed` | 0x05d0 to 0x05d4 | structural (platform/power) | kernel_gsp.c:6615-6618, :6621 | init | Desktop card: 0. |
| `bIsMigSupported` | 0x05d5 | unread on Turing | gpu.c:7715, reached only if `PDB_PROP_GPU_MIG_SUPPORTED` (gpu.c:2188-2190), which is false for TU10x (generated/g_gpu_nvoc.c:271-281) | none | 0 |
| `RTD3GC6TotalBoardPower`, `RTD3GC6PerstDelay` | 0x05d6, 0x05d8 | structural (power) | gpu_gspclient.c:208-209, from gpu.c:2165 | init | 0 |
| `bar1PdeBase` | 0x05e0 | structural (PDE base) | kern_bus.c:794, in `kbusPatchBar1Pdb_GSPCLIENT` called from kern_bus_gm107.c:1192 | init | FB offset of the GSP-owned BAR1 root page directory. The CPU describes it as an `ADDR_FBMEM` memdesc, swaps it into its BAR1 walker (:805-812) and invalidates TLBs against it (:823). |
| `bar2PdeBase` | 0x05e8 | structural (PDE base) | kern_bus.c:867, in `kbusPatchBar2Pdb_GSPCLIENT` called from kern_bus_gm107.c:1925 | init | FB offset of the GSP-owned BAR2 root page directory. The CPU then reads its own PDE3[0] through the BAR0 window and sends it in an UPDATE_BAR_PDE RPC (kern_bus.c:891-899). Both PDE bases must lie in FB that the device backs. |
| `bVbiosValid`, `vbiosSubVendor`, `vbiosSubDevice` | 0x05f0, 0x05f4, 0x05f8 | identity (PCI subsystem) | gpu_boost_mgr.c:708, :716-717 (and :738, :746-747) | demand (sync-boost groups) | `bVbiosValid` = 1 and the PCI subsystem vendor and device. |
| `vbiosRevision`, `vbiosOemRevision` | 0x05fc, 0x0600 | identity | gpu.c:2557-2569 (CPER firmware version) | demand | The VBIOS version packed as 0xAABBCCDD, plus the OEM byte. |
| `bPageRetirementSupported` | 0x0604 | structural | gpu_gspclient.c:125, from heap.c:1384-1385 and :2284-2285, only under `PDB_PROP_GPU_ALLOW_PAGE_RETIREMENT` | demand (allocation) | 0 on GeForce. |
| `bSplitVasBetweenServerClientRm` | 0x0605 | unread | only the vGPU copy is read (gpu_vgpu.c:177) | none | 0 |
| `bClRootportNeedsNosnoopWAR` | 0x0606 | structural | chipset_pcie.c:4956, via chipset_pcie.c:561 | init | 0 |
| `hInternalClient`, `hInternalDevice`, `hInternalSubdevice` | 0x0608, 0x060c, 0x0610 | structural (handles) | gpu.c:989-991 in `_gpuAllocateInternalObjects` (gpu.c:2181) | init | Handles of the device's own internal RM client, device and subdevice. The CPU uses them as `hClient`/`hObject` in every later internal control, for example gpu_gspclient.c:241-246, which arrive as GSP_RM_CONTROL. Non-zero and recognised by the device. |
| `bSelfHostedMode` | 0x0614 | unread on Turing | read only by kern_gpu_gh100.c:428; the TU10x HAL is a no-op (generated/g_gpu_nvoc.c:2286-2296) | none | 0 |
| `bAtsSupported` | 0x0615 | unread on Turing | kern_mem_sys.c:191, only on self-hosted C2C systems (:175-187) | none | 0 |
| `bSysL2CacheCoherentMode` | 0x0616 | structural | subdevice_ctrl_gpu_kernel.c:593 (GPU_INFO coherent-caching query); kern_mem_sys.c:198 only when self-hosted | demand | 0 |
| `bIsGpuUefi`, `bIsEfiInit` | 0x0617, 0x0618 | structural (platform) | kernel_gsp.c:6619-6620 | init | 0 is safe. Real values describe the VBIOS or GOP state. |
| `ecidInfo` | 0x061c | identity (chip ECID) | subdevice_ctrl_gpu_kernel.c:136, :156, :176 (GPU_INFO `ECID_LO32`/`HI32`/`EXTENDED`) | demand | The card's ECID, or 0. |
| `fwWprLayoutOffset` | 0x0628 | unread on Turing | kernel_gsp.c:4726-4727, only if `PDB_PROP_GPU_FW_WPR_OFFSET_SET_BY_ACR` (:4717), which is false for TU10x (generated/g_gpu_nvoc.c:365-375) | none | 0 |
| `bBusResetRequired` | 0x0638 | structural | gpu_gspclient.c:385, from `gpuStatePostLoad` (gpu.c:3773) | init | Must be 0. Otherwise the GPU is flagged for a bus reset (gpu.c:3775). |

### Counts

- `GspStaticConfigInfo` has **49 top-level members**.
- **47 of them have a reader somewhere** on the GSP-client path. The two without one are `bGpuInternalSku` and `bSplitVasBetweenServerClientRm`.
- **On a TU106 boot, 43 are read.** Four more are unread on this chip for HAL or property reasons: `bIsMigSupported`, `bSelfHostedMode`, `bAtsSupported` and `fwWprLayoutOffset`.
- **20 of the 43 are identity**:
  - `gidInfo` and `SKUInfo`;
  - `bPdiValid` and `pdi`;
  - `fb_length`;
  - `gpuNameString` and `gpuShortNameString`;
  - the seven branding flags;
  - `bVbiosValid`, `vbiosSubVendor`, `vbiosSubDevice`, `vbiosRevision` and `vbiosOemRevision`;
  - `ecidInfo`.
- **The other 23 are structural.**

Five structural fields decide whether the driver comes up at all: `fbRegionInfoParams`, `engineCaps`, the three internal handles, the two PDE bases and `bBusResetRequired`. The flags and power values can all be 0 for a desktop GeForce. None of the identity fields is checked against anything, apart from `gidInfo.data` having to be non-zero.

## (c) What the CPU sends: `GspSystemInfo` and `GSP_ARGUMENTS_CACHED`

Both are built before the GSP is started. The device can read each value back to learn how the guest sees it.

### `GspSystemInfo`, the payload of GSP_SET_SYSTEM_INFO (0x48)

`_kgspPrepareSystemInfo` fills it (kernel_gsp.c:4470-4634), after zeroing the whole struct (:4488). `kgspInitRm_IMPL` calls it (kernel_gsp.c:5062). `kgspSendInitRpcs` sends it as an **async** RPC, right after Booter Load and before the CPU links the status queue (kernel_gsp_tu102.c:574-584; kernel_gsp.c:4686-4709). `rpcGspSetSystemInfo_v17_00` copies the struct verbatim to `rpc_message_data` (rpc.c:10432-10469, copy at :10464, `_issueRpcAsync` at :10466). Its header length is 32 + 952 (rpc.c:10458-10460).

| field | offset | filled at (kernel_gsp.c) | source, and what the device learns |
|---|---:|---|---|
| `gpuPhysAddr` | 0x000 | 4490 | `busInfo.gpuPhysAddr` = the BAR0 CPU physical address (gpu_mgr.c:1974; osinit.c:854) |
| `gpuPhysFbAddr` | 0x008 | 4491 | BAR1 CPU physical address (gpu_mgr.c:1975; osinit.c:852, :913) |
| `gpuPhysInstAddr` | 0x010 | 4492 | BAR2/3 (IMEM) CPU physical address (gpu_mgr.c:1976; osinit.c:857). Without one it is BAR0 + 0x700000 (gpu_mgr.c:1995-2002). |
| `gpuPhysIoAddr` | 0x018 | 4493 | gpu_mgr.c:1977 |
| `nvDomainBusDeviceFunc` | 0x020 | 4494 | `nv_encode_pci_info` of the guest's view of the device (osinit.c:881; gpu_mgr.c:1979) |
| `simAccessBufPhysAddr` | 0x028 | 4534-4541 | 0 on silicon |
| `notifyOpSharedSurfacePhysAddr` | 0x030 | 4543-4550 | PA of the notify-op surface if one exists, else 0 |
| `pcieAtomicsOpMask` | 0x038 | 4551-4552 | from KernelBif |
| `consoleMemSize` | 0x040 | 4555 | `Ram.ReservedConsoleDispMemSize` |
| `maxUserVa` | 0x048 | 4556 | `osGetMaxUserVa()` |
| `pciConfigMirrorBase`, `pciConfigMirrorSize` | 0x050, 0x054 | 4505-4507 | `kbifGetPciConfigSpacePriMirror_HAL` (config-space mirror in BAR0) |
| `PCIDeviceID`, `PCISubDeviceID`, `PCIRevisionID` | 0x058, 0x05c, 0x060 | 4498-4500 | `pGpu->idInfo`. Device ID in bits 31:16 and vendor in 15:0, which is how gpu.c:2587-2590 decodes them. |
| `pcieAtomicsCplDeviceCapMask` | 0x064 | 4553-4554 | KernelBif |
| `oorArch` | 0x068 | 4495 | `OOR_ARCH_X86_64` etc. (gpu_mgr.c:1547-1557) |
| `clPdbProperties` | 0x070 | 4562 via chipset.c:730-776 | bit n = the n-th `PDB_PROP_CL_*` in the listed order |
| `Chipset`, `FHBBusInfo`, `chipsetIDInfo` | 0x078, 0x08a, 0x094 | 4562 via chipset.c:781-783 | host-bridge identity |
| `bGpuBehindBridge`, `bUpstreamL0sUnsupported`, `bUpstreamL1Unsupported`, `bUpstreamL1PorSupported`, `bUpstreamL1PorMobileOnly` | 0x07c, 0x084 to 0x087 | 4572-4576 | GPU PDB properties (ASPM) |
| `bFlrSupported`, `b64bBar0Supported` | 0x07d, 0x07e | 4527-4528 | KernelBif properties |
| `bMnocAvailable`, `chipsetL1ssEnable` | 0x07f, 0x080 | 4514, 4517 | KernelBif |
| `bSystemHasMux` | 0x088 | 4569 | |
| `upstreamAddressValid` | 0x089 | 4577 | |
| `acpiMethodData` | 0x0a0 | 4568 | the cached ACPI `_DOD`/JT/MUX/CAPS results (676 bytes) |
| `hypervisorType`, `virtualConfigBits`, `bIsPassthru` | 0x344, 0x348, 0x34a | 4579-4581 | Hypervisor detection as the guest sees it |
| `sysTimerOffsetNs` | 0x350 | 4594 | `OBJTMR.sysTimerOffsetNs` |
| `gspVFInfo` | 0x358 | 4584-4591 | `pGpu->sriovState` (0 without SR-IOV) |
| `bIsPrimary`, `bIsUnixHdmiFrlComplianceEnabled` | 0x380, 0x381 | 4596, 4598-4599 | |
| `isGridBuild`, `gridBuildCsp` | 0x382, 0x388 | 4603, 4605 | |
| `pcieConfigReg.linkCap` | 0x384 | 4531 | the link-capability register as the guest read it |
| `bPreserveVideoMemoryAllocations` | 0x38c | 4606 | |
| `bTdrEventSupported` | 0x38d | 4609 | |
| `bFeatureStretchVblankCapable`, `bWindowChannelAlwaysMapped` | 0x38e, 0x3a3 | 4614-4615 | only if KernelDisplay exists |
| `bEnableDynamicGranularityPageArrays` | 0x38f | 4557 | |
| `bClockBoostSupported` | 0x390 | 4621 | only if KernelFsp exists (not on Turing) |
| `hostPageSize` | 0x398 | 4565 | `osGetPageSize()` |
| `bIsCmcBasedHws` | 0x3a0 | none | never written, so stays 0 (grep over `src/nvidia/src`) |
| `bGspNocatEnabled` | 0x3a1 | 4624-4627 | Windows only, so 0 |
| `bS0ixSupport` | 0x3a2 | 4601 | |
| `pciePowerControlValue`, `bPciePowerControlPresent` | 0x3a4, 0x3a8 | 4520-4521 | |
| `pf0DeviceControl2Reg` | 0x3ac | 4524 | |
| `bIsCxlDevice` | 0x3b0 | 4629 | |

SET_REGISTRY (0x49) follows immediately, also async (kernel_gsp.c:4701). Its payload is the packed registry table. If the table does not fit in `maxRpcSize` − 32, it is split into continuation records (rpc.c:10509-10538).

### `GSP_ARGUMENTS_CACHED`, the `RMARGS` region

**How the device finds it:**

1. The PA of the libos init-args page goes to `NV_PGSP_FALCON_MAILBOX0`/`MAILBOX1` (kernel_gsp_tu102.c:398-402).
2. That page is an array of `LibosMemoryRegionInitArgument` (4096 bytes, kernel_gsp_tu102.c:172-175).
3. The first entries are the log buffers (kernel_gsp.c:6214-6221). The next entry has `id8` = 0x524d41524753 (`"RMARGS"`, :6224-6228). Its `pa` points to the 4 KiB page that holds `GSP_ARGUMENTS_CACHED` (kernel_gsp_tu102.c:178-202).

`kgspPopulateGspRmInitArgs_IMPL` fills it (kernel_gsp.c:5467-5553), called at :5056:

| field | offset | filled at (kernel_gsp.c) | value on a normal TU106 boot |
|---|---:|---|---|
| `messageQueueInitArguments.sharedMemPhysAddr` | 0x00 | 5482 | `pPageTbl[0]`, the PA of the first page of the shared region, which is the page table itself (message_queue_cpu.c:305-310, :329) |
| `.pageTableEntryCount` | 0x08 | 5483 | 129 (derived): 0x80000 bytes of queues are 128 pages, plus 1 page for 128 × 8 bytes of PTEs (message_queue_cpu.c:125-137) |
| `.cmdQueueOffset` | 0x10 | 5484 | `pageTableSize` = 0x1000 (derived, :136) |
| `.statQueueOffset` | 0x18 | 5485 | 0x1000 + 0x40000 = 0x41000 (derived; the queue sizes are at message_queue_cpu.c:71-72, :94-118) |
| `.queueElementHdrSize` | 0x20 | 5486 | 16 (message_queue_cpu.c:82) |
| `.queueElementSizeMin`, `.queueElementSizeMax` | 0x28, 0x30 | 5487-5488 | 0x1000 and 0x10000 (message_queue_cpu.c:88-89) |
| `.queueHeaderAlign`, `.queueElementAlign` | 0x38, 0x3c | 5489-5490 | 4 and 12 (message_queue_cpu.c:90-91). Both are log2. |
| `srInitArguments` | 0x40 | 5505-5507 | all 0 on a normal boot |
| `gpuInstance` | 0x4c | 5516 | |
| `bDmemStack` | 0x50 | 5502-5503 | from the `RmGspStackPlacement` regkey, default 1 |
| `profilerArgs` | 0x58 | 5518-5525 | 0 unless profiling |
| `sysmemHeapArgs` | 0x68 | 5527-5536 | 0 (not a zero-FB chip) |
| `rmStateMonitorBufferArgs` | 0x78 | 5539-5546 | PA and size if allocated |
| `bindataArgs` | 0x88 | 5548-5552 | radix3 PA and size of external bindata, if any |

The shared region is **not** physically contiguous (`NV_MEMORY_NONCONTIGUOUS`, message_queue_cpu.c:254-255). The device must walk the page table at `sharedMemPhysAddr` to reach the command queue at page 1 and the status queue at page 0x41.

The log buffers, the `LOGINIT` / `LOGRM` / ... regions, are handled the same way. The CPU writes the buffer PA into word 1 of each buffer (kernel_gsp.c:3649) and the same PA into the init-arg entry (:6219). The device can leave the put pointer at 0. Nothing is decoded until put moves (liblogdecode.c:1246-1251, :2543).

## (d) What the CPU checks

### Linking the status queue (`GspStatusQueueInit` → `msgqRxLink`)

The device must write the `msgqTxHeader` of the status queue at `statQueueOffset` before the CPU gives up. The CPU polls for 4 s (message_queue_cpu.c:345, :369-404). `msgqRxLink` checks the following (msgq.c:380-405):

- `size` = `statusQueueSize` = 0x40000 (else -7).
- `msgSize` = 0x1000 (else -8).
- `version` = `MSGQ_VERSION` = 0 (else -9).
- `rxHdrOff` ≥ 32, and `entryOff` ≥ the CPU's own `rxHdrOff` + 4 (else -10).
- `msgCount` = (`size` − `entryOff`) / `msgSize` (else -10).
- `size` ≥ `entryOff` + `msgSize` (else -6).

The CPU builds its own command-queue header the same way (msgq.c:236-251): `rxHdrOff` = 32, `entryOff` = 0x1000, `msgCount` = 63, and `flags` = `MSGQ_FLAGS_SWAP_RX` (message_queue_cpu.c:180).

Read pointers go to swapped locations only if both headers carry `SWAP_RX` (msgq.c:411-428). That convention is open question 7. Later, a `writePtr` ≥ `msgCount` in the status-queue header is treated as nothing available (msgq.c:655).

### Every element the CPU receives (`GspMsgQueueReceiveStatus`, message_queue_cpu.c:640-846)

**1. Length.**

- The number of 4 KiB slots is ceil((16 + `rpc.length`) / 4096). It is read from the first slot (:684-701).
- After the checks below, `msgLen` = 16 + `rpc.length` must lie in [16, 65536], or the status is `NV_ERR_INVALID_PARAM_STRUCT` (:824-833).
- `rpc.length` counts the 32-byte RPC header plus the payload, as the sender sets it (rpc_common.c:184).

**2. Checksum.** The checksum runs over 16 + `rpc.length` bytes starting at `mctpHeader` (:725-727). It must come out 0, or the CPU logs "Bad checksum" and retries (:730-735). This is the code, from message_queue_priv.h:197-209:

```c
static NV_INLINE NvU32 _checkSum32(void *pData, NvU32 uLen)
{
    NvU64 *p        = (NvU64 *)pData;
    NvU64 *pEnd     = (NvU64 *)((NvUPtr)pData + uLen);
    NvU64  checkSum = 0;

    NV_ASSERT_CHECKED(uLen > 0);

    while (p < pEnd)
        checkSum ^= *p++;

    return NvU64_HI32(checkSum) ^ NvU64_LO32(checkSum);
}
```

It XORs whole little-endian qwords and then folds the high and low halves. The last qword is read in full even when `uLen` is not a multiple of 8. **Therefore the bytes after the message, up to the next 8-byte boundary, must be zero.** The sender zero-pads for exactly this reason (message_queue_cpu.c:499-501).

To produce a valid element, set `checkSum` = 0, compute `_checkSum32` over 16 + `length` bytes, and store the result in `checkSum`. `checkSum` sits at offset 8, the low half of qword 1, so the fold then comes out 0. The CPU does exactly this when it sends (:515, :545).

**3. MCTP and NVDM headers** (:737-759).

- `mctpHeader` bits 3:0 must be 1 (`MCTP_HEADER_VERSION`, mctp_format.h:40).
- `nvdmHeader` bits 23:8 must be 0x10de (`MCTP_MSG_HEADER_VENDOR_ID`, mctp_format.h:53, :58).
- Nothing else is checked on receive: not SOM/EOM, not the message type, not the NVDM type.
- A failure returns `NV_ERR_INVALID_DATA` and the CPU retries.

Send what the CPU itself sends, as the tool computed it:

- `mctpHeader` = **0xc0000001**: version 1, EOM, SOM (message_queue_cpu.c:505-511; mctp_format.h:78-94).
- `nvdmHeader` = **0x2510de7e**: type `VENDOR_PCI` 0x7e, vendor 0x10de, NVDM type `RM_RPC` 0x25 (message_queue_cpu.c:512; mctp_format.h:108-118; nvdm_format.h:61).

**4. Transport sequence number.**

- `seqNum` must equal the CPU's `rxSeqNum` (:762).
- `rxSeqNum` starts at 0, because the collection is zeroed (:235).
- It increments after every record the CPU consumes, **including failed ones** (:835-836).
- If the element carries an older number, the CPU marks it consumed and retries (:768-783). With any other mismatch, the read fails after at most 3 attempts (:647, :654, :786-787, :794-805).
- In the other direction, the CPU numbers its own messages from `txSeqNum` = 0 up (:514). It increments only after a successful submit (:620). The async init RPCs count too.

**5. Consumption.** After the checks, the slots are marked consumed and the CPU's read pointer moves (:838; msgq.c:704-748).

### RPC-level matching and `rpc_result`

**Headers the CPU writes on every request** (rpc_common.c:154-184):

| field | value |
|---|---|
| `header_version` | 0x03000000 |
| `signature` | 0x43505256 (`"VRPC"`) |
| `rpc_result`, `rpc_result_private` | 0xffffffff (`RPC_PENDING`) |
| `u.spare` | 0 |
| `length` | 32 + payload |

The CPU **never checks** `header_version` or `signature` on receive. A grep of `src/nvidia/src/kernel/gpu/gsp`, rpc.c and rpc_common.c finds only these writes.

**Sequence numbers.**

- A synchronous RPC takes `sequence` = `pRpc->sequence++` (kernel_gsp.c:403-404). The counter starts at 0 (rpc_common.c:98).
- An async RPC carries 0 and does not advance the counter (kernel_gsp.c:405-406).

**Matching a reply.**

- A received record answers the pending call only if `function` equals the request's function **and** `sequence` equals its sequence (kernel_gsp.c:1825-1826).
- Anything else is handled as an event (`_kgspProcessRpcEvent`).
- The reply is read into the same staging buffer that held the request (message_queue_cpu.c:188, :643; kernel_gsp.c:3185). The reply's payload therefore replaces the request's payload.

**Expected order on a first boot:**

| # | RPC | direction | kind | header `sequence` | element `seqNum` |
|---|---|---|---|---|---|
| 1 | GSP_SET_SYSTEM_INFO | CPU → GSP | async | 0 | 0 |
| 2 | SET_REGISTRY | CPU → GSP | async | 0 | 1 (more if split) |
| 3 | GSP_INIT_DONE | GSP → CPU | event | 0 | the device's first, or after any events it sent first |
| 4 | SET_GUEST_SYSTEM_INFO | CPU → GSP, with reply | sync | 0 | 2 |
| 5 | GET_GSP_STATIC_INFO | CPU → GSP, with reply | sync | 1 | 3 |

The table is derived from the counters above, assuming no other sync RPC comes earlier. None is sent between INIT_DONE and SET_GUEST_SYSTEM_INFO (kernel_gsp.c:5146-5165).

### GSP_INIT_DONE (0x1001)

`kgspWaitForRmInitDone_IMPL` polls with `rpcRecvPoll(pGpu, pRpc, NV_VGPU_MSG_EVENT_GSP_INIT_DONE, 0)` (kernel_gsp.c:6282). So the event needs `function` = 0x1001 **and** header `sequence` = 0.

Then:

- It requires `rpc_result` = 0 (`NV_ASSERT_OK_OR_RETURN`, kernel_gsp.c:6288). Any other value becomes the boot's return status.
- It reads `rpc_init_done_v17_00.bIsD3Hot` at `rpc_message_data` + 0 (kernel_gsp.c:6293-6305). It uses the flag only on GC6 or RTD3 transitions, so 0 is fine.

While it waits, the CPU runs in the bootup context (kernel_gsp.c:2877-2881). Without the API lock it accepts only these events (kernel_gsp.c:1418-1439): `UCODE_LIBOS_PRINT`, `GSP_LOCKDOWN_NOTICE`, `GSP_POST_NOCAT_RECORD`, `GSP_INIT_DONE`, `OS_ERROR_LOG`, `PFM_REQ_HNDLR_STATE_SYNC_CALLBACK`, `GSP_LOAD_EXEC_GENERIC_BOOTLOADER` and `GSP_LOAD_EXEC_HS_BINARY`. Any other event trips `NV_ASSERT(0)` and is dropped.

After each drained event, the CPU also runs `kgspHealthCheck_HAL` (kernel_gsp.c:1876-1877).

### SET_GUEST_SYSTEM_INFO (0x01) reply

`RmRpcSetGuestSystemInfo` (rpc.c:8730-8906) sends `rpc_set_guest_system_info_v03_00` with these fields (rpc.c:8796-8837):

- `vgxVersionMajorNum` = 0x2e and `vgxVersionMinorNum` = 0x0d (vgpu_version.h:33-34).
- The three strings, each with a length of 256.
- `guestClNum`.

`_issueRpcAndWait` then applies the generic result rule (rpc.c:2012-2026):

1. `rpc_result` must be 0.
2. `0xFF100009` (`RPC_API_CONTROL_NOT_SUPPORTED`) is returned as is.
3. Any other value below 0xFF000000 is returned as the `NV_STATUS`.
4. Anything else becomes `NV_ERR_GENERIC`.

After that, this RPC checks more (rpc.c:8841-8893):

- `rpc_result_private` must also be 0. The CPU set it to 0xffffffff when it sent, so **the device has to clear it**. A non-zero value becomes the status. It also triggers the version negotiation (:8841-8880): if the echoed version differs from the CPU's own and is at least 0x25.0x0e, the CPU resends once. If it is older, the CPU fails.
- The reply payload's `vgxVersionMajorNum`/`vgxVersionMinorNum` are stored in the static `rpcVgxVersion` (:8891-8892; declared at rpc.c:203). A second GPU with a different echo fails with `NV_ERR_INVALID_STATE` (:8883-8888).
- `rpcSetIpVersion(RPC_VERSION_FROM_VGX_VERSION(major, minor))` then re-runs the RPC HAL selection (rpc.c:8898-8899; rpc.c:982-996). The next RPCs exist only above a version floor: `rpcGetGspStaticInfo` for ipVersion ≥ 0x14000000 and `rpcGspSetSystemInfo` for ≥ 0x17000000 (generated/g_rpc_private.h:2949-2950, :2955-2956). An unassigned one fails verification with `NV_ERR_NOT_SUPPORTED` (:3238-3239).
- So **the reply must carry the payload, with `vgxVersionMajorNum` = 0x2e and `vgxVersionMinorNum` = 0x0d echoed.** A reply that carries only the header leaves whatever the 4 KiB slot contained at those offsets (48 and 52 bytes into the element: 16 + 32 + 0 and 16 + 32 + 4).

On bare metal nothing follows (see the corrections above). GET_GSP_STATIC_INFO comes next, and its reply is checked only by the generic rule and the length check.

## (e) Generating Rust bindings for these structs per driver version

### How the ioctl ABI is generated today

`cargo xtask abi` (crates/xtask/src/main.rs:15-24) reads `crates/nvrm-sys/abi.toml` (crates/xtask/src/abi/mod.rs:61). For each version:

1. It runs bindgen over `crates/nvrm-sys/wrapper.h` with a fixed include list and fixed clang arguments (mod.rs:22-28, :302-324). The headers come from the per-version closure in `vendor/nvidia-rm-headers/<v>`, which `tools/build.sh vendor-abi` produces with `clang -MM` (tools/build.sh:86-141).
2. It records sizes and offsets in `manifests/<v>.json`.
3. It classifies each pair of versions.
4. It emits `stable.rs` plus one `v<NNN>.rs` per version, with `assert_layout` macro guards (crates/nvrm-sys/src/lib.rs:31-40).

Everything that would have to change for the GSP set is hard-coded in one of five places:

- the wrapper path (mod.rs:304);
- `INCLUDE_DIRS` (mod.rs:22-28);
- the clang args (mod.rs:312-314);
- the crate paths (mod.rs:61, :72; emit.rs:107, :180, :196);
- the sparse-checkout roots of `vendor-abi` (tools/build.sh:88-90).

### Recommendation: a sibling crate, same generator

Put the GSP structs in a sibling crate, `crates/nvgsp-sys`, generated by the same xtask. Do not add them to `nvrm-sys`. The reasons:

- The GSP header closure is about 4× the ioctl closure (398 against 102 files).
- It needs a different preprocessor configuration.
- It is consumed only by the device, never by the guest or the host backend.
- Keeping it separate keeps `nvrm-sys` CI and its `RmAbi` mediation (crates/xtask/src/abi/mediated.rs) untouched.

### The minimal change

1. **Configuration moves into `abi.toml`.** Add an optional `[bindgen]` table to `AbiToml` (crates/xtask/src/abi/config.rs:12-16) with `wrapper`, `include_dirs`, `clang_args` and `crate_dir`. Its defaults are today's constants, so `nvrm-sys/abi.toml` stays unchanged. `run_bindgen` (mod.rs:302-324) and the path joins in mod.rs and emit.rs read from it. `--config` and `--manifests` already exist (mod.rs:43-50). One more flag, `--crate nvgsp-sys`, selects `crates/nvgsp-sys/abi.toml`.
2. **`crates/nvgsp-sys/wrapper.h`** is the include block of `caraxes/tools/gsp-abi-sizes.c`: `kernel_gsp.h`, `vgpu/rpc.h`, `g_rpc-structures.h` and `g_rpc-message-header.h` under their guard macros, then `message_queue_priv.h`, `msgq_priv.h`, `mctp_format.h`, `nvdm_format.h`, `gsp_static_config.h`, `gsp_init_args.h`, `gsp_fw_wpr_meta.h`, `libos_init_args.h` and `vgpu_version.h`. `[bindgen]` then holds the 14 include roots and the 7 defines from (a).
3. **The footprint lists** exactly the types in the tables above. `structs` covers `msgq(Tx|Rx)Header`, `GSP_MSG_QUEUE_.*`, `rpc_message_header_v03_00`, the ten `rpc_*_v*` structs, `PACKED_REGISTRY_.*`, `GspStaticConfigInfo_t`, `GspSystemInfo`, `GSP_ARGUMENTS_CACHED`, `MESSAGE_QUEUE_INIT_ARGUMENTS`, `LibosMemoryRegionInitArgument` and `GspFwWprMeta`. `constants` covers `NV_VGPU_MSG_.*` (the X-macro enum in vgpu/rpc_global_enums.h:4-10 preprocesses to a plain anonymous enum), `VGX_.*_VERSION_NUMBER`, `NVDM_TYPE_RM_RPC`, `MCTP_MSG_HEADER_(TYPE_VENDOR_PCI|VENDOR_ID_NV)`, `MSGQ_.*` and `GSP_FW_WPR_META_.*`.

   One bindgen setting is essential: `opaque_type("rpc_generic_union")`. `rpc_message_data` is a flexible array of a union over every RPC struct of the driver (generated/g_rpc-structures.h:11544). Allowlisting the header would otherwise pull in hundreds of types. Opaque keeps the union's size and alignment (8), so the header stays 32 bytes with align 8.
4. **`tools/build.sh vendor-abi`** gets a second closure pass. The same `clang -MM -MG` runs over `crates/nvgsp-sys/wrapper.h`, with the RM roots added to the sparse checkout. The result goes into `vendor/nvidia-gsp-headers/<v>` with the same PROVENANCE format. At 7.5 MB per version it is still small enough to commit.
5. **The hand-written part** stays small and is tested against the C tool's constants table. Three things are hand-written because bindgen cannot express them:
   - the DRF bit ranges of `mctp_format.h:40-54`;
   - the two inline constructors (`mctpCreateTransportHeader`, `mctpCreateNvdmHeader`), whose expected values are 0xc0000001 and 0x2510de7e;
   - `_checkSum32`.

   Keep `caraxes/tools/gsp-abi-sizes.sh --check` as the independent cross-check in CI. It compiles the same headers with GCC and clang, outside bindgen. A mismatch between its tables and `manifests/<v>.json` means one of the two generators is wrong.

### Version selection

Version selection is simpler than for the ioctl ABI. The CPU accepts only the GSP firmware whose `.fwversion` string equals its own `NV_VERSION_STRING` (kernel_gsp.c:5720-5770). The device therefore emulates exactly one driver version per guest, and no `RmAbi`-style mediation is needed. A `v<NNN>` feature per version and a runtime refusal on mismatch are enough. The classification report is still useful: it shows, per new driver, which of these structs moved.
