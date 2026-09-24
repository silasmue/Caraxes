<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# The feature families: what TU106 refuses because it lacks the feature, implemented from source for the chips that have it

Category E of [../gsp-surface.md](../gsp-surface.md) and the 102 category D rows that
[d-device-model.md](d-device-model.md) §7 moved to E are refused today, one rule per feature
family (`crates/vfio-user-nvgpu/src/chipctl.rs:399-409`, label "refused (feature absent on
this chip)"). They are refused because the RTX 2070 does not have the feature, not because the
device could not answer them. Decision D14 keeps the device open to other chips and driver
versions (per-chip data under `crates/vfio-user-nvgpu/chips/<chip>/`, boards under `boards/`).
This document plans each family for the chips that do have it, from the 615.71.09 kernel
source. Where the source does not settle an answer, it says which capture or trace would.
The capture kit for rented cloud GPUs and for the workplace Blackwell card is in
[lab-kit.md](lab-kit.md).

The 28 families hold 302 controls in 615.71.09: 200 in the 12 E families
(`src/ver/v615/surface.rs:16`, `E_INVENTORY`) and 102 in the 16 families of the D rows
(`src/ver/v615/dmodel.rs:3719-3998`, `D_FAMILIES`). All crate paths in this document are
relative to `crates/vfio-user-nvgpu/`. Kernel paths without a prefix are relative to
`src/nvidia/src/kernel/` of the 615 tree (`/home/silas/git/Leandro/vendor/open-gpu-kernel-modules`,
`version.mk:1` 615.71.09). Other 615 paths carry their prefix:
`src/nvidia/generated/`, `src/nvidia-modeset/src/`, `src/common/` and the SDK headers under
`src/common/sdk/nvidia/inc/ctrl/`.

## Summary

- **What is sent on a chip that has the feature.** Across the 28 families, 199 of the 302
  controls have a call site in the 615 kernel. The rest (103) are user-space only, or have no
  caller at all. All call sites are in the per-family tables below.
- **Two E families are not "never sent" on GB206.** A stock guest sends controls from `cc`
  and `ccu` at every boot on the synthetic GB206:
  - `INTERNAL_CONF_COMPUTE_GET_STATIC_INFO` 0x20800AF3, from the state init of the
    ConfidentialCompute engine (`gpu/conf_compute/conf_compute.c:585-591`).
  - `INTERNAL_CCU_GET_SAMPLE_INFO` 0x20800AB2 and `INTERNAL_CCU_UNMAP` 0x20800AB4, from
    `kccuStateLoad` (`gpu/ccu/kernel_ccu.c:410-450`, `gpu/ccu/arch/blackwell/kernel_ccu_gb100.c:43-58`).

  The GB206 gate runs show them (`caraxes/runs/blackwell-2026-09-24/n2c/device-events.txt:313`,
  `:17437`, `:17697-17698`, `:17708`; `n2a/device-trace.txt:3124`; `n2b/device-events.txt:367`),
  answered NOT_SUPPORTED. The gate passes anyway. Both engines are in the GB20x child list
  (`gpu/arch/blackwell/kern_gpu_gb202.c:87`, `:93`), and the CC-capable HAL covers GB202 to
  GB207 (`src/nvidia/generated/g_gpu_nvoc.c:534-544`). The honest answers come from source:
  - CC off gives two false booleans, NV_OK (`ctrl2080internal.h:3654-3657`).
  - The CCU needs a sample size, and a map and unmap it only records.

  §3.9 and §3.11 have the details. This is the first work item of the order in §6.
- **Where the chips stand in the kernel's HAL tables.**
  - MIG is HAL-supported on GA100, GH100, GB100, GB102, GB110, GB112, GB202 and GB203
    (`g_gpu_nvoc.c:333-343`). GB205 to GB207 are not on that list, so GB206 is right to name
    `mig` absent.
  - NVLink is HAL-enabled on every chip from TU102 on except TU106, TU116 and TU117
    (`src/nvidia/generated/g_kernel_nvlink_nvoc.c:241-243`). Whether links exist is decided by
    the board's IOCTRL entries (`gpu/nvlink/kernel_nvlink.c:2011-2031`).
- **Which families need hardware.** 10 families stay refused on every chip: rm_test,
  vgpu, internal_uncalled, spdm, gsync, display_embedded, notebook_power, vgpu_paths,
  windows_only and legacy_uncalled. They need no hardware and no work. 10 more can be
  implemented from kernel logic alone: nvlink tier 0, hshub, fabric on a single GPU, the
  cc static info, ccu, display_later, ampere_later, ga10x_pcie_ro, hopper_platform and
  row_remap. The other 8 need a capture or a trace as their truth, or to confirm a design
  made from source: mig with mig_partitioning, dp2x, frl, ecc, c2c, blackwell, and nvlink
  tier 1 with nvlink_sli.
- **Estimates.** The single-GPU work, everything a stock single-GPU client can meet on
  GA10x, AD10x, GH100, GB10x or GB20x, is about 34 engineer-days. The multi-GPU work (NVLink
  links, SLI bridge, FLA and fabric memory) is about 22 more.
- **Order.** §6 gives it: the GB20x boot items first (cc static info, ccu), then Hopper and
  Blackwell data-center CUDA (the nvlink zero-link tier, hshub, fabric, mig), then the GB20x
  display (display_later, dp2x, frl) for Wayland on Blackwell consumer boards.

| # | Family | Src | Ctl | Get | Call sites | Chips with the feature | Truth source | HW | Answer design | Days |
|---:|---|---|---:|---:|---:|---|---|---|---|---:|
| 1 | `rm_test` | E | 13 | 0 | 5 | none in production | the flag (`RM_TEST_ONLY_CODE`) | none | refuse forever | 0 |
| 2 | `nvlink` | E | 93 | 31 | 60 | HAL: TU102, TU104, GA100, GA10x, AD10x, GH100, GB10x, GB20x; links: SXM and bridge boards | tier 0 kernel logic; tier 1 capture of link caps and counters | tier 0 none, tier 1 capture | protocol model, two tiers | 3 + 15 |
| 3 | `hshub` | E | 8 | 4 | 7 | GA100 and later with NVLink or EGM | kernel logic, the HSHUB count per chip from a capture | source, capture check | table from the nvlink model | 1.5 |
| 4 | `fabric` | E | 9 | 2 | 9 | GA100 and later (FLA), NVLink-capable chips (probe) | kernel logic (probe state machine) | source | state machine "no fabric"; FLA with nvlink tier 1 | 2 + 4 |
| 5 | `c2c` | E | 7 | 3 | 1 | GH100 and GB100 on Grace (GH200, GB200) | capture of a Grace system | capture | "C2C disabled" answers | 0.5 |
| 6 | `gsync` | E | 9 | 3 | 8 | boards with a Quadro Sync connector | board protocol (closed) | none | refuse | 0 |
| 7 | `vgpu` | E | 27 | 8 | 17 | vGPU hosts | isolation rule | none | refuse forever | 0 |
| 8 | `mig` | E | 12 | 2 | 12 | GA100, GH100, GB100, GB102, GB110, GB112, GB202, GB203 | profile tables: capture (MIG on) | capture | table model | 8 |
| 9 | `cc` | E | 9 | 2 | 9 | CC: GH100, GB10x, GB20x; APM: GA10x, AD10x | kernel logic (CC off); CC on: firmware secrets | source (off) | static info NV_OK; refusal when off | 0.5 |
| 10 | `spdm` | E | 1 | 0 | 1 | GH100, GB10x, GB20x with CC on | firmware | none | refuse (CC off) | 0 |
| 11 | `ccu` | E | 6 | 1 | 4 | GH100, GB100, GB102, GB20x | kernel logic, sample size from a native trace | source, trace check | ACK and table | 1.5 |
| 12 | `internal_uncalled` | E | 6 | 3 | 1 | none (one Tegra caller) | none | none | refuse forever | 0 |
| 13 | `frl` | D | 4 | 1 | 3 | display HALs C6 (GA10x, AD10x), C8, C9, CA (GB20x) | kernel logic, HDMI 2.1 sink model | source, capture check | display model extension | 4 |
| 14 | `dp2x` | D | 6 | 3 | 4 | chips whose `DP_GET_CAPS` reports DP 2.0 (GB20x) | capture of `DP_GET_CAPS`, the DP 2.1 spec | capture | DP sink and link-training extension | 5 |
| 15 | `display_later` | D | 4 | 0 | 1 | C8, C9, CA and later display HALs | kernel logic | source | record and ACK | 1.5 |
| 16 | `display_embedded` | D | 6 | 3 | 2 | Tegra display, LVDS boards | none | none | refuse | 0 |
| 17 | `notebook_power` | D | 18 | 4 | 18 | notebook boards (ACPI GPS, NVPCF) | platform ACPI | none | refuse or ACK (desktop) | 0 |
| 18 | `ecc` | D | 4 | 0 | 0 | ECC boards (HBM parts, pro GDDR boards) | capture (ECC mode, counters) | capture | pending/current mode record | 1.5 |
| 19 | `row_remap` | D | 2 | 0 | 0 | GA100 and later HBM, GB20x | header, B getters | source | clear an empty table | 0.5 |
| 20 | `mig_partitioning` | D | 4 | 1 | 4 | as `mig` | as `mig` | capture | part of the MIG model | (in 8) |
| 21 | `nvlink_sli` | D | 13 | 1 | 13 | TU102, TU104, GA102 bridges; GA100, GH100 multi-GPU | capture of a bridged pair | capture | part of nvlink tier 1 | 3 |
| 22 | `ampere_later` | D | 3 | 0 | 2 | GA10x and later | kernel logic | source | ACK, channel-model clear, zeros | 1.5 |
| 23 | `ga10x_pcie_ro` | D | 1 | 0 | 1 | GA100 to GA107 | kernel logic | source | ACK | 0.25 |
| 24 | `hopper_platform` | D | 2 | 0 | 2 | GH100, GB100 with Grace or CC | kernel logic | source | copy engine in the device; ACK | 1 |
| 25 | `blackwell` | D | 5 | 0 | 1 | GB10x, GB20x | kernel logic, header | source, capture check | record and ACK | 1.5 |
| 26 | `vgpu_paths` | D | 7 | 0 | 5 | vGPU host or guest | isolation rule | none | refuse forever | 0 |
| 27 | `windows_only` | D | 5 | 0 | 1 | Windows clients | none | none | refuse forever | 0 |
| 28 | `legacy_uncalled` | D | 18 | 2 | 8 | none | none | none | refuse forever (one ACK) | 0 |
| | **total** | | **302** | **74** | **199** | | | | | **34 + 22** |

The columns:
- **Src**: E for a family of `surface.rs`, D for one of `dmodel.rs`.
- **Ctl**: the controls in the family.
- **Get**: the controls whose name marks a getter, by the rule `sweep-list.py` uses (§1).
- **Call sites**: the controls with at least one kernel call site in the 615 tree, not
  counting the vGPU guest RPC table (`vgpu/rpc.c`) and generated headers.
- **HW**: whether hardware is needed at all.
  - "none": no hardware is needed.
  - "source": a pure-source implementation.
  - "source, capture check": designed from source, with a capture that only confirms it.
  - "capture": a capture or trace is the only truth.
- **Days**: engineer-days. Where two numbers are given, the second is the multi-GPU tier.

## 1. Method

- **Control lists.** From the generated tables of the device:
  - `surface.rs:22-355` (`E_FAMILIES`, generated by `caraxes/tools/gsp-tables.py`).
  - `dmodel.rs:3719-3998` (`D_FAMILIES`, generated by `caraxes/tools/gsp-dtable.py` from
    `d-controls.tsv`).

  The ids are the 615.71.09 ones. The 610 lists differ only where
  [../gsp-surface.md](../gsp-surface.md) "After E and A" says so (nvlink 86 against 93,
  vgpu 23 against 27).
- **Kernel call sites.** One `rg -w` of every control name over `src/` and `kernel-open/` of
  the 615 tree. It leaves out:
  - the SDK control headers, where the ids are defined;
  - the generated `g_*_nvoc.c` export tables;
  - the vGPU guest RPC table `vgpu/rpc.c`, which a GSP client never runs. It is listed as
    "vGPU guest RPC table only" where it is the only hit.

  A hit is a place where the kernel names the control. Almost all are
  `pRmApi->Control(...)` or `NV_RM_RPC_CONTROL` sends. A few are other uses of the name: a
  skip list (`rmapi/rmapi_utils.c:198-223`) and the parameter copy tables
  (`rmapi/embedded_param_copy.c`). The per-family tables show the first three call sites and
  how many more there are.
- **Access** comes from the exported control flags of `gsp-surface.py --json` on the 615
  tree:
  - `internal`: `RMCTRL_FLAGS_INTERNAL`, 0x80. User space cannot send the control.
  - `priv`: `PRIVILEGED`.
  - `user`: every other control.
  - `test`: `RM_TEST_ONLY_CODE`.
- **Kind** is a name rule. A name with `GET`, `QUERY`, `IS`, `READ`, `CHECK` or `DESCRIBE`,
  or ending in `_INFO`, `_CAPS`, `_STATUS` or `_COUNTERS`, is a getter, unless it also carries
  a verb that changes state (`SET`, `CLEAR`, `UPDATE`, `MAP`, ...). `caraxes/tools/sweep-list.py`
  applies the same rule when it builds the read-only sweep of the lab kit. It errs towards
  "set", so the sweep never issues a control that changes state.
- **Which chips have a feature**, from three kinds of kernel data:
  - The per-chip child lists (`gpuChildrenPresent_<CHIP>[]` in
    `gpu/arch/<arch>/kern_gpu_<chip>.c`) say which engine objects exist.
  - The NVOC property HALs in `src/nvidia/generated/g_*_nvoc.c` say which chips set a
    property. Each block carries a `/* ChipHal: ... */` comment with the chip list.
  - The display HAL table of nvkms (`src/nvidia-modeset/src/nvkms-hal.c:140-164`) maps each
    display class to its HAL and caps.

  Where the property depends on the board rather than the chip (links, ECC, a sync
  connector, notebook ACPI), this document says so.
- **Truth source** names one of the following:
  - the headers and kernel logic of the 615 tree;
  - a chip capture: `caraxes/tools/lab-capture.sh`, the lab kit's read-only sweep, or
    `nvidia-smi`;
  - a native trace: the LD_PRELOAD tracer of `matrix/traces/610.57.04/` (`lab-kit.md` §4);
  - a value only the closed GSP-RM knows.

