# Windows 11 guest (2026-09-25)

Goal: a Windows 11 guest under Cloud Hypervisor, first without the GPU, then with the
vfio-user device (`nova-guest.md` section 7, P23: which RM path the Windows driver takes).

## Why two hypervisors

- Cloud Hypervisor has no VGA and no GOP, so the Windows installer cannot be driven under it.
- The install runs once under QEMU (OVMF with Secure Boot, swtpm TPM 2.0, virtio disk and
  network, `install-qemu.sh`). The operator clicks through the setup and loads the virtio
  storage driver from the virtio-win ISO.
- The installed disk then boots under Cloud Hypervisor (`up-ch.sh`): EDK2 `CLOUDHV.fd`
  (built with `SECURE_BOOT_ENABLE` and `TPM2_ENABLE`), `--tpm socket=` to the same swtpm
  state, virtio-blk and virtio-net. Headless: serial log, RDP or Sunshine over the tap.

## Files

| Path | What |
|---|---|
| `/mnt/vmstore/caraxes/win11/` | state: `win11.qcow2` (80 GiB, thin), `swtpm/` (TPM state, shared by both), `OVMF_VARS.4m.fd` (QEMU vars), `CLOUDHV.fd` |
| `/mnt/vmstore/caraxes/win11/iso/` | `virtio-win.iso` (public download), the Windows ISO stays in `~/Downloads` |
| `target/win11/edk2/` | the EDK2 tree and build (`edk2.log`, `build.log`) |
| `install-qemu.sh` | the one-time install under QEMU (window on the operator's desktop) |
| `up-ch.sh`, `down-ch.sh` | the Cloud Hypervisor run (tap `caraxes4`, 192.168.104.0/24) |

## Setup notes

- Windows 11 checks TPM 2.0 and a Secure-Boot-capable UEFI at setup. QEMU with
  `OVMF_CODE.secboot.4m.fd` and swtpm satisfies both; the `LabConfig` registry bypass
  (`Shift+F10`, `reg add HKLM\SYSTEM\Setup\LabConfig /v BypassTPMCheck ...`) is the fallback.
- During setup, "Load driver" from the virtio-win CD: `viostor\w11\amd64` (virtio-blk), later
  `NetKVM\w11\amd64`, `Balloon`, `vioserial`, `viorng` from the same ISO.
- OOBE without a Microsoft account: `Shift+F10`, `OOBE\BYPASSNRO` (25H2 may need
  `start ms-cxh:localonly`).
- Under Cloud Hypervisor the disk must be virtio-blk and the firmware `CLOUDHV.fd`; OVMF does
  not run there. The TPM state directory is the same, so BitLocker or TPM-bound state survives
  the move. Windows may ask for re-activation after the hardware change.
- The GPU comes later: `--user-device socket=...` as in `tools/caraxes-vm.sh`, first in pure
  emulation to see which driver path Windows takes.

## Firmware build (done 2026-09-25 14:41)

- edk2-stable202508, `OvmfPkg/CloudHv/CloudHvX64.dsc`, RELEASE, GCC5, `SECURE_BOOT_ENABLE=TRUE`,
  `TPM2_ENABLE=TRUE`, four jobs, two minutes; result `/mnt/vmstore/caraxes/win11/CLOUDHV.fd`
  (4.2 MiB).
- BaseTools do not build with GCC 16 as shipped (`-Werror` on an unused variable in
  `StringFuncs.c`): the three `-Werror` in `BaseTools/Source/C/Makefiles/header.makefile` were
  removed in the build tree before `make -C BaseTools`.
- Host packages added for this: `swtpm`, `nasm`, `acpica`.

## The Windows driver's first contact

- **Driver**: 617.14 binds to the device (pure emulation, `gpu-up.sh`) as "NVIDIA GeForce RTX
  2070", reads the VBIOS through the PROM window, reads a handful of BAR0 registers the model
  does not serve (0), and stops with Code 43. No GSP boot is attempted; the driver never
  touches the falcons.
- **Read sequence** (one attempt, trace of 2026-09-25; names from the open headers where they
  exist, otherwise the block):
  - `NV_PMC_BOOT_42`, `NV_PMC_BOOT_1`, `NV_PMC_BOOT_0`, again 42 and 1;
  - `0x22410` (NV_PTOP), `0x1070` (NV_PBUS), six `NV_PCFG` mirror reads, `0x8` (NV_PMC),
    `NV_PCFG` 0x88700 read and written 0x20000;
  - `NV_FUSE_STATUS_OPT_DISPLAY` (0x21c04) twice, `0x610018` (NV_PDISP), `0x1078` (NV_PBUS);
  - a loop of `0x1373a0` (NV_PTRIM block) with `NV_PMC_BOOT_0`, `0x21434` and `0x21200`
    (NV_FUSE); per attempt 0x1373a0 is read 10 times, 0x21434 7 times;
  - `0xc80` written to `0xd96c` + n * 0x50, n = 0..6 (NV_PMGR, likely the AUX/I2C ports);
  - `0x22408` (NV_PTOP) four times, `0x1373a0`, `0x21640` (NV_FUSE); then Code 43 and the
    same sequence on the next start.
- **Capture** (`caraxes/tools/hostreg-read.c`, operator-approved, once, read-only): the host
  RM's `NV2080_CTRL_CMD_GPU_EXEC_REG_OPS` with READ_32 ops on the host RTX 2070. The RM
  allowed an unprivileged client 5 offsets (0x10c0 and 0x22430-0x2243c, its user register
  access map); the other 741 came back `INVALID_OFFSET` and were read as root (`sudo -n`).
  Result: `caraxes/reference/rtx2070/bar0-registers-2026-09-25.txt`, 737 registers (the ten,
  NV_PMC 0x0-0x100, NV_PBUS 0x1000-0x1100, NV_FUSE 0x21000-0x21800, NV_PTOP 0x22400-0x22440
  and its device-info table 0x22700-0x22800, NV_PDISP 0x610000-0x610040).
- **The ten on the card**: 0x8, 0x1070, 0x1078, 0x21200, 0x22408 and 0x22410 are 0 there
  too; the driver saw a difference only at `0x21434` = 1, `0x21640` = 1, `0x1373a0` = 0xa and
  `0x610018` = 0x04000000. `NV_PMC_BOOT_0` in the capture is 0x166000a1, the model's.
- **The table**: `vfio-user-nvgpu` loads the newest `bar0-registers-*.txt` beside the
  board's config capture (or `--bar0-registers <file|none>`) and answers reads that no model
  owns and nothing latched from it, traced `captured`; it never overrides a model. A missing
  or malformed file, or a foreign `NV_PMC_BOOT_0`, leaves no table (those reads stay 0).
  Frozen build: `/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-winregs-fd603118`.
- **Next**: after the ten, which registers the driver reads next (the first unnamed,
  un-`captured` offsets in the trace), and whether it reaches the GSP boot (`NV_PGSP_*`,
  0x110000) or programs the falcons and engines itself (the CPU-RM path of P23).

## Day one under QEMU with the device (2026-09-25, 15:50 to 17:35)

- **Option ROM off** (`rombar=0`): with it on, the firmware runs the VBIOS GOP driver, which
  spins on `NV_PTIMER_TIME` (0 in pure emulation) and hangs the boot. Device fix pending: serve
  PTIMER from the monotonic clock.
- **First boot with the device crashed** (`UNEXPECTED_STORE_EXCEPTION`), the next boots did not;
  memfd RAM alone and the plain disk boot fine. Not reproduced since; watch for it.
- **The GeForce driver 617.14 binds** ("NVIDIA GeForce RTX 2070", `VEN_10DE&DEV_1F02&SUBSYS_40111458`),
  reads the whole VBIOS chain through the PROM window to its end, identifies the chip (fuses,
  PTOP, PFB, one GSP falcon register), writes seven `0xc80` words at `0xd96c + n·0x50` and the
  config mirror, reads the identification again and stops with **Code 43**. It never touches the
  GSP queues, the falcons or the FIFO. No `nvlddmkm` event is logged.
- **Three capture rounds** from the host card (read-only through the RM as root): the ten
  identification registers, the display blocks, the GSP falcon block, PMC, PBUS, PTIMER (7941
  offsets, `bar0-registers-2026-09-25.txt`). With every value the driver read being the real
  one, the sequence and the trace length did not change (16920 lines): the gate is not a BAR0 value.
- **Not the display path**: as a 3D controller (class 03 02) the sequence is identical.
- **Not the GeForce driver**: as a Tesla T4 costume (`boards/tesla-t4-on-tu106.toml`, chip
  TU104, data-centre driver 616.92 recognises the T4) the sequence is identical, also with the
  captured values (`bar0-registers-t4-costume.txt`).
- **Open hypotheses**, in the order to test: (1) the missing expansion ROM BAR (`rombar=0`):
  serve a ROM without the EFI image so the BAR can stay on without the GOP hang; (2) a
  hypervisor or device-authenticity check that passes on passthrough hardware and fails here
  (VBIOS chain end, ID fuses against the VBIOS, the `0xd96c` AUX writes' read-back); (3) for
  WDDM later, a display model that answers the PDISP/SOR registers itself.
- **Also to try**: the vGPU guest driver with a vGPU profile's ids (the device already speaks
  the `NV_VGPU_MSG` dialect; licensing throttles unlicensed guests).
- RDP is forwarded to `127.0.0.1:3389` by the QEMU scripts.
