<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Nova-core against the synthetic TU106 (side assignment, package C)

Research and planning only, 2026-09-23. This note covers package C of
[briefing-display-nova.md](briefing-display-nova.md) §9-§11. No VM was started, the host
GPU was not touched and the device crate was not changed. The only host state read was
`/usr/lib/firmware/nvidia/tu10{2,6}/gsp` (package files) and the host kernel config.

The question: can the Linux `nova-core` driver probe the Caraxes device as a second,
independent guest driver? What does the device need for that, and what does it cost?

## Summary

- **Target.** Pin **Linux v7.2** (`8d3ae59288f1e7d58d76558a6ee96d533bc5019f`). It is the
  newest tagged kernel whose nova-core loads firmware that linux-firmware actually ships
  (`*-570.144.bin`).
  - v7.3-rc4 (`93f51579e7df`) and drm-rust-next (`896ed083362758b33c49a1b5e5a3423c5814d87e`)
    switched to unversioned `*.tlv` wrappers.
  - linux-firmware tag `20260916` has no NVIDIA `.tlv` file at all. So these two trees can
    boot no GPU with stock firmware, real or synthetic.
  - drm-rust-next is recorded below as the next target, because it also reads VRAM ranges
    and BAR1.
- **Today's device cannot talk to nova at all.** The very first queue element already
  fails.
  - r570.144 frames each message with a 48-byte element header. 610.57.04 uses a 16-byte
    MCTP/NVDM header.
  - Before that point, the probe already stops in the VBIOS parse, because the synthetic
    FWSEC image has a ROM signature and application id that nova does not accept.
- **Difference list.** There are **15 differences**, all in §1.9:
  - 10 block v7.2 at gates N1-N4;
  - 4 concern only drm-rust-next (3 fixes and 1 verification);
  - 1 is the version selection.

  Another **14 points were checked and found compatible**.
- **ABI.** nova uses 23 structs (24 in drm-rust-next), 61 constants (62) and 29
  function/event numbers of r570.144.
  - Against 610.57.04, 13 structs (14) differ: 6 (7) in layout and 7 because 610 dropped
    them (the CPU sequencer). No constant or function number changed its value.
  - The device needs changes only in:
    - the element framing;
    - the static-info encoder (1656 bytes, name at 1260, `bar1PdeBase` at 1536, FB regions
      at 344/352 with a 48-byte stride);
    - a new `GSP_RUN_CPU_SEQUENCER` event.
- **Size of `ver/v570.rs`.** About **150-200 lines** (about 45 constants plus tests),
  written as an overlay on `v610`. A standalone GSP-only module is about 300 lines. On top
  of that come about 80-100 lines of generic changes in `msgq.rs`/`boot.rs`, and about
  150-250 lines of boot-model work (run classification, unload, re-link, runtime version
  selection).
- **Effort.** About **3.5-5 agent-days** on the device side and **1.5-2.5 agent-days** for
  the guest, so **5-7.5 agent-days** in total to N4 on v7.2 (§4).

## Pinned sources

| What | Pin | Where it was read |
|---|---|---|
| Linux, tagged | v7.2, tag object `237a1c39e8dfd3e1c6f1f023eea37a48ec04cc63`, commit `8d3ae59288f1e7d58d76558a6ee96d533bc5019f` (2026-08-16) | sparse clone of `drivers/gpu/nova-core`, `Documentation/gpu/nova` (scratchpad `v72/`) |
| Linux, newest rc | v7.3-rc4, commit `93f51579e7df248780214094418f205253383cc5` (2026-09-20) | checked only for the firmware scheme (TLV) |
| drm-rust-next | `896ed083362758b33c49a1b5e5a3423c5814d87e` (2026-09-23), gitlab.freedesktop.org/drm/rust/kernel | scratchpad `drn/` |
| NVIDIA r570.144 headers | tag 570.144, commit `8ec351aeb96a93a4bb69ccc12a542bf8a8df2b6f` | measured with a copy of `caraxes/tools/gsp-abi-sizes.sh` |
| NVIDIA 610.57.04 | `vendor/open-gpu-kernel-modules` `e4a5faa2567f` | [gsp-abi-610.md](gsp-abi-610.md) |
| linux-firmware | tag `20260916`, commit `ab23307cfe7f9366c819025ca3e4778299bc2db2`; `main` `965ee9b6ec25` has identical `nvidia/tu10{2,6}` | gitlab.com/kernel-firmware/linux-firmware |
| This tree | `79b7d0b875d9` plus the uncommitted `crates/vfio-user-nvgpu` of 2026-09-23 | read only |

Paths below such as `gpu.rs` are relative to `drivers/gpu/nova-core/` at v7.2 unless
marked (drn).

---

## 1. What nova-core needs from the device to probe a TU106

The probe order is `driver.rs` `NovaCoreDriver::probe` → `gpu.rs` `Gpu::new` →
`gsp/boot.rs` `Gsp::boot` → `gsp/hal/tu102.rs` `Tu102::boot` / `post_boot`. Unbind runs
`Gpu::drop` → `Gsp::unload`.

### 1.1 PCI identity and BARs

- **Binding.** `driver.rs` `PCI_TABLE` matches **class** VGA or 3D with vendor 0x10de. It
  checks no device id.
  - The device serves class 0x030000 and vendor 0x10de from the RTX 2070 capture (`10de:1f02`
    rev a1, subsystem `1458:4011`), so nova binds.
  - Nothing else may claim the function, in particular `nouveau`, which also matches.
- **Resources.** `probe` enables memory and bus mastering and maps **BAR0 with exactly
  16 MiB** (`iomap_region_sized::<SZ_16M>(0)`).
  - `GspSetSystemInfo::init` (`gsp/fw/commands.rs`) reads `resource_start` of BAR 0, 1 and
    3.
  - The device has BAR0 16 MiB (32-bit), BAR1 256 MiB and BAR3 32 MiB (64-bit), and BAR5
    I/O ([environment.md](environment.md)), so this matches.
  - (drn) `driver.rs` `bar1_resource_index` finds BAR1 at index 1, because BAR0 is not
    64-bit, and maps all of it.
- **DMA and interrupts.** nova sets a 47-bit DMA mask (`gpu/hal/tu102.rs`
  `Tu102::dma_mask`) and never requests an interrupt: every wait polls.
  - The guest needs neither the `caraxes_dma_range` module nor an IOMMU for N0-N4. Its DMA
    addresses are guest-physical, and the device reads them through the `DMA_MAP` fds as it
    does for the stock driver in E2-E4.

### 1.2 Registers, in the order nova touches them

