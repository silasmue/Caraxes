<!-- SPDX-License-Identifier: MIT -->
# The Blackwell N gate: a stock 610.57.04 guest on a synthetic GB206, in pure emulation

The first Blackwell gate of decision D14 ([README.md](README.md), [versatility.md](versatility.md)):
the device presents the RTX 5060 Ti identity of the lab capture
([blackwell-gb206.md](blackwell-gb206.md)) and emulates its FSP boot
([gsp-boot-blackwell.md](gsp-boot-blackwell.md)); the guest runs the unmodified 610.57.04
driver it already has. There is no Blackwell card in the house and none is used: no host
RM, no host execution, no VRAM reservation, no host register. The RTX 2070 is not touched
(the device refuses the host flags for any chip but tu106, `src/main.rs`, and the launcher
refuses them with `CARAXES_CHIP=gb206`).

The gate stops at the first channel allocation. Past it the guest pushes Blackwell methods
and writes GMMU v3 page tables, which neither the channel emulation nor `fb.rs` executes yet
(blackwell-gb206.md section 4, "What cannot be done").

## 1. Stages

Each stage names what is read, where, and what passes. Every wall is read from the guest
dmesg and the device trace and fixed from the 610 source, never from a trace of a real card.

| Stage | Passes when | Read from |
|---|---|---|
| N0 device | the server log says `GB206 (BOOT_0 0x1b6000a1, driver 610.57.04) ... boot model on` and the guest name `Leandro VFIO RTX 5060 Ti-4G` | `$W/gpu.log` |
| N1 PCI identity | guest `lspci -nn -s <bdf>` shows `[10de:2d04] (rev a1)`, subsystem `1043:8a08`; `lspci -vvv` shows BAR0 64 MiB, BAR1 256 MiB 64-bit prefetchable, BAR3 32 MiB, I/O 128 B, ROM 512 KiB, and the capabilities PM, MSI and Express and nothing else (no vendor-specific capability, no MSI-X, no extended capabilities) | guest `lspci` |
| N2 driver load | `modprobe nvidia` returns 0, the log has no `RmInitAdapter failed`, `/dev/nvidia0` exists; the device trace shows the FSP sequence (`fsp: COT version 2 size 860`, `acr: fb ...`, `gsp: target mask released`), `doorbell 1: libos args PA ... (mapped)`, `queues linked`, and the init RPCs answered through GET_GSP_STATIC_INFO | `probe-<label>/guest-dmesg.txt`, `probe-<label>/device-trace.txt` |
| N3 nvidia-smi | `nvidia-smi -q` in the guest shows Product Name `Leandro VFIO RTX 5060 Ti-4G`, FB memory total 4096 MiB, VBIOS 98.06.1F.40.4A, no Xid in dmesg | guest `nvidia-smi -q`, dmesg |
| N4 first channel | the first allocation of a Blackwell channel class (BLACKWELL_CHANNEL_GPFIFO_B 0xCA6F or _A 0xC96F) returns status 0 in the device trace (`alloc: ... class 0xca6f ... -> status 0x0`); stop there | `uvm-<label>/device-trace.txt` |

What each stage is expected to hit first (predictions to falsify, from the source):

- N1: nothing; the identity is the capture's (the unit test `config::tests::gb206_capture_without_msix`
  and the client selftest `--selftest ... --board-data rtx5060ti` pass on the build).
- N2, before kgspInitRm: the RmInitAdapter phases before the GSP boot read registers the
  Turing trace never showed (XAL/XTL windows, GB20x fuses, the display fuse of GB20x,
  PTOP). They latch and read 0 today. The first read that 0 does not satisfy is the first
  wall; its offset is in the trace summary as unnamed (chips/gb206/regmap.toml names the
  known ones).
- N2, the GSP boot: `boot_fsp.rs` answers the sequence of gsp-boot-blackwell.md section 2;
  the unit test `gb206_bootstrap_reaches_the_message_queues` runs it register by register.
  A wall here is an access that section misses.
