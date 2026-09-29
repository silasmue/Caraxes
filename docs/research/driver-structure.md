<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Driver structure

Scope: how the 615.71.09 kernel RM picks its runtime variant and HAL, what it must be configured with, and what it reads from the device on a TU106 identity.

## Facts

### Chip and HAL selection

- `gpumgrGetGpuHalFactor` reads `NV_PMC_BOOT_0` (0x0) as chip id 0 and `NV_PMC_BOOT_42` (0xA00) as chip id 1 through BAR0, then `NV_PMC_BOOT_1` (0x4) for the vGPU field [615 tree: src/nvidia/src/kernel/gpu_mgr/gpu_mgr.c:1266-1270; src/common/inc/swref/published/nv_ref.h:108, :139, :150].
- `rm_is_supported_device` reads the same three registers at probe and selects the HAL with `halmgrGetHalForGpu(pmc_boot_0, pmc_boot_42)` [615 tree: src/nvidia/arch/nvalloc/unix/src/osapi.c:3777-3779, :3809].
- `NV_PMC_BOOT_42` carries the architecture in 29:24 (`TU100` = 0x16) and the chip id in 29:20 [615 tree: src/common/inc/swref/published/nv_ref.h:171-179].
- The tree maps no PCI device id to a chip; the kernel reads the chip from the card [docs/caraxes/gsp-plan/e-families.md §2].
- A TU104 costume (BOOT 0x164000a1/0x164a1000, devid 10de:1e82) is indistinguishable from TU106 to the driver, and a GA106 costume (BOOT 0x176000a1/0x176a1000) gets the Ampere HAL and its VBIOS and boot-register expectations [docs/caraxes/costume-ladder.md §3].
- `osHandleGpuLost` rereads `NV_PMC_BOOT_0` and declares the GPU lost when it differs from the cached value, so the emulated value must never change [615 tree: src/nvidia/arch/nvalloc/unix/src/osinit.c:341-360].

### The runtime variants

| Variant | Chosen when | On a TU106 identity | Evidence |
|---|---|---|---|
| `UCODE` | GSP or DCE build | never in a kernel build | [615 tree: src/nvidia/src/kernel/gpu_mgr/gpu_mgr.c:1292-1293] |
| `VF` (vGPU guest) | `NV_PMC_BOOT_1` 17:16 = `_VF` (2) | dead, see below | [615 tree: src/nvidia/src/kernel/gpu_mgr/gpu_mgr.c:1013-1019, :1294-1295; src/common/inc/swref/published/nv_ref.h:146-149] |
| `PF_KERNEL_ONLY` (GSP client) | firmware policy allows it and BOOT_42 architecture >= TU100 | the only one that runs (D1) | [615 tree: src/nvidia/src/kernel/gpu_mgr/gpu_mgr.c:1074-1077, :1287-1297; docs/caraxes/README.md §Decisions D1] |
| `PF_MONOLITHIC` | otherwise | absent: the open tree has no physical engine code, only a "Falling back to monolithic RM" notice | [615 tree: src/nvidia/arch/nvalloc/unix/src/osinit.c:2143-2154; docs/caraxes/driver-modes.md "Variant 2"] |

- The VF transport exists only as the GSP-plugin offload: `initRpcInfrastructure_VGPU` returns `NV_ERR_NOT_SUPPORTED` unless `bGspPlugin` is set from `IS_VGPU_GSP_PLUGIN_OFFLOAD_ENABLED` [615 tree: src/nvidia/src/kernel/vgpu/rpc.c:1727-1743].
- The offload property `PDB_PROP_GPU_VGPU_OFFLOAD_CAPABLE` is set only for AD102 to AD107, GH100, the GB chips and GR100/GR102, not for TU10X or GA10X [615 tree: src/nvidia/generated/g_gpu_nvoc.c:477-481; src/nvidia/src/kernel/gpu/gpu_registry.c:141].
- The VF doorbell and scratch-register HALs are stubs for TU10X and GA100 [615 tree: src/nvidia/generated/g_rpc_private.h:271, :292].
- A failed VF RPC setup asserts in `vgpuCreateObject` [615 tree: src/nvidia/src/kernel/vgpu/objvgpu.c:97-101].
- Presenting an Ada or later identity to reach the VF transport is ruled out because class lists, MMU formats and SASS targets would mismatch the TU106 (D2) [docs/caraxes/README.md §Decisions D2].
- A GSP client loads `gsp_tu10x.bin` and accepts only an image whose `.fwversion` equals its own version string [615 tree: src/nvidia/arch/nvalloc/common/inc/nv-firmware.h:117; src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:4190, :6401].
- The device reads the booter images and the GSP-RM radix3 image not at all; it never executes firmware [docs/caraxes/gsp-plan/boot-exchange.md §2 step 2].

