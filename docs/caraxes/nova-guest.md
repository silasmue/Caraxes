<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# nova-core as a guest driver of the synthetic TU106

The operator's direction (2026-09-25): let nova-core run in a 1 GiB guest against the device,
pure emulation first. P23 names nova-core as the next client of the emulated RM core. The
study of 2026-09-23 ([nova.md](nova.md)) read the sources only; this note is the build, the
device changes and the boot. Evidence: `caraxes/runs/nova-2026-09-25/`.

Paths `gsp/...`, `falcon/...` are `drivers/gpu/nova-core/` of Linux 7.2.7 unless marked.

## 1. Which kernel

- **Linux 7.2.7** (kernel.org stable, 2026-09-21, tarball sha256 `4ac34c47...145a`).
  - Its nova-core requests `nvidia/<chip>/gsp/*-570.144.bin` (`firmware.rs:36`
    `FIRMWARE_VERSION = "570.144"`).
  - v7.3-rc4 and drm-rust-next moved to unversioned `.tlv` files that linux-firmware does not
    ship ([nova.md](nova.md) "Summary"), so 7.2.y is the newest kernel that boots stock
    firmware.
- **Firmware**: the host's linux-firmware, `nvidia/tu102/gsp/` (tu106/gsp is a link to it):
  `booter_load`, `booter_unload`, `bootloader` (= the 535.113.01 file),
  `gen_bootloader`, `gsp`, all `-570.144.bin.zst`. The guest kernel has
  `FW_LOADER_COMPRESS_ZSTD`, so the packaged `.zst` files are copied as they are.
- **What 7.2.7's nova-core does on Turing** (probe to completion, then idle):
  - VBIOS parse through the PROM window, FWSEC-FRTS through the generic bootloader on the GSP
    falcon, a bare GSP start with the libos arguments, Booter-Load on SEC2
    (`gsp/hal/tu102.rs:268-333`).
  - GSP-RM boot: `SET_SYSTEM_INFO` and `SET_REGISTRY` without reply, wait for
    `GSP_RUN_CPU_SEQUENCER` and run it, wait for `GSP_INIT_DONE`, `GET_GSP_STATIC_INFO`, print
    the name (`gsp/boot.rs:104-164`).
  - Unbind: `UNLOADING_GUEST_DRIVER`, poll GSP `MAILBOX0` bit 31, FWSEC-SB, Booter-Unload
    (`gsp/boot.rs:167-225`, `gsp/hal/tu102.rs:128-178`).
  - No RM alloc, no control, no channel, no interrupt (every wait polls), DMA mask 47 bit
    (`gpu/hal/tu102.rs:88`).
- **DRM side** (`drivers/gpu/drm/nova`, module `nova-drm`): an auxiliary driver on nova-core's
  `nova-drm` device. It registers a render-only DRM device with three ioctls: `GETPARAM`
  (only `VRAM_BAR_SIZE`, the BAR1 length), `GEM_CREATE`, `GEM_INFO` (`drm/nova/driver.rs:81-83`,
  `file.rs:25-60`). No KMS: modetest has no connector to show, whatever the device does.

## 2. What nova sends and reads

### 2.1 RPCs (all numbers equal in 570.144, 610.57.04 and 615.71.09)