- N2, after the queues: phase-5 controls the GB20x kernel RM asks and the TU106 dispatcher
  does not answer (the trace's `unanswered` list); BAR1/BAR2 page-table walks in the GMMU v3
  format, which `fb.rs` does not implement (`MMU_FORMATS_UNWALKED`, chip/mod.rs).
- N3/N4: FB accounting against the ACR layout the model computed (gsp-boot-blackwell.md
  section 6), and the channel class checks in `ctrl.rs`/`channel.rs` for 0xCA6F.

## 2. Preconditions

- A build of this tree: `cargo build --release -p vfio-user-nvgpu` (or the worktree build
  `target/gb206/release/vfio-user-nvgpu` through `CARAXES_SERVER`).
- `caraxes/reference/gb206/gpu-0000_01_00.0/rom.bin` and `config-space.txt` in the tree the
  launcher runs from (the launcher and the device fail closed without them).
- The guest image has the GB20x GSP firmware of its driver: 610.57.04 maps GB20x to
  `gsp_ga10x.bin` (`src/nvidia/arch/nvalloc/common/inc/nv-firmware.h:97-104`), so
  `/lib/firmware/nvidia/610.57.04/gsp_ga10x.bin` must exist in the guest (the TU106 runs
  used `gsp_tu10x.bin`). Check with the `ssh` command below before N2.
- The guest's module options of [quickstart.md](quickstart.md) section 2 step 3
  (`/etc/modprobe.d/caraxes.conf`, `RmDisableGlobalCeUtils=1;RMDisableScrubOnFree=1;RmRcWatchdog=0`).
  Without `RmRcWatchdog=0` N2 fails at `numGpcs > 0` (section 5); the device warns about a
  missing key in its log (`WARNING guest registry ...`).
- vm3's state directory (`/mnt/vmstore/caraxes/vm3`: rootfs.qcow2, seed.img) and its tap.
  Without a tap, the serial-console variant of the gate is
  `caraxes/runs/emu-regress-2026-09-25/emu-check.sh <dir> CARAXES_CHIP=gb206 CARAXES_FB_MIB=4096`
  on vm7 (its `guest-check.sh` covers N1 to N4).

## 3. The command

From the tree with this change (paths relative to it; `CARAXES_SERVER` only when running
the worktree build):

```sh
# N0/N1: start the device and the VM (pure emulation: no host RM, no reservation, no DMA table)
VM=vm3 CARAXES_TAP=caraxes3 CARAXES_IP=192.168.103.23 CARAXES_MAC=02:00:00:00:00:23 \
CARAXES_CHIP=gb206 CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096 \
tools/caraxes-vm.sh up
VM=vm3 CARAXES_IP=192.168.103.23 tools/caraxes-vm.sh wait
VM=vm3 CARAXES_IP=192.168.103.23 tools/caraxes-vm.sh ssh 'lspci -nn -d 10de:; sudo lspci -vvv -d 10de:2d04; ls -la /lib/firmware/nvidia/610.57.04/'
# N2: load the driver, keep the guest log and the device trace of the window
VM=vm3 CARAXES_IP=192.168.103.23 CARAXES_CHIP=gb206 tools/caraxes-vm.sh probe n2
# N3
VM=vm3 CARAXES_IP=192.168.103.23 tools/caraxes-vm.sh ssh 'nvidia-smi -q | head -60; sudo dmesg | grep -iE "xid|nvrm" | tail -20'
# N4: UVM registration allocates the first channels
VM=vm3 CARAXES_IP=192.168.103.23 CARAXES_CHIP=gb206 tools/caraxes-vm.sh uvm n4
VM=vm3 tools/caraxes-vm.sh down
```

The tap, IP and MAC follow the vm2 pattern of [memory-model.md](memory-model.md) section 3;
use vm3's own values if they differ. `CARAXES_HOST_RM`, `CARAXES_RESERVE_VRAM`,
`CARAXES_HOST_EXEC` and `CARAXES_DMA=register` are refused with `CARAXES_CHIP=gb206`.
Evidence goes to `caraxes/runs/blackwell-<date>/` (the `probe-*`/`uvm-*` directories, the
`lspci` and `nvidia-smi` output, `gpu.log`), rows to the "Blackwell N gate" section of
[experiments.md](experiments.md).

## 4. Status

2026-09-24: defined; the device side is built and unit-tested (GB206 identity, FSP boot to
the linked queues). First runs on vm1 (12:06-12:19, VM=vm1 CARAXES_TAP=caraxes1
CARAXES_IP=192.168.101.21 CARAXES_MAC=02:00:00:00:00:21, otherwise section 3): N0 and N1
pass; N2 reaches the GSP boot, the init RPCs, the static info and the phase-5 controls, and
stops at `kbusVerifyBar2_GB202` because the device cannot walk GMMU v3 BAR2 page tables
(experiments.md rows b1, b2). Preconditions learnt: the guest image needs `gsp_ga10x.bin`
(installed in vm1 from the host's 610.57.04 firmware directory).

2026-09-24, 12:31-12:51 on vm1 (rows b4 to b8): with the GMMU v3 walker, the heartbeat
model and four physical-RM answers from the capture (`src/chip/physical.rs`: device-info
table, CE physical caps, CE PCE masks, GR info) **N0 to N4 pass**: RmInitAdapter succeeds,
`nvidia-smi -q` shows "Leandro VFIO RTX 5060 Ti-4G" with 4096 MiB and VBIOS 98.06.1F.40.4A
(`caraxes/runs/blackwell-2026-09-24/n3/nvidia-smi-q.txt`), and UVM's first channel
(BLACKWELL_CHANNEL_GPFIFO_B 0xCA6F, chid 1, COPY4) is allocated with status 0. Beyond the
gate: the channel's first push never completes (no Blackwell method execution), so
UVM_REGISTER_GPU hangs; that is the next package (CA6F/CAB5 method subsets in channel.rs,
blackwell-gb206.md 2.2).

## 5. Pure emulation regressions of 2026-09-24

**Report.** The costume ladder ([costume-ladder.md](costume-ladder.md) section 2.1) found two
failures of pure emulation on main that the frozen main binary 7621cfb shows too: the TU106
no longer reached `RmInitAdapter succeeded` (`memmgrMemSet ... TRANSFER_FLAGS_PREFER_CE`
timed out, mem_mgr.c:445, then `memmgrInitCeUtils` mem_mgr.c:508, RmInitAdapter 0x25:0x65;
the device trace said `chid 2: VAS root unknown, nothing run`, `src/channel.rs:841`), and
this gate's GB206 stopped at `numGpcs > 0` (kernel_graphics_object.c:192) in `krcWatchdogInit`.
Both had passed the day before (section 4; the TU106 boots of the 615 transport).

**Bisect** (`caraxes/runs/emu-regress-2026-09-25/`, 2026-09-25, VM vm7 without a network
device, no host RM, no reservation, no GPU; each commit built into `target/bisect`, booted
through a frozen copy of the HEAD launcher, verdict from the serial log). In the ladder's
rig, the declared good end 61e0b6a fails for both chips, and so does 6650623, the commit
where this gate was recorded as passing. In the rig of every other Caraxes guest, the
declared bad end 7621cfb boots both chips, and so does main 541b9a9 (TU106 with the 610
and the 615 guest, GB206 N0 to N4). No commit between the ends changes the verdict in
either rig, so there was nothing to bisect.

**Cause.** The rig, not the device. The ladder's vm6 was a fresh overlay on the 610 image,
and that image has no `/etc/modprobe.d/caraxes.conf`: vm0 to vm5 got it by the quickstart's
step 3 ([quickstart.md](quickstart.md) section 2), the 615 base image ships it. Without the
three registry keys the guest RM runs work the device cannot serve yet:

- `RmDisableGlobalCeUtils` (interface/nvrm_registry.h:975-977, read at mem_mgr.c:331-335)
  absent: the kernel builds its global CeUtils and tests it with a CE memset (mem_mgr.c:445)
  on a channel whose VA space the software executor never learns (the CeUtils VAS gets its
  root through `COPY_SERVER_RESERVED_PDES` and is freed before the channel runs). This is the
  wall [mapping.md](mapping.md) section 9 measured on 2026-09-24 when it dropped the key on
  purpose; the TU106's failure.
- `RmRcWatchdog` (nvrm_registry.h:1584-1587, default enabled, read at kernel_rc.c:198-203)
  absent: `krcWatchdogInit` allocates a GR object, which needs a GPC in the floorsweeping masks
  (kernel_graphics_object.c:190-192); without a host, `INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS`
  is answered with zero masks. The GB206's failure (its CeUtils test does not stop it first).
- `RMDisableScrubOnFree` (nvrm_registry.h:926, mem_mgr.c:204-208) absent: freed video
  memory is scrubbed through the copy engine.

The registry the device receives shows the difference: 40 `SET_REGISTRY` entries from the
ladder's guest, 43 from vm1 in this gate's runs (`caraxes/runs/blackwell-2026-09-24/n2c/device-events.txt`)
and from vm7 with the file. The first failure the ladder recorded before its own KGR change
(`pGrInfo` NULL, kernel_channel_group_api.c:914) is the same cause: with the keys, 61e0b6a
boots the TU106 although it still answered `INTERNAL_STATIC_KGR_GET_INFO` with NOT_SUPPORTED.

**Fix.** Nothing in the device's answers changed, so nothing was moved or re-categorised:
no family refusal, D rule, class database entry, RPC or channel ordering is involved.
- The rig: `prepare-vm7.sh` installs the quickstart's `caraxes.conf` (step 2 of its README);
  vm6 itself is the ladder's and was left as it is (it needs the same file before its
  pure-emulation results can be compared with any other VM's).
- The device (dcce8ea) now reads the 610 guest's `SET_REGISTRY` table (`rpc::packed_registry`,
  generated/g_os_nvoc.h:222-235) as it reads the 615 `GSP_INIT` registry, traces every entry,
  and warns in the trace and in `gpu.log` for each of the three keys that is absent or wrong
  (`gspboot::missing_guest_keys`; unit tests `rpc::tests::set_registry_entries`,
  `gspboot::tests::guest_keys_the_device_needs`, and the captured 615 registry in
  `boot::tests::opt_in_registry_suspend_and_resume` gives none).
- A check without a GPU: `tools/emu-boot-check.sh` boots the TU106 in pure emulation to
  RmInitAdapter and nvidia-smi (vm7; `--driver 615` vm7-615, `--chip gb206`), names missing
  module options before the boot and prints the device's warnings on a failure; it exits 77
  with a SKIP note when Cloud Hypervisor, the overlay, the firmware or the check service in
  the image, or the ROM capture is absent. `tools/check.sh` runs it as the step `emu-boot`
  with `CARAXES_CHECK_EMU_BOOT=1` (reported as SKIP otherwise).

**Boots** (device dcce8ea, `caraxes/runs/emu-regress-2026-09-25/final/`): the TU106 with the
610 guest and with the 615 guest reaches `RmInitAdapter succeeded` and nvidia-smi
(`tools/emu-boot-check.sh`, PASS both), and this gate's GB206 passes N0 to N4 on vm7 over the
serial console (N4: `alloc 0xca6f ... chid 1 ... -> status 0x0`). The costume ladder's six
pure-emulation rungs, rerun with the module options, all reach nvidia-smi as well (the run
README, "Ladder").
