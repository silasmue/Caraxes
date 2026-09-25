<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Blackwell and Hopper

Scope: the GB206 identity from the lab capture, the FSP Chain-of-Trust boot, GMMU v3, the N gate in pure emulation, GH100 in emulation, and what a lab capture or a cloud card adds.

## Facts

### GB206 identity (lab capture, RTX 5060 Ti, driver 615.71.09)

| Item | GB206 | TU106 |
|---|---|---|
| PCI id, revision | 10de:2d04 rev a1 | 10de:1f02 rev a1 |
| `MC_GET_ARCH_INFO` | arch 0x1B0, impl 0x6, chip id 0x1B6 | Turing |
| BOOT_0 / BOOT_42 (composed) | 0x1B6000A1 / 0x1B6A1000 | 0x166000a1 / 0x166a1000 |
| Short name | `GB206-A` | `TU106` |
| BAR0 / BAR1 / BAR3 | 64 MiB / 16 GiB (resizable 64 MiB to 16 GiB) / 32 MiB | 16 MiB / 256 MiB / 32 MiB |
| Capabilities | MSI-X (9 vectors), SR-IOV, ARI, resizable BAR, DSN, PBI mailbox VSEC | PM, MSI, PCIe, no MSI-X |
| Link cap | 32 GT/s x16 | 8 GT/s x16 |
| VBIOS | 98.06.1F.40.4A | 90.06.3D.00.76 |
| GSP firmware | `gsp_ga10x.bin` | `gsp_tu10x.bin` |
| Compute capability | 12.0 | 7.5 |
| GR | 3 GPCs x 6 TPCs | 36 SMs in 3 GPCs of 6 TPCs |
| Engines | GR0, COPY0/4/7, 1 NVDEC, 1 NVENC, NVJPG, OFA, SEC2 | GR0, CE0-CE4, 3 NVDEC |
| GR MMU fault id | 384 | 64 |
| Channel / copy / 3D / compute classes | CA6F / CAB5 / CE97 / CEC0 | C46F / C5B5 / C597 / C5C0 |
| DMA address width | 52 bits | 47 bits |
| FB | 16311 MiB, one region | profile |

[docs/caraxes/blackwell-gb206.md §1.2-§1.8, §2.1; docs/caraxes/costume-5060.md §2; docs/caraxes/costume-ladder.md §1]

- The 615 driver maps GB20X to `gsp_ga10x.bin` [615 tree: src/nvidia/arch/nvalloc/common/inc/nv-firmware.h:97-104].
- The 615 name table knows 2D04 as "NVIDIA GeForce RTX 5060 Ti" [615 tree: src/nvidia/generated/g_nv_name_released.h:841].
- Interrupt vectors differ (non-replayable fault 0x84, replayable 0x40, access counter 0x48, timer 0x94) and two subtrees are new, RUNLIST 0x10 and RUNLIST_NOTIFICATION 0x20 [docs/caraxes/blackwell-gb206.md §1.9].
- The PBI mailbox capability at 0x9C carries four UUID bytes at 0xA8, so `config.rs` removes and zeroes it for every board [docs/caraxes/blackwell-gb206.md §7.1].
- The synthetic board `rtx5060ti` unlinks MSI-X, presents BAR1 as 256 MiB, drops the extended space and keeps subsystem 1043:8a08 [docs/caraxes/blackwell-gb206.md §7.1].
- The constructed falcon with class 0x4f32fd is in neither open tree; by elimination it is most likely the NNE falcon (confidence medium) [docs/caraxes/blackwell-gb206.md §7.2].

### FSP Chain-of-Trust boot (instead of FWSEC and Booter)

