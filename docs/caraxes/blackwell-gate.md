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
- vm3's state directory (`/mnt/vmstore/caraxes/vm3`: rootfs.qcow2, seed.img) and its tap.

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
