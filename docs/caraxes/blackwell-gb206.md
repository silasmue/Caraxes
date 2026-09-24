<!-- SPDX-License-Identifier: MIT -->
<!-- Written 2026-09-24 from caraxes/reference/gb206/ (commit 22f3609), vendor/open-gpu-kernel-modules
     (610.57.04) and an open-gpu-kernel-modules clone at tag 615.71.09 (/mnt/vmstore/caraxes/ogkm-615.71.09,
     commit 61dcc93). Section 3 carries generated output of a 615-adapted gsp-abi-sizes; re-run, do not edit. -->
# Blackwell GB206 (RTX 5060 Ti): what the lab capture says, and the way to a GB206 identity

The operator's lab run of `caraxes/tools/lab-capture.sh` on host korngold (RTX 5060 Ti, GB206,
PCI 10de:2d04, driver 615.71.09) is filed under `caraxes/reference/gb206/`. This note reads it
item by item against the TU106 reference, turns it into the D14 identity work list
([README.md](README.md) D14, [versatility.md](versatility.md) "Blackwell"), records the
`gsp-abi-sizes` item the lab run skipped, and orders the steps to a first Blackwell N gate.

Paths: `gb206/` means `caraxes/reference/gb206/`, `gpu/` means
`caraxes/reference/gb206/gpu-0000_01_00.0/`, `rtx2070/` means `caraxes/reference/rtx2070/`.
Source paths without a prefix are relative to `vendor/open-gpu-kernel-modules` (610.57.04, the
guest driver in use); `615:` prefixes a path in the 615.71.09 clone. Every capture fact carries
file:line.

## Summary

- The capture is complete and consistent (`gb206/MANIFEST.txt`, 34 items ok, one skipped):
  GB206 revision A1, 16 GiB GDDR7 board (16311 MiB usable), 3 GPCs x 6 TPCs, three copy
  engines, one NVDEC, one NVENC, one NVJPG, one OFA, VBIOS 98.06.1F.40.4A, GSP firmware
  615.71.09 from `gsp_ga10x.bin`.
- The guest driver we use, **610.57.04, supports GB206** (`README.md:1003` lists 2D04; the name
  table `src/nvidia/generated/g_nv_name_released.h:839`; the HAL `src/nvidia/src/kernel/gpu/gpu.c:4922`).
  A first GB206 identity needs no new `ver` module, only new names in `src/ver/v610.rs`.
- On GB206 the 610 kernel RM boots GSP **through FSP**, not by programming the RISC-V boot
  registers itself: it polls an FSP boot-complete scratch, sends an NVDM COT message to FSP over
  the FSP EMEM queue and waits for FSP to release the GSP target mask
  (`src/nvidia/src/kernel/gpu/gsp/arch/hopper/kernel_gsp_gh100.c:879-905`). The versatility checklist describes the FSP-less fallback
  (`_kgspBootstrapGspFmc_GH100`); the device has to emulate FSP (section 2.2).
- **615.71.09 redesigns the CPU-GSP transport**: message queue v2 with the read/write pointers in
  `NV_PGSP_QUEUE_*`/`NV_PGSP_MSGQ_*` registers instead of shared memory, an MCTP-framed queue
  element without checksum and sequence number, and one `GMCAPI_CMD_GSP_INIT` request with
  key/value (NVKV) encoded system info and static info that replaces `GSP_SET_SYSTEM_INFO`,
  `SET_REGISTRY`, `GET_GSP_STATIC_INFO` and `GSP_INIT_DONE`. That is a protocol change, not a
  layout change: a D14 `v615` module cannot be generated from the 610 one (section 3).

## 1. The capture, item by item

### 1.1 Run and host

| Fact | GB206 capture | Source |
|---|---|---|
| Run | 2026-09-24 10:47:51, sudo yes, runtime 2 s, part 2 with rustc | `gb206/capture.log:2-4`, `gb206/MANIFEST.txt:3-4` |
| Kernel, OS | Linux 7.0.0-31-generic, Ubuntu 24.04.5 | `gb206/host/uname.txt:3`, `gb206/host/os-release.txt:3` |
| Command line | no IOMMU or nvidia options | `gb206/host/cmdline.txt:3` |
| CPU | AMD, 24 CPUs | `gb206/host/cpu.txt:7-9` |
| Uptime at capture | 14 days (booted 2026-09-10) | `gb206/host/boot-time.txt:3-4` |
| Other GPUs | ASPEED BMC (ast, primary console fb0) and the AMD iGPU (amdgpu) | `gb206/host/kernel-log.txt:390-394, 407-425` |
| IOMMU group | 12 = 01:00.0 and 01:00.1 only | `gb206/host/iommu-groups.txt:7` |
| Upstream port | AMD 1022:14db at 00:01.1, prefetchable window 16704 MiB | `gpu/lspci-upstream.txt:3,13` |
| Chip name in the run | `10de-2d04`, not `gb206`: the lab machine's pci.ids does not know 2d04 (`Device [10de:2d04]`), so `chip_of` fell back (`caraxes/tools/lab-capture.sh:254-260`) | `gb206/MANIFEST.txt:3`, `gb206/host/lspci-nn.txt:25` |

### 1.2 PCI identity and config space

| Item | GB206 | TU106 (rtx2070/) | Consequence for the synthetic device |
|---|---|---|---|
| Vendor:device, revision | 10de:2d04 rev a1 | 10de:1f02 rev a1 | board file |
| Subsystem | 1043:8a08 (ASUS; ROM strings name the TUF RTX 5060 Ti, `rom.bin` 0xd1c) | Gigabyte 1458:4011 | free to choose (identity-profile.md) |
| Class | 0x030000 VGA | same | same |
| Other functions | 01:00.1 HDA 10de:22eb | HDA, USB, UCSI on TU106 | device models function 0 only |
| Header, command | `Control: I/O+ Mem+ BusMaster+ ... DisINTx+` | same | |
| Capabilities (standard) | PM v3 @0x40, MSI @0x48 (1/16, maskable, 64-bit, disabled), PCIe v2 Legacy Endpoint @0x60, vendor-specific @0x9c, **MSI-X @0xb0 (9 vectors, enabled, table BAR0+0xB90000, PBA BAR0+0xBA0000)** | PM @0x60, MSI @0x68 (1/1), PCIe @0x78, no MSI-X | see "MSI-X" below |
| Link | LnkCap 32 GT/s x16, running 2.5 GT/s x8 (idle, slot is x8) | 8 GT/s x16 | the guest reads LnkCap for `GspSystemInfo.pcieConfigReg.linkCap` |
| Extended capabilities | Secondary PCIe @0x100, LTR @0x12c, Physical Resizable BAR @0x134, **Virtual Resizable BAR @0x140**, Data Link Feature @0x14c, PL 16 GT/s @0x158, PL 32 GT/s (0x2a) @0x188, AER @0x1b8, Lane Margining @0x200, ARI @0x248, **SR-IOV @0x250 (1 VF, VF id 2d04, VF BARs 256 KiB/256 MiB/32 MiB)**, VSEC ID 1 @0x2a4, Power Budgeting @0x2bc, Device Serial Number @0x2f4 | VC @0x100, Power Budgeting, LTR, L1 PM Substates, AER @0x420, VSEC @0x600, Sec PCIe @0x900, Resizable BAR @0xbb0 | drop SR-IOV, ARI, DSN and the VF-BAR capability; do not copy the serial number |
| Reset | `reset_method: flr bus`, FLReset+ | | |
| MSI in use on the host | `msi_irqs: 8 vectors` (MSI-X) | MSI | |

Sources: `gpu/lspci-vvv.txt:3-93`, `gpu/config-space.txt` (4096 bytes), `gpu/sysfs-attributes.txt:6-69`,
`gpu/lspci-functions.txt:19-29`; TU106 `rtx2070/lspci-vvv.txt:2-86`.

**MSI-X.** A 610 guest uses MSI-X whenever the capability is present and `rm_is_msix_allowed`
agrees (`kernel-open/nvidia/nv.c:1436`); for Blackwell outside the AD100/GA100 check that is always
true (`src/nvidia/src/kernel/gpu_mgr/gpu_mgr.c:4950-4966`). Today `ConfigSpace::from_dump` finds
PM, MSI and PCIe only (versatility.md "How the pieces meet"). Either the GB206 config space drops
the MSI-X capability (the guest then uses MSI, as on TU106) or the device implements MSI-X over
vfio-user. Dropping is the smaller step for the N gate.

### 1.3 BARs

| BAR | GB206 | TU106 | Source |
|---|---|---|---|
| BAR0 | 64 MiB, 32-bit, non-prefetchable | 16 MiB | `gpu/bars.txt:3,14` |
| BAR1 | 16 GiB, 64-bit prefetchable, resizable 64 MiB to 16 GiB (`resource1_resize` 0x7fc0, bits 6 to 14) | 256 MiB | `gpu/bars.txt:4,12,15,20` |
| BAR3 | 32 MiB, 64-bit prefetchable | 32 MiB | `gpu/bars.txt:5,16` |
| BAR5 | I/O 128 bytes | same | `gpu/bars.txt:6,17` |
| ROM | 512 KiB | 512 KiB | `gpu/bars.txt:18` |
| VF BARs | 256 KiB, 256 MiB (resizable up to 8 EB in the capability), 32 MiB | none | `gpu/bars.txt:7-10,21-25` |
| RM view | `BUS_GET_PCI_BAR_INFO`: 64 MiB, 16384 MiB, 32 MiB (three BARs; bar[3] empty) | | `gb206/rm-tools/host-facts.txt:4-8` |

The firmware first placed BAR1 at 0xf800000000 and the kernel moved it to 0x1400000000
(`gb206/host/kernel-log.txt:64,271`); `NVreg_EnableResizableBar` is 0
(`gb206/host/proc-driver-nvidia/params.txt:29`), so the 16 GiB is the board's firmware default.
For the synthetic device a 16 GiB BAR1 is not required; the guest takes the BAR1 size from the PCI
BAR (it arrives in `GspStaticConfigInfo`/BAR info answers the device writes itself), so a 256 MiB
BAR1 without the resizable-BAR capability, as on TU106, is the conservative first choice.

### 1.4 ROM chain, BIT, VBIOS version