| # | Where (v7.2) | Register (BAR0) | nova expects | Device |
|---|---|---|---|---|
| 1 | `gpu.rs` `Spec::new` | `NV_PMC_BOOT_0` 0x0 | not older than Fermi: `arch1 == 0 && arch0 < 0xc` must be false | 0x166000A1: arch0 0x16, ok |
| 2 | same | `NV_PMC_BOOT_42` 0xA00 | 29:24 architecture 0x16 (Turing), 23:20 implementation 6, so chipset 0x166 = TU106; revision 19:16/15:12 | 0x166A1000 (`chips/tu106/chip.toml`): TU106, revision a.1 |
| 3 | `gpu/hal/tu102.rs` `wait_gfw_boot_completion` (4 s poll) | 0x118128 bit 0, then 0x118234 bits 7:0 == 0xFF | FWSEC lowered the PLM, `GFW_BOOT` completed | `boot.toml` consts 1 and 0xFF. nova does **not** poll GSP `CPUCTL` halted first, unlike OpenRM A5.1 |
| 4 | `fb.rs` `SysmemFlush::register` / `Drop` | `NV_PFB_NISO_FLUSH_SYSMEM_ADDR` 0x100C10 | written with page PA >> 8 (`EINVAL` if PA >= 2^40); read back at drop and compared | latched by `bar0.rs`; guest PAs are below 4 GiB |
| 5 | `falcon/gsp.rs` `clear_swgen0_intr` | GSP `IRQSCLR` 0x110004 | write only | latched |
| 6 | `fb.rs` `FbLayout::new` → `fb/hal/tu102.rs` | `NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE` 0x100CE0 | `mag << scale` MiB, ECC bit 30 | encoded from the profile (`turing.rs` `local_memory_range`) |
| 7 | same, `display_enabled_gm107` | `NV_FUSE_STATUS_OPT_DISPLAY` 0x21C04 | bit 0 = display disabled; if enabled, `NV_PDISP_VGA_WORKSPACE_BASE` 0x625F04 is read | const 1 (disabled). The VGA workspace is the last 1 MiB, as in OpenRM, so FRTS = FB − 2 MiB |
| 8 | `vbios.rs` `VbiosIterator` | PROM 0x300000.. (32-bit reads, up to 1 MiB); 0x300000/4/8 IFR check | §1.3 | served from `synth-vbios.bin` |
| 9 | `gsp/hal/tu102.rs` `run_fwsec_frts` | `NV_PFB_PRI_MMU_WPR2_ADDR_HI` 0x1FA828 | must be 0 before FWSEC-FRTS (else `EBUSY` "GPU needs to be reset") | 0 until FWSEC |
| 10 | `falcon.rs` `Falcon::reset` | `ENGINE` +0x3C0 set/clear with a 10 µs sleep; poll `DMACTL` +0x10C bits 2:1 (10 ms); write `RM` +0x84 = BOOT_0 | reset handshake | `falcon.rs` resets on the rising edge, `DMACTL` reads 0 |
| 11 | `firmware/fwsec/bootloader.rs` `FwsecFirmwareWithBl::run` → `falcon.rs` `pio_load` | GSP `FBIF_CTL` +0x624 bit 7; `DMACTL` = 0; `IMEMC/IMEMT/IMEMD` (non-secure, at 64 KiB − BL size, tags from `start_tag`); `DMEMC/DMEMD` (84-byte `BootloaderDmemDescV2` at DMEM 0); `BOOTVEC` = start_tag << 8; `FBIF_TRANSCFG(ctx_dma=4)` = coherent sysmem, physical | PIO load of the generic bootloader | the same writes as OpenRM C2.1-C2.9 |
| 12 | `falcon.rs` `boot` | `MAILBOX0` = 0, `CPUCTL` (or `CPUCTL_ALIAS` if bit 6) start, poll halted 2 s, read `MAILBOX0` == 0 | FWSEC-FRTS ran | halts at once, `MAILBOX0` stays 0 |
| 13 | `run_fwsec_frts` | `NV_PBUS_SW_SCRATCH(0xE)` 0x1438 bits 31:16 == 0; `WPR2_ADDR_LO` 0x1FA824 << 12 == `frts.start`; `WPR2_ADDR_HI` != 0 | WPR2 created where asked | `turing.rs` `fwsec_done` reads the FRTS offset from guest RAM at `dataDmaBase` (DMEM 0x40) + 0xA0. nova's descriptor has `data_dma_base` at 0x40 as well, and the DMEM mapper command buffer puts `frts_region.addr` at +0x20 of `cmd_in` = 0xA0 |
| 14 | `Tu102::boot` | GSP reset (#10), then `MAILBOX0/1` = libos args PA, start, **wait for halt**, read the mailboxes | a second GSP start in every boot; OpenRM only writes the mailboxes | halts at once (run 2) |
| 15 | `firmware/booter.rs` `BooterFirmware::run` | SEC2 (0x840000) reset, PIO load (Turing `load_method` = PIO: `IMEMC/T/D` non-secure and secure, `DMEMC/D`, `BOOTVEC`), `MAILBOX0/1` = `GspFwWprMeta` PA, start, halt, `MAILBOX0` == 0 | Booter-Load | `turing.rs`: SEC2 run 1 = Booter-Load, `MAILBOX0` := 0, GSP RISC-V active, `BootEvent::WprMeta` |
| 16 | `gsp/boot.rs` `Gsp::boot` | GSP `FALCON_OS` 0x110080 = bootloader `app_version` | write only | latched |
| 17 | same (5 s poll, 10 ms step) | `NV_PRISCV_RISCV_CORE_SWITCH_RISCV_STATUS` 0x111240 bit 0 | RISC-V active | set by Booter-Load |
| 18 | `gsp/cmdq.rs` `Cmdq::notify_gsp` | `NV_PGSP_QUEUE_HEAD` 0x110C00 = 0 after every command | doorbell | `CmdQueueDoorbell` |
| 19 | `gsp/sequencer.rs` (only if the sequence has commands) | arbitrary BAR0 read/write/poll; `CORE_RESUME` also: GSP reset, mailboxes, SEC2 start, poll `NV_PGC6_BSI_SECURE_SCRATCH_14` 0x1180F8 bit 26 (2 s), SEC2 `MAILBOX0` == 0, `FALCON_OS`, RISC-V active | GSP-driven register sequence | not modelled; an empty sequence avoids it (§1.6) |
| 20 | `gsp/boot.rs` `shutdown_gsp` (5 s poll) | GSP `MAILBOX0` bit 31 (`LIBOS_INTERRUPT_PROCESSOR_SUSPENDED`) | set after `UNLOADING_GUEST_DRIVER` | never set |
| 21 | `gsp/hal/tu102.rs` `Sec2UnloadBundle::run` | FWSEC-SB on GSP (as #10-#12); if `WPR2_HI` != 0: SEC2 reset, Booter-Unload load, `MAILBOX0/1` = 0xFF sentinel, start, halt, `MAILBOX0` == 0, `WPR2_HI` == 0 | reset for the next probe | SEC2 run >= 2 clears WPR2 and `MAILBOX0`, stops RISC-V |
| 22 | (drn) `mm/pramin.rs`, `mm/tlb.rs`, `mm/bar_user.rs` | `NV_PBUS_BAR0_WINDOW` 0x1700 plus PRAMIN 0x700000; TLB flush `PDB_LO/HI` 0xB830A0/A4, `CTRL` 0xB830B0 (poll trigger clear, 2 s); BAR1 accesses through page tables under `bar1PdeBase` | VRAM page tables written by nova, BAR1 translation | modelled (`fb.rs` walk, `chip.toml` `mmu_invalidate*`, trigger reads back 0); not tested with a nova-written table |

Not read by nova on Turing:
- the debug fuse 0x2174C (`signature_reg_fuse_version` returns 0 on TU102, `falcon/hal/tu102.rs`);
- `HWCFG` IMEM size (the bootloader is placed at 64 KiB minus its size, which the device's
  0x100 blocks match);
- PTIMER, `NV_PGRAPH_PRI_FECS_FEATURE_READOUT`, `DEBUGINFO`/CrashCat.

### 1.3 The VBIOS nova parses

`vbios.rs` `Vbios::new` walks the images through the PROM window:
- It accepts the ROM signatures **0xAA55 and 0x4E56 only** (`PciRomHeader::new`) and the
  PCIR signatures "PCIR"/"NPDS".
- NPDE `subimage_len` takes precedence over the PCIR length, and NPDE `last_image` decides
  where the walk ends.
- It keeps the first PCI-AT image (type 0x00) and concatenates everything from the first
  FWSEC image (type 0xE0) on. Other types are ignored.

`FwSecBiosImage::new` then works as follows:
1. It finds the BIT (`FF B8 'BIT' 00`) in the PCI-AT image.
2. It takes token **0x70 FALCON_DATA** (6-byte token layout, 16-bit `data_offset`).
3. It reads the u32 table pointer and subtracts the PCI-AT image length. That equals
   OpenRM's `expansionRomOffset` arithmetic.
4. It parses the PMU lookup table and takes the entry with **application id 0x85
   (`APPID_FWSEC_PROD`) only**.

`header()` accepts descriptor V2 or V3. For V2, `signature_count` is 0, so no signature is
patched. `new_fwsec` then:
- finds the application-interface table at `imem_load_size + interface_offset`
  (`version == 1`);
- finds the DMEM mapper entry (id 4) at any position;
- writes `init_cmd` = 0x15 (FRTS) or 0x19 (SB), and the FRTS command at
  `cmd_in_buffer_offset` (`ReadVbios` 24 bytes, then `FrtsRegion` with addr/size in 4 KiB
  units, type FB = 2).

Compared with the synthetic image
([vbios-synthesis.md](vbios-synthesis.md), `caraxes/tools/synth-vbios.py`
`build_ext_image`):
- The BIT, the table layout (header 6/6/1), the V2 descriptor of 60 bytes, the app-interface
  header and entries and the DMEM mapper all fit.
- Two values do not:
  - the extension image starts with **0xBB77** (`PCI_EXP_ROM_SIGNATURE_NV2`);
  - the default `--appid` is **0x05** (`FIRMWARE_SEC_LIC`).

  OpenRM accepts both. nova accepts neither, so the probe ends in `Vbios::new`.
- The fix serves both drivers: 0x4E56 as the extension signature and `--appid 0x85`. OpenRM
  accepts 0x85 because the device reports the production fuse (0x2174C = 1).
- So the answer to the briefing's question is **yes**: nova needs the BIT/FALCON_DATA/PMU
  table/FWSEC/NPDE structure. It never executes the code bytes.

### 1.4 LIBOS arguments and RMARGS

`gsp.rs` `Gsp::new` fills one 4 KiB page of `LibosMemoryRegionInitArgument` (32 bytes
each; kind CONTIGUOUS = 1, loc SYSMEM = 1):
- **by position**: 0 `LOGINIT`, 1 `LOGINTR`, 2 `LOGRM` (each 16 pages, the PTE array from
  qword 1), 3 `RMARGS`;
- the rest is zero.

`id8` is the ASCII name right-aligned in a big-endian u64, the same encoding as OpenRM.
`RMARGS` is a whole page (`GspArgumentsPadded`) holding `GSP_ARGUMENTS_CACHED`:
`MESSAGE_QUEUE_INIT_ARGUMENTS` with `sharedMemPhysAddr`, `pageTableEntryCount` = 0x81,
`cmdQueueOffset` = 0x1000, `statQueueOffset` = 0x41000, and `bDmemStack` = 1.

The device (`boot.rs` `link_queues`) scans up to 16 entries **by id8** until a zero id and
reads the four `MQ_*` fields at 0/8/16/24. r570 and 610 place them identically; r570's
`GSP_ARGUMENTS_CACHED` is 72 bytes against 152 in 610, but the device reads nothing beyond
the queue arguments. So this part is compatible as it is.

### 1.5 Boot sequence (`gsp/hal/tu102.rs`, Turing path)

1. `Vbios::new`.
2. Prepare the unload bundle, which needs the FWSEC-SB image and `booter_unload`. On
   failure it only warns.
3. `run_fwsec_frts`: check that WPR2 is empty, load FWSEC-FRTS through the generic
   bootloader and run it on the GSP falcon, then check `SCRATCH_0E` and WPR2 against
   `FbLayout::frts`.
4. Reset the GSP falcon, put the libos PA into its mailboxes, and start it and wait for the
   halt (an extra step compared with OpenRM).
5. Run Booter-Load on SEC2 with the `GspFwWprMeta` PA.
6. In `Gsp::boot`:
   - write `FALCON_OS` and poll RISC-V active;
   - v7.2 then sends **`GSP_SET_SYSTEM_INFO`** and **`SET_REGISTRY`** (no reply);
   - `post_boot` runs `GspSequencer::run`, which **waits up to 5 s for
     `GSP_RUN_CPU_SEQUENCER`** and executes its commands;
   - `wait_gsp_init_done` waits for **`GSP_INIT_DONE`** (5 s per message);
   - finally it sends **`GET_GSP_STATIC_INFO`** (with reply) and prints the name.
7. (drn) The two init commands are queued **before** step 3 (`gsp/boot.rs` `Gsp::boot`), and
   `GET_GSP_STATIC_INFO` moves to `Gpu::new` (`gsp.rs` `get_static_info`).

**WPR2 and FB layout.** `fb.rs` `FbLayout::new` computes the whole layout itself:
- the VGA workspace: last 1 MiB;
- FRTS: 1 MiB below the 128 KiB-aligned workspace;
- the bootloader, then the ELF (64 KiB aligned);
- the WPR2 heap (LIBOS2 on Turing, 64-256 MiB clamp, 8 MiB base + 96 MiB client +
  96 KiB/GiB);
- `GspFwWprMeta` (1 MiB aligned);
- a 1 MiB non-WPR heap below it.

It passes all of that in `GspFwWprMeta` (`gsp/fw.rs` `GspFwWprMeta::new`: 256 bytes,
magic 0xdc3aae21371a60b3, revision 1, the same layout as 610). The device reads the meta at
Booter-Load (`rpc.rs` `WprLayout::read`) and derives its FB regions and BAR roots from it,
so a different heap size only moves addresses.

After `GSP_INIT_DONE`, v7.2 needs nothing but the static-info reply. drn uses the
static info for:
- a buddy allocator over the first usable FB region;
- BAR1 page tables rooted at `bar1PdeBase`, written through PRAMIN;
- TLB flushes;
- the optional `CONFIG_NOVA_CORE_SELFTESTS`.

### 1.6 Queue protocol

`gsp/cmdq.rs` and `gsp/fw.rs` define the queues:

- **Shared memory.** `GspMem` is 0x81000 bytes: 1 PTE page (512 entries), then two
  `Msgq`s of 0x40000 bytes each, each with `msgqTxHeader` (version 0, size 0x40000,
  msgSize 0x1000, msgCount 63, flags 1 = `SWAP_RX`, rxHdrOff 32, entryOff 0x1000), a 4-byte
  rx header and 63 data pages. This is the same geometry as OpenRM. The device checks it in
  `msgq.rs` `QueueState::link` and accepts it.
- **Element.** `GSP_MSG_QUEUE_ELEMENT` r570 is 48 bytes: authTag[16], aad[16],
  `checkSum`@32, `seqNum`@36, `elemCount`@40, pad. The RPC header (32 bytes: version
  0x03000000, signature "VRPC", length = 32 + payload, function, `rpc_result`/`_private` =
  0xffffffff, sequence) follows at 48.
- **Checksum.** It is `_checkSum32` (XOR of LE qwords, folded) over element header + RPC
  header + payload. It is computed in `send_single_command` and must fold to 0 on receive
  (`wait_for_msg`: "bad checksum" → `EIO`). The algorithm equals the device's `checksum32`;
  only the header size and field offsets differ.
- **Sequence numbers.** nova numbers its commands (`CmdqInner::seq`, element `seqNum`) but
  **checks neither `seqNum` nor the RPC `sequence` of what it receives**. It matches a reply
  **by function only** (`receive_msg`). There is no MCTP/NVDM check.
- **Reply or not.** The typed commands are in `gsp/commands.rs`:

  | Command | Function | Reply |
  |---|---|---|
  | `SetSystemInfo` | 72 | `NoReply` |
  | `SetRegistry` | 73; 3 DWORD keys (v7.2: `RMSecBusResetEnable`, `RMForcePcieConfigSave`, `RMDevidCheckIgnore`; drn adds `RMSetSriovMode` when vGPU is on) | `NoReply` |
  | `GetGspStaticInfo` | 65, zeroed payload of the full struct | reply of the same function |
  | `UnloadingGuestDriver` | 47 | reply of function 47, payload ignored |

  nova waits for the events `GspInitDone` (0x1001, payload ignored) and `GspSequence`
  (0x1002, `rpc_run_cpu_sequencer_v17_00`: `bufferSizeDWord`@0, `cmdIndex`@4, regSaveArea,
  then the commands). **`rpc_result` is never checked.**
- **Consumption.** A message of another known function is consumed and dropped (`ERANGE`,
  loop).
  - A function code outside nova's `MsgFunction` list returns **`EINVAL` without advancing
    the read pointer**. The queue is then stuck.
  - So the device must only ever send nova-known codes: the 20 functions and 9 events in
    `gsp/fw.rs` `MsgFunction`.
  - A consequence of dropping: an event nova is not waiting for at that moment is lost. The
    order must be **`RUN_CPU_SEQUENCER` first, then `GSP_INIT_DONE`**.
- **Read pointer.** nova advances it by ceil(length / 4 KiB) of its own computed length and
  ignores `elemCount`.

### 1.7 GET_GSP_STATIC_INFO

`GetGspStaticInfoReply::read` takes `GspStaticConfigInfo_t` as a prefix of the reply
payload (`from_bytes_prefix`). A payload shorter than **1656 bytes** (r570) is `EIO`, and
the probe fails.

- **v7.2** reads only `gpuNameString`, 64 bytes at **1260**. It must be NUL-terminated
  UTF-8, otherwise nova only warns "GPU name unavailable".
- **drn** also reads `bar1PdeBase` at **1536** and `fbRegionInfoParams`: `numFBRegions` at
  344, regions at 352 with a **48-byte stride**.
  - A usable region needs `reserved == 0`, `bProtected == 0`, **`supportCompressed != 0`**
    and `supportISO != 0`.
  - The first usable region feeds the buddy allocator (`ENODEV` if none). The maximum limit
    + 1 becomes `total_fb_end`.

The device builds the 610 layout: 1600 bytes, name at 1352, `bar1PdeBase` at 1504, regions
at 320/328 with a stride of 56 (`rpc.rs` `build_static_info`). It sets
`supportCompressed = 0` on purpose, so that OpenRM picks no compressible kinds.

### 1.8 Unload, reset, re-probe

`Gsp::unload` (`gsp/boot.rs`) runs in this order:
1. `UNLOADING_GUEST_DRIVER` with `newLevel` = 0, `bInPMTransition` = 0.
2. It waits for the reply.
3. It polls GSP `MAILBOX0` bit 31 for 5 s.
4. It always runs the unload bundle: FWSEC-SB on the GSP falcon, then Booter-Unload on SEC2
   if WPR2 is set, and checks that WPR2 is gone.
5. Success prints "GSP successfully unloaded".

The same guard also runs this path when the boot fails after the unload bundle exists.
A second probe does **not** reset the device: Linux issues no function reset on
unbind/bind, and nova relies on the unload bundle having returned the GPU to its
post-GFW state.

### 1.9 The difference list

The first group needs device work. "nova expects X, device does Y".

| # | Gate | nova expects (file, function @ v7.2 unless noted) | device does (file, function) | Fix |
|---|---|---|---|---|
| 1 | N1 | extension-image ROM signature 0xAA55 or 0x4E56 (`vbios.rs` `PciRomHeader::new`) | 0xBB77 (`synth-vbios.py` `build_ext_image`) | write 0x4E56; OpenRM accepts it too |
| 2 | N1 | PMU table entry with application id 0x85 (`vbios.rs` `FwSecBiosImage::new`) | default `--appid` 0x05 (`synth-vbios.py`) | generate with 0x85 (OpenRM accepts it under fuse = 1) |
| 3 | N2 | queue element header 48 bytes, `checkSum`@32, `seqNum`@36, `elemCount`@40, RPC header @48 (`gsp/fw.rs` `GspMsgElement`) | 16-byte MCTP/NVDM header, `checkSum`@8, `seqNum`@12, RPC header @16 (`msgq.rs` `receive`/`send`, `ver::v610::ELEMENT_HEADER_SIZE`) | parameterize `msgq.rs` by the version (header size, field offsets, optional MCTP words); max RPC 65488 instead of 65520 |
| 4 | N2 | a `GSP_RUN_CPU_SEQUENCER` event (0x1002) before `GSP_INIT_DONE`, within 5 s of `SET_REGISTRY` (`gsp/sequencer.rs` `GspSequencer::run`, `gsp/hal/tu102.rs` `post_boot`) | never sends it; 610 removed the sequencer structs (`boot.rs` `link_queues`) | send an empty sequence (`bufferSizeDWord` 0, `cmdIndex` 0, 40-byte payload) for r570 guests |
| 5 | N2 | order: sequencer, then `INIT_DONE`; a message nova is not waiting for is dropped (`cmdq.rs` `receive_msg`, `Cmdq::send_command` loop) | sends `GSP_INIT_DONE` at queue-link time, before any other message (`boot.rs` `link_queues`) | emit `[SEQUENCER, INIT_DONE]` once `SET_REGISTRY` has been read (or at link, in that order) |
| 6 | N3 | static-info reply >= 1656 bytes, `gpuNameString` @1260 (`gsp/fw/commands.rs` `GspStaticConfigInfo`, `commands.rs` `GetGspStaticInfoReply::read`) | 1600 bytes, name @1352 (`rpc.rs` `build_static_info`, `ver::v610::SI_*`) | r570 encoder (§2) |
| 7 | N4 | a reply to `UNLOADING_GUEST_DRIVER` (47), then GSP `MAILBOX0` bit 31 within 5 s (`gsp/boot.rs` `shutdown_gsp`) | the default arm answers `NV_ERR_NOT_SUPPORTED` (accepted, since nova ignores `rpc_result`) and never sets bit 31 (`boot.rs` `service_queues`) | handle 47: `NV_OK`, set `MAILBOX0` bit 31, mark the GSP suspended, stop servicing |
| 8 | N4 | a re-probe without device reset. FWSEC-FRTS is then the GSP falcon's 4th start (every probe starts GSP for FRTS and once bare; unload runs FWSEC-SB) (`gsp/hal/tu102.rs`) | only GSP run == 1 counts as FWSEC-FRTS; the counter survives falcon resets (`turing.rs` `write32`, `falcon.rs` `runs`) | classify a GSP start by what the CPU loaded: bootloader descriptor present and DMEM-mapper `init_cmd` 0x15 (FRTS) or 0x19 (SB) in guest RAM |
| 9 | N4 | Booter-Load of the next probe is SEC2's 3rd start (`firmware/booter.rs` `run`) | SEC2 run 1 = Booter-Load, run >= 2 = Booter-Unload (`turing.rs` `write32`) | a state machine instead of a counter: SEC2 start with WPR2 up and GSP not running = Load; with GSP running or suspended = Unload |
| 10 | N4 | fresh libos args and queues on every probe (`gsp.rs` `Gsp::new`, `cmdq.rs` `Cmdq::new`) | links the queues once and keeps them until a device reset (`boot.rs` `service_queues`, `if self.queues.is_none()`) | drop the link at Booter-Unload or at a new `WprMeta`, re-link from the new libos PA |
| 11 | drn N2 | queue service without a later doorbell: `SET_SYSTEM_INFO`/`SET_REGISTRY` are queued and rung **before** FWSEC and Booter, while the GSP mailboxes are still 0, and the next doorbell comes only after `INIT_DONE` (drn `gsp/boot.rs` `Gsp::boot`) | links and services only on a doorbell, taking the libos PA from the GSP mailboxes at that moment (`turing.rs` `write32` `gsp_queue_head`, `boot.rs` `CmdQueueDoorbell`) | at `BootEvent::WprMeta` take the libos PA from the GSP mailboxes, link and drain; ignore doorbells with mailbox PA 0 (the Hopper note in [versatility.md](versatility.md) needs the same) |
| 12 | drn N3 | `bar1PdeBase` @1536 and FB regions @344/352 with stride 48 (drn `gsp/fw/commands.rs`) | @1504 and @320/328, stride 56 | r570 encoder (§2) |
| 13 | drn N3 | at least one region with `supportCompressed != 0` and `supportISO != 0`, else `ENODEV` (drn `gpu.rs` `Gpu::new`, `mm`) | `supportCompressed = 0` on purpose for OpenRM (`rpc.rs` `build_static_info`) | a version- or guest-dependent flag: 1 for r570/nova |
| 14 | drn N3 | nova builds BAR1 page tables under `bar1PdeBase` through PRAMIN and flushes the TLB at 0xB830A0/A4/B0 (drn `mm/bar_user.rs`, `mm/tlb.rs`) | walks BAR1 from the root it reported itself and acks invalidates (`fb.rs`, `chip.toml`) | expected to work; verify with the drn self-tests (`CONFIG_NOVA_CORE_SELFTESTS`) |
| 15 | all | nothing that names the driver: nova never sends `SET_GUEST_SYSTEM_INFO` | ABI chosen at build time (`ver::active`, feature `v610`); the only mismatch check is on `SET_GUEST_SYSTEM_INFO` (`boot.rs`) | choose at runtime (§2.4) |

The second group was checked and found compatible. Nothing is needed; keep it that way:
- BOOT_0/BOOT_42 identify TU106.
- The GFW scratch registers.
- The sysmem flush page (latched, and below 2^40 without the DMA module; Q31 does not
  arise).
- The display fuse and VGA workspace.
- The falcon reset handshake and PIO loads.
- The FWSEC descriptor pointer and FRTS offset at data + 0xA0.
- The WPR2 register encoding.
- The Booter mailbox protocol (`MAILBOX0` == 0, sentinel 0xFF at unload).
- RISC-V status at 0x111240.
- The `FALCON_OS` write.
- The libos arguments found by id8.
- `GspFwWprMeta` (identical layout).
- The queue geometry and `SWAP_RX` pointer placement, the checksum algorithm and sequence
  numbers (nova checks none).
- `GspSystemInfo` (every field nova writes is at the same offset in 928 and 952 bytes).

Two harmless warnings are expected. nova prints "GSP message ... has unprocessed data" for
`GSP_INIT_DONE` (the device sends a 1-byte payload; r570's struct is 4 bytes, and nova
reads none of it) and for the echoed unload payload.

---

## 2. ABI: r570.144 against 610.57.04

### 2.1 What nova uses

Only `gsp/fw.rs` and `gsp/fw/commands.rs` touch `bindings::` (`gsp/fw/r570_144.rs` includes
the bindgen output `r570_144/bindings.rs`). v7.2 and drn differ in one binding
(`GSP_FW_HEAP_SIZE_VGPU_DEFAULT`).

- **23 structs (drn 24):**
  - `GspFwWprMeta` and its unions;
  - `msgqTxHeader`, `msgqRxHeader`, `GSP_MSG_QUEUE_ELEMENT`, `rpc_message_header_v`;
  - `GSP_ARGUMENTS_CACHED`, `MESSAGE_QUEUE_INIT_ARGUMENTS`, `LibosMemoryRegionInitArgument`;
  - `GspSystemInfo`, `PACKED_REGISTRY_TABLE`/`_ENTRY`;
  - `GspStaticConfigInfo_t` (plus `FB_REGION_INFO` in drn);
  - `rpc_unloading_guest_driver_v1F_07`;
  - `rpc_run_cpu_sequencer_v17_00`, `GSP_SEQUENCER_BUFFER_CMD` and 5 sequencer payloads;
  - the Hopper-only `GSP_FMC_BOOT_PARAMS`, `GSP_ACR_BOOT_GSP_RM_PARAMS`, `GSP_RM_PARAMS`.
- **61 constants (62):** heap parameters, WPR meta magic/revision, 9 sequencer opcodes, DMA
  targets, the element maximum, libos kind/loc, power levels, registry DWORD, the RPC
  signature.
- **29 function/event numbers:** 20 functions, 9 events.

### 2.2 What differs

Measured with the compiler against both header trees (C `offsetof`/`sizeof`, cross-checked
by compiling nova's `bindings.rs`):

| Item | r570.144 | 610.57.04 | Matters to the device |
|---|---|---|---|
| `GSP_MSG_QUEUE_ELEMENT` | 80 bytes (header 48): authTag@0, aad@16, checkSum@32, seqNum@36, elemCount@40, rpc@48 | header 16: mctp@0, nvdm@4, checkSum@8, seqNum@12, payload@16 | yes (#3) |
| max RPC per element | 65536 − 48 = 65488 | 65520 | yes |
| `rpc_run_cpu_sequencer_v17_00`, `GSP_SEQUENCER_BUFFER_CMD`, 5 payloads, 9 opcodes | present (40 bytes; opcodes 0..8) | removed (only event number 0x1002 remains) | yes (#4) |
| `GspStaticConfigInfo_t` | 1656 bytes; gidInfo 32/36; fbRegionInfoParams 344/352; engineCaps 1204; fb_length 1224; **gpuNameString 1260**; short name 1324; **bar1PdeBase 1536**; bar2PdeBase 1544; bVbiosValid 1552; vbios sub vendor/device 1556/1560; hInternal* 1600/1604/1608 | 1600 bytes; 8/12; 320/328; 1324; 1344; **1352**; 1416; **1504**; 1512; 1520; 1524/1528; 1544/1548/1552; plus vbiosRevision/OEM 1532/1536, bBusResetRequired 1592 (absent in r570) | yes (#6, #12) |
| `NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO` | 48 bytes (blackList[17]) | 56 bytes (blackList[18]); field offsets equal | yes (stride) |
| `GSP_ARGUMENTS_CACHED` / `MESSAGE_QUEUE_INIT_ARGUMENTS` | 72 / 32 bytes | 152 / 64 bytes; nova's fields at 0/8/16/24 unchanged | no |
| `GspSystemInfo` | 928 bytes | 952; every field nova writes unchanged | no |
| `rpc_init_done_v17_00` | 4 bytes | 1 byte | no (nova ignores it) |
| `GSP_FMC_BOOT_PARAMS` | 80 | 88 (tail added) | Hopper+ only |
| outside nova: `rpc_gsp_rm_control_v03_00` | params@24 | params@40 | not for nova |

Identical in both versions:
- `GspFwWprMeta` (256 bytes);
- the queue headers, `rpc_message_header_v`, `LibosMemoryRegionInitArgument` (incl. RMARGS
  id8), the registry structs, `rpc_unloading_guest_driver_v1F_07` (8 bytes, 0/1/4);
- the ACR/RM params;
- **all 29 function/event numbers**;
- all heap constants (renamed `GSP_FW_HEAP_PARAM_SIZE_PER_GB_FB` → `_SIZE_PER_GB`, same
  value).

No constant nova uses has a different value in 610. 9 are absent there: the sequencer
opcodes.

### 2.3 The second version module, sized

`ver/mod.rs` is not a trait. It is a cfg-selected alias, `pub use v610 as active`, over
310 public names:
- The GSP path (`rpc.rs`, `msgq.rs`, `boot.rs`) uses 122 of them. About 25 need other
  values for r570:
  - `ELEMENT_HEADER_SIZE` 48, MCTP/NVDM absent, `MAX_RPC_SIZE` 65488;
  - `STATIC_INFO_SIZE` 1656, `FB_REGION_SIZE` 48;
  - 17 `SI_*` offsets.
- New names, which v610 should export too, as absent, to keep the interface uniform:
  - `FN_UNLOADING_GUEST_DRIVER` 47, `EV_GSP_RUN_CPU_SEQUENCER` 0x1002;
  - element checksum/seq/elemCount offsets (32/36/Some(40) against 8/12/None);
  - the sequencer layout (40 bytes, 0/4/40) and opcodes;
  - the unload payload (8 bytes);
  - the `INIT_DONE` size (4).

Two ways to write it:
- **Minimal `ver/v570.rs`, about 150-200 lines.** `pub use super::v610::*;` followed by
  about 45 overriding or new constants with header citations (90-120 lines), plus 60-80
  lines of tests against a `gsp-abi-570.md` produced by `gsp-abi-sizes.sh` on the r570
  tree.
  - This is valid only because nova sends no `GSP_RM_ALLOC`/`GSP_RM_CONTROL`. It would be
    wrong for a proprietary 570 guest (`rpc_gsp_rm_control_v03_00` moved; `nvrm-sys` has no
    `v570`).
- **Standalone GSP-only module, about 300 lines.**

Either way, the generic code changes by about 80-100 lines:
- `msgq.rs` element offsets (30-40);
- `boot.rs` sequencer and order (30-40);
- the unload reply (5-10);
- the version selection (about 8).

The boot-model items #7-#11 and #15 add about 150-250 lines with tests.

### 2.4 Mirror nova's versioning?

nova's arrangement:
- **Per-version bindings in a private module:** `gsp/fw/r570_144.rs` includes the bindgen
  file and is "not to be used directly".
- **One version-free wrapper layer:** `gsp/fw.rs`, `gsp/fw/commands.rs`, with `repr(transparent)`
  newtypes (`GspMsgElement`, `GspStaticConfigInfo`, `MsgFunction`) and accessors.
- **Typed commands on top:** `gsp/commands.rs`.

The task list (`Documentation/gpu/nova/core/todo.rst`, "GSP firmware abstraction") plans
the next step:
1. per-version generated structs in separate namespaces;
2. abstraction structures in a generic namespace, annotated with version differences;
3. a procedural macro generating the per-version implementations;
4. **choosing the version at runtime** behind a common trait.

`Documentation/gpu/nova/core/guidelines.rst` forbids leaking firmware version details
through the driver API. Today nova still binds exactly one version at build time.

Recommendation for Caraxes: mirror the shape, not the macro.
- **Generated bindings per version.** This is the `nvgsp-sys` sibling crate that
  [gsp-abi-610.md](gsp-abi-610.md) (e) already proposes, generated by `cargo xtask abi` for
  610 and 570.
- **One trait for the GSP transport and boot vocabulary**, `GspAbi`:
  - element framing;
  - init-RPC encoders and decoders;
  - static-info encoder;
  - sequencer;
  - unload;
  - WPR meta and libos decoding.

  It is implemented by `ver::v610` and `ver::v570` and **selected at runtime**.
- **The RM-control half stays build-selected.** It is where the phase-5 dispatcher lives,
  and nova does not use it.

Runtime selection is needed because one device serves both guest drivers, and nova never
names its version. Two ways to select:
- **By firmware version.** The WPR meta, which the device already reads at Booter-Load,
  points at the radix3-mapped GSP ELF (`sysmemAddrOfRadix3Elf`). The ELF's `.fwversion`
  section reads `570.144\0` in `gsp-570.144.bin`. The 610 image carries its own.
  - This keeps the rule that the firmware defines the ABI: the device speaks whatever
    firmware the guest handed it.
- **Fallback: by the shape of the first command element.** It is MCTP word 0xC0000001 at
  offset 0 for 610, and "VRPC" at offset 52 for r570.

This fits D14: the chip axis stays as it is, and the driver-version axis gets a runtime
dimension for the GSP part.

---

## 3. Guest recipe

### 3.1 Kernel

- **Source.** Linux **v7.2**, commit `8d3ae59288f1e7d58d76558a6ee96d533bc5019f`, built on
  the host into `target/caraxes/nova/`. The tree is outside the repository checkout
  (clone under `target/`); no patches.
- **Toolchain.**
  - v7.2 requires rustc >= 1.85 and bindgen >= 0.71.1 (`scripts/min-tool-version.sh`).
  - The host has rustc 1.89.0 (rustup; `rust-src` must be added:
    `rustup component add rust-src --toolchain 1.89.0`), bindgen 0.73.2 and clang 22.1.8.
  - `ld.lld` was not found. Either install lld and build with `LLVM=1`, or build with GCC
    (Rust with a GCC-built kernel works but is less tested).
  - Check: `make LLVM=1 rustavailable`.
- **Config.** Start with `make LLVM=1 x86_64_defconfig kvm_guest.config`, then:

  | Setting | Why |
  |---|---|
  | `CONFIG_RUST=y` | requires `!MODVERSIONS` (or `GENDWARFKSYMS`), `!RANDSTRUCT`, and no `DEBUG_INFO_BTF` unless pahole supports lang exclusion |
  | `CONFIG_NOVA_CORE=m` | selects `AUXILIARY_BUS`, `RUST_FW_LOADER_ABSTRACTIONS`; drn also `GPU_BUDDY` |
  | `CONFIG_DRM_NOVA=n` | N0-N4 need no DRM |
  | `CONFIG_DRM_NOUVEAU=n`, no NVIDIA module | nothing else may claim `10de:*` |
  | `CONFIG_DEBUG_FS=y`, `CONFIG_DYNAMIC_DEBUG=y` | nova's `dev_dbg` lines (RPC trace, WPR2, FB layout) and the `loginit`/`logintr`/`logrm` buffers under `/sys/kernel/debug/nova-core/<pci>/` |
  | built in (`=y`): `VIRTIO_PCI`, `VIRTIO_BLK`, `VIRTIO_NET`, `EXT4_FS` | boot without an initramfs |
  | `PVH=y` | Cloud Hypervisor direct boot of `vmlinux` |

  nova is a module, so `request_firmware` runs after the root filesystem is mounted, and no
  `EXTRA_FIRMWARE` is needed.

### 3.2 Firmware

The files come from linux-firmware `20260916`: `nvidia/tu102/gsp/`, reached as
`nvidia/tu106/gsp -> ../tu102/gsp` (a `Link:` line in `WHENCE`, created by
`copy-firmware.sh`; the git tree has no `tu106/gsp` entry). They are identical to this host's
`linux-firmware-nvidia 20260810-2` (zstd).

| File (requested as `nvidia/tu106/gsp/…`) | Bytes | sha256 |
|---|---:|---|
| `booter_load-570.144.bin` | 59272 | `7bb181f544a942299bbf4642da6d9bb58bfed8459725094e2ace56647cd3ec8c` |
| `booter_unload-570.144.bin` | 39304 | `5c7f1aa37d045cc20bb81584a830e8dfa4483f1cd63943de83a58c4be19a1782` |
| `bootloader-570.144.bin` (= `bootloader-535.113.01.bin`) | 4196 | `12e987b636c2f00fa40f42fd95097515c0817b158119c584049a37faf38f8f96` |
| `gen_bootloader-570.144.bin` | 816 | `b37776a511b4a00901e4e3ac568db917086d3bf439f85bc9b3e4adc7338a0aff` |
| `gsp-570.144.bin` (RISC-V ELF: `.fwimage`, `.fwversion` = "570.144", `.fwsignature_tu10x/tu11x/ga100`) | 28542040 | `3052aee2872182a14d8d7c069e3a14fe4642405894b24692c4aca4101dfb1809` |

Requested by `firmware.rs` `request_firmware` (`nvidia/{chip}/gsp/{name}-{FIRMWARE_VERSION}.bin`,
`FIRMWARE_VERSION = "570.144"`), `firmware/booter.rs`, `firmware/gsp.rs` and
`firmware/fwsec/bootloader.rs`.

linux-firmware has only 535.113.01 and 570.144 GSP images, and no 610. (drn and v7.3 would
request `booter_load.tlv`, `booter_unload.tlv`, `gen_bootloader.tlv`, `gsp_bootloader.tlv`
and `gsp.tlv` with `FILE`/`SIZE` or `BLOB` tags, per
`Documentation/gpu/nova/core/tlv.rst` in drn. None exist upstream. A local wrapper around
the `.bin` files is possible but would no longer be "stock firmware".)

### 3.3 VM

- **A new state directory `vm4`.** It keeps nova out of vm0-vm3 and matches the briefing's
  "separate guest".
  - `VM=vm4 CARAXES_KERNEL=target/caraxes/nova/vmlinux` with the same Ubuntu 24.04 base
    image as a fresh overlay.
  - The base image has no NVIDIA kernel module. Its `/lib/modules/6.8.0-136-generic` is not
    used.
  - Install the nova modules with `make modules_install INSTALL_MOD_PATH=` into the
    overlay (via `qemu-nbd` or `guestfish`, as the environment notes describe).
  - Copy the five firmware files to `/lib/firmware/nvidia/tu102/gsp/` and create the
    `tu106/gsp` symlink.
  - Blacklist `nova_core` from autoload, so the gate script decides when it binds.
- **Launcher.** `tools/caraxes-vm.sh` always passes `--initramfs`; it needs a
  `CARAXES_INITRD=none` switch (a small launcher change, outside this package).
- **Device.** The device runs as for E3, with `CARAXES_BOOT_MODEL=1` and the regenerated
  `synth-vbios.bin` (#1, #2). It runs without `CARAXES_HOST_RM`, `CARAXES_DMA`,
  `CARAXES_RESERVE_VRAM` and `CARAXES_HOST_EXEC`: nova sends no RM control, and the static
  info comes from the profile.
- **Run.** `sudo modprobe nova_core dyndbg=+p`, then:
  - unbind and bind through `/sys/bus/pci/drivers/nova-core/`;
  - `rmmod nova_core` and `modprobe nova_core` again.

  Every run archives the kernel commit, `.config`, firmware hashes, the device commit and
  version module, the profile, the last good step, the first error and the device trace
  (briefing §10).

### 3.4 What success looks like

v7.2, with the device fixed (PCI address as in the guest; the name from the profile):

```
nova-core 0000:00:04.0: NVIDIA (Chipset: TU106, Architecture: Turing, Revision: a.1)
nova-core 0000:00:04.0: GPU name: Leandro RTX 2070-4G
```

The first line is `gpu.rs` `Gpu::new` (`Spec` display), the second `gsp/boot.rs`
`Gsp::boot`. With `dyndbg`, the lines between them include:
- `GSP RPC: send: seq# 0, function=GspSetSystemInfo`;
- `... receive: ... function=GspRunCpuSequencer`, `Running CPU Sequencer commands`;
- `... function=GspInitDone`, `... function=GetGspStaticInfo`;
- `WPR2: 0x…-0x…`, `RISC-V active? true`.

Unbind prints `GSP successfully unloaded`. drn adds `Usable FB regions` / `Total usable
VRAM: … MiB` as `dev_dbg` lines.

Today's device stops much earlier, and that is the N1 baseline to record before any change.
Expected lines on unchanged hardware:
- `ROM signature unknown 0xbb77`, with `Missing required images` not reached;
- or, once #1 is fixed, `PmuLookupTableEntry not found`.

---

## 4. Gates N0-N4

| Gate | Evidence | Falsifiable expectation |
|---|---|---|
| **N0** build and firmware pinned | `nova.md` pins (this note); kernel `.config`, `make LLVM=1 rustavailable` output, `modinfo nova_core` (firmware list: `nvidia/tu106/gsp/{booter_load,booter_unload,bootloader,gsp,gen_bootloader}-570.144.bin`), `sha256sum` of the installed firmware = table §3.2, device commit | any hash or commit that differs from the table fails N0; `modinfo` listing `.tlv` means the wrong tree |
| **N1** device recognized, boot registers consistent | guest `dmesg` line `NVIDIA (Chipset: TU106, Architecture: Turing, Revision: a.1)`; device trace: BOOT_0/42, 0x118128/0x118234, 0x100C10 write, PROM reads to the end of the extension image, FWSEC PIO load, `fwsec: ... frtsOffset ... (from the FWSEC command in guest RAM)`, SEC2 Booter-Load with a mapped WPR meta PA | nova passes `Vbios::new`, `run_fwsec_frts` (WPR2 LO == `FbLayout::frts.start`, logged as `WPR2: …` under `dyndbg`) and `RISC-V active? true`. Falsified by `ROM signature unknown`, `PmuLookupTableEntry not found`, `WPR2 region created at unexpected address`, or a RISC-V poll timeout |
| **N2** GSP boot and queues with the r570 ABI | device notes `libos arg 0..3` (LOGINIT, LOGINTR, LOGRM, RMARGS), `queues linked`, `rx seq 0 GSP_SET_SYSTEM_INFO ... checksum ok`, `rx seq 1 SET_REGISTRY ...`, `tx ... GSP_RUN_CPU_SEQUENCER`, `tx ... GSP_INIT_DONE`; guest `dyndbg` receive lines in that order | nova passes `GspSequencer::run` and `wait_gsp_init_done` within 5 s each. Falsified by `bad checksum`, `ETIMEDOUT` in `post_boot`, an `EINVAL` from an unknown function, or a device `checksum BAD` on a nova command |
| **N3** static data | guest `GPU name: Leandro RTX 2070-4G`; device `static-info` labels from the r570 encoder; (drn) usable region list and a passing selftest | the printed name equals the profile's. Falsified by `EIO` (short reply) or `GPU name unavailable` (wrong offset); for drn, by `ENODEV` from the region filter |
| **N4** cleanup and repetition | unbind: `GSP successfully unloaded`, device notes `UNLOADING_GUEST_DRIVER`, FWSEC-SB, Booter-Unload, `WPR2 down`; then bind again and `rmmod`/`modprobe` again, **three cycles without a device reset**; the same N1-N3 lines each time | every cycle reaches `GPU name:` and `GSP successfully unloaded`. Falsified by `GSP shutdown failed`, `WPR2 region already exists`, `WPR2 region not created`, or a sequencer/INIT_DONE timeout on the second cycle (stale queue link, #10) |

### 4.1 Effort (agent-days)

| Work | Differences | Estimate |
|---|---|---|
| Synthetic VBIOS: signature 0x4E56, app id 0x85; OpenRM regression (E1-E4 probe on vm0 still `RmInitAdapter succeeded`) | #1, #2 | 0.25-0.5 |
| r570 measurements (`gsp-abi-570.md` from `gsp-abi-sizes.sh`, two extra include flags) and `ver/v570.rs` | #3, #6, #12 | 0.75-1 |
| Generic changes: element framing in `msgq.rs`, sequencer and order, unload reply and mailbox | #3, #4, #5, #7 | 0.75-1 |
| Boot-model state machine: GSP/SEC2 start classification, re-link on a new boot, queue service at Booter-Load | #8, #9, #10, #11 | 1-1.5 |
| Runtime ABI selection (`GspAbi` trait, `.fwversion` via radix3, fallback sniff), unit tests for both versions | #15 | 0.75-1 |
| **Device side** | | **3.5-5** |
| Kernel v7.2 build with Rust (toolchain, config, `rustavailable`), `vm4` overlay, modules and firmware install, launcher `CARAXES_INITRD=none` | N0 | 0.75-1 |
| Gate script `nova` in `tools/caraxes-gate.sh` (N1-N4 greps, three-cycle loop, archive), first runs and fixes | N1-N4 | 0.75-1.5 |
| **Guest side** | | **1.5-2.5** |
| drm-rust-next later: TLV firmware (upstream or local wrapper), FB-region flag, BAR1/PRAMIN verification with selftests | #13, #14 | +1-2, not in the total |

Order:
1. **Guest N0**, then an **N1 baseline** on the unchanged device (records the VBIOS
   failure).
2. **VBIOS fix** → N1.
3. **Framing + sequencer + static info** → N2/N3.
4. **Unload + state machine** → N4.
5. **Runtime selection** last. Until then, a `v570` build of the device serves the nova
   VM only.

### 4.2 Top risks

1. **ABI drift in nova itself.**
   - Upstream moved from versioned `.bin` to `.tlv` between v7.2 and v7.3-rc4.
   - drn reads more static info (regions, BAR1) and runs the init RPCs before the boot.
   - nova's todo list announces runtime multi-version support, and a later firmware version
     (not 570.144) is likely.
   - Mitigation: pin v7.2 for N0-N4; keep the r570 module small and generated; re-derive
     §1.6-§1.7 from `gsp/fw.rs` and `gsp/commands.rs` at every re-pin.
2. **The synthetic VBIOS.**
   - nova parses it more strictly than OpenRM, in other places (signatures, app id, NPDE
     last bit, BIT token layout).
   - A change for nova must not break OpenRM's walk.
   - Mitigation: one image for both, verified by `synth-vbios.py`'s OpenRM oracle *and* a
     nova-side check (a Python mirror of `vbios.rs` `Vbios::new`, about 100 lines, in the
     same tool).
3. **Boot model differences.**
   - nova starts the GSP falcon an extra time.
   - It expects a CPU sequencer that 610 no longer has.
   - It re-probes without a device reset and needs the suspend mailbox bit.
   - A counter-based model will keep breaking. A state machine keyed on what the CPU loaded
     (DMEM mapper command, WPR2 state, GSP running/suspended) serves both drivers.
   - Real r570 firmware sends a non-empty sequence with `CORE_RESUME`. An empty one is
     sufficient for nova but is not what hardware does. If nova ever validates the sequence
     (or needs `NV_PGC6_BSI_SECURE_SCRATCH_14`), the device must model `CORE_RESUME`: SEC2
     start without Booter semantics, BSI scratch 14 bit 26, RISC-V staying active.
4. **Our FB layout.**
   - nova computes its own WPR2/heap layout (LIBOS2 64-256 MiB heap, 1 MiB non-WPR heap,
     PMU reserved 0), which differs from OpenRM's.
   - The device derives its regions and BAR roots from the WPR meta, so the layout itself
     is fine.
   - It must also stay above the host VRAM reservation when `CARAXES_RESERVE_VRAM` is on:
     the `fb layout: ... above the reservation` note checks this.
   - For drn, the region flags (`supportCompressed`) are a per-driver policy, and BAR1
     page tables written by nova into the fake FB are the first guest-built BAR1 tables the
     walker sees ([exec-formats.md](exec-formats.md) lists 20 walker divergences to keep in
     mind).
5. **Toolchain and guest plumbing.** No `ld.lld` on the host; the Rust-in-kernel config
   constraints (MODVERSIONS, BTF); a launcher that always passes an initramfs. None of these
   is hard, and each costs a run if missed.

## Not verified

- Nothing here ran. Every "compatible" point is from reading both sides of the code.
- The BOOT_42 value on real hardware (R2) is still a prediction; nova's decoding of
  0x166A1000 as TU106 a.1 is computed, not observed.
- Whether the guest kernel issues a function reset anyway on nova unbind/bind (it should
  not) is to be seen in the device trace of N4.
- Whether real r570 firmware on Turing sends a sequence containing `CORE_RESUME`: inferred
  from nova's comments (`gsp/sequencer.rs`), not captured.
