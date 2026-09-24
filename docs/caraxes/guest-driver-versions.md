<!-- SPDX-License-Identifier: MIT -->
# Guest driver versions against the 610.57.04 device

The operator's question: what happens with a guest that runs a different driver version from
the one the device emulates? Every guest so far ran 610.57.04, the host's version, from
NVIDIA's package. The device (`crates/vfio-user-nvgpu`) speaks the 610.57.04 GSP ABI through
`src/ver/v610.rs` (D14, [README.md](README.md)). This note covers the versions tried, where
each one stopped, the structural cause in the sources, the size differences, and what D14
needs so that the device can serve another version.

Evidence and exact commands: `caraxes/runs/guest-driver-2026-09-24/` (`commands.txt`). The
runs were on vm3 on 2026-09-24, in pure emulation (`CARAXES_HOST_RM=1 CARAXES_BOOT_MODEL=1`).
Each version ran in a fresh overlay on top of vm3's 610 overlay, which was left untouched
(section 7). No Xid was logged on the host.

## Summary

| Guest driver | Source | Device | Where it stops |
|---|---|---|---|
| 610.57.04 (baseline) | NVIDIA package (vm3 image) | stock | driver up (`RmInitAdapter succeeded!`) |
| 595.91.07 | Ubuntu `nvidia-driver-595-open`, the package `ubuntu-drivers` recommends | stock | First RPC. Guest: `GspMsgQueueReceiveStatus: Incorrect message length 0`, then `rpcRecvPoll(... GSP_INIT_DONE ...)` fails at `kernel_gsp.c:6105`. Device: `bad rpc length 0 in command slot 0 (seqNum 0), element skipped` |
| 595.91.07 | same | `CARAXES_GUEST_ABI=595`, 3 switches (section 2) | Driver up. `nvidia-smi` shows `Leandro VFIO RTX 2070-4G`, 49 C, P8, 4096 MiB. The GR static controls are misread without any error (section 2.4) |
| 610.43.02 | Ubuntu `nvidia-driver-610-open`, noble-security | stock | Driver up. `nvidia-smi` 610.43.02 is complete. No ABI difference from 610.57.04 |
| 615.71.09 | NVIDIA CUDA repository (`nvidia-headless-open`), as on korngold | stock | Status-queue link. Guest: `GspStatusQueueInit: msgqRxLink failed: -7` at `kernel_gsp_tu102.c:469`. Device: `queues: no RMARGS region in the libos init arguments` on the pre-boot `NV_PGSP_QUEUE_HEAD(0)=0` write, and nothing after that |

Just installing the Ubuntu driver does not give a different version. The newest open-module
package in the Ubuntu 24.04 archive is `nvidia-driver-610-open 610.57.04-0ubuntu0.24.04.3`
(noble-updates), which is the host's own version. It uses the same ABI, so it would work
unchanged. For this card (10de:1f02), `ubuntu-drivers devices` marks
`nvidia-driver-595-open` (595.91.07) as `recommended`. An operator who runs
`ubuntu-drivers install` therefore gets 595.91.07, and against the stock device that guest
stops at its first message (section 2).

## 1. What the Ubuntu archive offers

The Ubuntu archive, read on 2026-09-24:

| Package | Version | Pocket |
|---|---|---|
| nvidia-driver-610-open | 610.57.04-0ubuntu0.24.04.3 | noble-updates/multiverse |
| nvidia-driver-610-open | 610.43.02-0ubuntu0.24.04.1 | noble-security/multiverse |
| nvidia-driver-595-open | 595.91.07-0ubuntu0.24.04.1 | noble-updates/multiverse (`ubuntu-drivers`: recommended) |
| nvidia-driver-590-open | 590.48.01-0ubuntu0.24.04.5 | noble-updates/restricted |
| nvidia-driver-580-open | 580.178.04-0ubuntu0.24.04.1 | noble-updates/restricted |
| nvidia-driver-570-open | 570.211.01-0ubuntu1.24.04.1 | noble-updates/restricted |

Notes on the setup:

- The prebuilt `linux-modules-nvidia-595-open-6.8.0-136-generic` package holds 595.84, not
  595.91.07: it depends on `nvidia-kernel-common-595 (<= 595.84-1)`. The 595.91.07 modules
  therefore came from DKMS (`nvidia-dkms-595-open`).
- The guest has no route to the internet. The host fetched the indexes and `.deb` files with
  plain GETs, and apt in the guest installed them with `--no-download` (`commands.txt`).
- The 610 modules of NVIDIA's package live in `/lib/modules/6.8.0-136-generic/updates/`, and
  the 610 user space in `/opt/nvrm/lib`. Both were moved aside in the experiment overlays so
  that the new version is the one that loads.