| Direction | Function | Payload | Reply | nova |
|---|---|---|---|---|
| CPU -> GSP | `GSP_SET_SYSTEM_INFO` 72 | `GspSystemInfo` 928 B (610: 952; nova's fields at equal offsets) | none | `gsp/boot.rs:146-147` |
| CPU -> GSP | `SET_REGISTRY` 73 | 3 DWORDs: `RMSecBusResetEnable`, `RMForcePcieConfigSave`, `RMDevidCheckIgnore` = 1 | none | `gsp/commands.rs:73-100` |
| GSP -> CPU | event `GSP_RUN_CPU_SEQUENCER` 0x1002 | `rpc_run_cpu_sequencer_v17_00`, 40 B + commands | - | `gsp/sequencer.rs:358-367` |
| GSP -> CPU | event `GSP_INIT_DONE` 0x1001 | 4 B, ignored | - | `gsp/commands.rs:177-185` |
| CPU -> GSP | `GET_GSP_STATIC_INFO` 65 | zeroed `GspStaticConfigInfo` | same function, >= 1656 B, name at 1260 | `gsp/boot.rs:157-161` |
| CPU -> GSP | `UNLOADING_GUEST_DRIVER` 47 | 8 B (`newLevel` 0) | same function, payload ignored | `gsp/boot.rs:174` |

- A reply is matched by function only; `rpc_result` and sequence numbers are never checked.
- A message of another known function is consumed and dropped; an unknown function code is
  `EINVAL` without advancing the read pointer (`gsp/cmdq.rs:813-848`). The device therefore
  sends the sequencer before `GSP_INIT_DONE` and nothing else unsolicited.
- Every element is checksummed; a bad checksum is `EIO` (`gsp/cmdq.rs:776-790`).

### 2.2 BAR0 registers on the Turing path, and whether the device serves them

| Register | nova | Device |
|---|---|---|
| `NV_PMC_BOOT_0` 0x0, `BOOT_42` 0xA00 | chip id (`gpu.rs`) | `chips/tu106/chip.toml` |
| GFW: 0x118128 PLM, 0x118234 `GFW_BOOT` | `gpu/hal/tu102.rs:58` | `boot.toml` consts |
| `NV_PFB_NISO_FLUSH_SYSMEM_ADDR` 0x100C10 | written, read back (`fb/hal/tu102.rs:29`) | latched (`regmap.toml:32`) |
| `LOCAL_MEMORY_RANGE` 0x100CE0 | FB size (`fb/hal/tu102.rs`) | encoded from the profile |
| `NV_FUSE_STATUS_OPT_DISPLAY` 0x21C04 | display off, so 0x625F04 is not read | const 1 |
| PROM 0x300000.. | VBIOS (`vbios.rs`) | `synth-vbios-nova.bin` (section 3.1) |
| `WPR2_ADDR_LO/HI` 0x1FA824/828 | 0 before FWSEC, FRTS after (`gsp/hal/tu102.rs:192-263`) | FWSEC model |
| `NV_PBUS_SW_SCRATCH(0xE)` 0x1438 | FRTS error 31:16 == 0 | reads 0 |
| GSP falcon 0x110000 + `ENGINE`, `DMACTL`, `RM`, `IMEMC/D/T`, `DMEMC/D`, `BOOTVEC`, `FBIF_*`, `CPUCTL(_ALIAS)`, `MAILBOX0/1`, `IRQSCLR`, `OS` | reset, PIO load, start, halt poll (`falcon.rs:388-399`, `falcon/hal/tu102.rs`) | `falcon.rs` model |
| `NV_PRISCV_RISCV_CORE_SWITCH_RISCV_STATUS` 0x111240 | RISC-V active | set at Booter-Load |
| SEC2 falcon 0x840000 (same set) | Booter load/unload | `chip/turing.rs` |
| `NV_PGSP_QUEUE_HEAD` 0x110C00 | doorbell after every command | `CmdQueueDoorbell` |
| `NV_PGC6_BSI_SECURE_SCRATCH_14` 0x1180F8, `HWCFG2` | only in the sequencer's `CORE_RESUME` and on GA102+/GH100 | not read with an empty sequence |

- Not read on Turing: `HWCFG1/2`, the FPF fuses, `NV_THERM_I2CS_SCRATCH`, the FSP queues.
- No register nova reads on the Turing path is unserved; nothing was added to the chip
  tables.

## 3. The delta: 570.144 against the 615 baseline

The snapshot `caraxes/abi/gsp-570.144/` (`gsp-abi.py snapshot`, with a 570 field-list variant
in `gsp-abi-sizes.{c,sh}`) and the report `caraxes/abi/delta-615.71.09-570.144.md`
(`gsp-abi.py diff`).

- Whole tree: 1146 driver-side changes in 42 groups, 78 of them in what the device implements;
  routed controls 844 -> 795, RPC functions 232 -> 227, events 45 -> 34, classes 224 -> 186.
- The control and alloc changes matter only for a 570 OpenRM guest; nova sends none of them.

What nova touches:

| Struct | 570.144 | 610.57.04 | 615.71.09 | Device for nova |
|---|---|---|---|---|
| `GSP_MSG_QUEUE_ELEMENT` header | 0x30: authTag, aad, checkSum@32, seqNum@36, elemCount@40, rpc@48 | 0x10: MCTP, NVDM, checkSum@8, seqNum@12 | 0x18 (v2: no checksum, no seqNum) | the pre-610 layout of the 595 work (`ver::ElementLayout::PRE_610`) |
| message queue | v0 (`msgqTxHeader`/`RxHeader` 32/4, pointers in memory) | v0 | v2 (pointers in registers) | the 610 transport |
| `rpc_message_header_v` | 32 | 32 | 32 | same |
| `GspStaticConfigInfo` | 1656; fbRegion 344/352 (48 B each), name 1260, bar1PdeBase 1536 | 1600; 320/328 (56 B), 1352, 1504 | 1608 (wire: NVKV) | remapped (section 3.2) |
| `GspSystemInfo` | 928 | 952 | 952 (NVKV) | read at 610 offsets; nova's fields agree |
| `GSP_ARGUMENTS_CACHED` / `MESSAGE_QUEUE_INIT_ARGUMENTS` | 72 / 32 | 152 / 64 | 184 / 64 | only the four queue fields at 0/8/16/24 are read: equal in 570 and 610 |
| `rpc_init_done_v17_00` | 4 | 1 | 1 | 4 bytes for 570 |
| `rpc_run_cpu_sequencer_v17_00` | 40 | removed | removed | empty sequence for 570 |
| `GspFwWprMeta`, `LibosMemoryRegionInitArgument`, registry table, `rpc_unloading_guest_driver_v1F_07` | 256, 32, 8+16, 8 | equal | equal | unchanged |

- Transport: nova speaks the 610 transport (v0 queue, checksum, sequence numbers), not 615's
  v2 with GMCAPI. The 610 build serves it; the 615 build would need the v0 transport back.
- Function and event numbers: all 20 functions and 9 events of nova's `MsgFunction`
  (`gsp/fw.rs:270-303`) have the same values in all three versions.

### 3.1 What the device lacked, and what was added (commit c3cf7592)

- **Guest ABI 570** in the 610 build: `CARAXES_GUEST_ABI=570` (`src/ver/mod.rs`
  `older_guest`, `check_guest_abi`), module `src/ver/v570.rs`.
  - elements: the pre-610 layout, as for 595;
  - link events: an empty `GSP_RUN_CPU_SEQUENCER` (40 B, `cmdIndex` 0), then `GSP_INIT_DONE`
    (4 B) (`ver::link_events`);
  - static info: the 610 struct the device builds, copied field by field into the 570 layout
    by `src/ver/v570/static_info.rs`, generated by the new `caraxes/tools/gsp-remap.py` from
    the two snapshots' `layouts.tsv` (28 runs, per-element for the 48- and 56-byte FB regions;
    10 fields 570 lacks dropped, 10 fields 610 lacks left zero);
  - no OpenRM registry warnings for nova's three keys.