## 2. Which chips have what

**The engine objects.** One line per chip, from the child lists of the 615 tree. A 1 means
present; the number after `@` is the line of the entry.

| Chip (child list) | KernelNvlink | KernelMIGManager | ConfidentialCompute | Spdm | KernelCcu | KernelFsp | KernelGsplite |
|---|---|---|---|---|---|---|---|
| TU106 (`turing/kern_gpu_tu102.c:182`) | @205 | @185 | - | - | - | - | - |
| GA100 (`ampere/kern_gpu_ga100.c:144`) | @167 | @147 | @173 | - | - | - | - |
| GA102 (`ampere/kern_gpu_ga100.c:195`) | @218 | @198 | @224 | - | - | - | - |
| AD102 (`ada/kern_gpu_ad102.c:49`) | @72 | @52 | @78 | - | - | - | - |
| GH100 (`hopper/kern_gpu_gh100.c:412`) | @435 | @415 | @439 | @438 | @444 | @440 | - |
| GB100 (`blackwell/kern_gpu_gb100.c:83`) | @106 | @86 | @110 | @109 | @115 | @111 | - |
| GB202, all GB20x (`blackwell/kern_gpu_gb202.c:60`) | @83 | @63 | @87 | @86 | @93 | @88 | @92 (4) |

**What the child lists mean.** A present object is not yet a present feature:
- **NVLink.** TU106 lists KernelNvlink, but its NVLink property HAL leaves TU106 out
  (`g_kernel_nvlink_nvoc.c:241-243`). The kernel then drops the object. Today the device
  also removes it for the guest (the NVLink refusal of `ctrl.rs`,
  [../gsp-surface.md](../gsp-surface.md) §3.E).
- **Confidential Computing.** The ConfidentialCompute object exists from Ampere on. CC
  itself turns on only when a register says so (`gpu/conf_compute/conf_compute.c:104-110`):
  - GH100 reads `NV_PGC6_AON_SECURE_SCRATCH_GROUP_20_CC` (`gpu/arch/hopper/kern_gpu_gh100.c:521-522`).
  - GB100 reads `NV_PMC_ZB_SCRATCH_RESET_2_CC` (`gpu/arch/blackwell/kern_gpu_gb100.c:1154-1155`).

**The property HALs.** From `src/nvidia/generated/`, file:line of the `ChipHal` comment:

| Property | Chips | Where |
|---|---|---|
| `PDB_PROP_GPU_MIG_SUPPORTED` | GA100, GH100, GB100, GB102, GB10B, GB110, GB112, GB202, GB203, GR100, GR102 | `g_gpu_nvoc.c:335` |
| `PDB_PROP_GPU_MIG_GFX_SUPPORTED` | GB10B, GB10C, GB202 to GB207, GB20B, GB20C | `g_gpu_nvoc.c:371` |
| `PDB_PROP_GPU_MIG_TIMESLICING_SUPPORTED` | GB202 to GB207, GB20B, GB20C | `g_gpu_nvoc.c:382` |
| `PDB_PROP_GPU_CC_FEATURE_CAPABLE` | GH100, GB100, GB102, GB10B, GB10C, GB110, GB112, GB202 to GB207, GB20B, GB20C, GR100, GR102 | `g_gpu_nvoc.c:536` |
| `PDB_PROP_GPU_APM_FEATURE_CAPABLE` | GA100 to GA107, AD102 to AD107 | `g_gpu_nvoc.c:547` |
| `PDB_PROP_KNVLINK_ENABLED` | TU102, TU104, GA100 to GA107, AD102 to AD107, GH100, GB100 to GB112, GB202 to GB207, GB20B, GB20C, GR100, GR102 | `g_kernel_nvlink_nvoc.c:243` |
| `PDB_PROP_KNVLINK_ENCRYPTION_ENABLED` (NVLE) | GB100, GB102, GB110, GB112, GR100, GR102 | `g_kernel_nvlink_nvoc.c:400` |
| `PDB_PROP_KNVLINK_NVSWITCH_SUPPORTS_SLI` | GH100 | `g_kernel_nvlink_nvoc.c:361` |
| `PDB_PROP_KHWPM_HES_CWD_SUPPORTED` | GB100, GB102, GB10B, GB10C, GB110, GB112, GB202 to GB207, GB20B, GB20C, GR100, GR102 | `g_kern_hwpm_nvoc.c:246` |
| `PDB_PROP_KFSP_IS_MISSING` (the FSP object is dropped) | GH100, GB100, GB102, GB110, GB112, GB202 to GB207, GR100, GR102 | `g_kern_fsp_nvoc.c:235` |
| `PDB_PROP_GPU_VGPU_OFFLOAD_CAPABLE` | AD102 to AD107, GH100, all GB, GR | `g_gpu_nvoc.c:479` |

**The display HAL table.** From `src/nvidia-modeset/src/nvkms-hal.c:140-164`:

| Class | HAL | Chips | HDMI FRL | Shared generic packets |
|---|---|---|---|---|
| C5 | nvEvoC5 | Turing | FALSE (`nvkms-evo3.c:7945`) | 0 |
| C6 | nvEvoC6 | Ampere | TRUE (`nvkms-evo3.c:8030`) | 0 |
| C7 | nvEvoC6 | Ada | TRUE | 0 |
| C8 | nvEvoC8 | T239 | TRUE (`nvkms-evo3.c:8115`) | 1 |
| C9 | nvEvoC9 | Blackwell | TRUE (`nvkms-evo4.c:2897`) | 1 |
| CA | nvEvoCA | Blackwell GB20X | TRUE (`nvkms-evo4.c:2982`) | 1 |

The data-center chips GA100, GH100 and GB100 have no display head. On them a display family
answers nothing, because the kernel never allocates a display object.

**Cloud parts.** For the lab kit, the rented parts and their device ids, from the 615 tree's
`README.md`:

| Part | Device id | README line | Chip |
|---|---|---|---|
| Tesla T4 | 1EB8 | :354 | TU104 |
| A10G | 2237 | :751 | GA102 |
| A100 | 20B2 | :664 | GA100 |
| L4 | 27B8 | :915 | AD104 |
| L40S | 26B9 | :885 | AD102 |
| H100 SXM | 2330 | :761 | GH100 |
| H200 | 2335 | :764 | GH100 |
| B200 | 2901 | :947 | GB100 |
| GB200 | 2941 | :951 | GB100 with Grace |
| RTX PRO 6000 Blackwell Server Edition | 2BB5 | :977 | GB202 |

The chip column is the operator's mapping. The tree maps no device id to a chip; the kernel
reads the chip from the card. The kit checks the mapping on the instance with
`MC_GET_ARCH_INFO` (`caraxes/tools/lab-rm-query.rs:176-187`, `:209`), the same way the GB206 capture
did (`caraxes/reference/gb206/rm-tools/lab-rm-query.txt:3`).

## 3. The families

Each section has the same parts:
- what the feature is;
- the chips that have it;
- the kernel callers;
- the truth source;
- the design;
- the estimate;
- what a cloud or home validation compares;
- the per-control table.

### 3.1 `rm_test` (13): RM self-tests and error injection

- **What.** Test and injection controls: mark and unmark for reset, the GSP RPC and core
  tests, the GDMA fuzzer, ECC and ctxsw-ucode error injection.
- **Chips.** None in production. Several carry `RMCTRL_FLAGS_RM_TEST_ONLY_CODE`, which
  production GSP-RM builds leave out (`surface.rs:24-49`).
- **Callers.**
  - The kernel unmarks devices on its own teardown paths (`gpu/gpu.c:7090`, `:7180`).
  - It copies the parameters of the GSP test controls (`rmapi/embedded_param_copy.c:740-750`).
  - It calls the GDMA fuzzer only from its own test command (`gpu/subdevice/subdevice_ctrl_gpu_kernel.c:4504`).