| Item | GB206 | TU106 | Source |
|---|---|---|---|
| File | 162304 bytes, sha256 64bfafc4...6231; not in git | 130048 bytes of PCI images | `gpu/rom-walk.txt:2`, `gb206/MANIFEST.txt:30` |
| Image 0 | 0x0, 55AA, PCIR vendor 10de device 2d04, code type 0 (x86), 64000 bytes, NPDE rev 0x101 | x86, 60416 bytes, 10de:1f02 | `gpu/rom-walk.txt:3` |
| Image 1 | 0xfa00, EFI GOP (code type 3), 98304 bytes, **PCIR vendor 0000 device 0000**, PCI last-image set, NPDE rev 0x100 | EFI, 69632 bytes, PCIR 10de:1f02 | `gpu/rom-walk.txt:4` |
| Chain | PCI view ends at 0x27a00; NVIDIA view continues past the dump | same pattern | `gpu/rom-walk.txt:5` |
| No IFR | no `NVGI` in the file (checked on the file, 2026-09-24) | | |
| BIT | header at 0xdf0, version 0x0100, 20 tokens: 2 B C D I M N P S T U V x d p u k i E s | TU106 BIT at the same place in image 0 | `gpu/rom-walk.txt:6-7` |
| VBIOS version | 98.06.1f.40.4a (BIT BIOSDATA) = `Video BIOS` 98.06.1f.40.4a = nvidia-smi 98.06.1F.40.4A; OEM string `RTX5060TI VB Ver 98.06.1F.40.AS11` at 0xce0 | 90.06.3d.00.76 | `gpu/rom-walk.txt:8`, `gpu/proc-information.txt:6`, `gpu/nvidia-smi-q.txt:34` |
| Falcon data | BIT FALCON_DATA v2 points to 0x447de, in the NVIDIA extension (FWSEC) image not in the dump | 0xed24 | `gpu/rom-walk.txt:9` |
| InfoROM | G005.0000.98.01, OEM object 2.1 | | `gpu/nvidia-smi-q.txt:48-51` |