- **Boot model** (`chip/turing.rs`, `boot.rs`):
  - FWSEC-FRTS is recognized at every GSP start that finds WPR2 down and GSP-RM unloaded, not
    only at the first start: nova's re-probe runs it as the GSP's 4th start.
  - A Booter-Load that is not a resume drops the old v0 queue link, so a re-probe links the
    fresh queues nova built.
  - `UNLOADING_GUEST_DRIVER` (NV_OK plus `MAILBOX0` bit 31) was already served
    (`boot.rs`, `crate::gsprpc::FN_UNLOADING_GUEST_DRIVER`).
- **VBIOS**: `caraxes/tools/synth-vbios.py --ext-sig 0x4E56 --appid 0x85` writes
  `synth-vbios-nova.bin`. nova accepts only ROM signatures 0xAA55 and 0x4E56
  (`vbios.rs:548-553`) and only the FWSEC application id 0x85 (`vbios.rs:905`); the default
  image keeps 0xBB77/0x05 so the stock guests see the same bytes as before.
- **Tests**: `boot::tests::nova_570_boot_through_the_queue` (nova's elements through the
  queue: no reply to 72/73, sequencer then INIT_DONE, the 1656-byte reply with the name at
  1260, unload bit, re-probe relink), `ver::v570::tests`, `chip::turing::tests::nova_reprobe_runs_fwsec_frts_again`.

## 4. The guest