- **Truth.** The flag itself.
- **Design.** NOT_SUPPORTED on every chip, as today. Never `present` in any chip file.
- **Estimate.** 0.
- **Validation.** None. The sweep leaves these out: none is a getter.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x208001a9` | `NV2080_CTRL_CMD_GPU_MARK_DEVICE_FOR_RESET` | user test | set | none in the tree |
| `0x208001aa` | `NV2080_CTRL_CMD_GPU_UNMARK_DEVICE_FOR_RESET` | user test | set | `gpu/gpu.c:7090` |
| `0x208001ac` | `NV2080_CTRL_CMD_GPU_MARK_DEVICE_FOR_DRAIN_AND_RESET` | user test | set | none in the tree |
| `0x208001ad` | `NV2080_CTRL_CMD_GPU_UNMARK_DEVICE_FOR_DRAIN_AND_RESET` | user test | set | `gpu/gpu.c:7180` |
| `0x208001e8` | `NV2080_CTRL_CMD_GPU_RPC_GSP_TEST` | user test | set | `rmapi/embedded_param_copy.c:740`, `rmapi/embedded_param_copy.c:1166` |
| `0x208001f2` | `NV2080_CTRL_CMD_GSP_CRYPTO_CONTROL` | priv test | set | `rmapi/embedded_param_copy.c:750`, `rmapi/embedded_param_copy.c:1173` |
| `0x2080123e` | `NV2080_CTRL_CMD_GR_TEST_CTXSW_ERROR_LOGS` | user test | set | none in the tree |
| `0x20803403` | `NV2080_CTRL_CMD_ECC_INJECT_ERROR` | priv | set | none in the tree |
| `0x20803405` | `NV2080_CTRL_CMD_ECC_INJECTION_SUPPORTED` | priv | set | none in the tree |
| `0x20803605` | `NV2080_CTRL_CMD_GSP_GDMA_FUZZ_TEST` | priv test | set | `gpu/subdevice/subdevice_ctrl_gpu_kernel.c:4504` |
| `0x20803606` | `NV2080_CTRL_CMD_GSP_CORE_TEST` | priv test | set | none in the tree |
| `0x208f0510` | `NV208F_CTRL_CMD_FB_ECC_INJECTION_SUPPORTED` | user | set | none in the tree |
| `0x208f1206` | `NV208F_CTRL_CMD_GR_INJECT_CTXSW_UCODE_ERROR` | user test | set | none in the tree |

### 3.2 `nvlink` (93): links, counters, NVLE, INTERNAL_NVLINK

- **What.** Link discovery, training (ALI), peer setup, the HSHUB mux, counters, error
  recovery and Blackwell NVLink encryption (NVLE).
- **Chips.** The HAL property holds on TU102, TU104, GA100, GA10x, AD10x, GH100, GB10x and GB20x
  (`g_kernel_nvlink_nvoc.c:241-243`). Links exist only where the board has IOCTRL units:
  `knvlinkCopyIoctrlDeviceInfo` reads them (`gpu/nvlink/kernel_nvlink.c:2011-2031`), and
  `knvlinkSyncLinkMasksAndVbiosInfo` takes the enabled masks from the VBIOS
  (`gpu/nvlink/kernel_nvlink.c:1670-1708`). The GB206 board has none: its capture shows no
  fabric (`caraxes/reference/gb206/gpu-0000_01_00.0/nvidia-smi-q.txt:261-274`).
- **Callers.** 60 of the 93 controls, almost all in the core-library training and callback
  code:
  - `gpu/nvlink/kernel_nvlinkcorelibtrain.c`
  - `gpu/nvlink/kernel_nvlinkcorelibcallback.c`
  - `gpu/nvlink/kernel_nvlink.c`
  - the per-arch HALs under `gpu/nvlink/arch/`

  The 33 without a caller are user controls for `nvidia-smi nvlink` and the fabric manager:
  counters, error injection, EOM, L1 threshold, PRM access and metric records.
- **Truth.** Two tiers.
  - **Tier 0: an NVLink-capable chip whose board has no links.** This is L4, L40S, the
    GB20x boards and a PCIe H100 without a bridge. It is pure kernel logic. The kernel
    constructs KernelNvlink, asks for the IOCTRL device info and the link masks, finds zero
    links and stops. Every kernel-sent control of the discovery path has one answer: NV_OK
    with empty masks.
  - **Tier 1: linked peers.** Needs the link capabilities, the NVLink version, the
    per-link state machine and the counter layout of a real linked system. The headers
    define the structures, but the values are GSP-RM's. A capture of an NVLink machine is
    the truth.
- **Design.**
  - **Tier 0.** A table answer per discovery control:
    - `GET_IOCTRL_DEVICE_INFO` with zero IOCTRLs;
    - `SYNC_LINK_MASKS_AND_VBIOS_INFO` with zero masks;
    - `GET_LINK_MASK_POST_RX_DET` 0;
    - `ARE_LINKS_TRAINED` with an empty mask;
    - the `GET_NUM_ACTIVE_LINK_PER_IOCTRL` and `GET_TOTAL_NUM_LINK_PER_IOCTRL` pair 0.

    User getters answer NV_OK with zeros where the card does. The native trace already
    shows that shape for `NVLINK_GET_PLATFORM_INFO` (nvml.rs; [../gsp-surface.md](../gsp-surface.md)
    "After E and A").
  - **Tier 1.** A protocol model between two device instances. Each Caraxes device plays one
    GPU, and a shared "link" object between them carries:
    - discovery (the IOCTRL table of each side);
    - the SID exchange (`UPDATE_REMOTE_LOCAL_SID`);
    - ALI training (`PRE_LINK_TRAIN_ALI`, `LINK_TRAIN_ALI`, `ARE_LINKS_TRAINED`);
    - peer setup (`ENABLE_NVLINK_PEER`, `PRE_`/`POST_SETUP_NVLINK_PEER`, `UPDATE_PEER_LINK_MASK`);
    - the HSHUB mux and buffer-ready programming.

    The data path of a peer mapping lands on the host's P2P, or on sysmem when the host has
    no second GPU. NVLE (`GET_NVLE_ENCRYPT_EN_INFO`, `SET_NVLE_ENABLED_STATE`) stays
    disabled unless a CC identity asks for it.
- **Estimate.** Tier 0: 3 days (15 controls, all table answers, plus the tests). Tier 1:
  about 15 days (training state machine, peer setup and the two-instance link object;
  `nvlink_sli` and FLA add their own, §3.21 and §3.4).
- **Validation.**
  - **Tier 0.** On an L4, a PCIe H100 or the RTX PRO 6000 Server Edition: the 31 NVLink
    getters of the sweep on the card's own subdevice, then
    `nvidia-smi nvlink --status` and `-c` under the tracer. Compare the statuses and the
    link masks with the tier-0 answers.
  - **Tier 1.** An HGX H100 or A100 node with two or more GPUs: the same sweep on every GPU,
    and a trace of `nvidia-smi nvlink -s -c -e`. The trace shows the per-link counters and
    the capability words the model has to reproduce.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800a21` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_ENABLE_NVLINK_PEER` | internal | set | `gpu/bus/arch/pascal/kern_bus_gp100.c:390`, `gpu/bus/arch/pascal/kern_bus_gp100.c:407`, `gpu/bus/arch/pascal/kern_bus_gp100.c:564` (+1) |
| `0x20800a24` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_CORE_CALLBACK` | internal | set | `gpu/nvlink/kernel_nvlinkcorelibtrain.c:2028`, `gpu/nvlink/kernel_nvlinkcorelibtrain.c:2120`, `gpu/nvlink/kernel_nvlinkcorelibcallback.c:635` (+15) |
| `0x20800a25` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_UPDATE_REMOTE_LOCAL_SID` | internal | set | `gpu/nvlink/kernel_nvlinkcorelibtrain.c:496`, `gpu/nvlink/kernel_nvlinkcorelibtrain.c:551`, `gpu/nvlink/kernel_nvlinkcorelibcallback.c:1331` |
| `0x20800a29` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_GET_ALI_ENABLED` | internal | get | `gpu/nvlink/arch/hopper/kernel_nvlink_gh100.c:62` |
| `0x20800a42` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_UPDATE_HSHUB_MUX` | internal | set | `gpu/nvlink/kernel_nvlinkcorelibtrain.c:1945` |
| `0x20800a4e` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_PRE_SETUP_NVLINK_PEER` | internal | set | `gpu/nvlink/arch/pascal/kernel_nvlink_gp100.c:293` |
| `0x20800a50` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_POST_SETUP_NVLINK_PEER` | internal | set | `gpu/nvlink/arch/pascal/kernel_nvlink_gp100.c:305`, `gpu/nvlink/arch/hopper/kernel_nvlink_gh100.c:162` |
| `0x20800a5f` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_REMOVE_NVLINK_MAPPING` | internal | set | `gpu/nvlink/arch/pascal/kernel_nvlink_gp100.c:180`, `gpu/nvlink/arch/ampere/kernel_nvlink_ga100.c:159` |
| `0x20800a62` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_SAVE_RESTORE_HSHUB_STATE` | internal | set | `gpu/nvlink/kernel_nvlinkcorelibtrain.c:2000`, `gpu/nvlink/kernel_nvlinkcorelibtrain.c:2260`, `gpu/nvlink/kernel_nvlinkcorelibtrain.c:2313` |
| `0x20800a64` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_PROGRAM_BUFFERREADY` | internal | set | `gpu/nvlink/kernel_nvlinkstate.c:1428`, `gpu/nvlink/kernel_nvlinkcorelibtrain.c:1987`, `gpu/nvlink/kernel_nvlinkcorelibtrain.c:2277` (+1) |
| `0x20800a6e` | `NV2080_CTRL_CMD_INTERNAL_MEMSYS_DISABLE_NVLINK_PEERS` | internal | set | `gpu/bus/arch/maxwell/kern_bus_gm107.c:3765` |
| `0x20800a78` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_UPDATE_CURRENT_CONFIG` | internal | set | `gpu/nvlink/kernel_nvlink.c:725` |
| `0x20800a7d` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_UPDATE_PEER_LINK_MASK` | internal | set | `gpu/nvlink/kernel_nvlink.c:1170` |
| `0x20800a82` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_UPDATE_LINK_CONNECTION` | internal | set | `gpu/nvlink/kernel_nvlink.c:1782` |
| `0x20800a83` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_ENABLE_LINKS_POST_TOPOLOGY` | internal | set | `gpu/nvlink/arch/volta/kernel_nvlink_gv100.c:107` |
| `0x20800a84` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_PRE_LINK_TRAIN_ALI` | internal | set | `gpu/nvlink/kernel_nvlink.c:1827` |
| `0x20800a85` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_GET_LINK_MASK_POST_RX_DET` | internal | set | `gpu/nvlink/kernel_nvlink.c:1906` |
| `0x20800a86` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_LINK_TRAIN_ALI` | internal | set | `gpu/nvlink/kernel_nvlink.c:1873` |
| `0x20800a8e` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_GET_IOCTRL_DEVICE_INFO` | internal | get | `gpu/nvlink/kernel_nvlink.c:2031` |
| `0x20800a8f` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_PROGRAM_LINK_SPEED` | internal | set | `gpu/nvlink/arch/pascal/kernel_nvlink_gp100.c:356`, `gpu/nvlink/arch/volta/kernel_nvlink_gv100.c:429` |
| `0x20800a90` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_ARE_LINKS_TRAINED` | internal | set | `gpu/nvlink/arch/blackwell/kernel_nvlink_gb100.c:492`, `gpu/nvlink/kernel_nvlinkcorelibtrain.c:698`, `gpu/nvlink/kernel_nvlinkcorelibtrain.c:786` (+2) |
| `0x20800a91` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_RESET_LINKS` | internal | set | `gpu/nvlink/kernel_nvlink.c:1391` |
| `0x20800a92` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_DISABLE_DL_INTERRUPTS` | internal | set | `gpu/nvlink/kernel_nvlinkstate.c:1030` |
| `0x20800a93` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_GET_LINK_AND_CLOCK_INFO` | internal | get | `gpu/gpu_fabric_probe.c:249`, `gpu/nvlink/arch/volta/kernel_nvlink_gv100.c:364`, `gpu/nvlink/common_nvlinkapi.c:935` |
| `0x20800a94` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_SETUP_NVLINK_SYSMEM` | internal | set | `gpu/nvlink/kernel_nvlinkcorelibtrain.c:1916` |
| `0x20800a95` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_PROCESS_FORCED_CONFIGS` | internal | set | `gpu/nvlink/arch/pascal/kernel_nvlink_gp100.c:55`, `gpu/nvlink/arch/volta/kernel_nvlink_gv100.c:180`, `gpu/nvlink/arch/ampere/kernel_nvlink_ga100.c:95` |
| `0x20800a96` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_SYNC_NVLINK_SHUTDOWN_PROPS` | internal | set | `gpu/nvlink/kernel_nvlink.c:2185` |
| `0x20800a97` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_ENABLE_SYSMEM_NVLINK_ATS` | internal | set | `gpu/nvlink/kernel_nvlinkstate.c:1457` |
| `0x20800aab` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_HSHUB_GET_SYSMEM_NVLINK_MASK` | internal | get | `gpu/ce/arch/ampere/kernel_ce_ga100.c:608`, `gpu/ce/arch/ampere/kernel_ce_ga102.c:99`, `gpu/nvlink/kernel_nvlinkcorelibtrain.c:2609` |
| `0x20800aad` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_ENABLE_COMPUTE_PEER_ADDR` | internal | set | `rmapi/rmapi_utils.c:223`, `gpu/mmu/kern_gmmu.c:317`, `gpu/nvlink/arch/volta/kernel_nvlink_gv100.c:503` |
| `0x20800aae` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_GET_SET_NVSWITCH_FABRIC_ADDR` | internal | set | `gpu/nvlink/arch/volta/kernel_nvlink_gv100.c:471`, `gpu/nvlink/arch/volta/kernel_nvlink_gv100.c:659`, `gpu/nvlink/kernel_nvlink.c:1515` |
| `0x20800ac7` | `NV2080_CTRL_INTERNAL_NVLINK_GET_NUM_ACTIVE_LINK_PER_IOCTRL` | internal | get | `gpu/nvlink/kernel_nvlink.c:2216` |
| `0x20800ac8` | `NV2080_CTRL_INTERNAL_NVLINK_GET_TOTAL_NUM_LINK_PER_IOCTRL` | internal | get | `gpu/nvlink/kernel_nvlink.c:2246` |
| `0x20800aea` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_POST_FATAL_ERROR_RECOVERY` | internal | set | `gpu/nvlink/kernel_nvlink.c:2690`, `gpu/nvlink/kernel_nvlink.c:2998` |
| `0x20800af6` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_LFM_RM_ACTION` | internal | set | `gpu/nvlink/kernel_nvlink.c:2581` |
| `0x20800b01` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_REPLAY_SUPPRESSED_ERRORS` | internal | set | `gpu/nvlink/arch/blackwell/kernel_nvlink_gb100.c:737` |
| `0x20800b08` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_RC_USER_MODE_CHANNELS` | internal | set | `gpu/nvlink/arch/blackwell/kernel_nvlink_gb100.c:446` |
| `0x20800b9c` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_FORCE_DEBUG_STALL_AND_UPDATE_AMAP` | internal | set | `gpu/nvlink/kernel_nvlink.c:2593` |
| `0x20800bad` | `NV2080_CTRL_CMD_INTERNAL_NVLINK_RESUME_DISABLE_CHANNELS` | internal | set | `gpu/nvlink/kernel_nvlink.c:2720` |
| `0x20803003` | `NV2080_CTRL_CMD_NVLINK_GET_ERR_INFO` | user | get | `gpu/nvlink/arch/hopper/kernel_nvlink_gh100.c:377`, `gpu/nvlink/arch/blackwell/kernel_nvlink_gb100.c:69` |
| `0x20803004` | `NV2080_CTRL_CMD_NVLINK_GET_COUNTERS` | user | get | none in the tree |
| `0x20803005` | `NV2080_CTRL_CMD_NVLINK_CLEAR_COUNTERS` | priv | set | none in the tree |
| `0x20803009` | `NV2080_CTRL_CMD_NVLINK_GET_LINK_FATAL_ERROR_COUNTS` | user | get | none in the tree |
| `0x2080300c` | `NV2080_CTRL_CMD_NVLINK_SETUP_EOM` | user | set | none in the tree |
| `0x2080300e` | `NV2080_CTRL_CMD_NVLINK_GET_POWER_STATE` | user | get | `gpu/gpu_fabric_probe.c:345`, `gpu/gpu_fabric_probe.c:1403` |
| `0x20803011` | `NV2080_CTRL_CMD_NVLINK_GET_LINK_FOM_VALUES` | priv | get | none in the tree |
| `0x20803014` | `NV2080_CTRL_CMD_NVLINK_GET_NVLINK_ECC_ERRORS` | priv | get | none in the tree |
| `0x20803015` | `NV2080_CTRL_CMD_NVLINK_READ_TP_COUNTERS` | user | get | none in the tree |
| `0x20803018` | `NV2080_CTRL_CMD_NVLINK_GET_LP_COUNTERS` | user | get | none in the tree |
| `0x20803023` | `NV2080_CTRL_CMD_NVLINK_SET_LOOPBACK_MODE` | user | set | none in the tree |
| `0x20803028` | `NV2080_CTRL_CMD_NVLINK_GET_REFRESH_COUNTERS` | user | get | none in the tree |
| `0x20803029` | `NV2080_CTRL_CMD_NVLINK_CLEAR_REFRESH_COUNTERS` | user | set | none in the tree |
| `0x20803038` | `NV2080_CTRL_CMD_NVLINK_GET_SET_NVSWITCH_FLA_ADDR` | user | set | `gpu/bus/arch/ampere/kern_bus_ga100.c:646`, `gpu/nvlink/kernel_nvlink.c:1647` |
| `0x20803039` | `NV2080_CTRL_CMD_NVLINK_SYNC_LINK_MASKS_AND_VBIOS_INFO` | user | set | `gpu/nvlink/kernel_nvlink.c:1708` |
| `0x2080303a` | `NV2080_CTRL_CMD_NVLINK_ENABLE_LINKS` | user | set | `gpu/nvlink/kernel_nvlinkstate.c:604` |
| `0x2080303b` | `NV2080_CTRL_CMD_NVLINK_PROCESS_INIT_DISABLED_LINKS` | user | set | `gpu/nvlink/kernel_nvlink.c:2275` |
| `0x2080303c` | `NV2080_CTRL_CMD_NVLINK_EOM_CONTROL` | user | set | none in the tree |
| `0x2080303e` | `NV2080_CTRL_CMD_NVLINK_SET_L1_THRESHOLD` | priv | set | none in the tree |
| `0x2080303f` | `NV2080_CTRL_CMD_NVLINK_GET_L1_THRESHOLD` | user | get | none in the tree |
| `0x20803040` | `NV2080_CTRL_CMD_NVLINK_INBAND_SEND_DATA` | user | set | `gpu/nvlink/kernel_nvlink.c:957` |
| `0x20803041` | `NV2080_CTRL_CMD_NVLINK_IS_GPU_DEGRADED` | priv | get | none in the tree |
| `0x20803042` | `NV2080_CTRL_CMD_NVLINK_DIRECT_CONNECT_CHECK` | user | get | `gpu/nvlink/arch/hopper/kernel_nvlink_gh100.c:923` |
| `0x20803043` | `NV2080_CTRL_CMD_NVLINK_POST_FAULT_UP` | user | set | `gpu/nvlink/arch/hopper/kernel_nvlink_gh100.c:307` |
| `0x20803044` | `NV2080_CTRL_CMD_NVLINK_GET_PORT_EVENTS` | priv | get | `gpu/nvlink/kernel_nvlinkcorelibtrain.c:2662` |
| `0x20803045` | `NV2080_CTRL_CMD_NVLINK_CYCLE_LINK` | priv | set | none in the tree |
| `0x20803046` | `NV2080_CTRL_CMD_NVLINK_IS_REDUCED_CONFIG` | user | get | `gpu/nvlink/arch/ampere/kernel_nvlink_ga100.c:281` |
| `0x20803050` | `NV2080_CTRL_CMD_NVLINK_GET_COUNTERS_V2` | user | get | none in the tree |
| `0x20803051` | `NV2080_CTRL_CMD_NVLINK_CLEAR_COUNTERS_V2` | priv | set | none in the tree |
| `0x20803052` | `NV2080_CTRL_CMD_NVLINK_CLEAR_LP_COUNTERS` | priv | set | none in the tree |
| `0x20803074` | `NV2080_CTRL_CMD_NVLINK_GET_SUPPORTED_COUNTERS` | user | get | none in the tree |
| `0x20803081` | `NV2080_CTRL_CMD_NVLINK_SET_HW_ERROR_INJECT` | priv | set | none in the tree |
| `0x20803082` | `NV2080_CTRL_CMD_NVLINK_GET_HW_ERROR_INJECT` | user | set | none in the tree |
| `0x20803089` | `NV2080_CTRL_CMD_NVLINK_INJECT_SW_ERROR` | priv | set | none in the tree |
| `0x2080308a` | `NV2080_CTRL_CMD_NVLINK_POST_LAZY_ERROR_RECOVERY` | user | set | `gpu/nvlink/kernel_nvlink.c:2415` |
| `0x2080308b` | `NV2080_CTRL_CMD_NVLINK_GET_NVLE_ENCRYPT_EN_INFO` | priv | get | `gpu/nvlink/arch/blackwell/kernel_nvlink_gb100.c:645` |
| `0x2080308c` | `NV2080_CTRL_NVLINK_UPDATE_NVLE_TOPOLOGY` | priv | set | `gpu/nvlink/kernel_nvlink.c:4023` |
| `0x2080308d` | `NV2080_CTRL_NVLINK_GET_UPDATE_NVLE_LIDS` | priv | set | `gpu/nvlink/arch/blackwell/kernel_nvle_gb100.c:96`, `gpu/nvlink/arch/blackwell/kernel_nvle_gb100.c:117` |
| `0x2080308e` | `NV2080_CTRL_CMD_NVLINK_CONFIGURE_L1_TOGGLE` | priv | set | none in the tree |
| `0x2080308f` | `NV2080_CTRL_CMD_NVLINK_GET_L1_TOGGLE` | user | set | none in the tree |
| `0x20803091` | `NV2080_CTRL_NVLINK_GET_FIRMWARE_VERSION_INFO` | user | get | none in the tree |
| `0x20803092` | `NV2080_CTRL_CMD_NVLINK_SET_NVLE_ENABLED_STATE` | user | set | `gpu/nvlink/arch/blackwell/kernel_nvlink_gb100.c:710` |
| `0x20803095` | `NV2080_CTRL_CMD_NVLINK_PRM_ACCESS` | priv | set | none in the tree |
| `0x20803097` | `NV2080_CTRL_CMD_NVLINK_GET_LINK_ACCUMULATIVE_METRIC_DATA` | priv | get | none in the tree |
| `0x20803098` | `NV2080_CTRL_CMD_NVLINK_GET_LINK_RECORD_METRIC_DATA` | priv | get | none in the tree |
| `0x20803099` | `NV2080_CTRL_CMD_NVLINK_GET_DEVICE_RECORD_METRIC_DATA` | priv | get | none in the tree |
| `0x2080309a` | `NV2080_CTRL_CMD_NVLINK_SAVE_NODE_HOSTNAME` | priv | set | none in the tree |
| `0x2080309b` | `NV2080_CTRL_CMD_NVLINK_GET_SAVED_NODE_HOSTNAME` | user | get | none in the tree |
| `0x2080309c` | `NV2080_CTRL_CMD_NVLINK_UPDATE_CLID` | user | set | `gpu/nvlink/kernel_nvlink.c:4609` |
| `0x2080309d` | `NV2080_CTRL_CMD_NVLINK_LOCK_REMAP_TABLE_AND_MSE` | priv | set | `gpu/nvlink/kernel_nvlink.c:4562` |
| `0x2080309e` | `NV2080_CTRL_CMD_NVLINK_GET_REMAP_TABLE_INFO` | priv | get | `gpu/nvlink/kernel_nvlink.c:4169` |
| `0x2080309f` | `NV2080_CTRL_CMD_NVLINK_GET_NVLE_PKT_COUNTERS` | priv | get | none in the tree |
| `0x208030a2` | `NV2080_CTRL_CMD_NVLINK_GET_REMAP_TABLE_INFO_V2` | priv | get | `gpu/nvlink/kernel_nvlink.c:4278` |
| `0x208030a3` | `NV2080_CTRL_NVLINK_GET_UPDATE_NVLE_LIDS_V2` | priv | set | `gpu/nvlink/arch/blackwell/kernel_nvle_gb100.c:83`, `gpu/nvlink/arch/blackwell/kernel_nvle_gb100.c:127` |

### 3.3 `hshub` (8): the high-speed hub

- **What.** The hub behind NVLink and C2C. The CE mapping asks it for HSHUB ids per link.
- **Chips.** GA100 and later, on the NVLink-to-CE and EGM paths.
- **Callers.**
  - `kceMapPceLceForSysmemLinks_GA100` (`gpu/ce/arch/ampere/kernel_ce_ga100.c:579-652`)
  - `kceMapPceLceForNvlinkPeers_GH100` (`gpu/ce/arch/hopper/kernel_ce_gh100.c:656-766`)
  - `kceMapPceLceForNvlinkPeers_GB100` (`gpu/ce/arch/blackwell/kernel_ce_gb100.c:748-888`)
  - `_ceGetAlgorithmPceIndex` (`gpu/ce/arch/blackwell/kernel_ce_gb100.c:677-704`)
  - `memmgrStatePostLoad` for EGM (`gpu/mem_mgr/mem_mgr.c:3951-4016`)

  `HSHUB_GET_AVAILABLE_MASK` is only on a skip list (`rmapi/rmapi_utils.c:198-214`).
- **Truth.** Kernel logic for when the controls are sent. The HSHUB count per chip and the
  link-to-hub map are chip data. The count reaches user space only through
  `HSHUB_GET_AVAILABLE_MASK`, a user getter that the sweep reads.
- **Design.** A chip table, `hshub_count` and `link_to_hshub`, answered from the nvlink model.
  With tier 0 there are no links, so no HSHUB id is ever asked for a link. The mask getter
  answers the capture's value.
- **Estimate.** 1.5 days.
- **Validation.** `HSHUB_GET_AVAILABLE_MASK` in the sweep, on every data-center part.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800a79` | `NV2080_CTRL_CMD_INTERNAL_HSHUB_GET_MAX_HSHUBS_PER_SHIM` | internal | get | `gpu/ce/arch/blackwell/kernel_ce_gb100.c:704` |
| `0x20800a88` | `NV2080_CTRL_CMD_INTERNAL_HSHUB_PEER_CONN_CONFIG` | internal | set | `gpu/bus/arch/maxwell/kern_bus_gm200.c:406`, `gpu/bus/arch/maxwell/kern_bus_gm200.c:416`, `gpu/bus/arch/maxwell/kern_bus_gm200.c:458` (+6) |
| `0x20800a8a` | `NV2080_CTRL_CMD_INTERNAL_HSHUB_GET_HSHUB_ID_FOR_LINKS` | internal | get | `gpu/ce/arch/ampere/kernel_ce_ga100.c:652`, `gpu/ce/arch/ampere/kernel_ce_ga100.c:846`, `gpu/ce/arch/blackwell/kernel_ce_gb100.c:888` (+2) |
| `0x20800a8b` | `NV2080_CTRL_CMD_INTERNAL_HSHUB_GET_NUM_UNITS` | internal | get | `gpu/ce/arch/ampere/kernel_ce_ga100.c:1132` |
| `0x20800a8c` | `NV2080_CTRL_CMD_INTERNAL_HSHUB_NEXT_HSHUB_ID` | internal | set | `gpu/ce/arch/ampere/kernel_ce_ga100.c:420` |
| `0x20800a8d` | `NV2080_CTRL_CMD_INTERNAL_HSHUB_EGM_CONFIG` | internal | set | `gpu/mem_mgr/mem_mgr.c:4016` |
| `0x20804101` | `NV2080_CTRL_CMD_HSHUB_GET_AVAILABLE_MASK` | user | get | `rmapi/rmapi_utils.c:214` |
| `0x20804102` | `NV2080_CTRL_CMD_HSHUB_SET_EC_THROTTLE_MODE` | user | set | none in the tree |