- The `nvidia-driver-*` metapackages pull in `xserver-xorg-video-nvidia-*`. gdm then starts
  an X session on the device (section 2.5). The 615 guest was therefore installed headless.

## 2. 595.91.07: the first divergence and the ones after it

### 2.1 Stop 1: the queue element header (the first message)

Guest boot log (`595-stock/guest-dmesg.txt`):

```
NVRM: GPU0 GspMsgQueueReceiveStatus: Incorrect message length 0
NVRM: GPU0 nvCheckOkFailedNoLog: Check failed: Invalid structure parameter [NV_ERR_INVALID_PARAM_STRUCT] (0x0000003A) returned from rpcRecvPoll(pGpu, pRpc, NV_VGPU_MSG_EVENT_GSP_INIT_DONE, 0) @ kernel_gsp.c:6105
NVRM: GPU0 _kgspBootGspRm: unexpected WPR2 already up, cannot proceed with booting GSP
NVRM: GPU 0000:00:05.0: RmInitAdapter failed! (0x62:0x40:2168)
```

Device (`595-stock/device-events.txt`, `gpu.log`):

```
E boot: tx seq 0 event GSP_INIT_DONE
E boot: rx seq 0 rejected: bad rpc length 0 in command slot 0 (seqNum 0), element skipped
E boot: rx seq 0 rejected: bad rpc length 0 in command slot 1 (seqNum 0), element skipped
```

Cause: 610 redesigned `GSP_MSG_QUEUE_ELEMENT`.

- **595.91.07** (`message_queue_priv.h:43-51`): `authTagBuffer[16]`, `aadBuffer[16]`,
  `checkSum`, `seqNum`, `elemCount`, then `NV_DECLARE_ALIGNED(rpc_message_header_v rpc, 8)`
  at 0x30. The element header is therefore 0x30 bytes
  (`queueElementHdrSize = NV_OFFSETOF(GSP_MSG_QUEUE_ELEMENT, rpc)`,
  `message_queue_cpu.c:73`). The receiver takes the element count from `elemCount`
  (`:669`).
- **610.57.04** (`message_queue_priv.h:52-67`): `mctpHeader`, `nvdmHeader`, `checkSum`,
  `seqNum`, then `payload[]` at 0x10. The CC tag moved into a separate
  `GSP_MSG_QUEUE_ENCRYPTION_TAG`, and `elemCount` is gone: the element count is derived from
  the RPC length (`message_queue_cpu.c:82`, `:505`).

Both sides then read the RPC header's `length` from the wrong offset:

- The device (`msgq.rs`, `RpcHeader::parse(&first[ELEMENT_HEADER_SIZE..])` with
  `ELEMENT_HEADER_SIZE = 16`, `ver/v610.rs:55`) reads `length` at element offset 0x18. In a
  595 element, that offset is inside `aadBuffer`, which is zero without CC.
- The guest reads the device's `GSP_INIT_DONE` at 0x30 + 8 = 0x38, which lies in the
  device's (short) payload and is 0. Its check `msgLen < sizeof(GSP_MSG_QUEUE_ELEMENT)`
  (595 `message_queue_cpu.c:771-779`) prints `Incorrect message length 0`.

The checksum passed by chance. The 595 receiver folds only `hdrSize + rpc.length` = 0x30
bytes, and the rest of the device's element is zero. The `seqNum` check also passed: at 0x24
the 595 receiver reads the device's `rpc_result_private`, which is 0, the expected first
sequence number.

The switch: `CARAXES_GUEST_ABI=595` selects `ElementLayout::PRE_610`
(`src/ver/mod.rs:98-143`): header 0x30, checksum at 0x20, `seqNum` at 0x24, `elemCount` at
0x28, and no MCTP words. `msgq.rs` uses the layout for receiving (`:541-548`, `:596-597`)
and for sending (`:677-712`).

### 2.2 Stop 2: the tail of `GspStaticConfigInfo`

With the element switch, the GSP boot completes: `GSP_SET_SYSTEM_INFO`, `SET_REGISTRY`,
`SET_GUEST_SYSTEM_INFO` (`vgx 0x2d.0x3 driver="595.91.07"`, noted by the device) and
`GET_GSP_STATIC_INFO` all go through. The fourth control then fails
(`595-element/guest-dmesg.txt`, `device-events.txt`):

