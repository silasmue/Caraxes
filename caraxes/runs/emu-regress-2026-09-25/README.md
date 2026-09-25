<!-- SPDX-License-Identifier: MIT -->
# The pure-emulation regressions of 2026-09-24, bisected

The costume ladder (docs/caraxes/costume-ladder.md section 2.1) reported two regressions of
pure emulation (no host RM, no reservation, no GPU) on main: the TU106 boot times out in
`memmgrMemSet` (mem_mgr.c:445) with `chid 2: VAS root unknown` in the device trace, and the
Blackwell N gate's GB206 stops at `numGpcs > 0` (kernel_graphics_object.c:192), both with the
frozen main binary 7621cfb. The analysis is docs/caraxes/blackwell-gate.md section 5; this
directory holds the evidence. Result: **neither is a device regression**. The ladder's VM
(vm6, a fresh overlay on the 610 image) lacked the guest module options every other Caraxes
guest carries (`/etc/modprobe.d/caraxes.conf`, docs/caraxes/quickstart.md section 2 step 3:
`RmDisableGlobalCeUtils=1;RMDisableScrubOnFree=1;RmRcWatchdog=0`); with them, the declared bad
commit boots, and without them the declared good commit (and the N gate's own commit) fails.

## Rig

- `prepare-vm7.sh`: vm7, a copy of the ladder's vm6 overlay (610 image, the guest modules of
  vm0, the 610.57.04 `gsp_tu10x.bin` and `gsp_ga10x.bin` the ladder installed), and vm7-615, a
  fresh overlay on the 615 base image (which ships the 615.71.09 firmware and `caraxes.conf`),
  both with `guest-check.sh` as `caraxes-check.service` and `uvm-register.c`. The second step,
  `CARAXES_GUEST_CONF=1 prepare-vm7.sh`, gives vm7 the quickstart's `caraxes.conf`.
- `guest-check.sh`: at boot, lspci (with `-vvv`, N1), modprobe and the NVRM lines (N2),
  `/proc` information, nvidia-smi, the name/compute-capability query, `nvidia-smi -q` (N3)
  and on a GB206 the UVM registration (N4; killed after 60 s, as the gate stops at the
  first channel allocation), all onto the serial console, then power-off (a sysrq
  watchdog forces it after 420 s).
- `emu-check.sh <dir> [NAME=value ...]`: one boot with `CARAXES_TAP=none`,
  `CARAXES_BOOT_MODEL=1`, host flags unset; keeps `serial.log` and `gpu.log` (both ignored as
  `*.log`), `trace-notable.txt`, `gate-evidence.txt` (the N2/N4 trace lines), `setup.txt`.
  `CARAXES_RIG` runs the launcher of another tree: the bisect used a frozen copy of the HEAD
  tree (`target/rig`, from `git archive 541b9a9`), because the launchers of the older commits
  have no `CARAXES_TAP=none`.
- `bisect-step.sh tu106|gb206`: one `git bisect run` step: builds the checked-out commit with
  `CARGO_TARGET_DIR=target/bisect`, boots it through `emu-check.sh`, verdict good when
  nvidia-smi returns 0 without `RmInitAdapter failed`.