- From GH100 on `kgspExtractVbiosFromRom` is a stub, so the guest never parses the ROM and runs no FWSEC; FRTS is set up by FSP [docs/caraxes/gsp-boot-blackwell.md §1; docs/caraxes/lab-blackwell.md §7].
- GB206 and GH100 have a KernelFsp (`PDB_PROP_KFSP_IS_MISSING` false) [615 tree: src/nvidia/generated/g_kern_fsp_nvoc.c:233-237].
- The CPU first polls `NV_THERM_I2CS_SCRATCH` at 0xAD00BC for 0xFF (FSP boot complete) [615 tree: src/common/inc/swref/published/blackwell/gb202/dev_therm.h:27; 615 tree: src/nvidia/src/kernel/gpu/fsp/arch/blackwell/kern_fsp_gb202.c:68].
- Messages are MCTP-framed NVDM packets over FSP EMEM channel 0 (1024 bytes): `EMEMC`/`EMEMD` 0x8F2AC0/0x8F2AC4, `QUEUE_HEAD`/`TAIL` 0x8F2C00/0x8F2C04 (CPU to FSP), `MSGQ_HEAD`/`TAIL` 0x8F2C80/0x8F2C84 [615 tree: src/common/inc/swref/published/hopper/gh100/dev_fsp_pri.h:26-57; 615 tree: src/nvidia/arch/nvalloc/common/inc/fsp/fsp_emem_channels.h:34-37].
- NVDM types: COT 0x14, FSP_RESPONSE 0x15, CAPS_QUERY 0x16, CLOCK_BOOST 0x20 [615 tree: src/nvidia/arch/nvalloc/common/inc/nvdm_format.h:47-57].
- Order: CAPS_QUERY, CLOCK_BOOST enable, then COT; the device answers every message with FSP_OK [docs/caraxes/gsp-boot-blackwell.md §3; docs/caraxes/experiments.md "Blackwell N gate" b1].
- The COT payload is 860 packed bytes, version 2 on GB20x: GSP-FMC image PA, FRTS offsets and sizes, hash, key, signature, and `gspBootArgsSysmemOffset` pointing at `GSP_FMC_BOOT_PARAMS` [615 tree: src/nvidia/inc/kernel/gpu/fsp/kern_fsp_cot_payload.h:39-55; 615 tree: src/nvidia/generated/g_kern_fsp_nvoc.c:291].
- `GSP_FMC_BOOT_PARAMS` is 256 bytes in 615 with an "FMC " header and `NV_ABI_STABLE` [docs/caraxes/blackwell-gb206.md §3.2].
- The CPU writes only sizes into `GspFwWprMeta`; ACR fills the offsets, and the device computes that layout itself (`AcrLayout::compute`, derived, not documented) [docs/caraxes/gsp-boot-blackwell.md §4.3, §5].
- After the COT the CPU waits for target-mask release (GSP `HWCFG2` 0x1100F4 non-zero and not 0xBADF41xx), then for lockdown release (`HWCFG2` bit 13) with `MAILBOX0` 0, then checks `RISCV_CPUCTL.ACTIVE_STAT` at 0x111388 [615 tree: src/nvidia/src/kernel/gpu/gsp/arch/hopper/kernel_gsp_gh100.c:970-1067; docs/caraxes/gsp-boot-blackwell.md §2].
- On 615 the status queue is linked and `GMCAPI_CMD_GSP_INIT` is sent only after the RISC-V core is active [615 tree: src/nvidia/src/kernel/gpu/gsp/arch/hopper/kernel_gsp_gh100.c:1086-1091].
- The 615 queue v2 (pointers in `NV_PGSP_QUEUE_*`/`NV_PGSP_MSGQ_*`, MCTP-framed elements) uses one HAL for TU10x through GB20x, see [gsp-transport.md](gsp-transport.md) [docs/caraxes/blackwell-gb206.md §3.3].
- 610 delta: the N gate ran with the 610.57.04 guest, which queues SET_SYSTEM_INFO and SET_REGISTRY before the lockdown release and takes the static info through `GET_GSP_STATIC_INFO` [docs/caraxes/gsp-boot-blackwell.md §1; run: caraxes/runs/blackwell-2026-09-24].

### GMMU v3