### Module options and registry keys the guest needs

| Option or key | Effect | Evidence |
|---|---|---|
| `NVreg_GpuInitOnProbe=1` | `RmInitAdapter` runs at probe instead of first open | [docs/caraxes/init-map.md Phase 1; docs/caraxes/quickstart.md §2] |
| `RmDisableGlobalCeUtils=1` | no global CeUtils and no CE memset test on a channel whose VA space the software executor never learns | [615 tree: src/nvidia/interface/nvrm_registry.h:1003; src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr.c:348-349; docs/caraxes/blackwell-gate.md §5] |
| `RMDisableScrubOnFree=1` | freed video memory is not scrubbed through the copy engine | [615 tree: src/nvidia/interface/nvrm_registry.h:954; mem_mgr.c:221; docs/caraxes/blackwell-gate.md §5] |
| `RmRcWatchdog=0` | no watchdog GR channel (default is enabled) | [615 tree: src/nvidia/interface/nvrm_registry.h:1612-1615; src/nvidia/src/kernel/gpu/gr/kernel_watchdog.c:125-127] |
| `RMInstLoc` | bits 17:16 place RM-owned USERD (0 default, 1 COH, 2 NCOH, 3 VID) | [615 tree: src/nvidia/interface/nvrm_registry.h:165-173, :235; src/nvidia/src/kernel/gpu/gpu_registry.c:293] |
| `uvm_page_table_location` | UVM module parameter for the page-table aperture | [615 tree: kernel-open/nvidia-uvm/uvm_mmu.c:72-73] |

- The quickstart sets `NVreg_GpuInitOnProbe=1 NVreg_ResmanDebugLevel=0 NVreg_RegistryDwords="RmDisableGlobalCeUtils=1;RMDisableScrubOnFree=1;RmRcWatchdog=0"` in `/etc/modprobe.d/caraxes.conf` [docs/caraxes/quickstart.md §2].
- Without the three keys the TU106 in pure emulation stopped at the CeUtils memset and the GB206 at `numGpcs > 0` in `krcWatchdogInit`, and the device now warns per missing key [docs/caraxes/blackwell-gate.md §5; run: caraxes/runs/emu-regress-2026-09-25].
- `RMInstLoc=0x10000` (USERD in coherent sysmem) left the guest RM stuck before `RmInitAdapter` finished, in a `BOOT_0` poll loop [docs/caraxes/mapping.md "USERD in sysmem by configuration"].
- D17 names `uvm_page_table_location=vid` with the matching `RmInstLoc` fields as the only allowed configuration deviation, and today the UVM page-tree root is in vidmem with the parameter at its default [docs/caraxes/README.md §Decisions D17; docs/caraxes/sysmem-iova.md].
- In 615 the registry travels as `REGKEY_*` NVKV keys inside `GMCAPI_CMD_GSP_INIT`; the captured request carries 44 entries [docs/caraxes/gsp-transport-615.md §5.2].
- The device honours `RmLogonRC`, `RMSplitVasMgmtServerClientRm` and `EnableGpuFirmwareLogs` and only traces every other key [docs/caraxes/gsp-plan/boot-exchange.md Status B3].

### What the driver reads from where