- `ladder-conf.sh`: the ladder's six pure-emulation rungs again on vm7 with `caraxes.conf`.
- `keep.sh`: from every record's ignored `serial.log` and `gpu.log`, the committed
  `guest-check.txt` (the guest check's lines) and `device-log.txt`.
- vm7-bare: a copy of vm7 with `caraxes.conf` removed (guestfish `rm`), the negative case of
  `tools/emu-boot-check.sh` (`CARAXES_EMU_VM=vm7-bare`).

No host GPU state was read or written: every run is pure emulation (the launcher refuses the
host flags for gb206, and `emu-check.sh` unsets them for tu106). No network device, no tap.

## Runs

| Record | Device | Guest (VM) | Chip | caraxes.conf | Result |
|---|---|---|---|---|---|
| `head-541b9a9/tu106-610` | main 541b9a9 | 610 (vm7) | TU106 | no | memmgrMemSet timeout mem_mgr.c:445, RmInitAdapter 0x25:0x65 (the ladder's wall) |
| `head-541b9a9/gb206-610` | main 541b9a9 | 610 (vm7) | GB206 | no | `numGpcs > 0` kernel_graphics_object.c:192, krcWatchdogInit failed |
| `noconf/tu106-61e0b6a` | 61e0b6a (declared good) | 610 (vm7) | TU106 | no | fails: pGrInfo NULL kernel_fifo.c:2771, channel group kernel_channel_group_api.c:914, the scrubber's CeUtils, RmInitNvDevice |
| `noconf/gb206-61e0b6a` | 61e0b6a (declared good) | 610 (vm7) | GB206 | no | fails: `numGpcs > 0` |
| `noconf/gb206-6650623` | 6650623 (the N gate's pass, 2026-09-24 12:54) | 610 (vm7) | GB206 | no | fails: `numGpcs > 0` |
| `ends-conf/tu106-61e0b6a` | 61e0b6a | 610 (vm7) | TU106 | yes | good: nvidia-smi 0 (KGR info still NOT_SUPPORTED then) |
| `ends-conf/gb206-61e0b6a` | 61e0b6a | 610 (vm7) | GB206 | yes | good |
| `ends-conf/tu106-7621cfb` | 7621cfb (declared bad, the ladder's frozen binary) | 610 (vm7) | TU106 | yes | good: RmInitAdapter succeeded, nvidia-smi 0 |
| `ends-conf/gb206-7621cfb` | 7621cfb | 610 (vm7) | GB206 | yes | good |
| `head-541b9a9/tu106-615` | main 541b9a9, v615 build | 615 (vm7-615) | TU106 | yes (image) | good: "Leandro VFIO RTX 2070-1G", 615.71.09 |
| `head-541b9a9/tu106-610-conf` | main 541b9a9 | 610 (vm7) | TU106 | yes | good |
| `head-541b9a9/gb206-610-conf` | main 541b9a9 | 610 (vm7) | GB206 | yes | N0 to N4 pass |
| `check/tu106-610` | dcce8ea (this branch) | 610 (vm7) | TU106 | yes | `tools/emu-boot-check.sh`: PASS, 39 s |
| `check/tu106-610-bare` | dcce8ea | 610 (vm7-bare: vm7 without caraxes.conf) | TU106 | no | `tools/emu-boot-check.sh`: FAIL at mem_mgr.c:445, with the device's three `WARNING guest registry` lines |
| `ladder-conf/*` | dcce8ea | 610 (vm7) | the six ladder rungs | yes | section "Ladder" below |
| `final/*` | dcce8ea | 610 (vm7), 615 (vm7-615) | TU106, GB206 | yes | section "Final boots" below |

The registry the device receives says the same: the ladder rig's SET_REGISTRY carried 40
entries, vm1's in the N gate run 43 (`caraxes/runs/blackwell-2026-09-24/n2c/device-events.txt`),
and vm7 with `caraxes.conf` 43, among them `RmDisableGlobalCeUtils = U32(1)`,
`RMDisableScrubOnFree = U32(1)`, `RmRcWatchdog = U32(0)` (the trace lines the device writes
since dcce8ea).

## Bisect

Asked: `git bisect` between 61e0b6a (good) and 7621cfb (bad), per regression. In the ladder's
rig the good end is bad for both chips (and so is 6650623, where the N gate was recorded as
passing); in the rig of every other Caraxes guest the bad end is good for both. There is no
transition to bisect in either rig, so `git bisect run` was not started: its first two
steps would have been the end checks above. The two regressions have one cause, the guest
configuration, which the checks attribute per key from the source: the TU106 fails at the
global CeUtils self-test that `RmDisableGlobalCeUtils=1` switches off (mem_mgr.c:331-335,
mem_mgr.c:445); the GB206 fails in `krcWatchdogInit`, which `RmRcWatchdog=0` switches off
(kernel_rc.c:198-203, kernel_graphics_object.c:192).

## Ladder

`ladder-conf.sh` (device dcce8ea, vm7 with `caraxes.conf`; `ladder-conf/summary.txt`): every
rung reaches `RmInitAdapter succeeded` and nvidia-smi in pure emulation, where the ladder's
table recorded the baseline wall (rungs 0 to 4) and `numGpcs > 0` (rung 5):

| Rung | nvidia-smi query (name, compute capability, PCI id, VBIOS, FB) |
|---|---|
| baseline TU106 | Leandro VFIO RTX 2070-1G, 7.5, 0x1F0210DE, 90.06.3D.00.76, 1024 MiB |
| 1 TU104 (`rtx2080`) | Leandro VFIO RTX 2080-1G, 7.5, 0x1E8210DE, 90.06.3D.00.76, 1024 MiB |
| 2 GA106 (`rtx3060`) | Leandro VFIO RTX 3060-1G, 8.6, 0x250310DE, 90.06.3D.00.76, 1024 MiB |
| 3 AD106 (`rtx4060ti`) | Leandro VFIO RTX 4060 Ti-1G, 8.9, 0x280310DE, 90.06.3D.00.76, 1024 MiB |
| 4 GH100 (`h100`) | Leandro VFIO H100 PCIe-4G, 9.0, 0x233110DE, 98.06.1F.40.4A, 4096 MiB |
| 5 GB206 over TU106 (`rtx5060ti-gb206`) | Leandro VFIO RTX 5060 Ti-4G, 12.0, 0x2D0410DE, 98.06.1F.40.4A, 4096 MiB |

The rtx3060 and rtx4060ti rungs take `synth-vbios-fwsec-v3.bin`, generated for this run with
`caraxes/tools/synth-vbios.py --desc v3` from the ROM capture (ignored by the repository, as
in the ladder).

## Final boots

With the device of dcce8ea (the last device commit of this branch; 610 build
`target/release`, 615 build `target/v615`), vm7 and vm7-615 with `caraxes.conf`:

- `final/tu106-610` (`tools/emu-boot-check.sh --driver 610`): PASS, RmInitAdapter succeeded,
  "Leandro VFIO RTX 2070-1G", 7.5, 1024 MiB.
- `final/tu106-615` (`tools/emu-boot-check.sh --driver 615`): PASS, RmInitAdapter succeeded,
  615.71.09, "Leandro VFIO RTX 2070-1G".
- `final/gb206-610-ngate` (`emu-check.sh ... CARAXES_CHIP=gb206 CARAXES_FB_MIB=4096`), the N
  gate of docs/caraxes/blackwell-gate.md section 1: N0 `GB206 (BOOT_0 0x1b6000a1, driver
  610.57.04) ... boot model on`, guest name "Leandro VFIO RTX 5060 Ti-4G" (`gpu.log`); N1
  `[10de:2d04] (rev a1)`, subsystem ASUSTeK 8a08, BAR0 64M, BAR1 256M 64-bit prefetchable,
  BAR3 32M, I/O 128, ROM 512K, capabilities PM, MSI, Express only (`serial.log`); N2 modprobe
  0, no `RmInitAdapter failed`, `/dev/nvidia0`, the FSP sequence, `doorbell 1: libos args PA
  ... (mapped)`, `queues linked`, GET_GSP_STATIC_INFO answered (`gate-evidence.txt`); N3
  `nvidia-smi -q` Product Name "Leandro VFIO RTX 5060 Ti-4G", FB 4096 MiB, VBIOS
  98.06.1F.40.4A, no Xid; N4 `alloc 0xca6f ... chid 1 ... -> status 0x0` (UVM's first
  channel; the registration then hangs as in section 4 of the gate and is killed after 60 s,
  `uvm-register rc=137`). **N0 to N4 pass.**