```
E boot: rx seq 7 GSP_RM_CONTROL (76) ... hClient=0x76 hObject=0xca7a0001 cmd=0x208001b0 paramsSize=1284
E boot: ctrl 0x208001b0: control 0x208001b0 forwarded to the host: status 0x56
NVRM: ... returned from pRmApi->Control(pRmApi, pGpu->hInternalClient, pGpu->hInternalSubdevice, NV2080_CTRL_CMD_GPU_GET_CONSTRUCTED_FALCON_INFO, ...) @ gpu.c:5537
NVRM: GPU 0000:00:05.0: RmInitAdapter failed! (0x23:0x56:1330)
```

In the 610 baseline the same control carries `hClient=0xca7a0001 hObject=0xca7a0003`. The
cause is that 610 inserted `vbiosRevision` and `vbiosOemRevision` into `GspStaticConfigInfo`
before `bPageRetirementSupported` (610 `gsp_static_config.h:136-137`), which shifts the tail
of the struct:

- The 595 guest reads `hInternalClient` at 0x600, where the device wrote
  `vbiosOemRevision` (0x76).
- It reads `hInternalDevice` at 0x604, where the device wrote flag bytes (0).
- It reads `hInternalSubdevice` at 0x608, where the device wrote its `hInternalClient`
  (0xca7a0001).

610 also added `bSysL2CacheCoherentMode` (`:156`) and `bBusResetRequired` (`:165`). The
struct measures 0x640 bytes in 610 and 0x638 in 595. Everything before 0x5fc is identical.

The switch: `static_info_for_guest` (`src/ver/mod.rs:151-169`) remaps the 610 bytes into the
595 layout for a pre-610 guest. It is called at `boot.rs:827`.

### 2.3 Stop 3: `maxRpcSize` (the guest's Xid 119 and 154)

With the first two switches, `RmInitAdapter succeeded!`, nvidia-modeset and nvidia-drm load,
and `nvidia-smi` lists the card. The first large control of the user space then breaks the
queue (`595-static/`). `nvidia-smi` sends GSS legacy perf `0x2080a0a4`, whose 67396-byte
parameters need continuation records:

```
E boot: send: element of 65568 bytes does not fit the status queue
NVRM: Xid (PCI:0000:00:05): 119, Timeout after 295s of waiting for RPC response from GPU0 GSP! Expected function 76 (GSP_RM_CONTROL) sequence 150 (0x20800a70 0x0).
NVRM: Xid (PCI:0000:00:05): 154, GPU recovery action changed from 0x0 (None) to 0x1 (GPU Reset Required)
```