### 3.4 `fabric` (9): FLA, the fabric probe, fabric memory

- **Chips.** FLA exists on GA100 and later (`gpu/bus/arch/ampere/kern_bus_ga100.c:257`, `:370`,
  `:726`). The fabric probe starts in `gpuStatePostLoad` on every chip that is no vGPU host
  (`gpu/gpu.c:3658-3661`). It runs only where KernelNvlink exists (`gpu/gpu_fabric_probe.c:2192-2195`).
- **Callers.** The probe state machine: get info, start, suspend, resume and invalidate
  (`gpu/gpu_fabric_probe.c:668`, `:1970`, `:1998`, `:2030`, `:2106-2120`). The fabric memory
  describe (`mem_mgr/mem_fabric.c:462`). The fabric-manager sync of the client
  (`core/system.c:850`).
- **Truth.** Kernel logic. On a system without NVSwitch, the probe ends with no fabric: no
  cluster UUID and state "N/A", as the GB206 capture shows
  (`caraxes/reference/gb206/gpu-0000_01_00.0/nvidia-smi-q.txt:261-274`).
- **Design.** A state machine answering "probe done, not fabric-attached":
  - `START`, `SUSPEND`, `RESUME` and `INVALIDATE` move the state.
  - `GET_FABRIC_PROBE_INFO` reports it.
  - The FLA controls are refused until nvlink tier 1 gives them a fabric.
  - The NV00F8 describe needs fabric memory; it waits for the multi-GPU tier (4 days).
- **Estimate.** 2 days; FLA and fabric memory 4 more.
- **Validation.** The `nvidia-smi -q` Fabric section on a single H100 against an HGX node;
  the two user getters of the sweep.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x0000013c` | `NV0000_CTRL_CMD_SYSTEM_SYNC_EXTERNAL_FABRIC_MGMT` | priv | set | `core/system.c:850` |
| `0x00f80102` | `NV00F8_CTRL_CMD_DESCRIBE` | user | get | `mem_mgr/mem_fabric.c:462` |
| `0x208001f4` | `NV2080_CTRL_CMD_INTERNAL_GPU_GET_FABRIC_PROBE_INFO` | user | get | `gpu/gpu_fabric_probe.c:668` |
| `0x208001f5` | `NV2080_CTRL_CMD_INTERNAL_GPU_START_FABRIC_PROBE` | user | set | `gpu/gpu_fabric_probe.c:2106`, `gpu/gpu_fabric_probe.c:2120` |
| `0x208001f7` | `NV2080_CTRL_CMD_INTERNAL_GPU_SUSPEND_FABRIC_PROBE` | user | set | `gpu/gpu_fabric_probe.c:1970` |
| `0x208001f8` | `NV2080_CTRL_CMD_INTERNAL_GPU_RESUME_FABRIC_PROBE` | user | set | `gpu/gpu_fabric_probe.c:2030` |
| `0x208001f9` | `NV2080_CTRL_CMD_INTERNAL_GPU_INVALIDATE_FABRIC_PROBE` | user | set | `gpu/gpu_fabric_probe.c:1998`, `gpu/gpu_fabric_probe.c:2230` |
| `0x20803501` | `NV2080_CTRL_CMD_FLA_RANGE` | user | set | `gpu/bus/arch/ampere/kern_bus_ga100.c:257`, `gpu/bus/arch/ampere/kern_bus_ga100.c:370`, `gpu/bus/arch/ampere/kern_bus_ga100.c:726` |
| `0x20803502` | `NV2080_CTRL_CMD_FLA_SETUP_INSTANCE_MEM_BLOCK` | priv | set | `gpu/bus/arch/ampere/kern_bus_ga100.c:48`, `gpu/bus/arch/ampere/kern_bus_ga100.c:1349`, `gpu/bus/arch/ampere/kern_bus_ga100.c:1387` (+3) |

### 3.5 `c2c` (7): chip-to-chip links

- **Chips.** GH100 and GB100 on Grace boards (GH200, GB200). The kernel sends one control, the
  C2C peer mapping (`_kbusCreateC2CPeerMapping`, `gpu/bus/arch/hopper/kern_bus_gh100.c:1938-1964`).
  The other six are user getters and setters for C2C error and low-power statistics.
- **Truth.** A Grace system capture. A discrete GB206 says "GPU C2C Mode: Disabled"
  (`caraxes/reference/gb206/gpu-0000_01_00.0/nvidia-smi-q.txt:61`).
- **Design.** "C2C disabled" answers for the getters, with the status the capture of a
  non-Grace part shows. The coherent-memory model of a Grace board is out of scope: it
  needs a Grace CPU.
- **Estimate.** 0.5 days.
- **Validation.** The three getters of the sweep on an H100 PCIe (disabled) and, if the
  operator rents one, on a GH200.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800a74` | `NV2080_CTRL_CMD_INTERNAL_BUS_CREATE_C2C_PEER_MAPPING` | internal | set | `gpu/bus/arch/hopper/kern_bus_gh100.c:1964` |
| `0x20800a75` | `NV2080_CTRL_CMD_INTERNAL_BUS_REMOVE_C2C_PEER_MAPPING` | internal | set | none in the tree |
| `0x2080182d` | `NV2080_CTRL_CMD_BUS_GET_C2C_ERR_INFO` | user | get | none in the tree |
| `0x20801831` | `NV2080_CTRL_CMD_BUS_GET_C2C_LPWR_STATS` | user | get | none in the tree |
| `0x20801832` | `NV2080_CTRL_CMD_BUS_SET_C2C_LPWR_STATE_VOTE` | user | set | none in the tree |
| `0x20801836` | `NV2080_CTRL_CMD_BUS_SET_C2C_LPWR_IDLE_THRESHOLD` | user | set | none in the tree |
| `0x20801837` | `NV2080_CTRL_CMD_BUS_GET_C2C_PACKET_COUNTERS` | user | get | none in the tree |

### 3.6 `gsync` (9): the G-SYNC and Quadro Sync board

- **Chips.** Any chip on a board with a Quadro Sync header. The kernel drives the P2060 and
  P2061 external device through these controls
  (`gpu/external_device/arch/kepler/kern_gsync_p2060.c`, `gpu/external_device/arch/pascal/kern_gsync_p2061.c`,
  `gpu/external_device/kern_external_device.c:342-357`).