- Hopper and later use the 5-level, 57-bit VA format; the GB206 bounds physical addresses at 52 bits [docs/caraxes/blackwell-gb206.md §2.2, §4].
- Without a v3 walker the guest fails `kbusVerifyBar2_GB202` ("BAR2 readback ... returned garbage") and every BAR3 access is unmapped [docs/caraxes/experiments.md "Blackwell N gate" b2].
- `src/gmmu_v3.rs`, selected by the chip's MMU format, walks from `bar2PdeBase` (the ACR layout's `nonWprHeapOffset` 0xF0900000) with 0 unmapped BAR3 accesses [docs/caraxes/experiments.md "Blackwell N gate" b3-b4].
- The incremental shadow walker (`ptcache.rs`) is v2 only, so no GMMU v3 guest can execute on the TU106 [docs/caraxes/costume-ladder.md §3].

### The N gate (pure emulation, no host GPU)

| Stage | Passes when | Result | Evidence |
|---|---|---|---|
| N0 device | server log `GB206 (BOOT_0 0x1b6000a1 ...)`, name `Leandro VFIO RTX 5060 Ti-4G` | pass | [run: caraxes/runs/emu-regress-2026-09-25/final] |
| N1 PCI identity | `[10de:2d04] (rev a1)`, BARs 64M/256M/32M, I/O 128, ROM 512K, caps PM, MSI, Express only | pass | [run: caraxes/runs/emu-regress-2026-09-25/final] |
| N2 driver load | FSP sequence, queues linked, init RPCs answered, `RmInitAdapter succeeded` | pass | [run: caraxes/runs/blackwell-2026-09-24/n2c] |
| N3 nvidia-smi | name, 4096 MiB, VBIOS 98.06.1F.40.4A, Architecture Blackwell, no Xid | pass | [run: caraxes/runs/blackwell-2026-09-24/n3/nvidia-smi-q.txt] |
| N4 first channel | `BLACKWELL_CHANNEL_GPFIFO_B` 0xCA6F allocated with status 0 | pass (chid 1, COPY4) | [run: caraxes/runs/blackwell-2026-09-24/n4d] |