Two new points against TU106: the EFI image's PCIR carries no vendor/device (a GOP image that
does not bind by id), and the BIT is the same token set plus `k`, `i`, `E`, `s`. For the guest
this does not matter: from GH100 on `kgspExtractVbiosFromRom` is a stub
(`src/nvidia/generated/g_kernel_gsp_nvoc.c:1388-1402`), the kernel RM never reads the ROM to boot,
and the ROM layout (PCI ROM header, PCIR, NPDE, BIT, falcon data token, FWSEC image type 0xE0) of
the nova-core documentation (https://docs.kernel.org/gpu/nova/core/vbios.html) is needed only if
the nova-core path of [nova.md](nova.md) is extended to Blackwell. The ROM BAR can serve the two
captured images unchanged; `vbiosRevision`/`vbiosOemRevision` come from the BIOSDATA version, as
lab-blackwell.md §7 says.

### 1.5 Driver, firmware, GSP family

| Item | Value | Source |
|---|---|---|
| Driver | NVIDIA UNIX Open Kernel Module 615.71.09, built 2026-09-04, Dual MIT/GPL | `gb206/host/proc-driver-nvidia/version.txt:3`, `gb206/host/modinfo-nvidia.txt:9-11` |
| Firmware named by the module | `ucodes_tu10x.bin`, `ucodes_ga10x.bin`, `gsp_tu10x.bin`, `gsp_ga10x.bin` | `gb206/host/modinfo-nvidia.txt:12-15` |
| Files in `/usr/lib/firmware/nvidia/615.71.09` | gsp_ga10x.bin 27043992 bytes (c420726d...), gsp_tu10x.bin 22897536 (78e0a2c8...), ucodes_ga10x.bin 58215936 (6d56802a...), ucodes_tu10x.bin 8837632 (36aa927e...) | `gb206/host/firmware-files.txt:7-10` |
| linux-firmware chip directories | present for gb202/gb203/gb205/gb206/gb207 among others (contents not listed) | `gb206/host/firmware-files.txt:3-4` |
| GSP family | `family=unknown`: the script derives the family from the chip name, which was `10de-2d04` (1.1); the right answer is ga10x (`gsp_ga10x.bin`, LIBOS3), the same mapping as 610: `NV_FIRMWARE_CHIP_FAMILY_GB20X` falls through to `gsp_ga10x` (`src/nvidia/arch/nvalloc/common/inc/nv-firmware.h:97-104`) | `gb206/host/gsp-firmware.txt:2-7` |
| GSP in use | GSP firmware 615.71.09, `gsp.mode.current: Enabled` | `gpu/nvidia-smi-q.txt:67`, `gpu/nvidia-smi-query-gpu.txt:208` |
| Addressing, DMA | HMM; DMA size 52 bits (TU106: 47) | `gpu/nvidia-smi-q.txt:21`, `gpu/proc-information.txt:8-9`, `rtx2070/proc-information.txt:7-8` |
| Compute capability | 12.0 | `gpu/nvidia-smi-query-gpu.txt:67` |

`ucodes_*.bin` is a firmware file 610 already declares (`nv-firmware.h:106, 138-141`); the device does
not serve firmware, so only the GSP family matters, and for a 610 guest on GB206 that is the file
it already knows.

### 1.6 Engines, fault ids, FIFO (rm-tools/fifo-host-query.txt)

All tool output is "ABI UNVERIFIED" for 615.71.09 (`gb206/rm-tools/abi-check.txt:3`); the home
check shows two changed parameter sizes, `GPU_GET_PES_INFO` 56 to 76 and
`FIFO_GET_DEVICE_INFO_TABLE` 3212 to 3340 bytes (`gb206/rm-tools/abi-check-home.txt:4-8`); everything
else holds.

| Engine | NV2080 type | MMU fault id | Runqueue partners | PCE mask | Source line |
|---|---|---:|---|---|---|
| GR0 | 0x1 | 384 | rq0: COPY0 | | :5 |
| COPY0 | 0x9 | 65 | rq0: GR0 (GRCE) | 0x2 | :6 |
| COPY4 | 0xd | 69 | none | 0x1 | :7 |
| COPY7 | 0x10 | 72 | none | 0x2 | :8 |
| NVDEC0 | 0x13 | 28 | none | | :9 |
| NVENC0 | 0x1b | 44 | none | | :10 |
| SW | 0x22 | none (0xFFFFFFFF) | rq0: OFA0 | | :11 |
| SEC2 | 0x26 | 6 | none | | :12 |
| NVJPG0 | 0x2b | 36 | none | | :13 |
| OFA0 | 0x33 | 48 | none | | :14 |

Against TU106 ([fifo-device-info.md](fifo-device-info.md), `chips/tu106/device-info.toml`): GR's
fault id moves from 64 to 384, the copy engines are sparse (CE0, CE4, CE7 instead of CE0 to CE4),
there is one NVDEC instead of three, and NVJPG and OFA are new. The `rq1` partner query is refused
everywhere with 0x1F (`:5-14`), as lab-blackwell.md §7 expected for the Turing class it names.

Further lines: CE_GET_ALL_CAPS present=0x91 (CE0 caps0 0xe3 = GRCE, SHARED, sysmem, P2P; CE4 0xe4;
CE7 0xea = SHARED) (`:18-21`, decoded in `gb206/rm-tools/host-facts.txt:33-36`); FIFO_GET_INFO:
16384 channel groups, 128 channels per group, 64 subcontexts per group, timeslice 1024 us,
per-runlist CHRAM 1, 8 lower subcontexts (`:22`); 12 constructed falcons with register bases
(`:23-35`): FBFLCN 0x9a4000, DIP 0x651000, FECS 0x40c000, GPCCS 0x42f000, BSP 0x848000, PMU 0x8f4000,
MSENC 0x1c8000, SEC2 0x840000 (context buffer 64 KiB), NVJPG 0x9c0000, FSP (base 0xffffffff, not
CPU visible), OFA 0x844000 and one class 0x4f32fd with base 0 that is in neither open tree (the
other eleven class ids are the same NVOC ids in 610 and 615, `src/nvidia/generated/g_eng_desc_nvoc.h`).
Refused: GPU_GET_ENGINE_RUNLIST_PRI_BASE and CE_GET_FAULT_METHOD_BUFFER_SIZE with 0x1B
(`NV_ERR_INSUFFICIENT_PERMISSIONS`, `:37,39`), FIFO_GET_DEVICE_INFO_TABLE with 0x1F (`:38`).

**Correction to lab-blackwell.md §5.** It says the RM refuses the two resized controls with
`NV_ERR_INVALID_PARAM_STRUCT`. The capture shows status 0x1F, which is `NV_ERR_INVALID_ARGUMENT`
(`src/common/sdk/nvidia/inc/nvstatuscodes.h:60`; `INVALID_PARAM_STRUCT` is 0x3A, `:87`), also for
PES_INFO (`gb206/rm-tools/host-facts.txt:30-32`). The conclusion (refused, never misread) holds.

### 1.7 Architecture, classes, names (rm-tools/lab-rm-query.txt)

- MC_GET_ARCH_INFO: architecture 0x1B0 (GB20x), implementation 0x6, revision 0xA1, sub-revision 0,
  chip id 0x1B6 (`:3`).
- BUS_GET_PCI_INFO: 0x2d0410de, subsystem 0x8a081043, revision 0xa1, ext device 0x2d04 (`:4`).
- Names: "NVIDIA GeForce RTX 5060 Ti", short name "GB206-A" (`:5-6`); nvidia-smi adds GPU part
  number 2D04-300-A1, board id 0x100 (`gpu/nvidia-smi-q.txt:36-38`).
- 122 classes (`:7-130`). Named from `615:src/nvidia/generated/g_allclasses.h`, the device-relevant ones:

| Role | GB206 class | TU106 counterpart |
|---|---|---|
| Host channel | BLACKWELL_CHANNEL_GPFIFO_B 0xCA6F (`:110`), _A 0xC96F (`:109`), plus the older C36F/C46F/C56F/C86F | TURING_CHANNEL_GPFIFO_A 0xC46F |
| Usermode | BLACKWELL_USERMODE_A 0xC761 (`:104`) | TURING_USERMODE_A 0xC461 |
| Copy | BLACKWELL_DMA_COPY_B 0xCAB5 (`:118`) | TURING_DMA_COPY_A 0xC5B5 |
| 3D | BLACKWELL_B 0xCE97 (`:124`) | TURING_A 0xC597 |
| Compute | BLACKWELL_COMPUTE_B 0xCEC0 (`:125`) | TURING_COMPUTE_A 0xC5C0 |
| Inline to memory | BLACKWELL_INLINE_TO_MEMORY_A 0xCD40 (`:122`) | |
| Video | NVDEC 0xCFB0, NVENC 0xCFB7, NVJPG 0xCFD1, OFA 0xCFFA (`:126-129`) | |
| Display | NVCA70_DISPLAY, NVCA71_DISP_SF_USER, NVCA73_DISP_CAPABILITIES, cursor 0xCA7A, window-imm 0xCA7B, core NVCA7D, window NVCA7E (`:111-117`) | NVC570/C57D (display v04) |
| Other | NV_CONFIDENTIAL_COMPUTE 0xCB33, NV_COUNTER_COLLECTION_UNIT 0xCBCA, HOPPER_SEC2_WORK_LAUNCH_A 0xCBA2, MMU_VIDMEM_ACCESS_BIT_BUFFER 0xC763/0xC863, the Ampere SMC classes | |

All of these exist in 610.57.04 except NV_OPERATIONAL_EVENT_BUFFER_BIND 0x90D0 (`:74`) and
WPPS_CONFIG_SESSION 0xD0CD (`:130`), which are new in 615 and irrelevant to the device.

### 1.8 FB and GR facts (rm-tools/host-facts.txt)

| Fact | GB206 | Source |
|---|---|---|
| RAM size / total | 16702464 KiB = 16311 MiB | `:11-12`; nvidia-smi 16311 MiB total, 370 MiB reserved (`gpu/nvidia-smi-q.txt:120-121`) |
| Heap | 16323712 KiB (15941 MiB), largest free region 15923 MiB at 18 MiB | `:13,17-18` |
| Usable RAM | 16454080 KiB (16068 MiB) | `:23` |
| FB tax, heap base, offline | 0 | `:15-16,24` |
| BAR1 | 16 GiB, 16774528 KiB available | `:10,20,22` |
| Suspend/resume reserve | 512 MiB | `:25` |
| FB regions | one region 0x0 to 0x3E451FFFF (15941 MiB), performance 4, compressed, ISO, not protected | `:26-27` |
| GR | GPC mask 0x7, 18 graphics TPCs, per GPC TPC mask 0x3f (6 TPCs), ZCULL mask 0xf; PES info refused | `:28-32` |
| Memory type | not in the capture (GDDR7 from the product) | |

### 1.9 Interrupts (rm-tools/host-facts.txt H7)

MC_GET_STATIC_INTR_TABLE (`:37-53`): NON_REPLAYABLE_FAULT 0x84, NON_REPLAYABLE_FAULT_ERROR 0x85,
INFO_FAULT 0x86, REPLAYABLE_FAULT 0x40, REPLAYABLE_FAULT_ERROR 0x83, ACCESS_CNTR 0x48, TMR 0x94,
CPU_DOORBELL 0x81 (type numbers from `src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080mc.h:295-303`),
the eight FECS-log types unused. TU106 uses 64/65/195/194 for replayable, access counter,
non-replayable and timer, so the vector layout is chip data, as D14 assumed.
MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS is not supported (0x56, `:54`). Subtree map (`:55-62`):
ESCHED_DRIVEN_ENGINE 0x8, ESCHED_NOTIFICATION 0x1, UVM_OWNED 0x2, UVM_SHARED 0x4 as on TU106, plus
**RUNLIST 0x10 and RUNLIST_NOTIFICATION 0x20** (new subtrees 4 and 5).

### 1.10 Display

Four DRM connectors on card0, all disconnected, no modes, no EDIDs: DP-2, DP-3, DP-4 and HDMI-A-1
(`gpu/display/connectors.txt:22-30`); nvidia_drm modeset and fbdev on
(`gpu/display/connectors.txt:2-5`); nvidia-smi "Display Attached: No", "Display Active: Disabled"
(`gpu/nvidia-smi-q.txt:18-19`); the kernel reports "Cannot find any crtc or sizes" for the card
(`gb206/host/kernel-log.txt:416`); the console is on the ASPEED BMC (`:393`). Xorg still holds 4 MiB
on the card (`gpu/nvidia-smi-q.txt:279-282`). The display IP (v05, NVCA7D) is visible only through
the class list (1.7).

### 1.11 Kernel log

538 of 2312 lines match, no Xid (`gb206/host/kernel-log.txt:1-4`, `gb206/MANIFEST.txt:16`). The
card's lines: PCI enumeration with the three VF BARs, VF BAR 2/4 first unassignable then assigned
(`:62-76, 226-230, 271-274`), "limited by 32.0 GT/s PCIe x8 link at 0000:00:01.1" (`:76`), boot
VGA (`:218-220`), nvidia loaded at 2.678 s, "NVRM: loading ... 615.71.09" at 3.889 s, nvidia-modeset
at 4.048 s, nvidia-drm initialized at 4.086 s (`:395-416`), and "Using 52-bit DMA addresses" at
2653 s (`:460`), the first full GPU initialisation (`GpuInitOnProbe: 2`, auto,
`gb206/host/proc-driver-nvidia/params.txt:41`). **There is not one GSP, FSP or WPR line**: at the
default debug level the driver boots GSP silently, so the capture gives no reference sequence for
the new boot model; the source has to (section 2.2).

### 1.12 Registry and parameters

Registry empty (`gb206/host/proc-driver-nvidia/registry.txt:3`,
`gb206/host/proc-driver-nvidia/gpus__0000:01:00.0__registry.txt:3`); parameters at their defaults:
ResmanDebugLevel 0xFFFFFFFF, EnableMSI 1, EnableResizableBar 0, EnableGpuFirmware 18,
EnableGpuFirmwareLogs 2, OpenRmEnableUnsupportedGpus 1, DynamicPowerManagement 3,
PreserveVideoMemoryAllocations 1, GpuInitOnProbe 2 (`gb206/host/proc-driver-nvidia/params.txt:3-49`,
`gb206/host/sys-module-nvidia.txt:8-59`). Every parameter also exists in 610.57.04 except
`RegistryBinaryFilePerDevice` (checked against `kernel-open/nvidia/nv-reg.h`). Power: runtime D3
disabled by default, video memory off supported, no S0ix (`gb206/host/proc-driver-nvidia/gpus__0000:01:00.0__power.txt:3-15`).
Persistence mode on (`gpu/nvidia-smi-q.txt:20`); power limit 180 W (150 to 234 W), max clocks
3135/14001 MHz (`gpu/nvidia-smi-q.txt:209-218, 248-252`).

### 1.13 Identifiers in the capture

GPU UUID, GPU PDI and the PCIe Device Serial Number of the physical card are in the capture
(`gpu/proc-information.txt:5`, `gpu/nvidia-smi-q.txt:31-32`, `gpu/lspci-vvv.txt:91`). The synthetic
identity must not reuse them: the GID stays the Leandro GID (identity-profile.md) and the DSN
capability is dropped (1.2).

## 2. The D14 identity work list for a Blackwell device

### 2.1 What the capture fills now (drafts in `crates/vfio-user-nvgpu/chips/gb206/`)

The drafts use the TOML format of `chips/tu106/`, say in their header comment that they are drafts
from this capture and cite the capture file for every value. Nothing loads them: `src/chip` knows
only tu106, and required keys are still open (marked `OPEN(...)` with where the value comes from).

| File | Filled from the capture | Still open |
|---|---|---|
| `chip.toml` | identity (BOOT_0 0x1B6000A1 and BOOT_42 0x1B6A1000 composed from MC_GET_ARCH_INFO, `nv_ref.h:97-150`), short name, engine caps (10 engines), MIG off, channel classes CA6F/CAB5/C076/CEC0, memsys RAM type GDDR7 (product fact) | `[bar0]`, `[registers]`, `[chip_info]`, L2 and LTC counts, FIFO channel count and fault-method buffer, GMMU format and fault buffer, decompression LCE, `per_subctx_header` |
| `device-info.toml` | engines, MMU fault ids, GRCE pairing | runlist, PBDMA and PBDMA-fault columns follow the TU106 rule, unverified for GB20x |
| `interrupts.toml` | fault, access-counter and timer vectors, subtree map with the two new RUNLIST subtrees | GSP, FIFO, engine and display vectors of the kernel table |
| `boot.toml` | (source facts, not capture) GSP falcon, queue registers, FSP EMEM queue and boot-complete scratch, in a proposed new schema | WPR2, FB layout, fuses, the NVDM formats |
| `regmap.toml` | trace names for the registers above and the MSI-X table/PBA in BAR0 | the rest of the Blackwell map |

A board file is outside `chips/` and is therefore not drafted as a file; its content follows from
section 1: `name = "RTX 5060 Ti"`, `chip = "gb206"`, `config = "caraxes/reference/gb206/gpu-0000_01_00.0/config-space.txt"`,
`rom = "caraxes/reference/gb206/gpu-0000_01_00.0/rom.bin"`, `vbios_version = "98.06.1F.40.4A"`,
bars mem32 64 MiB, mem64 256 MiB (or 16 GiB, 1.3), mem64 32 MiB, io 0x80, `rom_bar_size = 0x8_0000`,
and a `[display]` connector list (one DP SST, D15). The config dump needs capability filtering
(SR-IOV, ARI, DSN, VF resizable BAR, and MSI-X or not, 1.2) before `ConfigSpace::from_dump` can use it.

Names the drafts use that `src/ver/v610.rs` does not know yet (all present in 610.57.04):
NV2080 engine types NVJPEG0 0x2B and OFA0 0x33 (`src/common/sdk/nvidia/inc/class/cl2080_notification.h:330,339`);
RM engine types COPY7 0x10, NVJPEG0 0x36, OFA0 0x3E (`src/nvidia/inc/kernel/gpu/gpu_engine_type.h`);
MC indices CE7 22, NVJPEG0 (= NVJPG), OFA0 81 (`src/nvidia/inc/kernel/gpu/intr/engine_idx.h:61,92-93,125`);
ENG_DESC classes OBJNVJPG 0x2B3A54, OBJOFA 0xDD7BAB (`src/nvidia/generated/g_eng_desc_nvoc.h:825,865`);
RAM type GDDR7 0x15 (`ctrl2080fb.h:423`). RUNLIST and RUNLIST_NOTIFICATION are already in
`INTR_CATEGORIES`.

### 2.2 What needs the source tree (610.57.04 for the guest in use)

The register maps are chip facts and live in the published headers of both trees; for a 610 guest
the 610 HAL selection decides which are touched. Measured so far:

**Boot model (new `FirmwareBoot`).** On GB206 `kgspBootstrap` is `kgspBootstrap_GH100`
(`src/nvidia/generated/g_kernel_gsp_nvoc.c:731-748`), and GB206 has a KernelFsp
(`src/nvidia/generated/g_kern_fsp_nvoc.c:234`). The sequence the device must satisfy
(`src/nvidia/src/kernel/gpu/gsp/arch/hopper/kernel_gsp_gh100.c:853-995`):

1. FSP boot complete: `kfspWaitForSecureBoot_GB202` polls `NV_THERM_I2CS_SCRATCH` (0xAD00BC on
   GB202) for 0xFF (`src/nvidia/src/kernel/gpu/fsp/arch/blackwell/kern_fsp_gb202.c:48-100`,
   `published/blackwell/gb202/dev_therm.h:27`, `dev_therm_addendum.h:27-30`). This replaces the
   Turing `GFW_BOOT` poll.
2. COT boot command to FSP: `kfspSendBootCommands_GH100` builds an NVDM payload of type COT (0x14)
   with the GSP-FMC image address, FRTS sysmem/vidmem offsets and sizes and
   `gspBootArgsSysmemOffset` (the `GSP_FMC_BOOT_PARAMS`), and sends it over the FSP EMEM port and
   queue (`src/nvidia/src/kernel/gpu/fsp/arch/hopper/kern_fsp_gh100.c:1030-1046, 1341-1458, 1500-1530`;
   registers `NV_PFSP_EMEMC/EMEMD` 0x8F2AC0/0x8F2AC4, `QUEUE_HEAD/TAIL` 0x8F2C00/04,
   `MSGQ_HEAD/TAIL` 0x8F2C80/84, `published/hopper/gh100/dev_fsp_pri.h:26-57`, included for all
   chips at `kern_fsp_gh100.c:42`). FSP answers with an NVDM FSP_RESPONSE (0x15,
   `src/nvidia/arch/nvalloc/common/inc/nvdm_format.h`, `fsp/nvdm_payload_cmd_response.h`). The
   MNOC mailbox is used only with `PDB_PROP_KFSP_USE_MNOC_CPU` (`kern_fsp_gb100.c:311-330`).
   The device reads `GSP_FMC_BOOT_PARAMS` from guest memory, finds the WPR meta and the RMARGS
   through it and raises `BootEvent::WprMeta`: this is where the Hopper model of versatility.md
   meets FSP.
3. Target mask release: GSP `HWCFG2` reads non-zero and not 0xBADF41xx
   (`kern_fsp_gh100.c:1069-1100`).
4. Init RPCs are queued before GSP runs (`kernel_gsp_gh100.c:958`); the device must accept the
   first queue doorbell before it has the WPR meta, or take the queue location from RMARGS first.
5. Lockdown release: `HWCFG2.RISCV_BR_PRIV_LOCKDOWN` = unlock (`kernel_gsp_gh100.c:542, 968`,
   `published/hopper/gh100/dev_falcon_v4.h:79`), then `MAILBOX0` must read 0 (`:980-995`).
6. WPR2 up: `kgspIsWpr2Up_GB100` (`g_kernel_gsp_nvoc.c:1341`) reads `NV_HUBMMU_PRI_MMU_WPR2_ADDR_HI`,
   offset 0xA828 in the HUBMMU block (`published/blackwell/gb100/dev_hubmmu_base.h:91-94`); the
   block base for GB202 is open.

versatility.md's Blackwell checklist (BCR DMA address registers, `kernel_gsp_gh100.c:744-774`)
describes `_kgspBootstrapGspFmc_GH100`, the branch taken only without FSP and without SEC2 boot;
with the FSP present it is not reached. Estimated size of the FSP part alone: an EMEM port with
auto-increment, two ring pointers, an MCTP/NVDM packet parser for one message type and a
fixed response.

**Kernel-RM surface (`[bar0]`, `[registers]`, `[chip_info]`).** Every Turing offset of
`chips/tu106/chip.toml` has to be re-derived. Evidence that they move: the BAR0 window is
`NV_XAL_EP_ZB_BAR0_WINDOW` at 0xD40 of a ZB block (`published/blackwell/gb100/dev_nv_xal_ep_zb.h:40`),
the usermode window base is `DRF_BASE(NV_VIRTUAL_FUNCTION)` through `GPU_GET_VREG_OFFSET`
(`src/nvidia/src/kernel/gpu/arch/blackwell/kern_gpu_gb100.c:1804-1822`), the usermode doorbell is
at +0x30090 of that window (`published/blackwell/gb202/dev_vm.h:27`), and the PCI config mirror is
in `published/blackwell/gb202/dev_xtl_ep_pcfg_gpu.h`. The GSP falcon stays at 0x110000
(`published/hopper/gh100/dev_gsp.h:25`) and the GSP queue registers keep the TU102 layout,
0x110C00/0x110C80 (1.5 of section 3). Deliverable: a GB20x counterpart of
[kernel-rm-hw-surface.md](kernel-rm-hw-surface.md) from a boot trace of the guest against a
device that answers 0 everywhere (the E0/E1 method).

**GMMU.** Hopper and later use the 5-level, 57-bit format; `fb.rs` needs a second walker before
the guest's first BAR1/BAR2 page tables can be resolved (versatility.md "GMMU format",
`src/nvidia/src/kernel/mmu/arch/hopper`, `.../blackwell`). The 52-bit DMA size (1.5) bounds the
system-memory addresses the walker must accept.

