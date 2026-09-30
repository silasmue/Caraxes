<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Limits: MIG, ECC and vGPU in pure emulation

Operator's direction, 2026-09-25: "test the MIG, vGPU and ECC commands". Each family is modelled
from the kernel source on the rungs that have it, then nvidia-smi drives the stock guest until
something breaks. This file records every limit found.

- **Rig.** vm8: a copy of vm7's overlay (610.57.04 guest with the caraxes.conf registry keys).
  `CARAXES_TAP=none`, pure emulation. No host RM, no reservation, no GPU.
  Records: `caraxes/runs/limits-2026-09-25/` (`run.sh <dir> <scenario> [NAME=value]`,
  scenarios in `scenarios/`).
- **Rungs.**
  - GH100: `CARAXES_COSTUME=h100`, `chips/gh100` over GB206's FSP boot model.
  - GA100: `CARAXES_COSTUME=a100`, `chips/ga100` over tu106, new. GA100 boots the TU10x GSP
    image (`nv-firmware.h:118-125`) and uses the TU102 FB-size HAL
    (`g_kern_mem_sys_nvoc.c:363-367`).
  - TU106 VF: `CARAXES_COSTUME=rtx2070vf`, `chips/tu106vf`, for the vGPU guest test.
- **Models.** `crates/vfio-user-nvgpu/src/families/`:
  - `ecc.rs` covers the `ecc` and `row_remap` families.
  - `mig.rs` covers `mig` and `mig_partitioning`.
  - `mod.rs` holds the persistent record, the InfoROM analogue: `--persist`, which becomes
    `gpu-persist.toml` in the VM directory.

  A family answers only when the chip lists it under `[features] present` and has an `[ecc]`
  or `[mig]` table.
- **Citations.** Line numbers are 615.71.09 unless marked 610. The guest runs 610.57.04, so
  its dmesg lines cite 610.

## 1. ECC (`ecc`, `row_remap`)

### Model

- **Mode.**
  - Current: `QUERY_ECC_STATUS` sets `units[].enabled` (`ctrl2080gpu.h:1120`).
  - Pending: `QUERY_ECC_CONFIGURATION.currentConfiguration` (`:1337`).
  - `SET_ECC_CONFIGURATION` (`:1366`) stores the pending setting. A GPU reset or a power-on
    applies it. Values other than 0 and 1 are refused with `INVALID_ARGUMENT`.
- **Units.** Only unit 0x1D (GSP) is supported, because it is the only unit the open headers
  name (`ctrl2080gpu.h:1123-1126`).
- **Counters.** Volatile, aggregate, SRAM-unique, repair status and the unrepairable flag are
  all 0. A non-zero `dbe` would make UVM fault channels.
- **Counter resets.** `-p 0/1` (`RESET_ECC_ERROR_STATUS`, `:1397`) returns `NV_OK`.
- **InfoROM.**
  - The "ECC" object has version 1.0 (invented).
  - `QUERY_INFOROM_ECC_SUPPORT` returns `NV_OK`.
  - Without the object, NVML reports ECC unsupported (run `ecc/gh100-1`).
- **Row remapper.** `FB_GET_REMAPPED_ROWS` (`ctrl2080fb.h:1961`) returns 0 entries. The
  histogram (`:2401`) is empty, and the offlined pages are empty.
- **Injection refused.** `NV208F FB_ECC_SET_KILL_PTR` and `_SET_WRITE_KILL` are refused, since
  there is no DRAM to corrupt.
- **FB info.** `bEnabledEccFBPA` in the memsys static config follows the current mode.

### Verdicts (GH100: `ecc/gh100-1b`, `ecc/gh100-2`; GA100: pending, see section 5)