- Four physical-RM answers come from the capture (`src/chip/physical.rs`): device-info table, CE physical caps, CE PCE masks and GR info [docs/caraxes/blackwell-gate.md §4; run: caraxes/runs/blackwell-2026-09-24/n4a, n4b, n4c].
- The emulated GR SM version is 0x0C00 (12.0); the 0x1200 of the first N4 runs decodes as SM 18.0 [615 tree: src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080gr.h:335; docs/caraxes/costume-ladder.md §1].
- Without `RmRcWatchdog=0` the GB206 stops at `numGpcs > 0` in `krcWatchdogInit`, since GR floorsweeping masks are zero without a host [docs/caraxes/blackwell-gate.md §5].
- The `cc` and `ccu` controls are sent at every GB206 boot and answered NOT_SUPPORTED without stopping the gate; see [feature-families.md](feature-families.md) [run: caraxes/runs/blackwell-2026-09-24/n2c/device-events.txt].
- Past N4 the first push (UVM's channel-init semaphore release) never completes because the device executes no Blackwell methods, so `UVM_REGISTER_GPU` hangs [docs/caraxes/experiments.md "Blackwell N gate" b8; run: caraxes/runs/blackwell-2026-09-24/n4d].

### GH100 in emulation (costume ladder rung 4)

- The rung presents the Hopper HAL (BOOT 0x180000a1/0x180a1000), 10de:2331 sub 10de:1626, GH100 classes, MIG on and NVLink/HSHUB/fabric/C2C present, on the GB206 FSP model and GMMU v3 [docs/caraxes/costume-ladder.md §3].
- Its first wall is guest Xid 143 in the FSP boot poll, because Hopper keeps `NV_THERM_I2CS_SCRATCH` at 0x200BC [615 tree: src/common/inc/swref/published/hopper/gh100/dev_therm.h:26; run: caraxes/runs/costume-ladder-2026-09-24/r4-gh100].
- `chips/gh100/boot.toml` adds that scratch, WPR2 in the FB unit (0x1FA824/8) and the GA100 debug fuse; then COT, GSP boot and init RPCs pass [docs/caraxes/costume-ladder.md §3.3].
- At boot the guest asks CC static info, the HUB PCE mask and the CCU buffer, none fatal, and none of NVLink, fabric, C2C or MIG [docs/caraxes/costume-ladder.md §3.3].
- nvidia-smi reports `Leandro VFIO H100 PCIe-4G`, compute capability 9.0, 4096 MiB [run: caraxes/runs/emu-regress-2026-09-25/ladder-conf].

### What a lab capture or a cloud card adds

- A read-only capture (`lab-capture.sh`, the lab kit) gives ids, config space, ROM, arch info, class list, engines, fault ids, PCE masks, FB/GR facts, the interrupt table, a feature-family sweep and native user-space traces [docs/caraxes/gsp-plan/lab-kit.md §6; docs/caraxes/lab-blackwell.md §7].
- A capture cannot give GSP-RM's internal state, the GSP RPC traffic or the INTERNAL controls; only a module built with queue logging, on a rented VM, would [docs/caraxes/gsp-plan/lab-kit.md §8].
- Without a Blackwell card the composed values (boot registers, register surface, FSP timing, physical-RM answers) show up only as guest failures; a passthrough trace would settle them [docs/caraxes/blackwell-gb206.md §4].
- A second GB206 capture would add the device-info table and PES info in 615 layouts, L2/LTC counts, EDIDs and the linux-firmware `gb206/` list [docs/caraxes/blackwell-gb206.md §2.3].
- The PROM read (`--prom`) is expected to be refused (mmap of `resource0`, EINVAL) while `nvidia` drives the card on a `CONFIG_IO_STRICT_DEVMEM=y` kernel [docs/caraxes/blackwell-gb206.md §7.3].
- The kit's self-test on the TU106 without sudo recorded 33 items: 30 ok, 2 reduced, 1 skipped, 0 failed [run: caraxes/runs/lab-selftest-2026-09-24].

| Chip | Cloud part | Validates |
|---|---|---|
| GH100 | H100, H200 | Hopper CUDA, `cc`, `ccu`, `mig`, nvlink tier 0/1 |
| GB100 | B200, GB200 | Blackwell data-center CUDA, `ccu`, `blackwell`, `mig`, NVLE |
| GB202 | RTX PRO 6000 Blackwell Server Edition | closest GB20x in a cloud, with MIG |
| GB206 | none (workplace card) | GB20x consumer, display families |

[docs/caraxes/gsp-plan/lab-kit.md §9]

## Consequence for Caraxes

- A Blackwell or Hopper identity is a chip-axis addition (chip tables, FSP boot model, GMMU v3 walker) on the 615 version module, with no new transport.
- The GB206 stays pure emulation up to "driver up": Blackwell methods and GMMU v3 tables cannot run on the TU106.
- Next device work past N4 is the CA6F/CAB5 method subset and full KGR static info.
- Hopper reuses the GB206 FSP model with a boot overlay for its own scratch, WPR2 and fuse registers.

## Open

- A 615.71.09 guest has not booted the GB206 or GH100 identity; every Blackwell gate run used the 610 guest.
- The ACR WPR layout is derived; whether GB206 FB accounting agrees with it is unconfirmed.
- The PMU reserve that sets `frtsVidmemOffset` assumes KernelPmu survives construction on GB206.
- `appVersion` of the GB20x GSP-RM boot descriptor was not extracted; the model accepts any value.
- The kernel-RM register surface before `kgspInitRm` (XAL/XTL windows, fuses, interrupt tree, display fuse) is composed from headers, not traced on a card.
- Whether class 0x4f32fd is NNE needs a driver whose RM exposes an NNE control.