**Channel and copy classes.** `channel.rs` decodes C46F and C5B5; CA6F and CAB5 need their method
subset checked (`src/common/sdk/nvidia/inc/class/clca6f.h`, `clcab5.h`).

**Physical-RM answers.** Everything the capture could not see because only physical RM knows it:
the kernel interrupt table's GSP/FIFO/engine vectors, the FIFO device-info runlists and PBDMAs,
INTERNAL_MEMSYS static config (L2, LTC), INTERNAL_GMMU static info, INTERNAL_STATIC_KGR_* for a
3x6-TPC GR, INTERNAL_CE_GET_PCE_CONFIG for three LCEs with PCE masks 0x2/0x1/0x2, the per-runlist
channel count, the display IP version and capabilities page (NVCA73). Source: the GB20x HALs
under `src/nvidia/src/kernel/gpu/*/arch/blackwell/` and the phase-5 inventory of a GB206 guest
boot (every `unanswered` is a row).

### 2.3 What needs a second capture

| Item | Why | How (read-only) |
|---|---|---|
| FIFO device-info table, PES info | refused on 615 because the tool uses 610 sizes | rebuild the three tools with 615 layouts (the fingerprint in `abi-check-home.txt` lists exactly these two) |
| L2 size, LTC/LTS counts, RAM type, bus width | not in nvidia-smi -q | add FB_GET_INFO_V2 indices L2_CACHE_SIZE, LTC_COUNT, LTS_COUNT, RAM_TYPE, BUS_WIDTH to host-facts H3 |
| EDIDs | 0 EDIDs, all connectors disconnected (1.10) | plug a monitor; only for the display model |
| FWSEC image behind the BAR0 PROM window | not needed by a 610/615 guest on GB20x (ROM not parsed); needed only for nova-core | needs a BAR0 register read, which the kit forbids (lab-blackwell.md §6); operator decision |
| linux-firmware `gb206/` file list | `firmware-files.txt:3` shows the directory, not its files | list `/lib/firmware/nvidia/gb206/**` with sizes and sums (nova-core) |
| pci.ids name | the run named the chip `10de-2d04` | map 2d04 in `chip_of` or update pci.ids |
| Class 0x4f32fd | constructed falcon not in the open tree (1.6) | nothing; ask whether it matters |

A verbose kernel log of the GSP boot would help the boot model, but it needs a module parameter
change, which the kit forbids; the source sequence of 2.2 replaces it.

### 2.4 What the guest driver 610.57.04 does with a GB206

It supports it: the README device table lists "NVIDIA GeForce RTX 5060 Ti 2D04" (`README.md:1003`),
the kernel name table has `{ 0x2D04, ..., "NVIDIA GeForce RTX 5060 Ti" }`
(`src/nvidia/generated/g_nv_name_released.h:839`), `HAL_IMPL_GB206` maps to
`GPU_IMPLEMENTATION_GB206` (`src/nvidia/src/kernel/gpu/gpu.c:4922-4925`), and the GB20x HAL files
are in the tree (`src/nvidia/src/kernel/gpu/*/arch/blackwell/*gb202.c`). The firmware is
`gsp_ga10x.bin` (1.5). So the minimum driver version question does not block anything: every SDK
in `target/abi-headers/` from 570.211.01 on names GB206
(`target/abi-headers/570.211.01/src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080mc.h:143`); the
first release that shipped 2D04 cannot be determined from the trees here.

For D14 this means: the first Blackwell identity is a chip axis addition (`chips/gb206`,
`src/chip/gb206.rs`, a new `FirmwareBoot`, a GMMU walker) on the existing `v610` version axis.
A 615.71.09 guest is a separate, larger step (section 3).

## 3. gsp-abi-sizes for 615.71.09 (the item the lab run skipped)

### 3.1 How it was produced