- **Truth.** A second board with a closed protocol.
- **Design.** Refuse. No cloud part has the board.
- **Estimate.** 0.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800a14` | `NV2080_CTRL_CMD_INTERNAL_GSYNC_GET_RASTER_SYNC_DECODE_MODE` | internal | set | `gpu/external_device/arch/pascal/kern_gsync_p2061.c:282` |
| `0x20800abe` | `NV2080_CTRL_CMD_INTERNAL_GSYNC_ATTACH_AND_INIT` | internal | set | `gpu/external_device/arch/kepler/kern_gsync_p2060.c:275` |
| `0x20800abf` | `NV2080_CTRL_CMD_INTERNAL_GSYNC_OPTIMIZE_TIMING_PARAMETERS` | internal | set | `gpu/external_device/arch/kepler/kern_gsync_p2060.c:1513` |
| `0x20800ac0` | `NV2080_CTRL_CMD_INTERNAL_GSYNC_GET_DISPLAY_IDS` | internal | get | `gpu/external_device/kern_external_device.c:357` |
| `0x20800ac1` | `NV2080_CTRL_CMD_INTERNAL_GSYNC_SET_STREO_SYNC` | internal | set | `gpu/external_device/arch/kepler/kern_gsync_p2060.c:3533` |
| `0x20800ac4` | `NV2080_CTRL_CMD_INTERNAL_GSYNC_GET_VERTICAL_ACTIVE_LINES` | internal | get | `gpu/external_device/arch/kepler/kern_gsync_p2060.c:5291` |
| `0x20800ac9` | `NV2080_CTRL_CMD_INTERNAL_GSYNC_IS_DISPLAYID_VALID` | internal | get | `gpu/external_device/gsync.c:1519` |
| `0x20800aca` | `NV2080_CTRL_CMD_INTERNAL_GSYNC_SET_OR_RESTORE_RASTER_SYNC` | internal | set | `gpu/external_device/arch/kepler/kern_gsync_p2060.c:2682` |
| `0x20800aed` | `NV2080_CTRL_CMD_INTERNAL_GSYNC_APPLY_STEREO_PIN_ALWAYS_HI_WAR` | internal | set | none in the tree |

### 3.7 `vgpu` (27): vGPU host and SR-IOV

- **Chips.** vGPU-offload-capable HALs on AD10x and later (`g_gpu_nvoc.c:479`), on hosts in vGPU
  mode. The kernel's callers are in `virtualization/kernel_vgpu_mgr.c`, `vgpuconfigapi.c`
  and `kernel_hostvgpudeviceapi.c`.
- **Truth.** The device isolation rules of [../README.md](../README.md): the guest's PF is no
  vGPU host.
- **Design.** Refuse forever. `NVENC_SW_SESSION_STATS` keeps its `empty_ok` answer
  (`chips/tu106/chip.toml:305-307`).
- **Estimate.** 0.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x00800296` | `NV0080_CTRL_GPU_SET_VGPU_VF_BAR1_SIZE` | priv | set | none in the tree |
| `0x2080016d` | `NV2080_CTRL_GPU_GET_NVENC_SW_SESSION_STATS` | user | get | none in the tree |
| `0x20800192` | `NV2080_CTRL_CMD_GPU_HANDLE_VF_PRI_FAULT` | user | set | `gpu/mmu/kern_gmmu.c:2977` |
| `0x208001af` | `NV2080_CTRL_GPU_GET_NVENC_SW_SESSION_INFO_V2` | user | get | none in the tree |
| `0x20800a57` | `NV2080_CTRL_CMD_INTERNAL_VMMU_GET_SPA_FOR_GPA_ENTRIES` | internal | get | `gpu/mem_mgr/objheap.c:83`, `gpu/mem_mgr/mem_desc.c:2790` |
| `0x20800ab1` | `NV2080_CTRL_CMD_INTERNAL_PERF_CF_CONTROLLERS_SET_MAX_VGPU_VM_COUNT` | internal | set | none in the tree |
| `0x20800ac5` | `NV2080_CTRL_CMD_INTERNAL_MEMMGR_GET_VGPU_CONFIG_HOST_RESERVED_FB` | internal | get | `gpu/mem_mgr/mem_mgr.c:3934` |
| `0x20801118` | `NV2080_CTRL_CMD_FIFO_SETUP_VF_ZOMBIE_SUBCTX_PDB` | user | set | `gpu/fifo/arch/maxwell/kernel_fifo_gm107.c:208` |
| `0x20804001` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_BOOTLOAD_GSP_VGPU_PLUGIN_TASK` | internal | set | `gpu/mig_mgr/kernel_mig_manager.c:7077` |
| `0x20804002` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_SHUTDOWN_GSP_VGPU_PLUGIN_TASK` | internal | set | none in the tree |
| `0x20804003` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_PGPU_ADD_VGPU_TYPE` | internal | set | `virtualization/kernel_vgpu_mgr.c:710`, `virtualization/kernel_vgpu_mgr.c:753` |
| `0x20804004` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_ENUMERATE_VGPU_PER_PGPU` | internal | set | `virtualization/kernel_vgpu_mgr.c:2438` |
| `0x20804005` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_CLEAR_GUEST_VM_INFO` | internal | set | `virtualization/kernel_vgpu_mgr.c:2455` |
| `0x20804006` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_GET_VGPU_FB_USAGE` | internal | get | `virtualization/kernel_vgpu_mgr.c:2511` |
| `0x20804007` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_SET_VGPU_ENCODER_CAPACITY` | internal | set | `virtualization/kernel_vgpu_mgr.c:2571` |
| `0x20804008` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_VGPU_PLUGIN_CLEANUP` | internal | set | none in the tree |
| `0x20804009` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_GET_PGPU_FS_ENCODING` | internal | get | `virtualization/kernel_vgpu_mgr.c:1867` |
| `0x2080400a` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_GET_PGPU_MIGRATION_SUPPORT` | internal | get | `virtualization/kernel_vgpu_mgr.c:1774` |
| `0x2080400b` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_SET_VGPU_MGR_CONFIG` | internal | set | `gpu/gpu.c:3511` |
| `0x2080400c` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_FREE_STATES` | internal | set | `virtualization/kernel_hostvgpudeviceapi.c:954` |
| `0x2080400d` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_GET_FRAME_RATE_LIMITER_STATUS` | internal | get | `virtualization/vgpuconfigapi.c:597` |
| `0x2080400e` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_SET_VGPU_HETEROGENEOUS_MODE` | internal | set | `gpu/device_ctrl.c:459` |
| `0x2080400f` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_SET_VGPU_MIG_TIMESLICE_MODE` | internal | set | `virtualization/vgpuconfigapi.c:978`, `virtualization/kernel_vgpu_mgr.c:903` |
| `0x20804011` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_SCRUB_GUEST_FB` | internal | set | none in the tree |
| `0x20804012` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_CHECK_SCRUB_COMPLETE` | internal | set | none in the tree |
| `0x20804013` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_ALLOC_GSP_CEUTILS` | internal | set | none in the tree |
| `0x20804014` | `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_FREE_GSP_CEUTILS` | internal | set | none in the tree |

### 3.8 `mig` (12) and `mig_partitioning` (4): GPU and compute instances

- **Chips.** `PDB_PROP_GPU_MIG_SUPPORTED`: GA100, GH100, GB100, GB102, GB10B, GB110, GB112,
  GB202, GB203 (`g_gpu_nvoc.c:333-343`). GB206 is not in the list. The 5060 Ti capture says
  "MIG Mode N/A" (`caraxes/reference/gb206/gpu-0000_01_00.0/nvidia-smi-q.txt:22-24`).
- **Callers.** All 16 controls have kernel call sites:
  - the scheduling-enable callback that sets the partitioning mode and the GPU instances
    (`_kmigmgrHandlePostSchedulingEnableCallback`, `gpu/mig_mgr/kernel_mig_manager.c:944-1166`);
  - the MIG memory configuration (`kmemsysReadMIGMemoryCfg_GA100`, `gpu/mem_sys/arch/ampere/kern_mem_sys_ga100.c:168-211`);
  - the CE mapping per instance (`gpu/ce/kernel_ce.c:1016`, `:1152`);
  - export and import (`gpu/mig_mgr/kernel_mig_manager.c:4337`, `:8307`, `:8360`);
  - the NVC637 exec partitions (`gpu/mig_mgr/gpu_instance_subscription.c:492-1040`).
- **Truth.** A capture. The profile tables are GSP-RM data: GPU instance profiles with slice
  counts, memory fractions and engine counts, compute instance profiles, and the memory
  partition table. They reach the kernel through the category A controls
  `INTERNAL_STATIC_KMIGMGR_GET_PROFILES`, `_GET_PARTITIONABLE_ENGINES`,
  `_GET_SWIZZ_ID_FB_MEM_PAGE_RANGES` and `_GET_COMPUTE_PROFILES`
  ([../gsp-surface.md](../gsp-surface.md), "After E and A", last list). User space cannot
  send those. What it can read is their result: `nvidia-smi mig -lgip`, `-lgipp` and `-lcip`,
  and the user getters of the GPU partition interface, all with MIG mode on. Turning MIG mode
  on is a setter and a GPU reset. On a rented VM the operator can do it; the kit never does
  (`lab-kit.md` §7).
- **Design.** A table model:
  - `chips/<chip>/mig.toml` holds the capture's profile tables.
  - The swizzId allocator follows the kernel's own rules in `kernel_mig_manager.c`.
  - The partition table answers `MEMSYS_GET_MIG_MEMORY_PARTITION_TABLE` and
    `KMEMSYS_GET_MIG_MEMORY_CONFIG`.
  - `SET_GPU_INSTANCES` and the exec partitions keep guest-local instance records.

  On the home rig a GA100 or GH100 identity backed by the RTX 2070 would present MIG
  instances as bookkeeping. The work of every instance still runs in the one host context.
  That is honest for CUDA's view (it sees the instance's SMs and memory), but it is no
  isolation. The D9 labels must say so.
- **Estimate.** 8 days for both families (tables 2, allocator and records 4, CE mapping and
  tests 2).
- **Validation.** An A100 or H100 VM with MIG on, done by the operator:
  - `nvidia-smi mig -lgip -lgipp -lcip` and the sweep's partition getters;
  - a traced `nvidia-smi mig -cgi 19 -C`, then the same destroy on the rented card;
  - the native CUDA probes in one instance.

  Compare the profile tables and the kernel's controls in the trace.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800a44` | `NV2080_CTRL_CMD_INTERNAL_KMIGMGR_PROMOTE_GPU_INSTANCE_MEM_RANGE` | internal | set | `gpu/mig_mgr/kernel_mig_manager.c:3543` |
| `0x20800a67` | `NV2080_CTRL_CMD_INTERNAL_KMEMSYS_GET_MIG_MEMORY_CONFIG` | internal | get | `gpu/mem_sys/arch/ampere/kern_mem_sys_ga100.c:181` |
| `0x20800a6b` | `NV2080_CTRL_CMD_INTERNAL_MEMSYS_GET_MIG_MEMORY_PARTITION_TABLE` | internal | set | `gpu/mem_sys/arch/ampere/kern_mem_sys_ga100.c:211` |
| `0x20800aa5` | `NV2080_CTRL_CMD_INTERNAL_MIGMGR_SET_GPU_INSTANCES` | user | set | `gpu/mig_mgr/kernel_mig_manager.c:1166`, `gpu/mig_mgr/kernel_mig_manager.c:7860`, `gpu/mig_mgr/kernel_mig_manager.c:7896` (+2) |
| `0x20800aa8` | `NV2080_CTRL_CMD_INTERNAL_MIGMGR_EXPORT_GPU_INSTANCE` | user | set | `gpu/mig_mgr/kernel_mig_manager.c:4337`, `gpu/mig_mgr/kernel_mig_manager.c:8307` |
| `0x20800aaa` | `NV2080_CTRL_CMD_INTERNAL_MIGMGR_IMPORT_GPU_INSTANCE` | user | set | `gpu/mig_mgr/kernel_mig_manager.c:8360` |
| `0x20802a13` | `NV2080_CTRL_CMD_INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_MIG_GPU_INSTANCE` | internal | get | `gpu/ce/kernel_ce.c:1152` |
| `0x20802a14` | `NV2080_CTRL_CE_UPDATE_PCE_LCE_MIG_MAPPINGS` | internal | set | `gpu/ce/kernel_ce.c:1016` |
| `0xc6370101` | `NVC637_CTRL_CMD_EXEC_PARTITIONS_CREATE` | user | set | `gpu/mig_mgr/kernel_mig_manager.c:9780` |
| `0xc6370102` | `NVC637_CTRL_CMD_EXEC_PARTITIONS_DELETE` | user | set | `virtualization/kernel_vgpu_mgr.c:4581`, `gpu/mig_mgr/gpu_instance_subscription.c:1040`, `gpu/mig_mgr/kernel_mig_manager.c:1134` (+2) |
| `0xc6370105` | `NVC637_CTRL_CMD_EXEC_PARTITIONS_EXPORT` | user | set | `gpu/mig_mgr/gpu_instance_subscription.c:492`, `gpu/mig_mgr/gpu_instance_subscription.c:549`, `gpu/mig_mgr/kernel_mig_manager.c:7556` |
| `0xc6370106` | `NVC637_CTRL_CMD_EXEC_PARTITIONS_IMPORT` | internal | set | `virtualization/kernel_vgpu_mgr.c:4606`, `gpu/mig_mgr/kernel_mig_manager.c:2835` |

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800a46` | `NV2080_CTRL_CMD_INTERNAL_GPU_CHECK_CTS_ID_VALID` | internal | get | `gpu/mig_mgr/kernel_mig_manager.c:10086` |
| `0x20800a51` | `NV2080_CTRL_CMD_INTERNAL_MEMSYS_SET_PARTITIONABLE_MEM` | internal | set | `gpu/mem_mgr/mem_mgr.c:2518` |
| `0x20800a53` | `NV2080_CTRL_CMD_INTERNAL_FIFO_PROMOTE_RUNLIST_BUFFERS` | internal | set | `gpu/mig_mgr/kernel_mig_manager.c:2634` |
| `0x20800aa3` | `NV2080_CTRL_CMD_INTERNAL_MIGMGR_SET_PARTITIONING_MODE` | user | set | `gpu/mig_mgr/kernel_mig_manager.c:1023`, `gpu/mig_mgr/kernel_mig_manager.c:7798` |

### 3.9 `cc` (9): Confidential Computing

- **Chips.** `PDB_PROP_GPU_CC_FEATURE_CAPABLE` holds on GH100, GB10x, GB20x and GR10x
  (`g_gpu_nvoc.c:534-544`); APM on GA10x and AD10x (`:546-555`). CC is on only when the
  scratch register says so (`gpu/conf_compute/conf_compute.c:104-110`,
  `gpu/arch/hopper/kern_gpu_gh100.c:521-522`, `gpu/arch/blackwell/kern_gpu_gb100.c:1154-1155`).
  The GB206 capture reports 0 MiB of protected memory
  (`caraxes/reference/gb206/gpu-0000_01_00.0/nvidia-smi-q.txt:128-131`).
- **Callers.**
  - Key derivation and rotation run only with CC on
    (`gpu/conf_compute/arch/hopper/conf_compute_gh100.c:128`, `:234`, `:545`).
  - `GET_STATIC_INFO` is different: the engine's state init sends it
    (`gpu/conf_compute/conf_compute.c:578-591`), and so does its post-load
    (`gpu/conf_compute/conf_compute.c:472-487`), on every chip whose child list has the
    engine. That covers GA100 to GB20x (§2). The GB206 runs show it four times, answered
    NOT_SUPPORTED (Summary).
- **Truth.** Kernel logic for CC off. The parameters are two booleans, `bIsBar1Trusted` and
  `bIsPcieTrusted` (`ctrl2080internal.h:3654-3657`). With CC off both are false. CC on needs
  the firmware's keys and an attestation that cannot honestly be synthesized.
- **Design.**
  - Answer `GET_STATIC_INFO` with NV_OK and two false booleans on every chip with the
    ConfidentialCompute object. It runs before the refusal rule: a new `static` answer in
    the chip file, or a split of the family into `cc_static` (always answered) and `cc`
    (refused while CC is off).
  - Keep the other eight refused. The device reports CC off in the scratch register of the
    identity.
  - A CC-on identity is not planned. It would be a fake attestation.
- **Estimate.** 0.5 days.
- **Validation.** Home: the GB206 guest's dmesg, where the conf-compute state-init assertion
  must disappear. Cloud: none needed. A CC-mode H100 is the operator's choice and shows only
  what this plan refuses.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800ae1` | `NV2080_CTRL_CMD_INTERNAL_CONF_COMPUTE_DERIVE_SWL_KEYS` | internal | set | `gpu/conf_compute/arch/hopper/conf_compute_gh100.c:234` |