| Command | Before the model | With the model |
|---|---|---|
| `nvidia-smi -q -d ECC` | every field N/A | works: Enabled/Enabled, every counter 0, repair and unrepairable "No" |
| `--query-gpu=ecc.mode.*,ecc.errors.*` | empty | works: `Enabled, Enabled, 0, 0, 0, 0` |
| `-q -d ROW_REMAPPER` | N/A | works: 0 rows, pending No, failure No; the histogram shows 0 banks (the bank count needs a capture) |
| `--query-remapped-rows` | `[N/A]` | works: `0, 0, No, No` |
| `-q -d PAGE_RETIREMENT` | not run | works: 0/0, no pending blacklist |
| `-e 0` | "ECC features not supported" | works: "Disabled ECC support ... Reboot required."; current Enabled, pending Disabled |
| `-e 1` (after the reboot) | as above | works: current Disabled, pending Enabled |
| `-e 1` again | | works: "already Enabled" |
| `-e 2` | | caught by nvidia-smi ("Invalid ECC configuration code: 2"); the device's own refusal is unit-tested |
| `-p 0`, `-p 1` | "not supported" | works: counters reset (they are 0) |
| reboot (next VM boot) | | works: the pending mode is in force (`persist` record) |
| `-r` (GPU reset) | "successfully reset" (nothing to apply) | works, and does not fail closed: the kernel tears down, the PCI function-level reset reaches the device (`BootModel::reset`), the pending mode is applied, the second `RmInitAdapter` succeeds, and ECC reads Enabled/Enabled |

- **Found: `-r` is not a limit.** A GPU reset in pure emulation is a full re-boot of the
  modelled GSP. That is also the reset path for the GA100 MIG mode.
- **Needed for real execution.**
  - Real counters and the unit list come from the host's ECC state. The executing RTX 2070
    has no ECC (GDDR6 without ECC), so pure emulation and this rig report a model.
  - A data-center host card would provide them through a forward of `QUERY_ECC_STATUS` and
    the counters. The mode setter must never reach a shared host card.

## 2. MIG (`mig`, `mig_partitioning`)

### Model

- **Static tables at load.** `kmigmgrLoadStaticInfo_KERNEL` treats every one of these as fatal:
  - the partitionable engines
  - the skyline
  - the per-swizzId FB ranges in VMMU segments
  - the compute profiles
  - the GI profiles
  - a VMMU segment size of 32 MiB, not 0
- **Profiles.** Scaled to the rung's 4 GiB and seven GPCs in an eight-slot span:

  | Rung | Profiles | Engines |
  |---|---|---|
  | GH100 | 7g, 4g, 3g, 2g, 1g, 1g+me. swizzId 14 is rejected by the hardware. | GB206's |
  | GA100 | 7g, 4g, 3g, 2g, 1g. No +me: the TU106 has no NVJPG or OFA. | GR, three async CEs, three NVDECs |

- **Mode.**
  - `SET_PARTITIONING_MODE` (`ctrl2080gpu.h:3290`) goes to `INTERNAL_MIGMGR_SET_PARTITIONING_MODE`.
    `GET_SMC_MODE` reports ENABLED, DISABLED or `*_PENDING`.
  - GH100 has resetless MIG (`g_gpu_nvoc.c:403-413`). The mode applies at once and is GSP
    runtime state, so it is off after every boot. `families/mod.rs` explains why keeping it
    breaks (run `mig/gh100-6`).
  - GA100 keeps the mode pending until a reset or a power-on. BAR0 then latches the devinit
    bit NV_PBUS_SW_SCRATCH(1) 15:15 (`kmigmgr_ga100.c:42-53`). That bit is the kernel's only
    way to see MIG on at load (`kernel_mig_manager.c:1203-1204`).
- **GPU instances.**
  - `GET_PARTITION_CAPACITY` (`:3195`) accounts for geometry and engines.
  - `INTERNAL_MIGMGR_SET_GPU_INSTANCES` creates or destroys an instance. The model picks the
    swizzId and a per-VM UUID.
  - `_EXPORT_GPU_INSTANCE` reports the engines, the GPC mask and the VEIDs.
  - `_GET_GPU_INSTANCES` answers for `-lgi`.
- **Copy engines of a GPU instance (GH100).** `INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_MIG_GPU_INSTANCE`
  returns one PCE per LCE, and `CE_UPDATE_PCE_LCE_MIG_MAPPINGS` records the mapping
  (`kmigmgr_gh100.c:98-168`, `kernel_ce_gh100.c:1340-1411`).
- **Compute instances.**
  - The subscription objects 0xC637/0xC638 are per-VM state, allocated through `GSP_RM_ALLOC`.
  - `NVC637 EXEC_PARTITIONS_CREATE/DELETE/EXPORT` (`ctrlc637.h:200, :236, :317`) slice the
    instance's GPCs.