- Kernel: `caraxes/runs/nova-2026-09-25/build-kernel.sh` (x86_64 defconfig + kvm_guest, Rust,
  `NOVA_CORE=m`, `DRM_NOVA=m`, root disk and ext4 built in, no initramfs; clang, rustc 1.89 of
  rustup's stable toolchain with rust-src, bindgen 0.73). `kernel.config` next to it. Built
  with `-j4` under `nice -n 10`; source, build tree and modules live on the NAS
  (`/mnt/vmstore/caraxes/nova-kernel/{src,build,modules}`, linked as `target/nova-src`,
  `target/nova-build`, `target/nova-modules`), because the root disk was full.
- VM vm9: `prepare-vm9.sh` (fresh overlay on the 610 base image, modules, firmware,
  `nova_core` blacklisted from autoload, `nova-check.sh` as the check service), booted by
  `nova-boot.sh` with `CARAXES_INITRD=none` (new launcher switch), 1 GiB, 2 vCPUs, no tap.

## 5. Boot log

Run `caraxes/runs/nova-2026-09-25/vm9-boot1/` (2026-09-25 14:51, device and kernel of this
branch, `setup.txt`): vm9 alone, 2 GiB guest RAM, 2 vCPUs, 1 GiB profile on a host VRAM
reservation at slot 4 (`CARAXES_RESERVE_VRAM=1 CARAXES_RESERVE_SLOT=4`, no host RM, no host
execution, no BAR1 pool), `CARAXES_GUEST_ABI=570`, no tap (see below).

- **nova-core ran to completion, five times**: first probe, three unbind/bind cycles, a module
  reload. Each probe printed `NVIDIA (Chipset: TU106, Architecture: Turing, Revision: a.1)`
  and `GPU name: Leandro VFIO RTX 2070-1G`; each unbind printed `GSP successfully unloaded`.
  `modprobe`/`bind`/`unbind`/`rmmod` all returned 0 (`guest-nova.txt`).
- **nova-drm** bound on every probe: `/dev/dri/card0` (render-only, driver `nova-drm`, no
  connector; modetest is not in the image).
- **Device side** (`device-boot.txt`): per probe FWSEC-FRTS on the GSP falcon (WPR2 up),
  Booter-Load on SEC2, the queues linked, `GSP_RUN_CPU_SEQUENCER` (40 B, empty) and
  `GSP_INIT_DONE` (4 B) sent, then `GSP_SET_SYSTEM_INFO`, `SET_REGISTRY` (3 entries),
  `GET_GSP_STATIC_INFO` (1656-byte 570 reply) received, all checksums good; per unbind
  `UNLOADING_GUEST_DRIVER` (MAILBOX0 bit 31), Booter-Unload (WPR2 down). 19 RPCs in total,
  none refused or unanswered; the re-probes relinked the fresh queues.
- **BAR0**: 25 distinct registers read (`bar0-reads.txt`), all in the table of section 2.2;
  none unserved. 164485 PROM reads (VBIOS parse, five times).
- **Host**: no NVRM kernel line, no Xid; the reservation was taken and freed in 0 ms each, no
  FB access outside it.
- **nova's two warnings**: `GSP message GspInitDone has unprocessed data` and
  `... UnloadingGuestDriver has unprocessed data`. nova reads both messages as empty
  (`gsp/commands.rs:163-174, 275-286`) and warns when a payload is left
  (`gsp/cmdq.rs:830-835`); the device sends the 570 payloads (4 B `rpc_init_done_v17_00`,
  the 8 B command echoed). Harmless.
- **No tap**: `caraxes9` exists with 192.168.109.1/24, but its link could not be set up from
  the agent's shell, and Cloud Hypervisor may not enable a down tap (`TapEnable`, EPERM). The
  check needs no network.

## 6. What a full nova client needs

- Today's nova (7.2.y) stops after the static info. drm-rust-next adds a VRAM buddy allocator
  over the usable FB regions (needs `supportCompressed != 0`, [nova.md](nova.md) item 13), BAR1
  page tables under `bar1PdeBase` written through PRAMIN, and TLB flushes.
- A later nova that allocates channels will send `GSP_RM_ALLOC`/`GSP_RM_CONTROL`: then the
  570 layouts of every control it uses matter (`rpc_gsp_rm_control_v03_00` params at 24 in 570
  against 40 in 610), and v570 needs the generated control tables like v610/v615
  (`gsp-tables.py`, `ctrl-layouts.py` over the 570 tree).
- nova's todo list plans runtime firmware-version selection; the device would then select its
  guest ABI from the firmware the guest hands over (`.fwversion` of the GSP ELF), not from
  `CARAXES_GUEST_ABI`.

## 7. Windows guest (the operator's next idea)

- **Image**: a Windows 11 guest under Cloud Hypervisor boots with its UEFI firmware
  (`hypervisor-fw` or OVMF `CLOUDHV.fd`); disk and network are virtio-blk and virtio-net, which
  need the virtio-win drivers slipstreamed into the install image.
- **Driver version**: NVIDIA's Windows driver branches follow the Linux ones; the branch whose
  GSP firmware matches the device's ABI is R615 (a Windows 615.xx release). A 610 branch
  Windows driver would match the 610 build.
- **The open point (P23)**: on Windows, GeForce Turing-to-Ada drivers run the RM on the CPU by
  default and use GSP-RM only where it is mandatory (Blackwell) or enabled for data-center
  parts. A Windows guest on the TU106 identity may therefore skip GSP and drive the registers
  itself, which the device does not model. The first check is which path the Windows driver
  takes for 10de:1f02 (its registry switch for GPU firmware); the GB206 identity is the likely
  way to force the GSP path.
- **The device**: the same process and BARs; the vfio-user device is presented to Windows as
  a PCI function like to Linux. Interrupts (MSI) and the display path matter more than for
  Linux, because the Windows driver sets a mode at boot.