`git clone --depth 1 --branch 615.71.09 https://github.com/NVIDIA/open-gpu-kernel-modules.git
/mnt/vmstore/caraxes/ogkm-615.71.09` (commit 61dcc93). `caraxes/tools/gsp-abi-sizes.sh` against it,
as `lab-capture.sh:726-741` would have run it, **fails to compile**: 24 distinct error messages, every one a
field or type that 615 removed or renamed (msgq headers, queue element, encryption tag,
`rpc_gsp_set_system_info_v17_00`, ECID info, GSP arguments, SR init arguments, WPR meta padding)
plus one missing nvport module define. That is the tool doing its job ("a field that disappears
... makes this file fail to compile", `caraxes/tools/gsp-abi-sizes.c:15-20`). The lab-capture
script would have recorded it as `failed`.

A copy of the tool adapted to the 615 field names (patch in appendix B, applied to a scratch copy;
`caraxes/tools/` is unchanged) compiles, and the minimal and driver flag sets give identical output
(gcc 16.2.1). The clang leg of `--check` fails in nvport's C11 atomics with
`-DPORT_MODULE_time=1`, a header issue, not a layout one. Full output: appendix A.

### 3.2 Sizes, 610.57.04 against 615.71.09

| Structure | 610.57.04 | 615.71.09 | What changed |
|---|---:|---:|---|
| `msgqTxHeader` | 32 | 32 | new layout: `versionMajor`/`versionMinor` u16, size, msgSize, msgCount, entryOff, reserved[3]; **no writePtr, flags, rxHdrOff** |
| `msgqRxHeader` | 4 | gone | the read pointer moved to a register |
| `GSP_MSG_QUEUE_ELEMENT` header | 16 | 24 (40 with CC) | `mctpMagic` "MCTP", `mctpPayloadSize`, `mctpHeader`, `nvdmHeader`, `nvdmPayloadSize`, reserved; **no checkSum, seqNum** |
| `GSP_MSG_QUEUE_ENCRYPTION_TAG` | 40 | 16 | auth tag only |
| `GMCAPI_HEADER` | none | 40 | new: command, size, sequence u64, max_response_size/status, reserved[5], data |
| `rpc_message_header_v` | 32 | 32 | unchanged |
| `rpc_gsp_rm_control_v03_00`, `rpc_gsp_rm_alloc_v03_00`, `rpc_free_v03_00`, `rpc_dup_object_v03_00` | 40, 32, 16, 28 | same | unchanged |
| `rpc_set_guest_system_info_v03_00`, `_ext_v25_1B` | 792, 268 | same | unchanged |
| `rpc_os_error_log_v17_00`, `rpc_post_event_v17_00`, `rpc_ucode_libos_print_v1E_08`, `rpc_gsp_lockdown_notice_v17_00`, `rpc_init_done_v17_00` | 272, 32, 8, 1, 1 | same | unchanged |
| `rpc_gsp_set_system_info_v17_00` | 4 | gone | GSP_SET_SYSTEM_INFO deprecated (`615:src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:82`) |
| `GspStaticConfigInfo` | 1600 | 1608 | no longer on the wire (3.3); one byte added before `bIsGc6Rtd3Allowed`, ECID info now `info[2]` (16 bytes) |
| `GspSystemInfo` | 952 | 952 | no longer on the wire (3.3) |
| `PACKED_REGISTRY_TABLE`/`_ENTRY` | 8/16 | 8/16 | now inside the NVKV system info |
| `GSP_ARGUMENTS_CACHED` | 152 | 184 | new header magic "GSP ", size, flags u64 (IN_PM_TRANSITION, STACK_IN_DMEM, recovery margin); `bDmemStack` gone; `vbiosOverrideArgs` added; everything after it shifts by 16 to 32 bytes |
| `MESSAGE_QUEUE_INIT_ARGUMENTS` | 64 | 64 | new `flags` u64 first (ENCRYPTION); the size fields shrink from u64 to u32; `pageTableEntryCount` moves from 8 to 52 |
| `GSP_SR_INIT_ARGUMENTS` | 12 | 16 | `bInPMTransition` replaced by reserved[8] (the flag moved to `GSP_ARGUMENTS_CACHED.flags`) |
| `LibosMemoryRegionInitArgument` | 32 | 32 | unchanged |
| `GspFwWprMeta` | 256 | 256 | `padding[2]` becomes `pagingConfig` (`615:src/nvidia/arch/nvalloc/common/inc/gsp/gsp_fw_wpr_meta.h:196`); revision still 1 |
| `GSP_FMC_INIT_PARAMS` | 4 | 8 | reserved word added |
| `GSP_FMC_BOOT_PARAMS` | 88 | 256 | new header magic "FMC ", size; every member moved (initParams 0 to 32, bootGspRmParams 8 to 64, gspRmParams 40 to 128, gspSpdmParams 56 to 160, gspRmMemParams 80 to 192) |

| Constant | 610.57.04 | 615.71.09 |
|---|---:|---|
| queueElementHdrSize (no CC / CC) | 16 / 56 | 24 / 40 |
| maxRpcSize | 65520 | 65512 |
| msgq entryOff for a 0x40000 queue | 4096 | 4096 |
| MSGQ_VERSION | 0 | major 2, minor 0 |
| VGX version | 0x2E.0x0D | 0x2F.0x06 |
| RPC function numbers | 0 to 229 | unchanged, plus 230, 231 (`CTRL_FIFO_VGPU_SWRUNLIST_*`) |
| Event numbers | to 0x1029 | unchanged, plus 0x102A to 0x102D |

The structures marked `NV_ABI_STABLE` in 615 (`615:src/common/sdk/nvidia/inc/nvtypes.h:520-532`:
a no-op tag that a checker may enforce across releases) are exactly the transport and boot ones:
`GSP_MSG_QUEUE_ELEMENT`, `msgqTxHeader`, `MESSAGE_QUEUE_INIT_ARGUMENTS`, `GSP_SR_INIT_ARGUMENTS`,
`GSP_ARGUMENTS_CACHED`, `GMCAPI_HEADER` and the `GSP_FMC_*` parameters. NVIDIA reset these layouts
in 615 and declares them stable from there on.

### 3.3 The protocol change behind the numbers

- **Queue pointers in registers.** 615's msgq keeps the write and read pointers in
  `NV_PGSP_QUEUE_HEAD/TAIL(0)` (command queue) and `NV_PGSP_MSGQ_HEAD/TAIL(0)` (status queue)
  (`615:src/nvidia/src/libraries/msgq/msgq.c:120-160`, `615:src/nvidia/inc/libraries/msgq/msgq_priv.h:82-104`,
  `615:src/nvidia/src/kernel/gpu/gsp/message_queue_cpu.c:171-183, 370-380`,
  `615:src/nvidia/src/kernel/gpu/arch/turing/kern_gpu_tu102.c:597-605`); the HAL is the same for
  TU10x through GB20x. On 610 the pointers are in the shared queue headers and the queue-head
  register is only a doorbell. `msgq.rs` (hard-coded, versatility.md "What is still hard-coded")
  has to become version-bound.
- **Element framing.** MCTP magic and payload size, then NVDM header and payload size; the
  payload is either a vGPU-style RPC (`NVDM_TYPE_RM_RPC` 0x25) or a GMCAPI message
  (`NVDM_TYPE_GMCAPI` 0x26, `615:src/nvidia/arch/nvalloc/common/inc/nvdm_format.h:62-63`); no
  checksum, no sequence number (`615:src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h:52-121`).
- **Init.** `kgspSendInitRpcs` sends one `GMCAPI_CMD_GSP_INIT` (0x10001) whose request is the
  NVKV-encoded system info including the registry, and whose response is the NVKV-encoded static
  info (`615:src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:4960-5345, 7864-7885`). NVKV is a 64-bit
  word stream: 16-bit key, 12-bit index, 4-bit opcode, 32-bit immediate or count
  (`615:src/nvidia/inc/libraries/nvkv/nvkv.h:36-100`). Keys: 112 `NVGMC_SI_*`
  (`615:src/nvidia/interface/gmcapi/gmcapi_system_info.h`), 80 `NVGMC_SC_*`
  (`615:src/nvidia/interface/gmcapi/gmcapi_gsp_config.h`), for example `NVGMC_SC_GPU_NAME_STRING`
  0x2000, `NVGMC_SC_FB_LENGTH` 0x1010, `NVGMC_SC_ENGINE_MASK` 0x1100, `NVGMC_SC_VBIOS_REVISION`
  0x52, `NVGMC_SC_CHIP_SUB_REV` 0x4. `GSP_SET_SYSTEM_INFO` and `SET_REGISTRY` are marked deprecated
  (`615:.../rpc_global_enums.h:82-83`), nothing sends `GET_GSP_STATIC_INFO` any more (the
  `NV_RM_RPC_GET_GSP_STATIC_INFO` macro has no caller), and `GSP_INIT_DONE` is not waited for
  (no reference in `615:.../kernel_gsp.c`).
- **Controls and allocations** still travel as `rpc_gsp_rm_control_v03_00`/`rpc_gsp_rm_alloc_v03_00`,
  unchanged in layout. For the static identity this is good news: the name, FB length, engine
  mask and VBIOS revision become keys the device emits, independent of struct layout.

### 3.4 Controls, classes and GSP messages the device emulates: 610.57.04 against 615.71.09

Method: every control id in `crates/vfio-user-nvgpu/src/ver/v610.rs` mapped to its macro and
parameter struct in both SDKs, the struct sizes compiled against each tree (101 ids; 100 map to a macro, `CMD_BINAPI_0108` has none).

| Kind | Item | 610.57.04 | 615.71.09 | Effect on a `v615` module |
|---|---|---|---|---|
| GSP transport | msgq, queue element | shared-memory pointers, checksum | register pointers, MCTP framing | new `msgq` implementation (not generated) |
| GSP init | SET_GUEST_SYSTEM_INFO(_EXT) | struct RPC | unchanged | none |
| GSP init | GSP_SET_SYSTEM_INFO, SET_REGISTRY, GET_GSP_STATIC_INFO, GSP_INIT_DONE | struct RPCs | one GMCAPI_CMD_GSP_INIT with NVKV request/response | new NVKV encoder/decoder, key tables generated from `gmcapi_*.h` |
| GSP boot | GSP_ARGUMENTS_CACHED, MESSAGE_QUEUE_INIT_ARGUMENTS, GSP_SR_INIT_ARGUMENTS, GSP_FMC_BOOT_PARAMS, GspFwWprMeta | as measured | new layouts (3.2) | offsets regenerated (the measured-layout test of versatility.md check 2) |
| Control | `NV2080_CTRL_CMD_INTERNAL_GPU_GET_CHIP_INFO` 0x20800A36 | 88-byte params | **removed**; the id is reused for `INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO` (`615:src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080internal.h:585`); chip sub-revision now `NVGMC_SC_CHIP_SUB_REV` | the handler must be version-gated, or a 615 guest's fabric-probe call would be answered as chip info |
| Control | `FIFO_GET_DEVICE_INFO_TABLE` | 3212 | 3340 (17 data types instead of 16) | layout from `nvrm_sys::v615` |
| Control | `GPU_GET_PES_INFO` | 56 | 76 | same |
| Control | `GR_GET_GLOBAL_SM_ORDER` | 9240 | 10264 (`virtualDpcId` per SM) | same |
| Control | `INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER` | 73760 | 81952 | same |
| Control | `INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS` | 10688 | 10848 | same |
| Control | `INTERNAL_GET_PCIE_P2P_CAPS` | 3 | 4 | same |
| Control | `INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO` | 1088 | 1072 | same |
| Control | the other 92 mapped ids | | same id, same size | none |
| Classes | every class the device allocates or answers (root, device, subdevice, channels, copy, compute, UVM SW, vaspace, context share, usermode) | | same numbers | none; 615 adds only 0x90D0 and 0xD0CD |
| Enums | NV2080_ENGINE_TYPE | | unchanged (`cl2080_notification.h`) | none |

The D14 generated-ABI approach covers the control and boot-struct rows (bindings and measured
layouts). The first three rows are code: a `v615` build needs a second msgq transport and an NVKV
static-info writer next to the struct-based one, selected by the version module.

## 4. First Blackwell N gate: order, estimates, limits

Goal: a VM with a stock 610.57.04 guest driver boots on a GB206 identity (pure emulation, no
physical Blackwell card, no host forwarding), `nvidia-smi` shows "Leandro VFIO RTX 5060 Ti-4G".
Estimates are working days for one person familiar with the TU106 device.

| # | Step | Needs | Estimate |
|---|---|---|---|
| 1 | Board data: `boards/rtx5060ti.toml`, capability filter for the GB206 config dump (drop SR-IOV, ARI, DSN, VF resizable BAR; MSI-X dropped), BAR1 256 MiB, ROM BAR with the two captured images | capture (done) | 1 |
| 2 | Version tables: add the names of 2.1 to `src/ver/v610.rs`; `chips/gb206` loadable with stub values; `src/chip/gb206.rs` | 610 headers | 1 |
| 3 | E0 for GB206: guest probes, reads BOOT_0/BOOT_42, picks the GB206 HAL; trace every BAR0 access of the early boot against a zero-answering device | steps 1-2 | 1-2 |
| 4 | Kernel-RM surface for GB20x from that trace and the headers: BAR0 window (XAL ZB), PCFG mirror (XTL), VF window, usermode, interrupt tree, local memory range, WPR2 (HUBMMU), fuses; fill `[bar0]`, `[registers]`, `[chip_info]`, `boot.toml` | 610 source, trace | 4-6 |
| 5 | FSP and GSP-FMC boot model (`src/chip/hopper.rs`): boot-complete scratch, EMEM port and queues, NVDM COT decode and FSP response, target mask and lockdown release, WprMeta from `GSP_FMC_BOOT_PARAMS`; measure the NVDM/COT structs with a gsp-abi-sizes section | 610 source | 6-9 |
| 6 | Queue service ordering (init RPCs queued before lockdown release) and the GB206 static identity: name, short name, FB profile, engine caps, VBIOS 98.06.1F.40.4A, BAR info | steps 4-5 | 2 |
| 7 | GMMU v3 walker (5 levels, 57-bit VA, 52-bit PA) in `fb.rs` for BAR1/BAR2 and the guest's page tables | source | 4-6 |
| 8 | Phase-5 inventory for GB20x: device-info table, kernel interrupt table, memsys, GMMU static info, KGR static info for 3x6 TPCs, CE PCE config for CE0/4/7, FIFO counts, headless display (the GB20x equivalent of `NV_FUSE_STATUS_OPT_DISPLAY`) | source, the drafts, a second capture for L2/LTC/device-info | 6-10 |
| 9 | Gate: `nvidia-smi`, NVML list, UVM registration (no work submission) | all | 2 |
| | Total | | about 27 to 39 days |

What cannot be done without a Blackwell card in this house:

- **Execution.** The house GPU is the RTX 2070. A GB206 guest pushes Blackwell methods
  (CA6F/CAB5/CE97/CEC0) and writes GMMU v3 page tables; neither can be forwarded to a Turing host
  (D17 shadow route, host-exec). The N gate therefore stops at "driver up, no work"; anything past
  it (CE copies, compute, display scan-out through a real engine) needs a Blackwell host card or an
  SR-IOV VF (P18).
- **Validation of composed values.** BOOT_0/BOOT_42, the kernel-RM register surface, FSP
  timing and responses, and all physical-RM answers are composed from source, not read from a
  card; a mismatch shows up only as a guest failure. A passthrough trace like R1 on a Blackwell
  card would settle them.
- **Firmware images.** The FWSEC image (BAR0 PROM window) and GSP-FMC behaviour are not observable
  here; the 610 guest does not need them, nova-core would.
- **Real device-info, L2/LTC, EDIDs**: need the second capture of 2.3 (lab card, read-only).

## 5. Corrections to other notes

- versatility.md "Blackwell": the GB20x boot of a 610 guest goes through FSP
  (`kfspSendBootCommands`), the BCR register path is the FSP-less fallback (2.2); "610.57.04 does
  (support Blackwell), so no new ver module is required" holds.
- versatility.md "What is still hard-coded", `msgq.rs`: "no version so far moved it" is no longer
  true; 615.71.09 moves it (3.3).
- versatility.md "Adding a driver version (615.71.09)": the table assumes struct-level changes;
  615 changes the init protocol and the queue transport (3.4), and `gsp-abi-sizes.c` needs the
  field changes of appendix B before step 2 works.
- lab-blackwell.md §5: the resized controls are refused with 0x1F `NV_ERR_INVALID_ARGUMENT`, not
  `NV_ERR_INVALID_PARAM_STRUCT` (1.6).
- `caraxes/tools/lab-capture.sh`: `chip_of` depends on the host's pci.ids; the GB206 run is named
  `10de-2d04` and its GSP family `unknown` (1.1, 1.5). A device-id table for the chips of
  lab-blackwell.md would fix both.

## Sources

- The capture: `caraxes/reference/gb206/` (commit 22f3609), every file cited above.
- TU106 reference: `caraxes/reference/rtx2070/`, `crates/vfio-user-nvgpu/chips/tu106/*.toml`.
- `vendor/open-gpu-kernel-modules` at 610.57.04; the 615.71.09 clone at
  `/mnt/vmstore/caraxes/ogkm-615.71.09` (commit 61dcc93, not in the repository).
- nova-core VBIOS documentation: https://docs.kernel.org/gpu/nova/core/vbios.html

## Appendix A: gsp-abi-sizes output for 615.71.09 (generated)

Produced 2026-09-24 by the adapted tool of appendix B, minimal flag set, against the clone of 3.1. Identical with `--driver-flags`.

<!-- compiler: cc (GCC) 16.2.1; flags: minimal; tree: ogkm-615.71.09 (commit 61dcc93) -->
<!-- generated by caraxes/tools/gsp-abi-sizes.sh; driver 615.71.09 -->

#### Message queue transport (msgq library, one header per queue)

##### `msgqTxHeader`: sizeof 32 (0x20), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 2 | `versionMajor` |
| 2 | 0x0002 | 2 | `versionMinor` |
| 4 | 0x0004 | 4 | `size` |
| 8 | 0x0008 | 4 | `msgSize` |
| 12 | 0x000c | 4 | `msgCount` |
| 16 | 0x0010 | 4 | `entryOff` |
| 20 | 0x0014 | 12 | `reserved` |

#### Queue element and RPC header

##### `GSP_MSG_QUEUE_ELEMENT`: sizeof 40 (0x28), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `mctpMagic` |
| 4 | 0x0004 | 4 | `mctpPayloadSize` |
| 8 | 0x0008 | 4 | `mctpHeader` |
| 12 | 0x000c | 4 | `nvdmHeader` |
| 16 | 0x0010 | 4 | `noEncryption.nvdmPayloadSize` |
| 20 | 0x0014 | 4 | `noEncryption.reserved` |
| 24 | 0x0018 | flex | `noEncryption.payload` (flexible array) |
| 16 | 0x0010 | 16 | `withEncryption.encryptionTag` |
| 32 | 0x0020 | 4 | `withEncryption.nvdmPayloadSize` |
| 36 | 0x0024 | 4 | `withEncryption.reserved` |
| 40 | 0x0028 | flex | `withEncryption.payload` (flexible array) |

##### `GSP_MSG_QUEUE_ENCRYPTION_TAG`: sizeof 16 (0x10), align 1

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 16 | `authTagBuffer` |

##### `GMCAPI_HEADER`: sizeof 40 (0x28), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `command` |
| 4 | 0x0004 | 4 | `size` |
| 8 | 0x0008 | 8 | `sequence` |
| 16 | 0x0010 | 4 | `request.max_response_size` |
| 16 | 0x0010 | 4 | `response.status` |
| 20 | 0x0014 | 20 | `reserved` |
| 40 | 0x0028 | flex | `data` (flexible array) |

##### `rpc_message_header_v`: sizeof 32 (0x20), align 8

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

#### Init RPCs: CPU to GSP and replies

##### `rpc_set_guest_system_info_v03_00`: sizeof 792 (0x318), align 4

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

##### `rpc_set_guest_system_info_ext_v25_1B`: sizeof 268 (0x10c), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 256 | `guestDriverBranch` |
| 256 | 0x0100 | 4 | `domain` |
| 260 | 0x0104 | 2 | `bus` |
| 262 | 0x0106 | 2 | `device` |
| 264 | 0x0108 | 4 | `gridBuildCsp` |

##### `PACKED_REGISTRY_TABLE`: sizeof 8 (0x8), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `size` |
| 4 | 0x0004 | 4 | `numEntries` |
| 8 | 0x0008 | 0 | `entries` |

##### `PACKED_REGISTRY_ENTRY`: sizeof 16 (0x10), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `nameOffset` |
| 4 | 0x0004 | 1 | `type` |
| 8 | 0x0008 | 4 | `data` |
| 12 | 0x000c | 4 | `length` |

##### `rpc_get_gsp_static_info_v14_00`: sizeof 4 (0x4), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `data` |

##### `rpc_init_done_v17_00`: sizeof 1 (0x1), align 1

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 1 | `bIsD3Hot` |

#### RM API passthrough RPCs

##### `rpc_gsp_rm_alloc_v03_00`: sizeof 32 (0x20), align 4

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

##### `rpc_gsp_rm_control_v03_00`: sizeof 40 (0x28), align 8

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

##### `rpc_free_v03_00`: sizeof 16 (0x10), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 16 | `params` |
| 0 | 0x0000 | 4 | `params.hRoot` |
| 4 | 0x0004 | 4 | `params.hObjectParent` |
| 8 | 0x0008 | 4 | `params.hObjectOld` |
| 12 | 0x000c | 4 | `params.status` |

##### `rpc_dup_object_v03_00`: sizeof 28 (0x1c), align 4

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

#### Events GSP to CPU that the boot path accepts

##### `rpc_os_error_log_v17_00`: sizeof 272 (0x110), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `exceptType` |
| 4 | 0x0004 | 4 | `preemptiveRemovalPreviousXid` |
| 8 | 0x0008 | 4 | `runlistId` |
| 12 | 0x000c | 4 | `chid` |
| 16 | 0x0010 | 256 | `errString` |

##### `rpc_post_event_v17_00`: sizeof 32 (0x20), align 4

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

##### `rpc_ucode_libos_print_v1E_08`: sizeof 8 (0x8), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `ucodeEngDesc` |
| 4 | 0x0004 | 4 | `libosPrintBufSize` |
| 8 | 0x0008 | flex | `libosPrintBuf` (flexible array) |

##### `rpc_gsp_lockdown_notice_v17_00`: sizeof 1 (0x1), align 1

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 1 | `bLockdownEngaging` |

#### GET_GSP_STATIC_INFO reply payload

##### `GspStaticConfigInfo`: sizeof 1608 (0x648), align 8

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
| 1490 | 0x05d2 | 1 | `bIsGc6Rtd3Allowed` |
| 1491 | 0x05d3 | 1 | `bIsGc8Rtd3Allowed` |
| 1492 | 0x05d4 | 1 | `bIsGcOffRtd3Allowed` |
| 1493 | 0x05d5 | 1 | `bIsGcoffLegacyAllowed` |
| 1494 | 0x05d6 | 1 | `bIsMigSupported` |
| 1496 | 0x05d8 | 2 | `RTD3GC6TotalBoardPower` |
| 1498 | 0x05da | 2 | `RTD3GC6PerstDelay` |
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
| 1568 | 0x0620 | 16 | `ecidInfo` |
| 1568 | 0x0620 | 16 | `ecidInfo.info` |
| 1584 | 0x0630 | 16 | `fwWprLayoutOffset` |
| 1584 | 0x0630 | 8 | `fwWprLayoutOffset.nonWprHeapOffset` |
| 1592 | 0x0638 | 8 | `fwWprLayoutOffset.frtsOffset` |
| 1600 | 0x0640 | 1 | `bBusResetRequired` |

##### `NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO`: sizeof 56 (0x38), align 8

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

#### GSP_SET_SYSTEM_INFO payload

##### `GspSystemInfo`: sizeof 952 (0x3b8), align 8

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

##### `MUX_METHOD_DATA_ELEMENT`: sizeof 12 (0xc), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `acpiId` |
| 4 | 0x0004 | 4 | `mode` |
| 8 | 0x0008 | 4 | `status` |

#### Boot arguments (RMARGS region, libos init args, WPR meta)

##### `GSP_ARGUMENTS_CACHED`: sizeof 184 (0xb8), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `magic` |
| 4 | 0x0004 | 2 | `size` |
| 8 | 0x0008 | 8 | `flags` |
| 16 | 0x0010 | 64 | `messageQueueInitArguments` |
| 16 | 0x0010 | 8 | `messageQueueInitArguments.flags` |
| 24 | 0x0018 | 8 | `messageQueueInitArguments.sharedMemPhysAddr` |
| 68 | 0x0044 | 4 | `messageQueueInitArguments.pageTableEntryCount` |
| 32 | 0x0020 | 8 | `messageQueueInitArguments.cmdQueueOffset` |
| 40 | 0x0028 | 8 | `messageQueueInitArguments.statQueueOffset` |
| 48 | 0x0030 | 4 | `messageQueueInitArguments.queueElementHdrSize` |
| 52 | 0x0034 | 4 | `messageQueueInitArguments.queueElementSizeMin` |
| 56 | 0x0038 | 4 | `messageQueueInitArguments.queueElementSizeMax` |
| 60 | 0x003c | 4 | `messageQueueInitArguments.queueHeaderAlign` |
| 64 | 0x0040 | 4 | `messageQueueInitArguments.queueElementAlign` |
| 80 | 0x0050 | 16 | `srInitArguments` |
| 80 | 0x0050 | 4 | `srInitArguments.oldLevel` |
| 84 | 0x0054 | 4 | `srInitArguments.flags` |
| 88 | 0x0058 | 8 | `srInitArguments.reserved` |
| 96 | 0x0060 | 4 | `reserved2` |
| 100 | 0x0064 | 4 | `gpuInstance` |
| 104 | 0x0068 | 16 | `profilerArgs` |
| 104 | 0x0068 | 8 | `profilerArgs.pa` |
| 112 | 0x0070 | 8 | `profilerArgs.size` |
| 120 | 0x0078 | 16 | `sysmemHeapArgs` |
| 120 | 0x0078 | 8 | `sysmemHeapArgs.pa` |
| 128 | 0x0080 | 8 | `sysmemHeapArgs.size` |
| 136 | 0x0088 | 16 | `rmStateMonitorBufferArgs` |
| 136 | 0x0088 | 8 | `rmStateMonitorBufferArgs.pa` |
| 144 | 0x0090 | 8 | `rmStateMonitorBufferArgs.size` |
| 152 | 0x0098 | 16 | `bindataArgs` |
| 152 | 0x0098 | 8 | `bindataArgs.radix3` |
| 160 | 0x00a0 | 8 | `bindataArgs.size` |
| 168 | 0x00a8 | 8 | `vbiosOverrideArgs.pa` |
| 176 | 0x00b0 | 8 | `vbiosOverrideArgs.size` |

##### `MESSAGE_QUEUE_INIT_ARGUMENTS`: sizeof 64 (0x40), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | `flags` |
| 8 | 0x0008 | 8 | `sharedMemPhysAddr` |
| 52 | 0x0034 | 4 | `pageTableEntryCount` |
| 16 | 0x0010 | 8 | `cmdQueueOffset` |
| 24 | 0x0018 | 8 | `statQueueOffset` |
| 32 | 0x0020 | 4 | `queueElementHdrSize` |
| 36 | 0x0024 | 4 | `queueElementSizeMin` |
| 40 | 0x0028 | 4 | `queueElementSizeMax` |
| 44 | 0x002c | 4 | `queueHeaderAlign` |
| 48 | 0x0030 | 4 | `queueElementAlign` |
| 56 | 0x0038 | 8 | `reserved` |

##### `GSP_SR_INIT_ARGUMENTS`: sizeof 16 (0x10), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `oldLevel` |
| 4 | 0x0004 | 4 | `flags` |
| 8 | 0x0008 | 8 | `reserved` |

##### `LibosMemoryRegionInitArgument`: sizeof 32 (0x20), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | `id8` |
| 8 | 0x0008 | 8 | `pa` |
| 16 | 0x0010 | 8 | `size` |
| 24 | 0x0018 | 1 | `kind` |
| 25 | 0x0019 | 1 | `loc` |

##### `GspFwWprMeta`: sizeof 256 (0x100), align 8

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
| 242 | 0x00f2 | 2 | `pagingConfig` |
| 244 | 0x00f4 | 4 | `pmuReservedSize` |
| 248 | 0x00f8 | 8 | `verified` |

#### GSP-FMC boot parameters (Hopper and later only, not used on Turing)

##### `GSP_FMC_INIT_PARAMS`: sizeof 8 (0x8), align 4

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 4 | `regkeys` |

##### `GSP_FMC_BOOT_PARAMS`: sizeof 256 (0x100), align 8

| offset | hex | size | field |
|---:|---:|---:|---|
| 32 | 0x0020 | 8 | `initParams` |
| 32 | 0x0020 | 4 | `initParams.regkeys` |
| 64 | 0x0040 | 32 | `bootGspRmParams` |
| 64 | 0x0040 | 4 | `bootGspRmParams.target` |
| 68 | 0x0044 | 4 | `bootGspRmParams.gspRmDescSize` |
| 72 | 0x0048 | 8 | `bootGspRmParams.gspRmDescOffset` |
| 80 | 0x0050 | 8 | `bootGspRmParams.wprCarveoutOffset` |
| 88 | 0x0058 | 4 | `bootGspRmParams.wprCarveoutSize` |
| 92 | 0x005c | 1 | `bootGspRmParams.bIsGspRmBoot` |
| 93 | 0x005d | 1 | `bootGspRmParams.bInstInSysMode` |
| 128 | 0x0080 | 16 | `gspRmParams` |
| 128 | 0x0080 | 4 | `gspRmParams.target` |
| 136 | 0x0088 | 8 | `gspRmParams.bootArgsOffset` |
| 160 | 0x00a0 | 16 | `gspSpdmParams` |
| 160 | 0x00a0 | 4 | `gspSpdmParams.target` |
| 168 | 0x00a8 | 8 | `gspSpdmParams.payloadBufferOffset` |
| 164 | 0x00a4 | 4 | `gspSpdmParams.payloadBufferSize` |
| 192 | 0x00c0 | 8 | `gspRmMemParams` |
| 192 | 0x00c0 | 4 | `gspRmMemParams.flushSysmemAddrValLo` |
| 196 | 0x00c4 | 4 | `gspRmMemParams.flushSysmemAddrValHi` |

#### LIBOS log buffer header (no struct; NvU64 words)

| offset | hex | size | field |
|---:|---:|---:|---|
| 0 | 0x0000 | 8 | put pointer, in NvU64 entries (liblogdecode.h:156, liblogdecode.c:2543) |
| 8 | 0x0008 | 8 | buffer PA, written by the CPU before boot (kernel_gsp.c:3649); also ring slot 0 |
| 8 | 0x0008 | ... | ring of `size/8 - 1` NvU64 slots starting at word 1 (liblogdecode.c:1231,1279) |

#### Derived constants (computed by the compiler from the same headers)

| name | decimal | hex |
|---|---:|---:|
| `queueElementHdrSize (no CC)` | 24 | 0x18 |
| `queueElementHdrSize (CC)` | 40 | 0x28 |
| `queueElementSizeMin` | 4096 | 0x1000 |
| `queueElementSizeMax` | 65536 | 0x10000 |
| `maxRpcSize = SizeMax - HdrSize` | 65512 | 0xffe8 |
| `max payload after rpc_message_header_v` | 65480 | 0xffc8 |
| `hdr + GspStaticConfigInfo (must be <= maxRpcSize)` | 1640 | 0x668 |
| `hdr + GspSystemInfo (must be <= maxRpcSize)` | 984 | 0x3d8 |
| `msgq entryOff (entryAlign 12)` | 4096 | 0x1000 |
| `msgq msgCount for a 0x40000 queue` | 63 | 0x3f |
| `mctpHeader = mctpCreateTransportHeader(1,1,0,0,0)` | 3221225473 | 0xc0000001 |
| `nvdmHeader = mctpCreateNvdmHeader(NVDM_TYPE_RM_RPC)` | 621862526 | 0x2510de7e |
| `MCTP_MSG_HEADER_VENDOR_ID_NV` | 4318 | 0x10de |
| `MCTP_MSG_HEADER_TYPE_VENDOR_PCI` | 126 | 0x7e |
| `NVDM_TYPE_RM_RPC` | 37 | 0x25 |
| `MSGQ_VERSION_MAJOR` | 2 | 0x2 |
| `MSGQ_VERSION_MINOR` | 0 | 0x0 |
| `MCTP_MAGIC` | 1296258128 | 0x4d435450 |
| `MCTP_MAGIC_SEEN` | 1397048654 | 0x5345454e |
| `GMCAPI_CMD_GSP_INIT` | 65537 | 0x10001 |
| `MSGQ_MSG_SIZE_MIN` | 16 | 0x10 |
| `header_version (rpc_common.c:155-156)` | 50331648 | 0x3000000 |
| `NV_VGPU_MSG_SIGNATURE_VALID` | 1129337430 | 0x43505256 |
| `NV_VGPU_MSG_RESULT_SUCCESS` | 0 | 0x0 |
| `NV_VGPU_MSG_RESULT_RPC_PENDING` | 4294967295 | 0xffffffff |
| `NV_VGPU_MSG_UNION_INIT` | 0 | 0x0 |
| `DRF_BASE(NV_VGPU_MSG_RESULT__VMIOP)` | 4278190080 | 0xff000000 |
| `VGX_MAJOR_VERSION_NUMBER` | 47 | 0x2f |
| `VGX_MINOR_VERSION_NUMBER` | 6 | 0x6 |
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

## Appendix B: changes to gsp-abi-sizes for 615.71.09

Applied to a scratch copy only; `caraxes/tools/gsp-abi-sizes.{c,sh}` are unchanged. The `.c` changes follow the 615 field names; the `.sh` change adds `-DPORT_MODULE_time=1`, which 615 headers need (`g_objtmr_nvoc.h:1192` calls `portTimeGetUptimeNanoseconds`). Applying it to the tool in the repository would break the 610 build (the 610 fields are gone), so a version switch is needed if it is merged.

```diff
--- a/caraxes/tools/gsp-abi-sizes.sh	2026-09-23 11:32:05.017917374 +0200
+++ b/caraxes/tools/gsp-abi-sizes.sh	2026-09-24 10:58:41.593927474 +0200
@@ -83,7 +83,8 @@
         -DPORT_MODULE_atomic=1 \
         -DPORT_MODULE_debug=1 \
         -DPORT_MODULE_memory=1 \
-        -DPORT_MODULE_thread=1
+        -DPORT_MODULE_thread=1 \
+        -DPORT_MODULE_time=1
 }
 
 # src/nvidia/Makefile:28-58 (includes) and :106-138 (defines), in order.
--- a/caraxes/tools/gsp-abi-sizes.c	2026-09-23 11:29:19.522388223 +0200
+++ b/caraxes/tools/gsp-abi-sizes.c	2026-09-24 10:58:41.593868604 +0200
@@ -40,7 +40,8 @@
 #undef RPC_MESSAGE_GENERIC_UNION
 
 #include "gpu/gsp/message_queue_priv.h" /* GSP_MSG_QUEUE_ELEMENT, _checkSum32 */
-#include "msgq/msgq_priv.h"             /* msgqTxHeader, msgqRxHeader */
+#include "msgq/msgq_priv.h"             /* msgqTxHeader (MSGQ v2: no msgqRxHeader) */
+#include "gmcapi/gmcapi_base.h"         /* GMCAPI_HEADER (615: replaces the init RPCs) */
 #include "mctp_format.h"                /* as message_queue_cpu.c:49 */
 #include "nvdm_format.h"                /* as message_queue_cpu.c:50 */
 #include "gpu/gsp/gsp_static_config.h"  /* GspStaticConfigInfo, GspSystemInfo */
@@ -102,16 +103,17 @@
 static void constants(void)
 {
     /* message_queue_cpu.c:82-91 */
-    const unsigned long hdr      = OFF(GSP_MSG_QUEUE_ELEMENT, payload);
-    const unsigned long hdr_cc   = hdr + sizeof(GSP_MSG_QUEUE_ENCRYPTION_TAG);
+    /* 615: message_queue_cpu.c:82-83 */
+    const unsigned long hdr      = GSP_MSG_QUEUE_ELEMENT_SIZE_NO_ENCRYPTION;
+    const unsigned long hdr_cc   = GSP_MSG_QUEUE_ELEMENT_SIZE_WITH_ENCRYPTION;
     const unsigned long elem_min = RM_PAGE_SIZE;
     const unsigned long elem_max = RM_PAGE_SIZE * 16;
     /* kernel_gsp.c:3186 -> message_queue_priv.h:112-115 */
     const unsigned long max_rpc  = gspMsgQueueGetMaxRpcSize(elem_max, hdr);
     /* msgq.c:236-238 with hdrAlign = 4 and entryAlign = RM_PAGE_SHIFT
      * (message_queue_cpu.c:90-91, passed at :178-179) */
-    const unsigned long rx_off    = NV_ALIGN_UP(sizeof(msgqTxHeader), 1u << 4);
-    const unsigned long entry_off = NV_ALIGN_UP(rx_off + sizeof(msgqRxHeader), 1u << RM_PAGE_SHIFT);
+    /* 615: msgq.c msgqTxCreate: entryOff = ALIGN_UP(sizeof(msgqTxHeader), 1 << entryAlign) */
+    const unsigned long entry_off = NV_ALIGN_UP(sizeof(msgqTxHeader), 1u << RM_PAGE_SHIFT);
 
     section("Derived constants (computed by the compiler from the same headers)");
     printf("\n| name | decimal | hex |\n|---|---:|---:|\n");
@@ -125,7 +127,6 @@
           sizeof(rpc_message_header_v) + sizeof(GspStaticConfigInfo));
     konst("hdr + GspSystemInfo (must be <= maxRpcSize)",
           sizeof(rpc_message_header_v) + sizeof(GspSystemInfo));
-    konst("msgq rxHdrOff (hdrAlign 4)", rx_off);
     konst("msgq entryOff (entryAlign 12)", entry_off);
     konst("msgq msgCount for a 0x40000 queue", (0x40000 - entry_off) / elem_min);
     konst("mctpHeader = mctpCreateTransportHeader(1,1,0,0,0)", mctpCreateTransportHeader(1, 1, 0, 0, 0));
@@ -133,8 +134,11 @@
     K(MCTP_MSG_HEADER_VENDOR_ID_NV);
     K(MCTP_MSG_HEADER_TYPE_VENDOR_PCI);
     K(NVDM_TYPE_RM_RPC);
-    K(MSGQ_VERSION);
-    K(MSGQ_FLAGS_SWAP_RX);
+    K(MSGQ_VERSION_MAJOR);
+    K(MSGQ_VERSION_MINOR);
+    K(MCTP_MAGIC);
+    K(MCTP_MAGIC_SEEN);
+    konst("GMCAPI_CMD_GSP_INIT", GMCAPI_CMD_GSP_INIT);
     K(MSGQ_MSG_SIZE_MIN);
     konst("header_version (rpc_common.c:155-156)",
           DRF_DEF(_VGPU, _MSG_HEADER_VERSION, _MAJOR, _TOT) |
@@ -188,31 +192,39 @@
     printf("<!-- generated by caraxes/tools/gsp-abi-sizes.sh; driver %s -->\n", NV_VERSION_STRING);
     SECTION("Message queue transport (msgq library, one header per queue)");
     TYPE(msgqTxHeader);
-    F(msgqTxHeader, version);
+    F(msgqTxHeader, versionMajor);
+    F(msgqTxHeader, versionMinor);
     F(msgqTxHeader, size);
     F(msgqTxHeader, msgSize);
     F(msgqTxHeader, msgCount);
-    F(msgqTxHeader, writePtr);
-    F(msgqTxHeader, flags);
-    F(msgqTxHeader, rxHdrOff);
     F(msgqTxHeader, entryOff);
-
-    TYPE(msgqRxHeader);
-    F(msgqRxHeader, readPtr);
+    F(msgqTxHeader, reserved);
 
     SECTION("Queue element and RPC header");
     TYPE(GSP_MSG_QUEUE_ELEMENT);
+    F(GSP_MSG_QUEUE_ELEMENT, mctpMagic);
+    F(GSP_MSG_QUEUE_ELEMENT, mctpPayloadSize);
     F(GSP_MSG_QUEUE_ELEMENT, mctpHeader);
     F(GSP_MSG_QUEUE_ELEMENT, nvdmHeader);
-    F(GSP_MSG_QUEUE_ELEMENT, checkSum);
-    F(GSP_MSG_QUEUE_ELEMENT, seqNum);
-    FX(GSP_MSG_QUEUE_ELEMENT, payload);
+    F(GSP_MSG_QUEUE_ELEMENT, noEncryption.nvdmPayloadSize);
+    F(GSP_MSG_QUEUE_ELEMENT, noEncryption.reserved);
+    FX(GSP_MSG_QUEUE_ELEMENT, noEncryption.payload);
+    F(GSP_MSG_QUEUE_ELEMENT, withEncryption.encryptionTag);
+    F(GSP_MSG_QUEUE_ELEMENT, withEncryption.nvdmPayloadSize);
+    F(GSP_MSG_QUEUE_ELEMENT, withEncryption.reserved);
+    FX(GSP_MSG_QUEUE_ELEMENT, withEncryption.payload);
 
     TYPE(GSP_MSG_QUEUE_ENCRYPTION_TAG);
-    F(GSP_MSG_QUEUE_ENCRYPTION_TAG, encryptedSize);
-    F(GSP_MSG_QUEUE_ENCRYPTION_TAG, reserved);
     F(GSP_MSG_QUEUE_ENCRYPTION_TAG, authTagBuffer);
-    F(GSP_MSG_QUEUE_ENCRYPTION_TAG, aadBuffer);
+
+    TYPE(GMCAPI_HEADER);
+    F(GMCAPI_HEADER, command);
+    F(GMCAPI_HEADER, size);
+    F(GMCAPI_HEADER, sequence);
+    F(GMCAPI_HEADER, request.max_response_size);
+    F(GMCAPI_HEADER, response.status);
+    F(GMCAPI_HEADER, reserved);
+    FX(GMCAPI_HEADER, data);
 
     TYPE(rpc_message_header_v);
     F(rpc_message_header_v, header_version);
@@ -246,8 +258,8 @@
     F(rpc_set_guest_system_info_ext_v25_1B, device);
     F(rpc_set_guest_system_info_ext_v25_1B, gridBuildCsp);
 
-    TYPE(rpc_gsp_set_system_info_v17_00);
-    F(rpc_gsp_set_system_info_v17_00, data);
+    /* 615: rpc_gsp_set_system_info_v17_00 is gone (GSP_SET_SYSTEM_INFO deprecated,
+     * rpc_global_enums.h:82); system info travels NVKV-encoded in GMCAPI_CMD_GSP_INIT. */
 
     TYPE(PACKED_REGISTRY_TABLE);
     F(PACKED_REGISTRY_TABLE, size);
@@ -416,9 +428,7 @@
     F(GspStaticConfigInfo, bIsGpuUefi);
     F(GspStaticConfigInfo, bIsEfiInit);
     F(GspStaticConfigInfo, ecidInfo); /* struct */
-    F(GspStaticConfigInfo, ecidInfo.ecidLow);
-    F(GspStaticConfigInfo, ecidInfo.ecidHigh);
-    F(GspStaticConfigInfo, ecidInfo.ecidExtended);
+    F(GspStaticConfigInfo, ecidInfo.info);
     F(GspStaticConfigInfo, fwWprLayoutOffset); /* struct */
     F(GspStaticConfigInfo, fwWprLayoutOffset.nonWprHeapOffset);
     F(GspStaticConfigInfo, fwWprLayoutOffset.frtsOffset);
@@ -539,7 +549,11 @@
 
     SECTION("Boot arguments (RMARGS region, libos init args, WPR meta)");
     TYPE(GSP_ARGUMENTS_CACHED);
+    F(GSP_ARGUMENTS_CACHED, magic);
+    F(GSP_ARGUMENTS_CACHED, size);
+    F(GSP_ARGUMENTS_CACHED, flags);
     F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments); /* struct */
+    F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.flags);
     F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.sharedMemPhysAddr);
     F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.pageTableEntryCount);
     F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.cmdQueueOffset);
@@ -552,9 +566,9 @@
     F(GSP_ARGUMENTS_CACHED, srInitArguments); /* struct */
     F(GSP_ARGUMENTS_CACHED, srInitArguments.oldLevel);
     F(GSP_ARGUMENTS_CACHED, srInitArguments.flags);
-    F(GSP_ARGUMENTS_CACHED, srInitArguments.bInPMTransition);
+    F(GSP_ARGUMENTS_CACHED, srInitArguments.reserved);
+    F(GSP_ARGUMENTS_CACHED, reserved2);
     F(GSP_ARGUMENTS_CACHED, gpuInstance);
-    F(GSP_ARGUMENTS_CACHED, bDmemStack);
     F(GSP_ARGUMENTS_CACHED, profilerArgs); /* struct */
     F(GSP_ARGUMENTS_CACHED, profilerArgs.pa);
     F(GSP_ARGUMENTS_CACHED, profilerArgs.size);
@@ -567,8 +581,11 @@
     F(GSP_ARGUMENTS_CACHED, bindataArgs); /* struct */
     F(GSP_ARGUMENTS_CACHED, bindataArgs.radix3);
     F(GSP_ARGUMENTS_CACHED, bindataArgs.size);
+    F(GSP_ARGUMENTS_CACHED, vbiosOverrideArgs.pa);
+    F(GSP_ARGUMENTS_CACHED, vbiosOverrideArgs.size);
 
     TYPE(MESSAGE_QUEUE_INIT_ARGUMENTS);
+    F(MESSAGE_QUEUE_INIT_ARGUMENTS, flags);
     F(MESSAGE_QUEUE_INIT_ARGUMENTS, sharedMemPhysAddr);
     F(MESSAGE_QUEUE_INIT_ARGUMENTS, pageTableEntryCount);
     F(MESSAGE_QUEUE_INIT_ARGUMENTS, cmdQueueOffset);
@@ -578,11 +595,12 @@
     F(MESSAGE_QUEUE_INIT_ARGUMENTS, queueElementSizeMax);
     F(MESSAGE_QUEUE_INIT_ARGUMENTS, queueHeaderAlign);
     F(MESSAGE_QUEUE_INIT_ARGUMENTS, queueElementAlign);
+    F(MESSAGE_QUEUE_INIT_ARGUMENTS, reserved);
 
     TYPE(GSP_SR_INIT_ARGUMENTS);
     F(GSP_SR_INIT_ARGUMENTS, oldLevel);
     F(GSP_SR_INIT_ARGUMENTS, flags);
-    F(GSP_SR_INIT_ARGUMENTS, bInPMTransition);
+    F(GSP_SR_INIT_ARGUMENTS, reserved);
 
     TYPE(LibosMemoryRegionInitArgument);
     F(LibosMemoryRegionInitArgument, id8);
@@ -635,7 +653,7 @@
     F(GspFwWprMeta, lsUcodeVersionPadding);
     F(GspFwWprMeta, gspFwHeapVfPartitionCount);
     F(GspFwWprMeta, flags);
-    F(GspFwWprMeta, padding);
+    F(GspFwWprMeta, pagingConfig);
     F(GspFwWprMeta, pmuReservedSize);
     F(GspFwWprMeta, verified);
 
```