| `0x20800ae2` | `NV2080_CTRL_CMD_INTERNAL_CONF_COMPUTE_DERIVE_LCE_KEYS` | internal | set | `gpu/conf_compute/arch/hopper/conf_compute_gh100.c:128` |
| `0x20800ae5` | `NV2080_CTRL_CMD_INTERNAL_CONF_COMPUTE_ROTATE_KEYS` | internal | set | `gpu/conf_compute/arch/hopper/conf_compute_gh100.c:545` |
| `0x20800ae6` | `NV2080_CTRL_CMD_INTERNAL_CONF_COMPUTE_RC_CHANNELS_FOR_KEY_ROTATION` | internal | set | `gpu/conf_compute/conf_compute_key_rotation.c:112` |
| `0x20800ae7` | `NV2080_CTRL_CMD_INTERNAL_CONF_COMPUTE_SET_GPU_STATE` | internal | set | `gpu/conf_compute/conf_compute_api.c:206`, `gpu/conf_compute/conf_compute.c:723` |
| `0x20800ae8` | `NV2080_CTRL_CMD_INTERNAL_CONF_COMPUTE_SET_SECURITY_POLICY` | internal | set | `gpu/conf_compute/conf_compute_api.c:462` |
| `0x20800af3` | `NV2080_CTRL_CMD_INTERNAL_CONF_COMPUTE_GET_STATIC_INFO` | internal | get | `gpu/conf_compute/conf_compute.c:486`, `gpu/conf_compute/conf_compute.c:591` |
| `0x20800b07` | `NV2080_CTRL_CMD_INTERNAL_CONF_COMPUTE_ROTATE_PER_CHANNEL_KEYS` | internal | set | `gpu/fifo/kernel_fifo_ctrl.c:901` |
| `0xc56f010b` | `NVC56F_CTRL_CMD_GET_KMB` | user | get | `gpu/conf_compute/ccsl.c:494`, `gpu/conf_compute/ccsl.c:694`, `gpu/ce/kernel_ce.c:127` |

### 3.10 `spdm` (1)

- **Chips.** GH100, GB10x, GB20x (`Spdm` in the child lists, §2).
- **Callers.** `spdmSendCtrlCall_GH100` (`gpu/spdm/arch/hopper/spdm_gh100.c:1433-1457`), on the
  CC session path only.
- **Design.** Refuse. The truth is the firmware.
- **Estimate.** 0.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800ad9` | `NV2080_CTRL_INTERNAL_SPDM_PARTITION` | internal | set | `gpu/spdm/arch/hopper/spdm_gh100.c:1457` |

### 3.11 `ccu` (6): the clock-counter unit

- **Chips.** KernelCcu is in the child lists of GH100 (`gpu/arch/hopper/kern_gpu_gh100.c:444`), GB100
  (`gpu/arch/blackwell/kern_gpu_gb100.c:115`), GB102 (`:172`) and the GB20x list
  (`gpu/arch/blackwell/kern_gpu_gb202.c:93`).
- **Callers.**
  - `kccuStateLoad` (`gpu/ccu/kernel_ccu.c:410-450`) calls `kccuGetBufSize_HAL`. On GB100 and
    GB20x that sends `GET_SAMPLE_INFO` (`gpu/ccu/arch/blackwell/kernel_ccu_gb100.c:43-58`).
  - It then allocates the shared buffer and maps it with `CCU_MAP` (`gpu/ccu/kernel_ccu.c:321`).
  - `CCU_UNMAP` runs on unload (`gpu/ccu/kernel_ccu.c:255`).
  - `SET_STREAM_STATE` comes from the profiler (`gpu/ccu/kernel_ccu.c:603`).

  The GB206 guest sends `GET_SAMPLE_INFO` and `UNMAP` (Summary).
- **Truth.** Kernel logic. The one value is `ccuSampleSize` (`ctrl2080internal.h:2396-2398`),
  a GSP-RM number the kernel only multiplies into the buffer size. A native trace cannot see
  it: the control is INTERNAL. A value consistent with the header's counter layout is enough
  for the kernel. The profiler's view, `RESERVE_CCU_PROF` on the NVB0CC object, is a user
  control a traced `ncu` run on Hopper would show.
- **Design.**
  - `GET_SAMPLE_INFO` answers a chip-table size.
  - `MAP` and `UNMAP` record the buffer and ACK.
  - The device writes no counters (zeros), or synthesized clock counters later.
  - `SET_STREAM_STATE` records the state.
  - The profiler pair refuses with `INSUFFICIENT_PERMISSIONS`, like the other profiler rows
    of d-device-model.md §6.16.
- **Estimate.** 1.5 days.
- **Validation.** Home: the GB206 guest's dmesg without CCU errors. Cloud: a traced
  `ncu --query-metrics` on H100 for the profiler controls.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800ab2` | `NV2080_CTRL_CMD_INTERNAL_CCU_GET_SAMPLE_INFO` | internal | get | `gpu/ccu/arch/blackwell/kernel_ccu_gb100.c:58` |
| `0x20800ab3` | `NV2080_CTRL_CMD_INTERNAL_CCU_MAP` | internal | set | `gpu/ccu/kernel_ccu.c:321` |
| `0x20800ab4` | `NV2080_CTRL_CMD_INTERNAL_CCU_UNMAP` | internal | set | `gpu/ccu/kernel_ccu.c:255`, `gpu/ccu/arch/hopper/kernel_ccu_gh100.c:83` |
| `0x20800abd` | `NV2080_CTRL_CMD_INTERNAL_CCU_SET_STREAM_STATE` | internal | set | `gpu/ccu/kernel_ccu.c:603` |
| `0xb0cc0119` | `NVB0CC_CTRL_CMD_RESERVE_CCU_PROF` | user | set | none (vGPU guest RPC table only) |
| `0xb0cc011a` | `NVB0CC_CTRL_CMD_RELEASE_CCU_PROF` | user | set | none (vGPU guest RPC table only) |

### 3.12 `internal_uncalled` (6)

- **Callers.** No kernel call site, except `CLIENT_LOW_POWER_MODE_ENTER` on Tegra
  (`gpu/dce_client/dce_client.c:97`, `gpu/arch/t23x/kern_gpu_t234d.c:143`).
- **Design.** Refuse forever.
- **Estimate.** 0.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800a5b` | `NV2080_CTRL_CMD_INTERNAL_FB_GET_HEAP_RESERVATION_SIZE` | internal | get | none in the tree |
| `0x20800aa0` | `NV2080_CTRL_CMD_INTERNAL_PERF_BOOST_SET_3X` | internal | set | none in the tree |
| `0x20800aa1` | `NV2080_CTRL_CMD_INTERNAL_PERF_BOOST_CLEAR_3X` | internal | set | none in the tree |
| `0x20800ae9` | `NV2080_CTRL_INTERNAL_GPU_CLIENT_LOW_POWER_MODE_ENTER` | internal | set | `gpu/dce_client/dce_client.c:97`, `gpu/arch/t23x/kern_gpu_t234d.c:143` |
| `0x20802a09` | `NV2080_CTRL_CMD_CE_GET_HUB_PCE_MASK` | internal | get | none in the tree |
| `0xc36f0301` | `NVC36F_CTRL_CMD_INTERNAL_GPFIFO_GET_WORK_SUBMIT_TOKEN` | internal | get | none in the tree |

### 3.13 `frl` (4): HDMI 2.1 fixed-rate link

- **Chips.** Every display HAL from C6 on has `supportsHDMIFRL` TRUE
  (`src/nvidia-modeset/src/nvkms-evo3.c:8030`, `:8115`; `src/nvidia-modeset/src/nvkms-evo4.c:2897`, `:2982`); Turing's C5
  does not (`nvkms-evo3.c:7945`). So GA10x, AD10x and GB20x have it; the displayless GA100,
  GH100 and GB100 do not.
- **Callers.** The HDMI packet library of nvkms,
  `src/common/modeset/hdmipacket/nvhdmipkt_C671.c:194-710`:
  - `SET_HDMI_FRL_CONFIG` trains;
  - `SET_HDMI_FRL_FLUSH_MODE` flushes before retraining;
  - `GET_HDMI_FRL_CAPACITY_COMPUTATION` computes the FRL capacity.
- **Truth.** Kernel logic plus the sink. Link training succeeds when the sink's EDID (HF-SCDB
  max FRL rate) and the SCDC handshake allow the rate. The capacity computation is the HDMI
  2.1 formula, which the library calls RM for. A capture of the real card with an HDMI 2.1
  monitor checks the mode list and the resulting link.
- **Design.** Display model extension:
  - The HDMI sink of `display/` gains an HF-SCDB with a max FRL rate and an SCDC status
    model.
  - `SET_HDMI_FRL_CONFIG` trains at once to the requested rate if the sink allows it, and
    fails otherwise.
  - Flush mode and `FRL_CONFIG_MACRO_PAD` are ACKs.
  - The capacity computation follows the formula. The result table is checked against the
    library's own expectations in `nvhdmipkt_C671.c:639-710`.
- **Estimate.** 4 days.
- **Validation.** The home rig's GB206 with an HDMI 2.1 4K120 monitor:
  - the lab kit's display pass (connectors, EDID, modes);
  - the same mode lit up by the guest on the synthetic GB206 in emulation.

  No cloud part has a display head.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x0073029a` | `NV0073_CTRL_CMD_SPECIFIC_SET_HDMI_FRL_CONFIG` | priv | set | `common/modeset/hdmipacket/nvhdmipkt_C671.c:194`, `common/modeset/hdmipacket/nvhdmipkt_C671.c:201` |
| `0x0073029b` | `NV0073_CTRL_CMD_SPECIFIC_SET_HDMI_FRL_FLUSH_MODE` | priv | set | `common/modeset/hdmipacket/nvhdmipkt_C671.c:231`, `common/modeset/hdmipacket/nvhdmipkt_C671.c:238` |
| `0x007302a8` | `NV0073_CTRL_CMD_SPECIFIC_GET_HDMI_FRL_CAPACITY_COMPUTATION` | priv | get | `common/modeset/hdmipacket/nvhdmipkt_C671.c:639`, `common/modeset/hdmipacket/nvhdmipkt_C671.c:645`, `common/modeset/hdmipacket/nvhdmipkt_C671.c:704` (+13) |
| `0x00730502` | `NV0073_CTRL_CMD_FRL_CONFIG_MACRO_PAD` | priv | set | none in the tree |

### 3.14 `dp2x` (6): DisplayPort 2.x

- **Chips.** The DP library picks the 2.x main link when `DP_GET_CAPS` reports DP 2.0 in
  `dpVersionsSupported` (`src/common/displayport/src/dp_evoadapter.cpp:2150-2167`). Which
  chips report that is a GSP-RM answer. `chips/gb206/chip.toml:184-190` names `dp2x` present
  for the GB20x display.
- **Callers.** `src/common/displayport/src/dp_evoadapter2x.cpp:742-1487`: link training
  stages, lane data get and set, and the USB-C cable id.
- **Truth.** A capture. `DP_GET_CAPS` is a user control on the display common object. The kit's
  display pass reads it with `subDeviceInstance` 0; the output has the UHBR rates and the DP
  versions. The link-training sequence for 128b/132b follows the DP 2.1 spec and the
  library's own state machine.
- **Design.** DP sink and link extension:
  - The DPCD model gains the 128b/132b capability (DPCD 0x2215) and the UHBR rates.
  - `DP2X_LINK_TRAINING_CTRL` walks the library's stages and succeeds for rates both sides
    support.
  - The lane-data and level-table getters and setters keep a per-lane table.
  - The cable id answers "not USB-C".