| Item | Source | Evidence |
|---|---|---|
| PCI ids and config dwords | BAR0 mirror `NV_PCFG` 0x88000-0x88FFF, read by `gpuGetIdInfo_GM107` | [615 tree: src/common/inc/swref/published/turing/tu102/dev_nv_xve.h:26; src/nvidia/src/kernel/gpu/arch/maxwell/kern_gpu_gm107.c:437] |
| MSI enable state | mirror 0x88000+0x68 bit 16, must track the real capability | [docs/caraxes/kernel-rm-hw-surface.md §C KernelBif] |
| VBIOS and FWSEC | PROM window at BAR0 + 0x300000, `kgspExtractVbiosFromRom_HAL` | [615 tree: src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:5565; docs/caraxes/init-map.md 4.1] |
| Static configuration (`GspStaticConfigInfo`, 1608 bytes, internal) | NVKV response of `GMCAPI_CMD_GSP_INIT` | [615 tree: src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:5338, :7730; docs/caraxes/gsp-transport-615.md §5.3] |
| Feature bits | `BRANDING`, `MISC_FEATURES`, `POWER_FEATURES` keys; `POWER_FEATURES` sets the GC6/GC8/GCOFF RTD3 permissions | [615 tree: src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:7807-7812] |
| Engine list | `ENGINE_MASK` key (TU106: GR 0x1, COPY 0x1FF, NVDEC 0x3, NVENC 0x1, SW, SEC2, HOST 0x1 each) | [docs/caraxes/gsp-transport-615.md §5.3] |
| FIFO device info | `NV2080_CTRL_CMD_FIFO_GET_DEVICE_INFO_TABLE` from `kfifoGetHostDeviceInfoTable_KERNEL` | [615 tree: src/nvidia/src/kernel/gpu/fifo/kernel_fifo.c:2096, :2163] |
| Class list | generated TU106 descriptor lists | [615 tree: src/nvidia/generated/g_gpu_class_list.c:286-397] |
| Interrupt table | `NV2080_CTRL_CMD_INTERNAL_INTR_GET_KERNEL_TABLE` | [615 tree: src/nvidia/src/kernel/gpu/intr/intr.c:1044] |
| PCI BAR sizes | `NV2080_CTRL_CMD_BUS_GET_PCI_BAR_INFO` | [615 tree: src/nvidia/src/kernel/gpu/bus/kern_bus.c:606] |

- The static-configuration handler is strict: a known key with the wrong type, index or length fails the whole init with `NV_ERR_INVALID_ARGUMENT`, and unknown keys are ignored [docs/caraxes/gsp-transport-615.md §5.3].
- The device sends GPU names, GID, the internal handles 0xCA7A0001-3, VBIOS fields, 2 FB regions, `FB_LENGTH`, the BAR1/BAR2 PDE bases, the three feature words as 0 and `ENGINE_MASK`, 424 bytes in all [docs/caraxes/gsp-transport-615.md §5.3].
- The system information the kernel sends includes `PCI_CONFIG_MIRROR_BASE/SIZE`, the BAR addresses and the driver version string [docs/caraxes/gsp-transport-615.md §5.2].

### The unstable-interface opt-in

- NVIDIA calls the RM RPC layer "unstable interfaces to GSP": key `NVGMC_SI_UNSTABLE_INTERFACE_SUPPORT` 0x1110 with bit `_RPC` 0:0 and bit `_EVENTS` 1:1 [615 tree: src/nvidia/interface/gmcapi/gmcapi_system_info.h:200-203].
- The 615 kernel sets both bits in the `GSP_INIT` system information [615 tree: src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:5203-5206].
- Everything that layer carries (`GSP_RM_CONTROL`, `GSP_RM_ALLOC`, all control and alloc layouts, events, RPC numbers, the internal handles) is outside the stable promise [docs/caraxes/gsp-abi-615.md §3].
- The device refuses RM RPCs with `NV_ERR_NOT_SUPPORTED` from a client without the RPC bit and sends no RM event to one without the EVENTS bit [docs/caraxes/gsp-plan/boot-exchange.md Status B7].

## Consequence for Caraxes

- The identity the driver acts on is the three BOOT registers plus the VBIOS and boot-model registers of the HAL they select; PCI ids and names are free (see [identity-and-costume.md](identity-and-costume.md)).
- `NV_PMC_BOOT_1` must read 0 in the VGPU field and `NV_PMC_BOOT_0` must stay constant for the life of the device.
- Every guest image needs `caraxes.conf` with the three registry keys until the device runs the CeUtils and watchdog channels.
- Every static-configuration key the device adds must match the handler's type and index rules exactly, or the guest fails init.
- Transport details are in [gsp-transport.md](gsp-transport.md), the stable and unstable layers in [abi-baseline.md](abi-baseline.md), the directly touched registers in [windows-and-traps.md](windows-and-traps.md), USERD and page-table placement in [memory-model.md](memory-model.md).

## Open

- Which static keys and values the real TU106 GSP-RM 615 sends in its `GSP_INIT` response (`POWER_FEATURES`, `CHIP_SUB_REV`, `SKU_*`, `ECID_INFO`) is unmeasured [docs/caraxes/gsp-plan/boot-exchange.md §7 Q1].
- What a real GSP-RM does for a client that does not opt in to unstable RPCs is not in the tree [docs/caraxes/gsp-plan/boot-exchange.md §7 Q2].
- Which `RMInstLoc*` fields the GSP-client kernel RM honours is unverified [docs/caraxes/open-questions.md §32].
- The registry keys the device only traces have no emulated GSP-RM behaviour yet [docs/caraxes/gsp-plan/boot-exchange.md §3.2].
