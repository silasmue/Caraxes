<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Kernel RM: HAL selection and the registry

The kernel RM decides which chip it drives and in which mode from three BAR0 registers,
before it reads anything else, and takes its configuration from module options and
registry keys. A synthetic device has to satisfy that choice exactly; everything the
guest later believes about its GPU follows from it.

## The three boot registers

| Register | BAR0 offset | Carries | Read by |
|---|---|---|---|
| `NV_PMC_BOOT_0` | 0x0 | architecture, implementation, revision (chip id 0) | `gpumgrGetGpuHalFactor` (`src/nvidia/src/kernel/gpu_mgr/gpu_mgr.c`), `rm_is_supported_device` (`src/nvidia/arch/nvalloc/unix/src/osapi.c`) |
| `NV_PMC_BOOT_42` | 0xA00 | architecture in bits 29:24 (`TU100` = 0x16), chip id in 29:20 (chip id 1) | the same two, `halmgrGetHalForGpu(pmc_boot_0, pmc_boot_42)` |
| `NV_PMC_BOOT_1` | 0x4 | bits 17:16 the vGPU field: 0 bare metal, `_VF` (2) a vGPU guest | `gpumgrGetGpuHalFactor` |

- Definitions: `src/common/inc/swref/published/nv_ref.h`. No PCI device id enters the
  choice; the kernel reads the chip from the card ([identity-and-costume](identity-and-costume.md)).
- `osHandleGpuLost` (`src/nvidia/arch/nvalloc/unix/src/osinit.c`) rereads `BOOT_0` and
  declares the GPU lost when it differs from the cached value, so an emulated value is fixed
  for the life of the device.

## The runtime variants

| Variant | Chosen when | On a Turing identity |
|---|---|---|
| `PF_KERNEL_ONLY` (GSP client) | firmware policy allows it and the architecture is TU100 or later | the only one that runs (decision D1) |
| `VF` (vGPU guest) | `BOOT_1` says `_VF` | dead: `initRpcInfrastructure_VGPU` (`src/nvidia/src/kernel/vgpu/rpc.c`) needs the SR-IOV plugin offload, which `PDB_PROP_GPU_VGPU_OFFLOAD_CAPABLE` grants from AD10x on; the VF doorbell HALs are stubs for TU10X |
| `PF_MONOLITHIC` | otherwise | absent from the open tree except a "Falling back to monolithic RM" notice in `osinit.c` |
| `UCODE` | a GSP or DCE build | never in a kernel build |

- A GSP client loads `gsp_tu10x.bin` (TU10x through GA100) or `gsp_ga10x.bin` (GA10x on,
  including GB20x; `src/nvidia/arch/nvalloc/common/inc/nv-firmware.h`) and accepts only an
  image whose `.fwversion` equals its own version string (`src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c`).
  The synthetic device reads none of it; nothing executes ([gsp-boot-exchange](gsp-boot-exchange.md)).

## Module options and registry keys

- Module options `NVreg_*` and `NVreg_RegistryDwords="Key=value;..."` are parsed in
  `kernel-open/nvidia/nv-reg.h` and `src/nvidia/arch/nvalloc/unix/src/registry.c`; the keys
  are named in `src/nvidia/interface/nvrm_registry.h`.
- In 615 every registry entry also travels to GSP-RM inside `GMCAPI_CMD_GSP_INIT` as
  `REGKEY_NAME`/`REGKEY_VALUE_*` NVKV pairs (`kgspInitRm` path in `kernel_gsp.c`; 44 entries
  in the captured request). The device honours `RmLogonRC`, `RMSplitVasMgmtServerClientRm`
  and `EnableGpuFirmwareLogs` and traces the rest.

| Key or option | Why a VFIO Leandro guest sets it |
|---|---|
| `NVreg_GpuInitOnProbe=1` | `RmInitAdapter` runs at probe instead of first open |
| `RmDisableGlobalCeUtils=1` | no global CeUtils object and no copy-engine memset test on a channel whose VA space the device never learns (`src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr.c`) |
| `RMDisableScrubOnFree=1` | freed guest video memory is not scrubbed through a copy engine (`mem_mgr.c`) |
| `RmRcWatchdog=0` | no watchdog GR channel (`src/nvidia/src/kernel/gpu/gr/kernel_watchdog.c`) |
| `RMInstLoc` | bits 17:16 place RM-owned USERD (0 default, 1 coherent sysmem, 2 non-coherent, 3 vidmem); `_PDE`, `_BAR_PDE`, `_PTE`, `_BAR_PTE` fields place page tables (`src/nvidia/src/kernel/gpu/mmu/kern_gmmu.c`) |
| `uvm_page_table_location` | the UVM module's page-table aperture, `vid` or `sys` (`kernel-open/nvidia-uvm/uvm_mmu.c`) |

- The quickstart puts the three keys into `/etc/modprobe.d/caraxes.conf`; without them the
  TU106 boot stops at the CeUtils memset and the GB206 at `numGpcs > 0` in `krcWatchdogInit`.
- These keys are configuration, not driver changes; decision D17 allows only the page-table
  location as a deviation from bare metal.

## What the kernel RM reads from the device

| Item | Source |
|---|---|
| PCI ids and config dwords | the BAR0 mirror `NV_PCFG` at 0x88000 (`gpuGetIdInfo_GM107`, `src/nvidia/src/kernel/gpu/arch/maxwell/kern_gpu_gm107.c`) |
| VBIOS and the FWSEC image | the PROM window at BAR0 0x300000 (`kgspExtractVbiosFromRom_HAL`) |
| static configuration (name, GID, FB, engines, features, 1608 bytes internal) | the NVKV response of `GMCAPI_CMD_GSP_INIT` |
| FIFO device info | `NV2080_CTRL_CMD_FIFO_GET_DEVICE_INFO_TABLE` (`kfifoGetHostDeviceInfoTable_KERNEL`, `src/nvidia/src/kernel/gpu/fifo/kernel_fifo.c`) |
| class list | the generated per-chip lists in `src/nvidia/generated/g_gpu_class_list.c` |
| interrupt table | `NV2080_CTRL_CMD_INTERNAL_INTR_GET_KERNEL_TABLE` (`src/nvidia/src/kernel/gpu/intr/intr.c`) |
| BAR sizes | `NV2080_CTRL_CMD_BUS_GET_PCI_BAR_INFO` (`src/nvidia/src/kernel/gpu/bus/kern_bus.c`) |

## Depth

- [research/driver-structure.md](../research/driver-structure.md): the cited 615 lines.
- [caraxes/driver-modes.md](../caraxes/driver-modes.md), [caraxes/init-map.md](../caraxes/init-map.md):
  from PCI probe to the first RPC.
- [caraxes/kernel-rm-hw-surface.md](../caraxes/kernel-rm-hw-surface.md): what the kernel RM
  touches directly besides GSP.