- **Estimate.** 5 days.
- **Validation.** Home: the display pass on the 5060 Ti (`DP_GET_CAPS`, connectors) and, with
  a DP 2.1 monitor, the trained link rate. Cloud: none.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x00731383` | `NV0073_CTRL_CMD_DP2X_LINK_TRAINING_CTRL` | priv | set | `common/displayport/src/dp_evoadapter2x.cpp:742`, `common/displayport/src/dp_evoadapter2x.cpp:759`, `common/displayport/src/dp_evoadapter2x.cpp:805` (+4) |
| `0x00731384` | `NV0073_CTRL_CMD_DP2X_GET_LANE_DATA` | priv | get | `common/displayport/src/dp_evoadapter2x.cpp:1322` |
| `0x00731385` | `NV0073_CTRL_CMD_DP2X_SET_LANE_DATA` | priv | set | `common/displayport/src/dp_evoadapter2x.cpp:1346` |
| `0x00731389` | `NV0073_CTRL_CMD_DP2X_SET_LEVEL_INFO_TABLE_DATA` | priv | set | none in the tree |
| `0x0073138a` | `NV0073_CTRL_CMD_DP2X_GET_LEVEL_INFO_TABLE_DATA` | priv | get | none in the tree |
| `0x0073138d` | `NV0073_CTRL_CMD_DP_GET_CABLEID_INFO_FROM_MACRO` | priv | get | `common/displayport/src/dp_evoadapter2x.cpp:1487` |

### 3.15 `display_later` (4)

- **Chips.** Shared generic packets are on for the C8, C9, CA and later HALs
  (`supportsGenericSharedInfoFrames`, `src/nvidia-modeset/src/nvkms-hal.c:140-164`).
  `NVC370 SET_SOR_FLUSH_MODE` is NV Display 5.0 and later, GB20x and T264
  (`chips/gb206/chip.toml:185-187` quoting `ctrlc370or.h:36-73`). The DP library uses it when
  the caps ask for the RG flush sequence (`src/common/displayport/src/dp_evoadapter2x.cpp:169`, `:1103`, `:1148`).
- **Truth.** Kernel logic and the header.
- **Design.** Record the packet slots (acquire, set, release against a small per-head table)
  and the flush mode, and answer NV_OK.
- **Estimate.** 1.5 days.
- **Validation.** None needed beyond the GB206 display gate.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x007302a9` | `NV0073_CTRL_CMD_SPECIFIC_SET_SHARED_GENERIC_PACKET` | priv | set | none in the tree |
| `0x007302aa` | `NV0073_CTRL_CMD_SPECIFIC_ACQUIRE_SHARED_GENERIC_PACKET` | priv | set | none in the tree |
| `0x007302ab` | `NV0073_CTRL_CMD_SPECIFIC_RELEASE_SHARED_GENERIC_PACKET` | priv | set | none in the tree |
| `0xc3700401` | `NVC370_CTRL_CMD_SET_SOR_FLUSH_MODE` | priv | set | `common/displayport/src/dp_evoadapter2x.cpp:1103`, `common/displayport/src/dp_evoadapter2x.cpp:1148` |

### 3.16 `display_embedded` (6)

- **Chips.** Tegra display: DSI panel timings and fixed timings from the device tree
  (`src/nvidia-modeset/src/nvkms-dpy.c:535`, `:660`), safety region RAM, and LVDS SOR modes.
- **Design.** Refuse. No discrete board in scope has them.
- **Estimate.** 0.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x00731166` | `NV0073_CTRL_CMD_DFP_GET_DSI_MODE_TIMING` | priv | get | `src/nvidia-modeset/src/nvkms-dpy.c:535` |
| `0x00731172` | `NV0073_CTRL_CMD_DFP_GET_FIXED_MODE_TIMING` | priv | get | `src/nvidia-modeset/src/nvkms-dpy.c:660` |
| `0x00731177` | `NV0073_CTRL_CMD_SYSTEM_SET_REGION_RAM_RECTANGLES` | priv | set | none in the tree |
| `0x00731178` | `NV0073_CTRL_CMD_SYSTEM_CONFIGURE_SAFETY_INTERRUPTS` | priv | set | none in the tree |
| `0x50700422` | `NV5070_CTRL_CMD_GET_SOR_OP_MODE` | priv | get | none in the tree |
| `0x50700423` | `NV5070_CTRL_CMD_SET_SOR_OP_MODE` | priv | set | none in the tree |

### 3.17 `notebook_power` (18)

- **Chips.** Any chip, on a notebook board with the ACPI GPS or NVPCF methods. The callers are
  all in `platform/platform_request_handler.c` and `platform_request_handler_ctrl.c`, plus:
  - the Dynamic Boost clear (`gpu/subdevice/subdevice.c:157`);
  - the aux power state (`gpu/perf/kern_perf_pwr.c:177-277`, `src/nvidia/arch/nvalloc/unix/src/osapi.c:1323`, `:1409`);
  - GCX (`src/nvidia/arch/nvalloc/unix/src/dynamic-power.c:1415`).
- **Truth.** The platform's ACPI tables.
- **Design.** Stays refused or ACK for desktop and data-center boards. A notebook identity
  would need ACPI `_DSM` emulation in QEMU first. Out of scope.
- **Estimate.** 0.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800a45` | `NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_DEPENDENCY_CHECK` | internal | set | `platform/platform_request_handler.c:119` |
| `0x20800a7b` | `NV2080_CTRL_CMD_INTERNAL_PMGR_UNSET_DYNAMIC_BOOST_LIMIT` | internal | set | `gpu/subdevice/subdevice.c:157` |
| `0x20800a81` | `NV2080_CTRL_CMD_INTERNAL_PERF_GET_AUX_POWER_STATE` | internal | get | `gpu/perf/kern_perf_pwr.c:177` |
| `0x20800acb` | `NV2080_CTRL_CMD_INTERNAL_SMBPBI_PFM_REQ_HNDLR_CAP_UPDATE` | internal | set | `platform/platform_request_handler.c:445` |
| `0x20800acc` | `NV2080_CTRL_CMD_INTERNAL_PMGR_PFM_REQ_HNDLR_STATE_LOAD_SYNC` | internal | set | `platform/platform_request_handler.c:502` |
| `0x20800acd` | `NV2080_CTRL_CMD_INTERNAL_THERM_PFM_REQ_HNDLR_STATE_INIT_SYNC` | internal | set | `platform/platform_request_handler.c:518` |
| `0x20800ace` | `NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_GET_PM1_STATE` | internal | get | `platform/platform_request_handler.c:774`, `platform/platform_request_handler_ctrl.c:1573` |
| `0x20800acf` | `NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_SET_PM1_STATE` | internal | set | `platform/platform_request_handler_ctrl.c:2083` |
| `0x20800ad0` | `NV2080_CTRL_CMD_INTERNAL_PMGR_PFM_REQ_HNDLR_UPDATE_EDPP_LIMIT` | internal | set | `platform/platform_request_handler_ctrl.c:1616`, `platform/platform_request_handler_ctrl.c:1673` |
| `0x20800ad1` | `NV2080_CTRL_CMD_INTERNAL_THERM_PFM_REQ_HNDLR_UPDATE_TGPU_LIMIT` | internal | set | `platform/platform_request_handler.c:597`, `platform/platform_request_handler.c:845`, `platform/platform_request_handler_ctrl.c:487` (+1) |
| `0x20800ad2` | `NV2080_CTRL_CMD_INTERNAL_PMGR_PFM_REQ_HNDLR_CONFIGURE_TGP_MODE` | internal | set | `platform/platform_request_handler_ctrl.c:1834` |
| `0x20800ad3` | `NV2080_CTRL_CMD_INTERNAL_PMGR_PFM_REQ_HNDLR_CONFIGURE_TURBO_V2` | internal | set | `platform/platform_request_handler_ctrl.c:2776` |
| `0x20800ad4` | `NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_GET_VPSTATE_INFO` | internal | get | `platform/platform_request_handler.c:479` |
| `0x20800ad5` | `NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_GET_VPSTATE_MAPPING` | internal | get | `platform/platform_request_handler.c:278` |
| `0x20800ad6` | `NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_SET_VPSTATE` | internal | set | `platform/platform_request_handler.c:316` |
| `0x2080205b` | `NV2080_CTRL_CMD_PERF_SET_POWERSTATE` | user | set | `arch/nvalloc/unix/src/osapi.c:1323`, `gpu/perf/kern_perf_pwr.c:260`, `gpu/perf/kern_perf_pwr.c:272` (+1) |
| `0x20802092` | `NV2080_CTRL_CMD_PERF_SET_AUX_POWER_STATE` | user | set | `arch/nvalloc/unix/src/osapi.c:1409`, `gpu/perf/kern_perf_pwr.c:222` |
| `0x2080a7d7` | `NV2080_CTRL_CMD_INTERNAL_GCX_ENTRY_PREREQUISITE` | internal | set | `arch/nvalloc/unix/src/dynamic-power.c:1415` |

### 3.18 `ecc` (4)

- **Chips.** ECC is a board property: HBM parts (A100, H100, B200) and pro GDDR boards
  (L4, L40S, A10G, RTX PRO 6000).
- **Callers.** None of the four has a kernel caller. `nvidia-smi -e` and `-p` send the two
  NV2080 setters. The two NV208F kill-pointer controls are diagnostics.
- **Truth.** A capture: the ECC mode and counters come from the category B getters the device
  already forwards (`crates/vfio-user-nvgpu/examples/qb1-sweep.rs:25`, `:48-51`).
- **Design.** A board record of the pending and current mode:
  - `SET_ECC_CONFIGURATION` writes pending.
  - A device reset makes pending current.
  - `RESET_ECC_ERROR_STATUS` clears the model's counters.
  - The NV208F controls are refused.
- **Estimate.** 1.5 days.
- **Validation.** The sweep's ECC getters and the ECC section of `nvidia-smi -q` on any ECC
  part. The kit never runs the setters.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800134` | `NV2080_CTRL_CMD_GPU_SET_ECC_CONFIGURATION` | priv | set | none in the tree |
| `0x20800136` | `NV2080_CTRL_CMD_GPU_RESET_ECC_ERROR_STATUS` | priv | set | none in the tree |
| `0x208f050e` | `NV208F_CTRL_CMD_FB_ECC_SET_KILL_PTR` | user | set | none in the tree |
| `0x208f0511` | `NV208F_CTRL_CMD_FB_ECC_SET_WRITE_KILL` | user | set | none in the tree |

### 3.19 `row_remap` (2)

- **Chips.** HBM parts from GA100 on, and GB20x: the GB206 capture has a remapped-rows section
  (`caraxes/reference/gb206/gpu-0000_01_00.0/nvidia-smi-q.txt:181-187`).
- **Callers.** None. The two NV208F diagnostics clear InfoROM entries.
- **Design.** NV_OK on an empty remap table where the family is present. GB206 names it
  present (`chips/gb206/chip.toml:199`) with nothing behind it, so today the controls fall
  through to "unanswered".
- **Estimate.** 0.5 days.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x208f0515` | `NV208F_CTRL_CMD_FB_CLEAR_REMAPPED_ROWS` | priv | set | none in the tree |
| `0x208f051b` | `NV208F_CTRL_CMD_FB_CLEAR_REMAPPED_BANKS` | priv | set | none in the tree |

### 3.20 `mig_partitioning`

Part of §3.8. The table is there.

### 3.21 `nvlink_sli` (13): peers, SLI bridge, boost sync

- **Chips.** Two GPUs and a bridge or NVSwitch. Its pieces:
  - SLI bridge detection (`gpu_mgr/gpu_mgr_sli.c:414`);
  - the SLI link GPIOs of display v04 (`gpu/disp/arch/v04/kern_disp_0400.c:76-159`);
  - boost sync (`gpu/perf/kern_perf_gpuboostsync.c:102`, `:258`);
  - NVLink P2P mappings (`gpu/bus/arch/pascal/kern_bus_gp100.c:169`, `:546`);
  - peer ATS (`gpu/mem_sys/arch/volta/kern_mem_sys_gv100.c:148-194`);
  - IMEX and fabric-manager recovery (`compute/imex_session_api.c:64`, `compute/fm_session_api.c:63`).
- **Truth.** A capture of a bridged pair.
- **Design.** Part of nvlink tier 1.
- **Estimate.** 3 days on top of tier 1.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x00802008` | `NV0080_CTRL_CMD_INTERNAL_FIFO_RC_AND_PERMANENTLY_DISABLE_CHANNELS` | user | set | `compute/imex_session_api.c:64` |
| `0x20800a4a` | `NV2080_CTRL_CMD_INTERNAL_RECOVER_ALL_COMPUTE_CONTEXTS` | internal | set | `compute/fm_session_api.c:63` |
| `0x20800a7e` | `NV2080_CTRL_CMD_INTERNAL_PERF_GPU_BOOST_SYNC_SET_CONTROL` | internal | set | `gpu/perf/kern_perf_gpuboostsync.c:102` |
| `0x20800a7f` | `NV2080_CTRL_CMD_INTERNAL_PERF_GPU_BOOST_SYNC_SET_LIMITS` | internal | set | `gpu/perf/kern_perf_gpuboostsync.c:258` |
| `0x20800add` | `NV2080_CTRL_CMD_INTERNAL_DETECT_HS_VIDEO_BRIDGE` | internal | set | `gpu_mgr/gpu_mgr_sli.c:414` |
| `0x20800ade` | `NV2080_CTRL_CMD_INTERNAL_DISP_SET_SLI_LINK_GPIO_SW_CONTROL` | internal | set | `gpu/disp/arch/v04/kern_disp_0400.c:76` |
| `0x20800afc` | `NV2080_CTRL_CMD_INTERNAL_MEMSYS_SET_PEER_ATS_CONFIG` | internal | set | `gpu/mem_sys/arch/volta/kern_mem_sys_gv100.c:148`, `gpu/mem_sys/arch/volta/kern_mem_sys_gv100.c:194` |
| `0x2080182e` | `NV2080_CTRL_CMD_BUS_SET_P2P_MAPPING` | user | set | `gpu/bus/arch/pascal/kern_bus_gp100.c:169`, `gpu/bus/arch/pascal/kern_bus_gp100.c:175` |
| `0x2080182f` | `NV2080_CTRL_CMD_BUS_UNSET_P2P_MAPPING` | user | set | `gpu/bus/arch/pascal/kern_bus_gp100.c:546` |
| `0x20802300` | `NV2080_CTRL_CMD_INTERNAL_GPIO_PROGRAM_DIRECTION` | internal | set | `gpu/disp/arch/v04/kern_disp_0400.c:102` |
| `0x20802301` | `NV2080_CTRL_CMD_INTERNAL_GPIO_PROGRAM_OUTPUT` | internal | set | `gpu/disp/arch/v04/kern_disp_0400.c:120` |
| `0x20802302` | `NV2080_CTRL_CMD_INTERNAL_GPIO_READ_INPUT` | internal | get | `gpu/disp/arch/v04/kern_disp_0400.c:137` |
| `0x20802303` | `NV2080_CTRL_CMD_INTERNAL_GPIO_ACTIVATE_HW_FUNCTION` | internal | set | `gpu/disp/arch/v04/kern_disp_0400.c:159` |

### 3.22 `ampere_later` (3)

- **Callers and design.**
  - Raw compression mode around FB save and restore (`gpu/mem_mgr/arch/ampere/fbsr_ga100.c:43`,
    the GA100 FBSR HAL): ACK, since the device's FB save goes through its own model.
  - The vidmem access-bit dump of UVM (`rmapi/nv_gpu_ops.c:8235`, class 0xC763): zeros.
  - `FIFO_CLEAR_FAULTED_BIT` (UVM's Ampere path per d-controls.tsv): clear the faulted flag
    of the channel in the channel model and ring its doorbell. This is really C.
- **Truth.** Kernel logic.
- **Estimate.** 1.5 days.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800a6f` | `NV2080_CTRL_CMD_INTERNAL_MEMSYS_PROGRAM_RAW_COMPRESSION_MODE` | internal | set | `gpu/mem_mgr/arch/ampere/fbsr_ga100.c:43` |
| `0x20801113` | `NV2080_CTRL_CMD_FIFO_CLEAR_FAULTED_BIT` | priv | set | none in the tree |
| `0xc7630103` | `NVC763_CTRL_CMD_VIDMEM_ACCESS_BIT_DUMP` | user | set | `rmapi/nv_gpu_ops.c:8235` |

