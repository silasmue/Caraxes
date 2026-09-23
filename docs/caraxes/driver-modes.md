<!-- SPDX-License-Identifier: MIT -->
# How the stock driver decides what it is talking to

All paths relative to `vendor/open-gpu-kernel-modules` at tag 610.57.04.

## Three registers select the runtime variant

`gpumgrGetGpuHalFactor` (`src/nvidia/src/kernel/gpu_mgr/gpu_mgr.c:1211-1300`) reads,
through the BAR0 mapping handed over at attach time:

| Register | BAR0 offset | Used for |
|---|---|---|
| `NV_PMC_BOOT_0` | 0x000 | `chipId0`; architecture 28:24, implementation 23:20, major 7:4, minor 3:0 (`nv_ref.h:108-137`) |
| `NV_PMC_BOOT_42` | 0xA00 | `chipId1`; HAL selection: `halmgrGetHalForGpu(pmc_boot_0, pmc_boot_42)`, architecture 29:24 (`TU100` = 0x16), chip id 29:20 (`nv_ref.h:150-175`) |
| `NV_PMC_BOOT_1` | 0x004 | `gpumgrGetGpuHalFactorOfVirtual`: field VGPU 17:16, value `_VF` (2) means SR-IOV vGPU guest (`gpu_mgr.c:1003-1020`, `nv_ref.h:139-149`) |

Then (`gpu_mgr.c:1287-1300`):

```
if (RMCFG_FEATURE_PLATFORM_GSP || DCE)   variant = RM_RUNTIME_VARIANT_UCODE          (never in a kernel build)
else if (isVirtual)                       variant = RM_RUNTIME_VARIANT_VF             (vGPU guest)
else if (isFwClient)                      variant = RM_RUNTIME_VARIANT_PF_KERNEL_ONLY (GSP client)
else                                      variant = RM_RUNTIME_VARIANT_PF_MONOLITHIC
```

`isFwClient` is `gpumgrCheckRmFirmwarePolicy(bRequestFwClientRm, pmc_boot_42)`, which
requires architecture ≥ TU100 (`_gpumgrIsRmFirmwareCapableChip`, `gpu_mgr.c:1075-1078`)
and the OS-level decision `request_fw_client_rm` (from `gpumgrGetRmFirmwarePolicy`,
`gpu_mgr.c:1042-1071`: firmware-capable chips are enabled by default; the regkey
`NVreg_EnableGpuFirmware` can disable).

The same three registers are read three times before that: `rm_is_supported_device`
(`src/nvidia/arch/nvalloc/unix/src/osapi.c:3727-3830`: "fallen off the bus" if
BOOT_0 and BOOT_42 are both 0xFFFFFFFF; legacy-architecture refusal; HAL lookup),
`RmInitPrivateState` (`osinit.c:1508-1616`: caches them in `nv_priv_t`, derives the
DMA address width from the `GpuArch` for BOOT_42), and `gpuDetermineVirtualMode`
(`src/nvidia/src/kernel/gpu/gpu.c:4486-4555`, asserting consistency with the early
detection). `osHandleGpuLost` (`osinit.c:341-411`) re-reads BOOT_0 and declares the
GPU lost if it changed, so the emulated value must be stable.

## Variant 1: vGPU VF guest — dead on Turing (and Ampere)

This would have been the cheapest transport: no firmware, a control/response buffer
in guest RAM, a doorbell register, and the same RPC function set as GSP. The code is
present and compiled unconditionally (`src/nvidia/src/kernel/vgpu/rpc.c`,
`initRpcInfrastructure_VGPU` :1663-1776, `vgpuGspSetupBuffers` :1497-1620,
`_vgpuGspSetupCommunicationWithPlugin` :1311-1454, control/response buffer layouts in
`src/nvidia/inc/kernel/vgpu/dev_vgpu.h:199-320`). But:

1. Only the "GSP plugin offload" flavour survives: `initRpcInfrastructure_VGPU` returns
   `NV_ERR_NOT_SUPPORTED` unless `bGspPlugin` (`rpc.c:1685-1690`), and
   `vgpuGspSetupBuffers` returns `NV_ERR_NOT_SUPPORTED` unless
   `IS_VGPU_GSP_PLUGIN_OFFLOAD_ENABLED` (`rpc.c:1507-1510`).
2. `bVgpuGspPluginOffloadEnabled` on a VF guest is the chip property
   `PDB_PROP_GPU_VGPU_OFFLOAD_CAPABLE` (`src/nvidia/src/kernel/gpu/gpu_registry.c:138-143`),
   which the generated NVOC tables set **true only for AD102 and later**
   (`src/nvidia/generated/g_gpu_nvoc.c:415-425`, chip list AD102…GR102). It is false for
   TU10X and GA10X.
3. The transport HAL entries `rpcVgpuGspRingDoorbell` and
   `rpcVgpuGspWriteScratchRegister` are `_STUB` for TU10X and GA100
   (`src/nvidia/generated/g_rpc_private.h:270-292`, tables at :4006-4011 and
   :4209-4214); the GA102+ implementation writes
   `NV_VIRTUAL_FUNCTION_PRIV_MAILBOX_SCRATCH` and `NV_VIRTUAL_FUNCTION_PRIV_DOORBELL`
   (`src/nvidia/src/kernel/vgpu/arch/ampere/rpcga102.c`).
4. Failure is fatal: `vgpuCreateObject` asserts and exits on `initRpcInfrastructure_VGPU`
   failure (`src/nvidia/src/kernel/vgpu/objvgpu.c:97-103`).

Presenting an Ada identity to a Turing GPU is not an option (D2 in
[README.md](README.md)): the kernel RM's HAL and class lists, and userspace's SASS
target, would follow the fake identity.

## Variant 2: monolithic PF — absent

With firmware disabled, `RmInitAdapter` logs "Falling back to monolithic RM"
(`osinit.c:2136-2144`), but the open tree ships only the kernel-side HAL objects; the
physical engine implementations live in GSP-RM. `rm_is_supported_device` refuses
non-firmware-capable chips outright (`osapi.c:3760-3800`: "does not include the required
GPU System Processor (GSP)"). For a Turing identity this variant cannot initialize.

## Variant 3: PF kernel-only (GSP client) — the target

`RmInitAdapter` (`osinit.c:1971-2378`) fetches `gsp_tu10x.bin` (`RmFetchGspRmImages`,
:2022), attaches the GPU (`osInitNvMapping`, :2048), and calls `kgspInitRm`
(:2115). Everything in [gsp-boot-turing.md](gsp-boot-turing.md) happens inside
`kgspInitRm`; everything in [gsp-rpc-protocol.md](gsp-rpc-protocol.md) starts at its
end. The kernel RM keeps direct control of a set of hardware blocks after boot
([kernel-rm-hw-surface.md](kernel-rm-hw-surface.md)); all object semantics go through
`GSP_RM_ALLOC` / `GSP_RM_CONTROL` / `FREE` / `DUP_OBJECT` RPCs, which is where the
Leandro backend plugs in.

## What the device does *not* need to be

- It does not need to run FWSEC, Booter, or GSP-RM. Signatures, HS ucode and the
  29 MB firmware image are only inputs to CPU-side bookkeeping (radix3 tables, WPR
  meta); the device consumes none of them. The guest still needs the firmware file
  because `RmFetchGspRmImages` fails without it and `.fwversion` must equal the driver
  version (`kernel_gsp.c:5720-5774`).
- It does not need the CPU sequencer (`GSP_RUN_CPU_SEQUENCER` is unhandled in
  610.57.04, `kernel_gsp.c:1587-1600`).
- It does not need interrupts for `kgspInitRm`: the init handshake polls. Interrupts
  become necessary at `osVerifySystemEnvironment` (`osinit.c:2212-2234`) and for
  event delivery afterwards.