- **GR global buffers.** The GR global buffer sizes are 0 instead of NV_U32_MAX.
  - Each GPU instance reserves their sum in its context-buffer pool (`ctx_buf_pool.c:315`).
  - NV_U32_MAX there means 4 GiB per buffer (run `mig/gh100-4`).

### Verdicts (GH100: `mig/gh100-1` to `gh100-7`; GA100: pending, see section 5)

| Command | Result | Where it breaks |
|---|---|---|
| `--query-gpu=mig.mode.*` | works: Disabled/Disabled at boot | |
| `-mig 1` | works on GH100: Enabled/Enabled at once (resetless) | |
| `-q -d MIG` | not a `-d` class in this nvidia-smi ("Failed to parse") | nvidia-smi |
| `mig -lgip` | works: 1g 7/7 at 0.44 GiB, 1g+me 1/1, 2g 3/3, 3g 2/2, 4g 1/1, 7g 1/1 at 3.50 GiB; SM, CE, DEC, ENC, JPEG and OFA counts per profile | |
| `mig -lgipp` | works: placements `{0..6}:1`, `{0,2,4}:2`, `{0,4}:4`, `{0}:4`, `{0}:8` | |
| `mig -cgi <p>` for every profile | works: GI created with the right placement (7g 0:8, 4g 0:4, 3g 0:4, 2g 0:2, 1g 0:1) | |
| second `mig -cgi 19` | caught by the model: "Insufficient Resources" (the rung has one GR engine) | `families/mig.rs` `available` |
| `mig -lgi` | works | |
| `mig -lcip` | works: 1g, 16 SMs, 1 CE | |
| `mig -cci 0 -gi 7`, `-cgi 19 -C` | breaks: "Insufficient Resources" | the kernel's golden-image channel for the compute instance: `numGpcs > 0` (`kernel_graphics_object.c:192`) from `kgraphicsCreateGoldenImageChannel` (`kernel_mig_manager.c:5555`); pure emulation has no GR (floorsweeping `gpcMask` 0) |
| `mig -lci` | "No compute instances found" (kernel view). GSP-RM (the model) keeps the compute instance, because the kernel returns before undoing it (`gpu_instance_subscription.c:559-560`) | |
| `nvidia-smi -L` | lists the GPU only, no MIG device | follows from the compute instance |
| `mig -dci` | "Not Found" (the kernel has no compute instance) | |
| `mig -dgi` | breaks: "Insufficient Resources" when the last GI goes; "In use by another client" when GSP-RM still holds the leaked compute instance | last GI: `kmigmgrSetMIGState` disable (`kernel_mig_manager.c:7910-7914`) rebuilds GR0 in legacy mode and its golden channel (`:6893`), the same `numGpcs > 0` |
| `-mig 0` | works when no GI exists (Disabled/Disabled); "In use by another client" while the leaked GI exists | the model refuses `IN_USE`, as GSP-RM does |
| CUDA on a MIG device | not reached: no MIG device (no compute instance); `cuInit` returns 100 (NO_DEVICE) | |
| reboot with MIG on (GH100) | the model resets resetless MIG at power-on (run `mig/gh100-7`); before the fix, `-cgi` was "Not Supported" (`kernel_mig_manager.c:7984`) | fixed |

- **First failure that matters.** Creating a compute instance. MIG's own bookkeeping works end
  to end up to that point: mode, profiles, placements, GPU instance create and destroy on
  GSP's side.
- **Needed for real execution.**
  - A GR engine the kernel can build a golden context on. That takes floorsweeping masks with
    GPCs, a GR channel whose context the FECS saves, and one GR engine per GPU instance.
    Pure emulation executes no GR.
  - The host path executes on an RTX 2070, which has one GR engine and no SMC.
  - MIG isolation (memory slices, a CE crossbar per instance, separate GR engines) needs
    hardware SMC. A model can only keep books. At most one GPU instance fits on this rig.

## 3. vGPU (`vgpu`, `vgpu_paths`)