### 3.23 `ga10x_pcie_ro` (1)

- **Callers.** `kernel_bif.c:604-619`, on the GA100-GA107 HAL.
- **Design.** ACK. Relaxed ordering is a config-space bit the device does not act on.
- **Estimate.** 0.25 days.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800ab9` | `NV2080_CTRL_CMD_INTERNAL_BIF_SET_PCIE_RO` | internal | set | `gpu/bif/kernel_bif.c:616`, `gpu/bif/kernel_bif.c:619` |

### 3.24 `hopper_platform` (2)

- **Callers.**
  - The probe filter clean on coherent C2C systems (`gpu/mem_sys/kern_mem_sys.c:1235`).
  - GSP memory transfers when the CPU cannot reach vidmem under CC (`gpu/mem_mgr/mem_utils.c:314`,
    `:476`, `:552`).
- **Design.** The transfer is a real data mover the device can run: copy between guest FB
  and sysmem. It matters only for a CC-on identity. The probe filter is an ACK.
- **Estimate.** 1 day.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800ab0` | `NV2080_CTRL_CMD_INTERNAL_MEMSYS_CLEAN_LTC_PROBE_FILTER` | internal | set | `gpu/mem_sys/kern_mem_sys.c:1235` |
| `0x20800afa` | `NV2080_CTRL_CMD_INTERNAL_MEMMGR_MEMORY_TRANSFER_WITH_GSP` | internal | set | `gpu/mem_mgr/mem_utils.c:314`, `gpu/mem_mgr/mem_utils.c:476`, `gpu/mem_mgr/mem_utils.c:552` |

### 3.25 `blackwell` (5)

- **Chips.** GB10x and GB20x.
- **Callers and design.**
  - GSP-lite CMC log buffers (`kgspliteInitLibosLoggingStructures`, `gpu/gsplite/kernel_gsplite.c:142-256`;
    four GSP-lite engines in the GB20x child list, `gpu/arch/blackwell/kern_gpu_gb202.c:92`).
    Record the buffer and ACK. No CMC traffic.
  - The DRAM encryption setter (user, `nvidia-smi`). Record it as pending, like ECC. The
    query getters are B.
  - The MMU GCC debug mode of the debugger (`ctrl83dedebug.h:1163-1191`). Record it in the
    debugger session.
  - The HES reserve and release of the profiler (`PDB_PROP_KHWPM_HES_CWD_SUPPORTED`,
    `g_kern_hwpm_nvoc.c:246`). Bookkeeping.

  GB206 names the family present (`chips/gb206/chip.toml:199`) with no implementation.
- **Estimate.** 1.5 days.
- **Validation.** Home: a traced `cuda-gdb` run on the 5060 Ti for GCC debug, and the sweep
  for the DRAM-encryption getters.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x20800a89` | `NV2080_CTRL_CMD_INTERNAL_SEND_CMC_LIBOS_BUFFER_INFO` | user | set | `gpu/gsplite/kernel_gsplite.c:256` |
| `0x20801356` | `NV2080_CTRL_CMD_FB_SET_DRAM_ENCRYPTION_CONFIGURATION` | priv | set | none in the tree |
| `0x83de032a` | `NV83DE_CTRL_CMD_DEBUG_SET_MODE_MMU_GCC_DEBUG` | user | set | none (vGPU guest RPC table only) |
| `0xb0cc0113` | `NVB0CC_CTRL_CMD_RESERVE_HES` | user | set | none (vGPU guest RPC table only) |
| `0xb0cc0114` | `NVB0CC_CTRL_CMD_RELEASE_HES` | user | set | none (vGPU guest RPC table only) |

### 3.26 `vgpu_paths` (7), 3.27 `windows_only` (5), 3.28 `legacy_uncalled` (18)

These paths only a vGPU host or guest, a Windows client, or no client at all takes.
d-controls.tsv gives the reason for each row.
- **Design.** Refuse forever. The one ACK is `INTERNAL_MEMSYS_SET_ZBC_REFERENCED`, which the
  kernel sends on Turing (`gpu/mem_mgr/arch/maxwell/mem_mgr_gm107.c:307`).
- **Estimate.** 0.

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x00000b05` | `NV0000_CTRL_CMD_GPUACCT_CLEAR_ACCOUNTING_DATA` | priv | set | none (vGPU guest RPC table only) |
| `0x20800ab5` | `NV2080_CTRL_CMD_INTERNAL_SET_P2P_CAPS` | internal | set | `gpu/gpu.c:3433` |
| `0x20800ab6` | `NV2080_CTRL_CMD_INTERNAL_REMOVE_P2P_CAPS` | internal | set | `gpu/gpu.c:3323`, `gpu/gpu.c:3481` |
| `0x20801121` | `NV2080_CTRL_CMD_FIFO_OBJSCHED_SET_STATE` | user | set | none in the tree |
| `0xb0cc0105` | `NVB0CC_CTRL_CMD_ALLOC_PMA_STREAM` | user | set | `gpu/hwpm/profiler_v2/kern_profiler_v2_ctrl.c:384` |
| `0xb0cc0106` | `NVB0CC_CTRL_CMD_FREE_PMA_STREAM` | user | set | `gpu/hwpm/profiler_v2/kern_profiler_v2_ctrl.c:526` |
| `0xb0cc0201` | `NVB0CC_CTRL_CMD_INTERNAL_QUIESCE_PMA_CHANNEL` | user | set | `gpu/hwpm/profiler_v2/kern_profiler_v2.c:279` |

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x00730159` | `NV0073_CTRL_CMD_SYSTEM_NOTIFY_DRR_MSCG_WAR` | user | set | none in the tree |
| `0x20800adb` | `NV2080_CTRL_CMD_INTERNAL_BIF_DISABLE_SYSTEM_MEMORY_ACCESS` | internal | set | `gpu/bif/arch/maxwell/kernel_bif_gm107.c:1049` |
| `0x50700119` | `NV5070_CTRL_CMD_SET_MEMPOOL_WAR_FOR_BLIT_TEARING` | user | set | none in the tree |
| `0x83de022d` | `NV83DE_CTRL_CMD_DEBUG_SET_NON_PREEMPTABLE_DEBUGGER_CONTEXT` | user | set | none in the tree |
| `0x83de022e` | `NV83DE_CTRL_CMD_DEBUG_SET_RECOVERY_SUPPRESSION_TIMEOUT` | user | set | none in the tree |

| Id | Control | Access | Kind | 615 kernel call sites (file:line, first three) |
|---|---|---|---|---|
| `0x0080170f` | `NV0080_CTRL_CMD_FIFO_SET_CHANNEL_PROPERTIES` | user | set | none in the tree |
| `0x00801812` | `NV0080_CTRL_DMA_SET_DEFAULT_VASPACE` | user | set | none (vGPU guest RPC table only) |
| `0x0080200a` | `NV0080_CTRL_CMD_INTERNAL_MEMSYS_SET_ZBC_REFERENCED` | user | set | `gpu/mem_mgr/arch/maxwell/mem_mgr_gm107.c:307` |
| `0x2080012d` | `NV2080_CTRL_CMD_GPU_INITIALIZE_CTX` | priv | set | `gpu/deferred_api.c:467` |
| `0x20800a98` | `NV2080_CTRL_CMD_INTERNAL_PERF_PERFMON_CLIENT_RESERVATION_CHECK` | internal | get | `gpu/perf/kern_perf_pm.c:105`, `gpu/perf/kern_perf_pm.c:199` |
| `0x20800a99` | `NV2080_CTRL_CMD_INTERNAL_PERF_PERFMON_CLIENT_RESERVATION_SET` | internal | set | `gpu/perf/kern_perf_pm.c:133`, `gpu/perf/kern_perf_pm.c:174`, `gpu/perf/kern_perf_pm.c:228` |
| `0x20800b12` | `NV2080_CTRL_CMD_INTERNAL_UCODE_INSTRUMENTATION_GET_STATE` | internal | get | `diagnostics/ucode_instrumentation_ctrl.c:88` |
| `0x20800b13` | `NV2080_CTRL_CMD_INTERNAL_UCODE_INSTRUMENTATION_SET_STATE` | internal | set | `diagnostics/ucode_instrumentation_ctrl.c:128` |
| `0x20801102` | `NV2080_CTRL_CMD_SET_GPFIFO` | user | set | none in the tree |
| `0x20801116` | `NV2080_CTRL_CMD_FIFO_UPDATE_CHANNEL_INFO` | priv | set | `gpu/deferred_api.c:349`, `gpu/deferred_api.c:479`, `gpu/deferred_api.c:711` |
| `0x20801220` | `NV2080_CTRL_CMD_GR_GFX_POOL_INITIALIZE` | user | set | none in the tree |
| `0x20801221` | `NV2080_CTRL_CMD_GR_GFX_POOL_ADD_SLOTS` | user | set | none in the tree |
| `0x20801222` | `NV2080_CTRL_CMD_GR_GFX_POOL_REMOVE_SLOTS` | user | set | none in the tree |
| `0x20802214` | `NV2080_CTRL_CMD_SET_RC_WATCHDOG_INFO` | priv | set | none in the tree |
| `0x208f0506` | `NV208F_CTRL_CMD_FB_CTRL_GPU_CACHE` | user | set | none in the tree |
| `0x50800101` | `NV5080_CTRL_CMD_DEFERRED_API` | user | set | none in the tree |
| `0x50800102` | `NV5080_CTRL_CMD_REMOVE_API` | user | set | none in the tree |
| `0x50800104` | `NV5080_CTRL_CMD_DEFERRED_API_INTERNAL` | internal | set | `gpu/deferred_api.c:392` |

## 4. How a present family is answered today, and what changes

- **Today.** A family named `present` in a chip file is not refused. Its controls fall
  through every handler and reach the "unanswered" NOT_SUPPORTED of the dispatcher
  (`src/ctrl.rs:1218-1223`). `chipctl::tests` checks this fall-through for every family
  (`src/chipctl.rs:552-591`). So GB206 declares six families present
  (`chips/gb206/chip.toml:199`), and nothing answers them yet.
- **The change.** One model module per family that has a design here. It is registered
  in the dispatcher after chipctl, and it takes a family's ids only when the chip names the
  family present. The chip data it needs goes into `chips/<chip>/`, as tables named after
  the family:
  - `mig.toml`
  - `nvlink.toml`
  - `ccu` sizes in `chip.toml`

  The board data (ECC, sync connector, links) goes into `boards/`. A family without a model
  stays refused when absent and unanswered when present. The chip-file loader can name that
  case at load time, so a present family without a model is visible and not silent.

## 5. Totals

| Tier | Families | Days |
|---|---|---:|
| Refused forever, no work | rm_test, gsync, vgpu, spdm, internal_uncalled, display_embedded, notebook_power, vgpu_paths, windows_only, legacy_uncalled | 0 |
| Single GPU, pure source | nvlink tier 0 (3), hshub (1.5), fabric (2), cc static (0.5), ccu (1.5), display_later (1.5), ampere_later (1.5), ga10x_pcie_ro (0.25), hopper_platform (1), row_remap (0.5) | 13.25 |
| Single GPU, capture needed or checked | mig with mig_partitioning (8), frl (4), dp2x (5), ecc (1.5), c2c (0.5), blackwell (1.5) | 20.5 |
| Multi-GPU | nvlink tier 1 (15), nvlink_sli (3), FLA and fabric memory (4) | 22 |

## 6. Recommended order

1. **The GB20x boot answers (1 to 2 days, home rig, no cloud).** `cc` static info and `ccu`
   (§3.9, §3.11), `row_remap` and `blackwell` (§3.19, §3.25). These are the only E and D-to-E
   controls the stock guest sends on the synthetic GB206 today (Summary). The GB206 gate
   (`docs/caraxes/blackwell-gate.md`) and the capture of `caraxes/reference/gb206/` are the
   test. The workplace card, through the lab kit, adds the sweep, the display pass and
   `DP_GET_CAPS`.
2. **Hopper and Blackwell data-center CUDA for the lab (about 15 days).** This needs, in
   order:
   - a GH100 and a GB100 identity (chip files, boot as in `docs/caraxes/gsp-boot-blackwell.md`
     for the FSP path);
   - nvlink tier 0, hshub, fabric on a single GPU, ccu and cc static;
   - the MIG model last. It is optional for CUDA, since MIG mode is off by default.

   The cloud captures of H100 and B200 (lab-kit.md) give the A tables and the sweep. The
   native CUDA traces of the kit's probes on those parts are the comparison for the guest
   runs.
3. **Wayland on Blackwell consumer boards (about 10 days).** `display_later`, `dp2x`, then `frl`.
   The home rig's GB206 is the only validation: no cloud part has a display head. The kit's
   display pass on the workplace card and on the 5060 Ti gives `DP_GET_CAPS`, the connector
   table and the EDIDs.
4. **Ampere and Ada (a few days each).** An L4 (AD104) or A10G (GA102) identity: ampere_later,
   ga10x_pcie_ro, frl, nvlink tier 0. The L4 is the cheapest cloud part and a good first test
   of the kit itself.
5. **The multi-GPU tier (about 22 days).** Only after one data-center identity runs CUDA. It
   needs an HGX capture with two or more GPUs.

**Without any cloud.** TU106 stays the reference: every family absent. `chipctl::tests` and
the 615 guest gates hold that. The GB206 capture exists already, and the 5060 Ti on the home
rig adds what the kit's steps give there: the sweep, the traces and the display pass.
Together they cover order steps 1 and 3 and the Blackwell half of step 2 for everything that
is not data-center-only. MIG, NVLink, C2C, ECC and the HBM row remapping need a rented part.
