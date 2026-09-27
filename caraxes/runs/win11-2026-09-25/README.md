# Windows 11 guest (2026-09-25)

## Result of the night (2026-09-26, 03:34): the GeForce 617.14 driver starts on the synthetic RTX 2070

- **Status**: Device Manager "NVIDIA GeForce RTX 2070" OK (CM_PROB_NONE), Win32_VideoController ConfigManagerErrorCode 0,
  AdapterRAM 1 GiB, no DxgKrnl start failure (run Y31, `night-2026-09-25/WINDOWS-LOOP.md`).
- **nvidia-smi -q in the guest**: KMD Version 617.14, Product Name "Leandro VFIO RTX 2070-1G", GeForce, Turing, Driver
  Model WDDM, **GSP Firmware Version 615.71.09**, VBIOS 90.06.3D.00.76, FB 1024 MiB, BAR1 256 MiB
  (`night-2026-09-25/Y31-headless-ackall-nvidia-smi-full.txt`).
- **Configuration**: pure emulation (no host RM), headless (`WIN_GPU_DISPLAY=0`: display fuse 1, no display engine), the
  registry keys `EnableGpuFirmware` = `RMEnableGpuFirmware` = 1 (GSP-RM mode, the switch), the full PROM window
  (`WIN_GPU_VBIOS`), legacy ROM with the ROM BAR, branch win-loop-2026-09-26 build 7dd91e5e with `CARAXES_EMU_GR=1
  CARAXES_RUSD=1 CARAXES_WIN_COMPBIT=1 CARAXES_WIN_DISPLAY_QUERIES=1 CARAXES_WIN_ACKS=1 CARAXES_PREFER_RECORDED=1
  CARAXES_LPWR_2806_OK=1` and the diagnostic `CARAXES_ACK_UNANSWERED=1` (bisect in WINDOWS-LOOP.md).
- **Still off**: the display engine (with it the start fails right after the 21 display channels are allocated, since
  run Y9), host execution (a GPA alias for a guest without caraxes_dma_range and a cap_sys_admin binary copy).


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
| `gpu-up.sh`, `gpu-down.sh` | the synthetic GPU in pure emulation on `$W/gpu.sock` (`WIN_GPU_SERVER`, `WIN_GPU_ROM`, `WIN_GPU_BOARD`, `WIN_GPU_REGS`, `WIN_GPU_CONFIG`) |
| `run-qemu-gpu.sh` | the installed Windows under QEMU with the device attached (`WIN_GPU=0` without it, `WIN_GPU_ROMBAR=1` keeps the expansion ROM BAR, `WIN_VGA=none` makes the card the only adapter) |
| `config-space-3d.txt`, `bar0-registers-t4-costume.txt` | the card as a 3D controller (class 03 02); the captured registers with the Tesla T4 costume's identity |
| `make-rom-legacy.py` | cuts the EFI GOP image out of the ROM: `$W/rom-legacy-only.bin` for the ROM-BAR experiment |

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

## Next: the expansion ROM BAR (prepared 2026-09-25 17:45)

- PTIMER is modelled since 365dff26 (`NV_PTIMER_TIME_0/1` from the device clock); `gpu-up.sh`
  defaults to `vfio-user-nvgpu-v615-ptimer-365dff26`.
- Step 1, the BAR on without a GOP: `WIN_GPU_ROM=/mnt/vmstore/caraxes/win11/rom-legacy-only.bin ./gpu-up.sh`,
  then `WIN_GPU_ROMBAR=1 ./run-qemu-gpu.sh`. The firmware ignores a legacy-only ROM, Windows
  sees a populated ROM BAR. Watch: does the driver's sequence change after the VBIOS read, or
  the Code 43 disappear?
- Step 2, the full ROM with the BAR on: `./gpu-up.sh` (capture ROM) and `WIN_GPU_ROMBAR=1
  ./run-qemu-gpu.sh`. The GOP now finds a running PTIMER; if the firmware still hangs, the GOP
  needs more than the timer (note where the trace stops: the last BAR0 offsets before silence).
- Both keep `rombar=0` as the default; the scripts are unchanged otherwise.

### Results (2026-09-25, 18:49 to 19:05)

- **Step 1, legacy-only ROM, ROM BAR on** (RTX 2070 identity, driver 617.14 via Windows Update):
  Code 43 again after a short time. The trace is the same as before: VBIOS read, identification,
  PCIe device-control write (No Snoop cleared), the seven `0xc80` AUX writes, identification
  again, stop. Windows never read the ROM BAR (one read, by the firmware). Hypothesis (1) is out.