- **What the kernel sends as a vGPU host.** Nothing a stock guest can reach.
  - The 27 `vgpu` and 7 `vgpu_paths` controls ([gsp-plan/e-families.md](gsp-plan/e-families.md)
    §3.7, §3.26) have their callers in `virtualization/kernel_vgpu_mgr.c`, `vgpuconfigapi.c` and
    `kernel_hostvgpudeviceapi.c`.
  - Those callers run only in a vgpu-kvm build. The open driver compiles vGPU hosting out:
    `os_is_vgx_hyper` and `os_is_grid_supported` are FALSE without NV_VGX_HYPER or
    NV_GRID_BUILD (`kernel-open/nvidia/os-interface.c:1517-1552`).
  - The one MIG-path caller, `BOOTLOAD_GSP_VGPU_PLUGIN_TASK` (`kernel_mig_manager.c:7077`),
    needs vGPU GSP plugin offload.
  - Traces of every run: 0 vGPU controls reached the device.
- **What a stock guest sees.**
  - `nvidia-smi vgpu` prints "No supported devices in vGPU mode" (NVML, no device involved).
  - `NV0080_CTRL_CMD_GPU_GET_VIRTUALIZATION_MODE` is answered by the guest kernel itself
    (`device_ctrl.c:220-250`): NMOS (passthrough) in an HVM guest, with `isGridBuild` FALSE.
- **SR-IOV host paths.** Refused on every chip, and the refusal names the reason (`chipctl.rs`
  `why_absent`): no vGPU host or guest in the open driver, and this device is no PF to split.
  Unit test: `vgpu_host_paths_are_refused_with_the_reason`.
- **Telling the guest it is a vGPU guest** (TU106 with NV_PMC_BOOT_1_VGPU 17:16 = VF,
  `nv_ref.h:146-149`). The expected path, from the source:
  1. `rm_is_supported_device` reads BOOT_1 (`osapi.c:3777`) and calls
     `rm_is_vgpu_supported_device` (`osapi.c:3855`).
  2. In an HVM guest with the VF bit set, and outside a GRID build, that returns NOT_SUPPORTED
     (`os-hypervisor.c:955-994`).
  3. The guest prints "The NVIDIA vGPU ... is not supported by open nvidia.ko" and
     `nv_pci_probe` fails (`nv-pci.c:2339`).
  4. `RmInitAdapter` is never reached.

  Even past that gate, the VF variant would be no GSP client: it talks to a host plugin through
  the vGPU RPC channel (`vgpu/rpc.c`). The run is pending (section 5).
- **Needed for real execution.** A vGPU host driver (vgpu-kvm, closed), SR-IOV on the host
  card and a GRID guest driver. None of these is in this project's scope, and the device
  isolation rules forbid splitting the host card.

## 4. What stays impossible on this rig

- GR work of any kind in pure emulation: CUDA contexts, MIG compute instances, golden
  contexts.
- Real ECC counters, and ECC mode changes that reach silicon (the RTX 2070 has no ECC).
- More than one MIG GPU instance at a time (one GR engine), and MIG isolation of any kind.
- vGPU as host or guest with the open driver.

## 5. Runs

| Run | Rung, scenario | Outcome |
|---|---|---|
| `before/gh100-survey` | GH100, survey, before the models | every ECC/MIG field N/A, "not supported" |
| `ecc/gh100-1` | ecc-1 | ECC unsupported: no InfoROM ECC object; object added |
| `ecc/gh100-1b` | ecc-1 | ECC read side and `-e 0` work |
| `ecc/gh100-2` | ecc-2 (next boot) | mode applied at boot; `-e 1`, `-r` applies it |
| `mig/gh100-1` | mig-1 | `-mig 1`, `-lgip` |
| `mig/gh100-2`, `-3` | mig-2 | `-cgi` "Insufficient Memory": the GI's CE mapping refused |
| `mig/gh100-4` | mig-2 | CE mapping answered; `-cgi` still "Insufficient Memory": ctx buffer pool (NV_U32_MAX) |
| `mig/gh100-5` | mig-2 | GI created; CI breaks at the golden-image channel |
| `mig/gh100-6` | mig-3 | MIG mode kept across boot: GSP on, kernel off, `-cgi` "Not Supported"; fixed |
| `mig/gh100-7` | mig-3 | every GI profile created; `-dgi` and CI break at `numGpcs > 0` |
| GA100, vGPU guest, TU106 check | ga100-1, ga100-2, vgpu-guest, `tools/emu-boot-check.sh` | pending: the operator stopped VM boots for host memory (2026-09-25); scenarios ready |
