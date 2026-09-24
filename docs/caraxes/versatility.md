<!-- SPDX-License-Identifier: MIT -->
# Versatility: chips, boards and driver versions

Decision D14 ([README.md](README.md)): implement for the RTX 2070 (TU106) on driver
610.57.04, but keep everything version- or silicon-specific where another driver
version or another chip can be added without touching the generic paths. This note
describes how `crates/vfio-user-nvgpu` is split after the refactor of 2026-09-23
([experiments.md](experiments.md), "D14 refactor"), where each kind of fact lives,
what is still hard-coded and why, and the checklists for a new chip and a new driver
version.

## The three axes

| Axis | What varies | Where it lives | Selected by |
|---|---|---|---|
| Silicon | boot registers (`NV_PMC_BOOT_0/1/42`), the firmware-boot model (falcons, GFW, fuses, WPR2), the BAR0 register map, the kernel-RM surface (PRAMIN, BAR blocks, TLB invalidate, interrupt tree, VF pointers, usermode window), the FIFO device-info table, engine caps, the interrupt table, memsys and GMMU facts, CE and GR properties, the channel and copy classes | `src/chip/` (trait `Chip`, the Turing boot model), `chips/<chip>/*.toml` | `--chip` (default: the board's chip, `tu106`) |
| Board | PCI ids, subsystem ids, class, revision and capability chain (from the config-space capture), the ROM (capture), the BAR layout, the ROM BAR size, the VBIOS version, the board name in the guest name | `src/board.rs`, `boards/<board>.toml`, `caraxes/reference/<board>/` | `--board-data` (default `rtx2070`), `--config`, `--rom`, `--vbios`, `--vbios-version`, `--board` |
| Profile | FB size, guest name, GID | `src/rpc.rs` (`StaticIdentity`), unchanged | `--fb-mib` / `LEA_VRAM_PROFILE_MIB`, `--board` |
| Driver version | GSP RPC function numbers, message layouts (`rpc_*_v03_00`, `rpc_*_v17_00`, `GspStaticConfigInfo`, `GspSystemInfo`, `GspFwWprMeta`, libos arguments), RM class and control numbers, control parameter layouts, the driver enums the chip tables name (NV2080_ENGINE_TYPE, RM_ENGINE_TYPE, ENG_DESC classes, MC_ENGINE_IDX, NV2080_INTR_CATEGORY, RAM type), the `nvidia-smi` control lists | `src/ver/mod.rs`, `src/ver/v610.rs` | cargo feature `v610` (default) |

Everything else is generic and names no register, engine, class or offset of its
own: the vfio-user device (`device.rs`), the BAR0 window (`bar0.rs`), the generic boot
model with the queue and RPC service (`boot.rs`, `msgq.rs`, `rpc.rs`), the phase-5
dispatcher and its extensions (`ctrl.rs`, `kgr.rs`, `nvml.rs`, `uvm.rs`), the channel
emulation (`channel.rs`), the falcon block (`falcon.rs`), the interrupt tree
(`intr.rs`), the fake FB (`fb.rs`), guest memory, host forwarding and the trace.

### Module layout

```
crates/vfio-user-nvgpu/
  boards/rtx2070.toml        board: name, chip, capture paths, VBIOS version, BARs, ROM BAR
  chips/tu106/chip.toml      identity, BAR0 windows, kernel-RM registers, engine caps,
                             chip info, memsys, FIFO, GMMU, CE, GR, channel classes
  chips/tu106/regmap.toml    BAR0 names for the trace (docs/caraxes/registers-turing.md)
  chips/tu106/device-info.toml  FIFO device-info table (fifo-device-info.md)
  chips/tu106/interrupts.toml   kernel interrupt table and subtree map
  chips/tu106/boot.toml      Turing firmware-boot registers and constants
  src/chip/mod.rs            trait Chip, trait FirmwareBoot, BootEvent, ChipData and its loader
  src/chip/turing.rs         the Turing firmware-boot model (was boot.rs), reusable by GA10x/AD10x
  src/chip/tu106.rs          TU106: the files above, include_str!, parsed once (OnceLock)
  src/board.rs               Board: boards/*.toml, BAR layout, VBIOS version
  src/ver/mod.rs             the version axis; `active` = the version this build emulates
  src/ver/v610.rs            610.57.04: RPC numbers, message and control layouts, classes,
                             control ids, driver enums, nvml lists; tests against the
                             bindings and against gsp-abi-610.md (a)
  src/boot.rs                generic boot model: kernel-RM surface, queues, RPC service
  ...                        the rest as before (architecture.md, "The device process")
```

### How the pieces meet

- `main.rs` loads the board (`Board::load`), the chip (`chip::by_name`) and the
  config-space capture (`ConfigSpace::from_dump(dump, board.bars, board.rom_bar_size)`,
  which finds the PM, MSI and PCIe capabilities by walking the chain), and builds the
  `StaticIdentity` from all three plus the profile.
- `Bar0` takes the chip: its boot registers, the `NV_PCFG` and `NV_PROM_DATA` windows
  and the register names come from `ChipData`.
- `BootModel` takes the chip: it asks `chip.firmware_boot(fb_bytes)` for the
  firmware-boot model and owns only the generic surface at the offsets in
  `chip.toml` `[registers]`. The firmware-boot model reports two milestones back as
  `BootEvent`s: `WprMeta { pa }` (Booter-Load handed over the `GspFwWprMeta`) and
  `CmdQueueDoorbell { libos_args_pa }` (the CPU rang the GSP queue doorbell). The
  generic model reads the WPR meta and services the queues; the notes appear in the
  trace in the same order as before.
- `Dispatcher` takes the chip tables and the identity; `kgr::answer`, `nvml::answer`
  and `uvm::answer` take the chip tables for their chip-bound answers (PDB property,
  MIG, decompression LCE) and reach every number through `crate::ver::active`.
- The channel `Registry` takes the chip's `ChannelClasses`; the usermode doorbell
  offset comes from the chip.

### The chip data files

Numbers are hexadecimal where the source is, provenance comments are kept next to
the value, and labels (the D9 provenance strings that appear in the trace) are part of
the data, so a chip file says both what the device answers and where it came from.
Columns that are driver enums are written by name (`rm_engine_type = "COPY0"`,
`mc = "GR0"`, `eng_desc = ["OBJCE", 0]`, `category = "UVM_OWNED"`, `ram_type =
"GDDR6"`, engine caps `["GR0", "COPY0", ...]`); `ChipData::load` resolves them
through the tables in `src/ver/v610.rs` and refuses the chip if a name has no number
in the emulated driver. A chip file therefore holds only chip facts, and the same file
serves every driver version whose enums still contain those names.

Loading is checked by tests: `chip::tu106::tests::tables_resolve_to_the_milestone_values`
compares the resolved tables with the literal tables of the E4/E5a device (device
info, interrupt table, engine-caps words `0x81bfe02 0xc4 0x0`, classes), and
`unknown_names_refuse_the_chip` checks the refusal. `serde(deny_unknown_fields)` makes
a misspelt key an error rather than a default.

### The version module

`src/ver/v610.rs` exports the same names every future `v<NNN>.rs` must export; generic
code imports them through `crate::ver::active`. Three kinds of checks keep the numbers
honest:

1. **Typed layouts.** Where `nvrm_sys::v610` has the parameter struct, the offset or
   size is computed with `offset_of!`/`size_of` from it (chip info, device-info table,
   interrupt table, memsys, BAR info, RUSD, GMMU static info, BIOS info, P2P caps,
   channel counts, the KGR static-info structs, GET_INFO_V2, chip details, the UVM
   buffer registrations, `COPY_SERVER_RESERVED_PDES`, the TSG and memory-descriptor
   params, the channel allocation through `nvrm_sys::V610: RmAbi`). The tests
   `control_layouts_are_the_ones_the_handlers_use` and `kgr::tests::layouts_are_the_documented_ones`
   pin these to the numbers the handlers were written against, so a regenerated
   binding that moves one fails there.
2. **Measured GSP layouts.** The GSP message structs are not in `nvrm-sys`. Their
   hand-written offsets (`RM_CONTROL_*`, `RM_ALLOC_*`, `FREE_*`, `DUP_*`, `GUEST_INFO_*`,
   `REGISTRY_*`, `SYSINFO_*`, `SI_*`, `FB_REGION_*`, `WPR_META_*`, `LIBOS_ARG_*`, `MQ_*`)
   are checked by `gsp_offsets_match_the_measured_abi` against the compiler-measured
   tables of [gsp-abi-610.md](gsp-abi-610.md) (a), which the test parses.
   The bindings are imported as `nvrm_sys::v610 as ffi`, deliberately not as `sys`:
   `cargo xtask abi --check` (a `tools/check.sh` step) treats every `sys::` path in
   the workspace as a use that must go through the multi-version `RmAbi`, and this
   module is single-version by construction.
3. **Classes and ids** against the bindings that have them (`NV01_ROOT`, `NV01_DEVICE_0`,
   `NV20_SUBDEVICE_0`, `NV2080_CTRL_CMD_BUS_GET_PCI_BAR_INFO`, `nvrm_sys::DRIVER_VERSION`).

A few layouts have neither a typed binding nor a measured table and stay hand-written
with their header citation: `rpc_update_bar_pde_v15_00`, the trace crash-buffer RPC,
`GSP_GET_FEATURES`, `CE_GET_CAPS_V2`, `CE_IS_DECOMP_LCE_ENABLED`,
`INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_TYPE`, `DMA_SET/UNSET_PAGE_DIRECTORY`,
`THERMAL_SYSTEM_EXECUTE_V2`. Control command numbers are hand-written too: `nvrm-sys`
carries almost none of them (only `NV2080_CTRL_CMD_BUS_GET_PCI_BAR_INFO` of the 77
the device uses).

At runtime the device notes a guest whose `SET_GUEST_SYSTEM_INFO` names another
driver than the emulated one (`guest driver "..." is not the emulated 610.57.04`); the
answer is unchanged, so the failure shows up where the layouts first differ. A guest of
the right version produces no extra event.

## What is still hard-coded, and why

| Where | What | Why it stays |
|---|---|---|
| `bar0.rs` | `NV_PMC_BOOT_0/1/42` at 0x0, 0x4, 0xA00 | Architectural on every NVIDIA GPU: the driver reads them before it knows the chip |
| `boot.rs` | `NV_PBUS_BAR0_WINDOW` reset value (last MiB of FB), BAR1 root one page above the BAR2 root in the non-WPR heap | Device conventions of the fake FB model (fb-model.md §4c), not chip facts |
| `fb.rs` | the GMMU walk: 49-bit VA, 4 levels plus dual PDE (format version 2) | Code, not data; Turing, Ampere and Ada share it. `chip.toml` names the format (`mmu_format = "gmmu-v2-49bit"`) and a chip with another format is refused until `fb.rs` implements it (Hopper and later: 5-level, 57-bit) |
| `channel.rs` | the method decoding of the host class (`clc46f.h`) and the Turing copy class (`clc5b5.h`), USERD GP_GET/GP_PUT at 0x88/0x8C | Code for the classes the chip file names; a chip whose host or CE class adds or moves methods needs a decoder of its own |
| `falcon.rs` | the falcon v4 register block layout (relative offsets) | The Turing-to-Ada falcon; GA102+ adds the BROM/PKC registers (see below) |
| `intr.rs` | the leaf/top layout of the interrupt tree, relative to the chip's base | The `dev_vm.h` layout of Turing to Ada |
| `msgq.rs` | the `msgqTxHeader` field order, 4 KiB slots, `_checkSum32` | The msgq library's layout, unchanged across chips (other-architectures.md §3); part of the version axis. 615.71.09 moves it: queue v2 with the read/write pointers in the `NV_PGSP_QUEUE_*`/`NV_PGSP_MSGQ_*` registers, no `msgqRxHeader`, MCTP-framed elements without checksum and sequence number ([blackwell-gb206.md](blackwell-gb206.md) §3.3). A `v615` build needs a second transport selected by the version module |
| `ctrl.rs` | internal handles `0xCA7A0001/2/3` | Device-owned, synthesized |
| `nvml.rs` | the routing rules (device-class controls to the host device, GSS legacy bit, BINAPI) | Driver-version behaviour of the guest kernel; the lists they apply to live in `ver` |
| `rpc.rs` | the naming rule `Leandro <board>-<size>` and the synthetic GID | Profile policy (D6), shared with Leandro v1 |
| `main.rs` | defaults: board data `rtx2070`, `LEA_VRAM_PROFILE_MIB` or 4096 | CLI defaults |
| `selftest.rs` | nothing chip- or board-specific; expectations come from the chip, the board and the config capture | |
| comments | TU106/Turing appear in comments and doc text as the example | Documentation |

## Adding a chip

1. Create `chips/<chip>/` with the five files, using `chips/tu106/` as the template.
   Sources: the chip's `published/<arch>/<chip>/dev_*.h` for offsets, the HAL
   selectors in `src/nvidia/generated/g_*_nvoc.c` for which implementation the driver
   uses, and a capture of a real card where one exists (fifo-device-info.md explains
   which device-info columns the host can supply through unprivileged controls).
2. Implement `Chip` in `src/chip/<chip>.rs` (as `tu106.rs`) and add it to
   `chip::by_name` and `CHIPS`. If the chip boots like Turing, `firmware_boot` returns
   a `TuringBoot` over its own `boot.toml`; otherwise write a new `FirmwareBoot`.
3. Add a board file for a card with the chip (`boards/<board>.toml`) and capture its
   config space and ROM under `caraxes/reference/<board>/`.
4. If `chip.toml` names driver enums the version module does not know (new engines,
   another RAM type), add them to the tables in `src/ver/v<NNN>.rs`.
5. Tests: a `tables_resolve` test with the literal values, a firmware-boot sequence
   test, and the E0 selftest through the protocol with the new board.

### Ampere GA10x and Ada AD10x (the least work, other-architectures.md §1 and "Assessment")

`kgspBootstrap`, `kgspPrepareForFwsecFrts`, `kgspExtractVbiosFromRom`, `kgspIsWpr2Up`,
`kgspWaitForGfwBootOk` and the Booter Load/Unload flow are the `_TU102` HALs, so
`turing.rs` carries over with a new `boot.toml`. New to model: the
`NV_PFALCON2_FALCON_BROM_PARAADDR/ENGIDMASK/CURR_UCODE_ID/MOD_SEL` registers of the
BOOT_FROM_HS path (`kernel_gsp_falcon_ga102.c:156-164`), which `falcon.rs` would latch
today but must accept on the new base; `bBootFromHs=TRUE` and CrashCat in
`kgspConfigureFalcon_GA102`; LIBOS3 and `gsp_ga10x.bin` in the guest (the device does
not serve firmware, so only the init-argument layout matters). Ada adds the
`kgspExecuteScrubberIfNeeded_AD102` step (`NV_PGC6_BSI_VPR_SECURE_SCRATCH_15`). The
GMMU format, channel classes (`AMPERE_CHANNEL_GPFIFO_A` C56F, `AMPERE_DMA_COPY_A/B`
C6B5/C7B5) and engine tables change and need their data and a check of the method
subset in `channel.rs`.

### Blackwell (GB10x, GB20x): checklist

Blackwell reuses the Hopper boot flow (other-architectures.md §1, §2, §5), which is a
new firmware-boot model, not a variant of `turing.rs`:

- **Boot model: GSP-FMC through FSP instead of FWSEC and Booter.** `kgspBootstrap_GH100`
  (the GB20x HAL, `generated/g_kernel_gsp_nvoc.c:731-748`): no FWSEC-FRTS, no SEC2 Booter, no
  `GFW_BOOT` scratch poll (`kgspWaitForGfwBootOk_GH100` defers to FSP,
  `gpuWaitForGfwBootComplete` is stubbed). **Default path (FSP present, as on GB206,
  `generated/g_kern_fsp_nvoc.c:234`):** `kfspWaitForSecureBoot_GB202` polls
  `NV_THERM_I2CS_SCRATCH` (0xAD00BC on GB202) for 0xFF
  (`fsp/arch/blackwell/kern_fsp_gb202.c:48-100`); `kfspSendBootCommands_GH100` sends an NVDM
  COT payload (GSP-FMC image address, FRTS offsets and sizes, the address of
  `GSP_FMC_BOOT_PARAMS`) over the FSP EMEM port and queue (`NV_PFSP_EMEMC/EMEMD` 0x8F2AC0/4,
  `NV_PFSP_QUEUE_*` 0x8F2C00, `NV_PFSP_MSGQ_*` 0x8F2C80) and reads FSP's NVDM response
  (`fsp/arch/hopper/kern_fsp_gh100.c:1030-1046, 1341-1458, 1500-1530`); then it waits for FSP to
  release the GSP target mask (GSP `HWCFG2` non-zero and not 0xBADF41xx, `:1069-1100`). The
  device therefore emulates FSP: boot-complete scratch, EMEM port, two rings, one NVDM message
  type and a fixed response; the WPR meta is found through the `GSP_FMC_BOOT_PARAMS` the COT
  payload names ([blackwell-gb206.md](blackwell-gb206.md) §2.2, `chips/gb206/boot.toml`).
  **Fallback (no FSP and no SEC2 GSP-FMC boot, `_kgspBootstrapGspFmc_GH100`):** the CPU points the RISC-V
  BootROM at the FMC image through `NV_PRISCV_RISCV_BCR_DMAADDR_FMCCODE/FMCDATA/
  PKCPARAM_LO/HI`, `NV_PRISCV_RISCV_BCR_DMACFG` and `NV_PRISCV_RISCV_CPUCTL`
  (`kernel_gsp_gh100.c:744-774`), and `NV_PFALCON_FALCON_MAILBOX0/1` carry the physical
  address of `GSP_FMC_BOOT_PARAMS` (measured in gsp-abi-610.md (a)), not a loader
  header. Both paths then share the rest: the model polls `NV_PFALCON_FALCON_HWCFG2_RISCV_BR_PRIV_LOCKDOWN` for the
  lockdown release (`:540-543`) and checks `NV_PGSP_FALCON_ENGINE_RESET_STATUS`
  (`:82-104`). A new `FirmwareBoot` (`src/chip/hopper.rs`) produces these side
  effects and raises `BootEvent::WprMeta` from the WPR meta that `GSP_FMC_BOOT_PARAMS`
  points to, and `CmdQueueDoorbell` as before.
- **Init RPC order.** `kgspBootstrap_GH100` sends the init RPCs before boot completes
  (`kgspSendInitRpcs`, `kernel_gsp_gh100.c:956-964`). The generic queue service answers
  whatever is queued when the doorbell rings; check that the first doorbell may come
  before `WprMeta`, in which case the static-info fallbacks of `rpc.rs` apply until the
  WPR meta is known.
- **WPR2 register.** `kgspIsWpr2Up_GB100` reads `NV_HUBMMU_PRI_MMU_WPR2_ADDR_HI`
  (`published/blackwell/gb100/dev_hubmmu_base.h:94`, `kernel_gsp_gb100.c:332-350`)
  instead of `NV_PFB_PRI_MMU_WPR2_ADDR_HI`.
- **Fuse base.** The debug fuse is `NV_FUSE_ZB_OPT_SECURE_GSP_DEBUG_DIS` off a variable
  fuse base (`kernel_gsp_gb100.c:296-298`), not `NV_FUSE_OPT_SECURE_GSP_DEBUG_DIS` at
  0x2174C.
- **Reset PLM.** `NV_PGSP_FALCON_RESET_PRIV_LEVEL_MASK` is checked before reset
  (`kernel_gsp_gb100.c:49-90`).
- **VBIOS.** `kgspExtractVbiosFromRom` is a stub from GH100 on; the PROM window and the
  synthetic FWSEC image of vbios-synthesis.md are not needed for boot. Check what the
  kernel still reads from the ROM.
- **Engine tables.** New engine caps, device-info table, interrupt table and subtree
  map, memsys values (GDDR7 or HBM: add the RAM type to `ver`), the chip-info register
  bases, and the channel and copy classes (Blackwell host and CE classes). The MC,
  RM engine and ENG_DESC names may be new: add them to the version module's tables.
- **GMMU format.** Hopper and later use a 5-level, 57-bit format: `fb.rs` needs a
  second walker and `chip.toml` a new `mmu_format` name.
- **Kernel-RM surface.** Re-derive every `[registers]` offset of `chip.toml` from the
  chip's `dev_vm.h`, `dev_ram.h` and `dev_bus.h`; the interrupt tree base and the
  usermode window may move.
- **Driver version.** Blackwell needs a driver that supports it; 610.57.04 does
  (the GB10x/GB20x HALs are in the tree), so no new `ver` module is required for the
  first Blackwell device.

## Adding a driver version (615.71.09)

A build emulates exactly one driver version; the guest refuses GSP firmware of
another version, so there is no runtime mediation (gsp-abi-610.md (e), "Version
selection").

615.71.09 is more than a layout change ([blackwell-gb206.md](blackwell-gb206.md) §3): the
message-queue transport moves to register-held pointers and MCTP framing (`msgq.rs`, above), and
`GSP_SET_SYSTEM_INFO`, `SET_REGISTRY`, `GET_GSP_STATIC_INFO` and `GSP_INIT_DONE` are replaced by
one `GMCAPI_CMD_GSP_INIT` whose request and response are key/value (NVKV) encoded. The steps
below cover the struct and control layouts; the transport and the NVKV static-info writer are
code of their own, selected by the version module.

1. **Vendor the headers.** Fetch 615.71.09 into `vendor/open-gpu-kernel-modules` (a
   second checkout or tag) and run `tools/build.sh vendor-abi` for the ioctl closure;
   `crates/nvrm-sys` already has `v615` (bindings generated by `cargo xtask abi`).
2. **Measure the GSP layouts.** Run `caraxes/tools/gsp-abi-sizes.sh` against the new
   tree (it picks the 615 field list from the tree's version, lab-blackwell.md §5) and write `docs/caraxes/gsp-abi-615.md` with the same section (a) format. The
   recommended step beyond that is gsp-abi-610.md (e): a sibling crate `nvgsp-sys`
   generated by the same xtask (`cargo xtask abi --crate nvgsp-sys`) with the wrapper,
   include roots and defines in its `abi.toml`, so the GSP structs become typed
   bindings like the ioctl ones and `src/ver` computes their offsets instead of
   stating them.
3. **Create `src/ver/v615.rs`** as a copy of `v610.rs` with
   `use nvrm_sys::v615 as sys`, `DRIVER_VERSION = "615.71.09"`,
   `RmAbi = nvrm_sys::V615`, the test pointing at `gsp-abi-615.md`, and then regenerate
   or re-check, item by item:

   | Item | Source in the new tree | How |
   |---|---|---|
   | RPC function numbers `FN_*`, `EV_GSP_INIT_DONE` | `src/nvidia/inc/kernel/vgpu/rpc_global_enums.h` | constants footprint `NV_VGPU_MSG_.*` of the generated crate, or read the X-macro |
   | Message structs and their versions (`rpc_gsp_rm_control_v03_00`, `rpc_init_done_v17_00`, `rpc_update_bar_pde_v15_00`, ...) | `src/nvidia/generated/g_rpc-structures.h` | gsp-abi-sizes.sh tables; a bumped `_vXX_YY` means a payload change |
   | `GspStaticConfigInfo`, `GspSystemInfo`, `GspFwWprMeta`, libos and RMARGS arguments | `gsp_static_config.h`, `gsp_fw_wpr_meta.h`, `gsp_init_args.h`, `libos_init_args.h` | gsp-abi-sizes.sh tables; the `SI_*`, `SYSINFO_*`, `WPR_META_*`, `MQ_*` test |
   | RPC header version, MCTP/NVDM constants, `maxRpcSize` | `g_rpc-message-header.h`, `mctp_format.h`, `nvdm_format.h` | gsp-abi-sizes.sh derived constants |
   | Control command numbers `CMD_*` | `sdk/.../ctrl/ctrl2080/*.h` (internal ids move most) | add `NV2080_CTRL_CMD_.*` to the constants footprint of the generated crate; until then grep each |
   | Control parameter layouts | `nvrm_sys::v615` | computed automatically; the pinning tests show what moved |
   | Driver enums (`NV2080_ENGINE_TYPES`, `RM_ENGINE_TYPES`, `ENG_DESC_CLASSES`, `MC_ENGINE_IDX`, `INTR_CATEGORIES`, `RAM_TYPES`) | `cl2080_notification.h`, `gpu_engine_type.h`, `g_eng_desc_nvoc.h`, `engine_idx.h`, `ctrl2080mc.h`, `ctrl2080fb.h` | grep; the chip files stay as they are |
   | `FORWARD`, `LEGACY`, `BINAPI` lists of `nvidia-smi` | `matrix/traces/615.71.09/nvml.tsv` (the host's own `nvidia-smi -q`) | re-capture with Leandro's matrix tooling; the sizes change with the structs |
   | `FECS_EVENT_RECORD_SIZE`, `INFO_CHIP_UNIQUE_*`, `INDEX_FORWARD_TO_PHYSICAL` | `fecs_event_list.c`, the host trace, `subdevice_ctrl_gpu_kernel.c` | read |
   | Phase-5 inventory (which controls arrive, in which order) | the guest's own boot | run E4 with the new guest; every `unanswered` is a new row |

4. **Feature and selection.** Add `v615 = ["nvrm-sys/v615"]` to the crate's
   `[features]`, `#[cfg(feature = "v615")] pub mod v615;` to `src/ver/mod.rs`, and make
   `active` pick exactly one version even when features unify (they are additive, as
   in `nvrm-sys`, and `tools/check.sh` builds with `--all-features`):

   ```rust
   #[cfg(feature = "v615")]
   pub use v615 as active;
   #[cfg(all(feature = "v610", not(feature = "v615")))]
   pub use v610 as active;
   ```

   A 615 build is then `cargo build --release -p vfio-user-nvgpu --no-default-features
   --features v615`.
5. **Verify** as for any change: the unit tests (the pinning tests list every moved
   offset), the E0 selftest, a guest boot with the 615 driver in the guest and on the
   host, the `nvml` gate and the UVM check.