(These Xids are in the guest's log; the host logged none.)

`maxRpcSize = queueElementSizeMax - queueElementHdrSize` (`message_queue_priv.h:112-115`) is
65520 with the 610 header and 65488 with the 595 header. The device's `v::MAX_RPC_SIZE`
(`ver/v610.rs:67`) is the 610 value. With it the device fails in both directions:

- It did not recognize the guest's 65488-byte first record as split, so it did not wait for
  the continuation records (`boot.rs:768`).
- It split its reply at 65520 - 32 bytes, which gives an element 0x20 bytes too large for a
  0x30-byte header (`boot.rs:1009-1024`).

The switch: `ElementLayout::max_rpc_size()` (`src/ver/mod.rs:113-115`) replaces the constant
at both places.

### 2.4 With all three switches: the driver is up, and the GR tables are silently wrong

`595-switches/`: `RmInitAdapter succeeded!`, no Xid. `nvidia-smi` shows:

- `Leandro VFIO RTX 2070-4G`
- `ERR!` for the fan, 49 C, P8
- 0 MiB / 4096 MiB

`nvidia-smi -q` reports `GSP Firmware Version: 610.57.04`: the version comes from the
device's answer, and the guest does not check it against its own. What is still wrong:

- **The GR static controls are misread, and nothing reports an error.**
  `control-param-sizes.txt` marks 11 of the 54 parameter structs the handlers use as
  different in size.
  - The bounds changed: `NV2080_CTRL_INTERNAL_GR_MAX_GPC` is 32 in 595 and 64 in 610
    (`ctrl2080internal.h:286` against `:289`), and `NV2080_CTRL_INTERNAL_GR_MAX_SM` is 240
    against 512 (`:224`, `:221`).
  - The device writes the floorsweeping masks at 610 offsets (`FS_TPC_COUNT`,
    `ver/v610.rs:628-640`, `kgr.rs:165-205`) into the 595 buffer (5568 bytes). The 595
    guest reads `tpcCount[]` at 4 + 32*4 = 132, where the device wrote `tpcMask[32..]`,
    which is zero.
  - The same applies to `GET_GLOBAL_SM_ORDER` (34592 bytes against 73760), `GET_INFO`
    (3776 against 4032, where engine 0 happens to line up), `GET_PPC_MASKS` and the skyline
    info.
  - `RmInitAdapter` accepts all of this, so the damage would only show up as zero TPCs per
    GPC when CUDA starts.
- **`NV2080_CTRL_GPU_INFO_MAX_LIST_SIZE`** is 0x46 in 595 and 0x48 in 610
  (`ctrl2080gpu.h:121`, `:127`). The list itself is compatible. Only the struct size
  differs.
- **`INTERNAL_GPU_GET_USER_REGISTER_ACCESS_MAP`** is 8204 bytes in 595 and 20492 in 610
  (`..._MAX_COMPRESSED_SIZE` 4096 against 16384, `ctrl2080internal.h:774`, `:777`). The
  answer (size 0 = allow all) sits at offset 0, so this one is harmless.

### 2.5 The same with 610.43.02: channel binds from Xorg

In both the 595 run and the 610.43.02 run, gdm started Xorg on the device. The kernel then
asserts at `kchannelBindToRunlist` (`kernel_channel.c:2828` in 595, `:2784` in 610.43.02),
because the device leaves `NVA06F_CTRL_CMD_BIND` (0xa06f0104) on a GR channel unanswered.
This is a gap in the device's channel emulation, not a version difference: the 610 baseline
image has no Xorg driver and never binds a GR channel.

## 3. 610.43.02: an intra-branch update works as it is

The Ubuntu security pocket's 610.43.02 was installed by DKMS with an apt pin. Against the
stock device it came up, and `nvidia-smi` was complete (55 %, 49 C, P8, 29 W / 175 W, 13 %;
`610.43.02/nvidia-smi.txt`). The device only notes:

```
guest driver "610.43.02" is not the emulated 610.57.04 (build with its ver module)
```

The static comparison agrees with the run:

- The `gsp-abi-sizes` output is identical to 610.57.04 apart from the header line
  (`abi-610.43.02.md`).
- The 54 control structs have the same sizes.
- The RPC numbers are the same.
- All 100 control ids of `ver/v610.rs` have the same values.

## 4. Changed sizes

The `gsp-abi-sizes.sh` tables are in the run directory (`abi-*.md`). The control structs
come from `ctrlsizes.c` compiled against `target/abi-headers/<version>`. All sizes are in
bytes.

| Structure | 595.91.07 | 610.43.02 | 610.57.04 | 615.71.09 |
|---|---:|---:|---:|---:|
| `msgqTxHeader` | 32 (v1: version, writePtr in memory, rxHdrOff) | 32 | 32 | 32 (v2: versionMajor/Minor, no writePtr/rxHdrOff, pointers in `NV_PGSP_QUEUE_*`/`MSGQ_*`) |
| `msgqRxHeader` | 4 | 4 | 4 | removed |
| `GSP_MSG_QUEUE_ELEMENT` (header) | 80 (header 0x30: tag, aad, checkSum, seqNum, elemCount) | 16 | 16 (header 0x10: mctp, nvdm, checkSum, seqNum) | 40 (header 0x18 without CC: MCTP magic, size, mctp, nvdm, nvdm size; no checksum, no seqNum) |
| `queueElementHdrSize` (no CC) | 0x30 | 0x10 | 0x10 | 0x18 |
| `maxRpcSize` | 65488 | 65520 | 65520 | as the header |
| `rpc_message_header_v` | 32 | 32 | 32 | 32 (RPCs now inside NVDM type 0x25) |
| `GspStaticConfigInfo` | 1592 | 1600 | 1600 | 1608 (replaced on the wire by the NVKV answer of `GMCAPI_CMD_GSP_INIT`) |
| `GspSystemInfo` | 944 (no `bIsCxlDevice`) | 952 | 952 | 952 (sent as NVKV, section 5) |
| `GSP_ARGUMENTS_CACHED` | 136 (no `bindataArgs`) | 152 | 152 | 184 (magic 'GSP ', size, flags before the queue arguments) |
| `MESSAGE_QUEUE_INIT_ARGUMENTS` | 64 | 64 | 64 | 64 (flags first, 32-bit element sizes, `pageTableEntryCount` at +0x34) |
| `GspFwWprMeta` | 256 | 256 | 256 | 256 (`padding` becomes `pagingConfig`) |
| `NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO` | 56 (no `regionTag`, which sat in padding) | 56 | 56 | 56 |
| `NV2080_CTRL_GPU_GET_INFO_V2_PARAMS` | 564 | 580 | 580 | |
| `NV2080_CTRL_GR_GET_INFO_V2_PARAMS` | 496 | 528 | 528 | |
| `..._INTERNAL_GPU_GET_USER_REGISTER_ACCESS_MAP_PARAMS` | 8204 | 20492 | 20492 | |
| `..._INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO_PARAMS` | 436 | 724 | 724 | |
| `..._INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS` | 696 | 1336 | 1336 | |
| `..._INTERNAL_STATIC_GR_GET_FLOORSWEEPING_MASKS_PARAMS` | 5568 | 10688 | 10688 | |
| `..._INTERNAL_STATIC_GR_GET_GLOBAL_SM_ORDER_PARAMS` | 34592 | 73760 | 73760 | |
| `..._INTERNAL_STATIC_GR_GLOBAL_SM_ORDER` | 4324 | 9220 | 9220 | |
| `..._INTERNAL_STATIC_GR_GET_INFO_PARAMS` | 3776 | 4032 | 4032 | |
| `..._INTERNAL_STATIC_GR_INFO` | 472 | 504 | 504 | |
| `..._INTERNAL_STATIC_GR_GET_PPC_MASKS_PARAMS` | 1024 | 2048 | 2048 | |

The rows without a 615 value were not measured here. For the seven control structs that
615.71.09 changes, see [blackwell-gb206.md](blackwell-gb206.md) §3.4.

What did not change between 595.91.07 and 610.57.04:

- the RPC function numbers (610 only appends `CTRL_GPU_SET_MIGRATION_BLOCK = 229`,
  `rpc_global_enums.h`)
- every `rpc_*_vXX_YY` payload the device uses
- `PACKED_REGISTRY_*`
- `LibosMemoryRegionInitArgument`
- the 100 control command numbers the device uses

To produce the 595 table, `caraxes/tools/gsp-abi-sizes.{c,sh}` gained field lists for majors
below 610 (`GSP_ABI 595`). The output for 610.57.04 and 615.71.09 is byte-identical to the
output before that change, and `--check` passes.

## 5. 615.71.09: what the guest sends, and where the 610 emulation stops

The operator's new baseline. The image is the fresh overlay `vm3/g615`
(`VM=vm3/g615 tools/caraxes-vm.sh up`, section 7). The guest has:

- nvidia 615.71.09 from NVIDIA's CUDA repository for ubuntu2404: `nvidia-headless-open`,
  `nvidia-dkms-open`, `libnvidia-compute`, `nvidia-firmware` and `nvidia-modprobe`, all
  615.71.09-2ubuntu1
- a DKMS build in `updates/dkms`, as on korngold (`caraxes/reference/gb206/host/modinfo-nvidia.txt`)
- the firmware in `/lib/firmware/nvidia/615.71.09/`
- `libcuda.so.1`, `libnvidia-ml.so.1` and `nvidia-smi` of 615.71.09

### 5.1 The order the guest drives (Turing, `kgspBootstrap_TU102`, 615 `kernel_gsp_tu102.c:392-478`)

1. **Before the boot.** `msgqTxCreate` for the command queue writes a v2 `msgqTxHeader` into
   the first command-queue page, then `WRITE_REG(NV_PGSP_QUEUE_HEAD(0), 0)` (`msgq.c:136-176`).
   The captured header in `615.71.09/unlinked/capture.txt`: `versionMajor 2 versionMinor 0
   size 0x40000 msgSize 0x1000 msgCount 63 entryOff 0x1000`.
   - Device: `W bar0 0x00110c00 0x0`, taken as `doorbell 1: libos args PA 0x0`, then
     `queues: no RMARGS region in the libos init arguments` (`boot.rs:1085`). In 610 the
     first write to this register comes after the boot, with three RPCs queued. In 615 it
     comes before the mailbox is programmed.
2. **The boot.** FWSEC (FRTS) and reset into RISC-V. Then `kgspProgramLibosBootArgsAddr`
   writes `NV_PGSP_FALCON_MAILBOX0/1` = the libos-args PA (`:432`, trace
   `W bar0 0x00110040 0x769d000`). Booter Load follows (`:436`), and the device reports
   `sec2 start 1: booter done, wpr meta PA ... gsp riscv active`. This part is the same as
   in 610, and the device's Turing boot model handles it.
3. **`GspStatusQueueInit`** (`:469`, `message_queue_cpu.c:341-400`). The guest polls the
   status queue's header in its own memory through `msgqRxLink` for 4 s
   (`timeoutUs = 4000000`, `:348`). It needs `versionMajor == 2`,
   `size == statusQueueSize` (0x40000), `msgSize == 0x1000` and
   `msgCount == (size - entryOff) / msgSize` (`msgq.c:183-273`). The device writes nothing
   there, so the guest reads zeros and fails with `-7` (`rx.size != size`, `msgq.c:248`):
   `GspStatusQueueInit: msgqRxLink failed: -7, nvStatus 0x00000065, retries: 133264`, then
   `RmInitAdapter failed! (0x62:0x40:2130)`. **This is where the 610 emulation stops.**
4. **`kgspSendInitRpcs`** (`:473`, `kernel_gsp.c:5313-5345`). It sends one
   `gmcapiCommand(GMCAPI_CMD_GSP_INIT, pEncodedSystemInfo, ...)` with a 48 KiB response
   buffer (`:5321`). This replaces 610's `GSP_SET_SYSTEM_INFO`, `SET_REGISTRY`,
   `GET_GSP_STATIC_INFO` and `GSP_INIT_DONE`.

### 5.2 What the guest prepares (host-side read of guest RAM, `615.71.09/unlinked/`)

`caraxes/runs/guest-driver-2026-09-24/capture615.py` reads guest RAM read-only, through the
device's own DMA-mapped memfds (`/proc/<device pid>/fd/<n>`).

- **libos init arguments** (the PA from the mailbox): `LOGINIT` (0x10000), `LOGRM`
  (0x10000) and `RMARGS` (0x1000). These are the same ids as in 610; `RMARGS` is still set
  up by `kgspSetupLibosInitArgs` (615 `kernel_gsp.c:7067`).
- **`GSP_ARGUMENTS_CACHED`** (`gsp_init_args.h:60-73`): magic 0x20505347 ('GSP '), size
  0xb8, flags 0x2 (`STACK_IN_DMEM`), then `messageQueueInitArguments` at +0x10 with:
  - `flags` 0 and `sharedMemPhysAddr`
  - `cmdQueueOffset` 0x1000 and `statQueueOffset` 0x41000
  - `queueElementHdrSize` 0x18, `queueElementSizeMin` 0x1000 and `queueElementSizeMax`
    0x10000
  - `queueHeaderAlign` 4 and `queueElementAlign` 12
  - `pageTableEntryCount` 129

  The device's 610 offsets (`MQ_*`, `ver/v610.rs:199-202`) read this as
  `implausible queue arguments: 2 pages, status at 0x88a00000`: flags 0x2 as the page count,
  and the shared-memory PA as the status offset.
- **The shared memory.** The page table (129 PTEs), the command-queue header (above), and
  the status-queue page, which is all zero.
- **The WPR meta.** The same as 610 except `pagingConfig`.

### 5.3 The first `GMCAPI_CMD_GSP_INIT` (`615.71.09/linked/`)

For this run, the capture script played the GSP's part of `msgqTxCreate` for the status
queue: it wrote one 32-byte v2 header,
`0200000000000400001000003f00000000100000000000000000000000000000`, into guest RAM at
`statQueueOffset`, within the 4 s window. The device was not changed. The guest then did
the following (`linked/device-events.txt`):

```
W bar0 0x00110c84 4  0x0     NV_PGSP_MSGQ_TAIL(0) = 0   (msgqRxLink, msgq.c:272)
W bar0 0x00110c00 4  0x1     NV_PGSP_QUEUE_HEAD(0) = 1  (the write pointer itself: one element queued)
E boot: doorbell 2: libos args PA 0xbea0c000 (mapped), wpr meta PA 0xbea0b000
E boot: queues: implausible queue arguments: 2 pages, status at 0x88a00000
R bar0 0x00110c80 ... 0x0    NV_PGSP_MSGQ_HEAD(0), polled 96366 times until the timeout
```

It then logged `_kgspLogGspTimeout: *** GSP Timeout: GSP-RM is slow (origin=RPC) ***`.

The command element in slot 0 (`gmcapi-gsp-init-slot0.bin`, decoded by `nvkv-decode.py` in
`gmcapi-gsp-init-decoded.txt`):

| Offset | Field | Value |
|---:|---|---|
| 0x00 | `mctpMagic` | 0x4d435450 "MCTP" (`message_queue_priv.h:92`) |
| 0x04 | `mctpPayloadSize` | 2432 (= 16 + 8 + 2408) |
| 0x08 | `mctpHeader` | 0xc0000001 (SOM, EOM, version 1) |
| 0x0c | `nvdmHeader` | 0x2610de7e: vendor 0x10de, type 0x7e, NVDM type 0x26 = `NVDM_TYPE_GMCAPI` (`nvdm_format.h:63`; RM RPCs are 0x25) |
| 0x10 | `noEncryption.nvdmPayloadSize` | 2408 |
| 0x18 | `GMCAPI_HEADER.command` | 0x10001 = `GMCAPI_ID(GSP_MGMT 0x01, 0x0001)` `GSP_INIT` (`gmcapi_table.h:84-86`) |
| 0x1c | `size` | 2368 (296 NVKV words) |
| 0x20 | `sequence` | 0 |
| 0x28 | `request.max_response_size` | 0xc000 |
| 0x40 | `data` | NVKV (`nvkv.h`: key 15:0, idx 27:16, opcode 31:28, count or imm32 63:32) |

There is no element checksum and no sequence number. The NVKV request carries (key names from
`interface/gmcapi/gmcapi_system_info*.h`):

- the PCI ids (1f0210de, 40111458, a1)
- the BAR addresses (0xe6000000, 0x3fffe0000000, 0x3ffffc000000), the BDF, the host arch
  and OS
- the config mirror (0x88000+0x1000), the PCIe caps and the notify surface
- `MAX_USER_VA`, `CL_FLAGS` and the ACPI blocks
- `HYPERVISOR_TYPE 3`
- the PCIe, platform and feature flags
- `KERNEL_DRIVER_NAME "NVRM"`, `KERNEL_DRIVER_VERSION_STRING "615.71.09"` and
  `UNSTABLE_INTERFACE_SUPPORT 3`
- the whole registry as `REGKEY_NAME`/`REGKEY_VALUE_U32` pairs, which was 610's
  `SET_REGISTRY`: `GpuInitOnProbe`, `ResmanDebugLevel`, `RmRcWatchdog`,
  `RMDisableScrubOnFree`, `RmDisableGlobalCeUtils` and the others

### 5.4 What the device needs for a 615 guest (for the msgq v2 / GMCAPI work)

Each item follows from the stops above.

1. **Queue creation after the boot, not on the doorbell.** Link on the `WprMeta` boot event,
   when the mailbox PA is known:
   - read `GSP_ARGUMENTS_CACHED` with the 615 layout
   - write the v2 status-queue header (versionMajor 2, size 0x40000, msgSize 0x1000,
     msgCount 63, entryOff 0x1000)
   - keep `NV_PGSP_MSGQ_HEAD(0)`/`NV_PGSP_MSGQ_TAIL(0)` (0x110c80/0x110c84,
     `kern_gpu_tu102.c:597-603`) as registers

   The pre-boot write of `QUEUE_HEAD(0)=0` must not count as a doorbell.
2. **Queue pointers in registers.** `NV_PGSP_QUEUE_HEAD(0)` is the command write pointer and
   `NV_PGSP_QUEUE_TAIL(0)` the command read pointer, which the device publishes.
   `NV_PGSP_MSGQ_HEAD(0)` is the status write pointer, which the guest polls and the device
   publishes; `NV_PGSP_MSGQ_TAIL(0)` is the status read pointer, which the guest writes.
   Nothing about the queue state lives in the queue memory.
3. **The element format.** The 0x18-byte MCTP/NVDM header without checksum or seqNum.
   Dispatch on NVDM type 0x25 (RPC) and 0x26 (GMCAPI).
4. **`GMCAPI_CMD_GSP_INIT`.** Parse the NVKV system info above, and answer with a GMCAPI
   response (flag 0x01000000, `gmcapi_base.h:56-60`) carrying the NVKV-encoded static info,
   which replaces `GspStaticConfigInfo`. After that, the RPC path (`GSP_RM_CONTROL`, ...) in
   NVDM type 0x25 elements, and the seven changed control structs of blackwell-gb206.md §3.4.

## 6. What D14 needs to serve another version

**Generated modules.** A `v595.rs`, like `v615.rs`, generated rather than copied:

- The GSP structs come from `gsp-abi-sizes` (now possible for pre-610 trees) or from the
  planned `nvgsp-sys` crate (versatility.md, "Adding a driver version", step 2).
- The control layouts come from `nvrm-sys`. It has `v595` bindings, but only the ioctl
  closure (2265 lines): the internal GR structs the handlers use need adding to its
  footprint.

The pinning tests of `ver/v610.rs` then show every moved offset. The sizes that matter for
595 are those in section 4.

**Boot-path switches, which must be runtime and not per build.** A single build cannot know
the guest's version beforehand, and the first thing to agree on is the element layout, which
comes before any version string. `ver::active` (a `pub use` alias) must become a value
selected at runtime: a `&'static dyn GuestAbi`, or a table of offsets per version. These are
the places that switch, in order of first use:

1. queue discovery and the queue transport: v1, or v2 with registers (615)
2. the element header (0x30 / 0x10 / 0x18) and `maxRpcSize`
3. `GSP_ARGUMENTS_CACHED` / `MESSAGE_QUEUE_INIT_ARGUMENTS`
4. `GspSystemInfo` (595 and 610 share the parsed fields) or NVKV system info (615)
5. `GspStaticConfigInfo` or NVKV static info
6. the control parameter layouts of the kgr, nvml and ctrl handlers

The three switches of this run are items 2 and 5 for 595, done by hand
(`CARAXES_GUEST_ABI=595`).

**Firmware-version detection.** The candidates, earliest first:

- (a) **The GSP firmware image the guest loads.** At the `WprMeta` boot event the device
  knows `sysmemAddrOfRadix3Elf` (`GspFwWprMeta`, the same in all three versions). The ELF's
  `.fwversion` section holds the exact version string, which the guest compares against its
  own `NV_VERSION_STRING` (gsp-abi-610.md (e)). This is available before the first queue
  element and gives the exact version. It has not been implemented or tested; reading a
  radix3 tree from guest RAM is the same walk the device already does for the queue page
  table.
- (b) **The shape of the first element or of the queue header.** A 615 command queue has
  `versionMajor 2` in its header. A 610 element starts with `0xc0000001 0x2510de7e`. A 595
  element starts with zeros (the CC tag, without CC). `GSP_ARGUMENTS_CACHED` starts with
  'GSP ' in 615. This is cheap, but it only tells the generation, not the version.
- (c) **`SET_GUEST_SYSTEM_INFO`'s driver string**, which the device already notes. It
  arrives third, after `GSP_SET_SYSTEM_INFO` and `SET_REGISTRY` have been parsed, and 615
  has no such RPC (the string is `NVGMC_SI_KERNEL_DRIVER_VERSION_STRING` in the NVKV).
  Useful as a cross-check only.

  Recommended: (a) chooses the module, and (b) checks it.

**Estimate.** The effort in working days (from this run's work: three hand switches took
about 1.5 hours to reach a 595 guest with the driver up):

| Work | Days |
|---|---:|
| Runtime version selection (`ver::active` to a runtime ABI object, the switches above, detection (a)+(b), tests with both a 595 and a 610 guest) | 3-4 |
| A generated `v595` (GSP layouts from gsp-abi-sizes, internal GR and GPU control layouts in `nvrm-sys`'s footprint, kgr/nvml offsets computed per version, the nvml lists re-captured from a 595 `nvidia-smi -q`), up to a CUDA gate on 595 | 3-5 |
| 615 on Turing (msgq v2 with register pointers, MCTP/NVDM elements, GMCAPI dispatch, the NVKV system-info parser and static-info encoder, the seven changed controls); the Blackwell-specific parts of blackwell-gb206.md are not included | 8-12 |
| Each further intra-branch update (610.43.02 kind): run the measurements, and add a version string to the table when nothing moved | under 0.5 |

## 7. State and repetition

- **vm3's own overlay** (`/mnt/vmstore/caraxes/vm3/rootfs.qcow2`, 610.57.04) was only used
  as a backing file. Its mtime is 2026-09-24 10:11, earlier than these runs.
- **`/mnt/vmstore/caraxes/vm3/g615`** is the 615.71.09 guest, an overlay on vm3's overlay.
  It is stopped. Select it with `VM=vm3/g615` and the usual vm3 variables (tap `caraxes3`,
  192.168.103.23, MAC `02:00:00:00:00:23`). The launcher's state directory is
  `$CARAXES_STORE/$VM`, so a subdirectory works without any change to the launcher.
  - nvidia is blacklisted from autoloading (vm3's `/etc/modprobe.d`), so
    `tools/caraxes-vm.sh probe <label>` loads it.
  - The guest holds `debs-615/` and `uris615.txt` from the install.
- **`/mnt/vmstore/caraxes/vm3/guestdrv`** is the 595.91.07, then 610.43.02, overlay. It is
  stopped. This session was not allowed to delete it; it is 3.7 GiB and can be removed
  (`rm -rf`).
- **Device changes** (uncommitted): `src/ver/mod.rs` (`ElementLayout`, `element_layout`,
  `static_info_for_guest`, `max_rpc_size`), `src/msgq.rs` and `src/boot.rs` (their use).
  - All three switches are off unless `CARAXES_GUEST_ABI=595` is set, and the default path
    is byte-for-byte the old behaviour. The 104 unit tests pass.
  - To revert, drop these three hunks.
- **Tool change**: `caraxes/tools/gsp-abi-sizes.{c,sh}` (`GSP_ABI 595`).
- **Repeating the runs**: `caraxes/runs/guest-driver-2026-09-24/commands.txt` has every
  command, including how the Ubuntu and CUDA indexes and packages reach a guest without
  internet access. The 615 captures take
  `python3 caraxes/runs/guest-driver-2026-09-24/capture615.py $(cat $W/gpu.pid) $W/gpu-trace.log <out> 180 &`
  before `tools/caraxes-vm.sh probe <label>`. Set `LINK=1` to write the status-queue header
  and get the `GMCAPI_CMD_GSP_INIT` element.