- **Step 2, full ROM with the GOP, PTIMER model, no VGA** (`WIN_VGA=none`): the GOP runs, does
  DP AUX transactions (`0xd954`, `0xda94`) and then polls the display front-end register
  `0x610800` (reads `0x80010341`, never changes) 1.5 million times a minute; the firmware never
  starts Windows. PTIMER alone is not enough: the display model must answer that register
  (`gop-trace-2026-09-25.txt` has the GOP's sequence and the register counts).
- **Step 3, legacy-only ROM, ROM BAR on, no VGA**: Windows boots headless with the card as its
  only adapter; result in the driver's behaviour pending (RDP only).
- **Step 3 result**: Code 43 again, same 35 registers. **Step 4, behind a PCIe root port**
  (`WIN_GPU_ROOTPORT=1`, 19:12): same 35 registers, same writes; the parent bus is not the gate.
- Config-space writes seen from the driver (through the BAR0 mirror): offset 0x80 (PCIe Device
  Control) `0x10291f` then `0x10211f` (No Snoop off), offset 0x700 `0x20000`.
- **Lead (19:20)**: the driver reads the *extended* config space through the mirror (0x700,
  0xbd8, 0xbe0, 0xbf4, 0xbfc) and the device answers 0 for all of them; the capture holds
  `0x00020ffe` at 0x700 (the driver's write `0x20000` is a read-modify-write on that value),
  `0x003f003f` at 0xbd8, `0x00010004` at 0xbe0, `0xc` at 0xbf4 and 0xbfc. config.rs zeroes
  0x100 to 0xfff on purpose; a build that serves the capture there is the next test.
- Windows side: ProblemCode 43 with ProblemStatus 0, no `nvlddmkm` or Kernel-PnP start event:
  the driver declares the device failed on its own, silently.
- The whole sequence, for reference (reads, count): BOOT_0 x102, 0x1373a0 x10, 0x21434 x7,
  0x21c04 x4, 0x610074 x3, BOOT_1 x3, 0x21200 x2, BOOT_42 x2, then once each 0x625480,
  0x610068, 0x610018, 0x110abc, the mirror at 0x7c/0x84/0x700/0xbd8/0xbe0/0xbf4/0xbfc, 0x22410,
  0x21640, 0x1478, 0x1400, 0x1078, 0x1070, 0x140, 0x8; writes: Device Control, 0x700, the seven
  `0xc80` at 0xd96c + n·0x50.

## Night run (2026-09-25, from 19:57): what gates the driver

Unattended driver-bind runs (`night-win.sh`, later `night-win2.sh`; one boot per configuration,
the driver starts at every boot, results in `night-2026-09-25/`, each `<run>-summary.txt` with
the diff against the 35-register baseline, `trace-saver` archives the full traces from run N on):

| Run | Device build | PROM window | Other | Registers | Falcon writes | Result |
|---|---|---|---|---|---|---|
| A, A2 | extcfg 9df39581 | rom.bin | | 35 | 0 | old loop |
| B, B2, G | extcfg | rom.bin | ext caps visible (+root port in G) | 35 | 0 | old loop |
| C, D | extcfg | rom.bin | Tesla T4 costume, 616.92 | 33 (+0x214d8, 0x2156c fuse ids, no PDISP) | 0 | old loop |
| F | extcfg | rom.bin | 3D controller class | 35 | 0 | old loop |
| H | extcfg | synth-vbios.bin | | 36 (+0x22408) | 0 | scans the chain end, old loop |
| K | extcfg | synth-vbios.bin | T4 | 34 | 0 | old loop |
| I, J | extcfg+hwcfg a5db4f5d (0x110abc = 0x20 from the capture) | synth / rom.bin | | 36 / 35 | 0 | old loop |
| **L** | extcfg | **flash chain from 0x28600** (`prom-window-2026-09-25-from-55aa.bin`) | | **554** | **345** | GSP/PMU/display programming, fails later |
| **M** | extcfg+hwcfg | flash chain | | 557 | 368 | as L |
| N | extcfg | flash chain | T4 | 0 | 0 | Windows did not boot that time (no device access in 20 min) |
| P, R | extcfg+ifr d4d37c2f | **full 1 MiB window with the IFR header** | GeForce (R: + root port, ext caps) | 557 | 368 | as M: the IFR header changes nothing beyond the chain |
| Q | extcfg+ifr | full window | T4, 616.92 | 365 | 368 | the data-centre driver also programs the falcons now |
| **X0** | extcfg+pmu2 2f6d7ff5 | full window | **GSP-firmware registry keys set** | **17976** | 699 (SEC2 port writes 25181, GSP queue accesses 558, PMU polls 0) | **GSP-RM path**: Booter on SEC2, 184 RPCs, then GMCAPI_GSP_SUSPEND; Code 43 |

- **The VBIOS content was the Code 43 gate.** With only the PCI expansion ROM (legacy + EFI, 127 KiB)
  in the PROM window the driver scanned the window to its end and gave up; with the card's flash
  chain (the two 0xE0 FWSEC images, the falcon ucode table, the type 0x70 image) it loads ucode
  into the GSP falcon (DMEMD stream at 0x110184, 0x1101c4, 0x110040, 0x110084) and the PMU
  falcon (0x10a184/0x10a1c4, not modelled: latched) and writes display 0x640000. The PROM window
  dump and its analysis: `caraxes/reference/rtx2070/prom-window-2026-09-25.md`; the research note
  that pointed there: `docs/research/windows-driver-code43.md`.
- Ruled out tonight: the extended config space (served, no change), 0x110abc, the display
  adapter role, the root port, the 3D-controller class, the T4 identity.
- Follow-ups running or queued: the GSP-path analysis (`night-2026-09-25/ANALYSIS-gsp-path.md`),
  ETW of the driver start over the guest agent (`ETW-driver-start.md`), the VBIOS content bisect
  (`VBIOS-BISECT.md`: which images or tables are the minimal ingredient, so a synthetic VBIOS can
  carry them and no per-card original is needed).
- **Registry test (22:36, `regkey-test.sh`)**: `EnableGpuFirmware` and `RMEnableGpuFirmware` = 1 set
  under `Services\nvlddmkm`, its `Parameters` subkey and the class key `{4d36e968-…}\0007`; the
  driver restart and the reboot still ended in Code 43. The verdict windows (100 s after the
  restart, 120 s after the boot) saw no GSP RISC-V or queue access; the boot's later trace showed
  the 60 s PMU poll and then about 25000 SEC2 port writes (a large ucode load) before the trace
  was overwritten by the next chain, so whether the keys switch the driver to the GSP-RM path is
  open and is checked by the night loop (keys stay set in the guest until the loop removes them).
- **Registry keys work (22:42, run X0, PMU build + keys)**: with `EnableGpuFirmware`/`RMEnableGpuFirmware` = 1
  the driver takes the **GSP-RM path**: Booter load on SEC2 (25k port writes), GSP queue
  registers, GMCAPI_GSP_INIT, 33 allocs, 107 controls, 36 frees over 186 doorbells, then
  GMCAPI_GSP_SUSPEND (a regular unload) and Code 43. The PMU ENGCTL poll is gone. The device ran
  in pure emulation, so the controls that need the host RM answered NOT_SUPPORTED
  (BUS_GET_INFO_V2 retried 24 times, fb info v2, GR FS info, ECC/function status, ...); the
  night loop now works on the RPC phase, the CPU-RM chain is secondary.
- **The key is official**: `kernel-open/common/inc/nv-firmware-registry.h` (shared Windows/Unix) defines
  `EnableGpuFirmware` (mode bits: 0 off, 1 on, 2 default; policy 0x10 allow fallback, 0x20 default
  on WS/server; driver default 0x12) and `EnableGpuFirmwareLogs` (1 = firmware logs into the
  System log). We set mode 1 without fallback, so a failure is a GSP-RM failure. The night loop
  also sets the logs key and resolves the device's class key via `DEVPKEY_Device_Driver`.
- **The RPC-phase controls, named** (SDK headers): 0x800292 `NV0080_CTRL_CMD_GPU_GET_CLASSLIST_V2`
  (804 B), 0x801707 `FIFO_GET_ENGINE_CONTEXT_PROPERTIES`, 0x20800aaf
  `INTERNAL_GET_ENABLED_SEC2_CLASSES`, 0x20800afe `INTERNAL_INIT_USER_SHARED_DATA` (succeeded),
  0x20802806 an LPWR control not in the public 615 header, 0x20803801 `GRMGR_GET_GR_FS_INFO`
  (per-query errors tolerated, an outer error fails the call). ECC status and function status
  document NOT_SUPPORTED as a valid answer. Fix order for pure emulation: BUS_GET_INFO_V2 and
  FB_GET_INFO_V2 (answer the requested indices), classlist, engine context properties, the GR FS
  queries, SEC2 classes. GSP releases from r615 share one ABI (r000), so 617 against the
  615-based endpoint is compatible in principle.
- Unattended tooling added: `qga.py` (QEMU guest agent: PowerShell as SYSTEM, file transfer),
  `qmp.py` (screenshot, keys, clean power-down), `WIN_GPU_VBIOS`, `WIN_GPU_ROOTPORT`, `WIN_VGA`.
