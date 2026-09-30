<!-- SPDX-License-Identifier: MIT -->
# A stock NVIDIA driver on a synthetic RTX 2070

- One host process, `vfio-user-nvgpu`, serves one VM a synthetic TU106 over vfio-user; Cloud
  Hypervisor attaches it with `--user-device`. The guest runs its own driver package.
- Linux x86-64 host with KVM and the NVIDIA open kernel modules 615.71.09 with GSP firmware
  active. The host driver stays bound and untouched; see "Host access" below.
- Use trusted guests ([isolation rules](../wiki/isolation-rules.md)). One GPU VM at a time is
  the tested case ([known issues](KNOWN-ISSUES.md), item 4).
- The launcher `tools/caraxes-vm.sh` and the gates `tools/caraxes-gate.sh` are the commands
  below; the recorded wrappers under `caraxes/runs/` show the exact settings of every run.

## Prerequisites

| What | Where |
|---|---|
| Cloud Hypervisor v53.0 with patches 0005 to 0008 | built from `vendor/cloud-hypervisor` ([patches](../../patches/README.md)); the launcher expects `$LEA_CH` (default `/mnt/vmstore/leandro/cloud-hypervisor/bin/cloud-hypervisor`) |
| Config space and ROM of the physical card | `caraxes/reference/rtx2070/config-space.txt` (tracked) and `rom.bin` (a sysfs capture, git-ignored) |
| Synthetic VBIOS | `python3 caraxes/tools/synth-vbios.py caraxes/reference/rtx2070/rom.bin caraxes/reference/rtx2070/synth-vbios.bin` once (decision D8: the missing FWSEC image is constructed, nothing in it executes) |
| Tap device | `caraxes0`, owned by the user, `192.168.100.1/24`; once per host boot |
| VM state | `$CARAXES_STORE/<vm>` (default `/mnt/vmstore/caraxes/<vm>`, else `target/caraxes/<vm>`): `rootfs.qcow2`, `seed.img`, sockets, logs |
| Guest image | Ubuntu 24.04 with the distribution's `nvidia-driver-open` 615.71.09 packages; the recorded base image is `/mnt/vmstore/caraxes/images/guest-baked-2026-09-24-615.71.09.qcow2` ([guest-driver-versions.md](guest-driver-versions.md), section 8) |
| Test material for the gates | a checkout of the private Leandro-Test repository beside this one; `sync-tests` copies it into the guest |

```sh
sudo ip tuntap add dev caraxes0 mode tap user "$USER"
sudo ip addr add 192.168.100.1/24 dev caraxes0
sudo ip link set caraxes0 up
```

## 1. Build

```sh
cargo build --release -p vfio-user-nvgpu      # target/release/vfio-user-nvgpu
./tools/check.sh                              # software checks, no GPU
CARAXES_CHECK_EMU_BOOT=1 ./tools/check.sh     # also boots the synthetic GPU without a GPU
```

## 2. Prepare a Linux guest

- Create the VM's overlay and cloud-init seed next to each other:

```sh
W=/mnt/vmstore/caraxes/vm0; mkdir -p "$W"
qemu-img create -f qcow2 -F qcow2 -b /mnt/vmstore/caraxes/images/guest-baked-2026-09-24-615.71.09.qcow2 "$W/rootfs.qcow2"
mkfs.fat -C -n cidata "$W/seed.img" 1440
mcopy -i "$W/seed.img" seed/user-data seed/meta-data seed/network-config ::   # hostname, 192.168.100.20/24
```

- A guest made from a plain Ubuntu 24.04 image needs the stock packages and two registry
  keys; the scripted form is `caraxes/runs/sottr-2026-09-24/guest/sottr-prepare.sh`:

```sh
sudo apt-get install --no-install-recommends nvidia-driver-open libnvidia-gl   # 615.71.09
printf 'options nvidia NVreg_GpuInitOnProbe=1 NVreg_RegistryDwords="RmDisableGlobalCeUtils=1;RmRcWatchdog=0"\n' | sudo tee /etc/modprobe.d/caraxes.conf
```

- The keys skip two channels the device does not serve yet (the copy-engine self-test and
  the watchdog, [kernel-rm-hal-and-registry](../wiki/kernel-rm-hal-and-registry.md)). The
  provisioning script also sets `RMDisableScrubOnFree=1`; keep it only when the device runs
  without compression (`CARAXES_COMPRESSION=off`), because the compressed slice needs the
  guest's scrubber and the device warns when the key is set.
- No Leandro module, no firmware copy, no module load order: the GSP firmware comes with
  the package and the device reads the guest's driver version at its GSP boot.

## 3. Run a Linux guest

The settings of the recorded Linux desktop (Linux B) with one VRAM slot and the fast paths
the Linux gates use; the device implies the host-execution defaults (shadow and mirror,
interrupt page, hybrid doorbell reactor, a 32 MiB BAR1 pool, GPA DMA):

```sh
export VM=vm0 CARAXES_DRIVER=615
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_RESERVE_VRAM=1 CARAXES_HOST_EXEC=1 CARAXES_DMA=register
export CARAXES_DISPLAY=1 CARAXES_DISPLAY_PAGE=1 CARAXES_DISPLAY_PAGE_READONLY=1
export CARAXES_FB_MIB=2048 CARAXES_RESERVE_SLOT=0 CARAXES_MEM=8G CARAXES_CPUS=8
export CARAXES_BAR2_MEMFD=1 CARAXES_DOORBELL_TOKENS=64 CARAXES_MSI_REARM_IOEVENTFD=1 CARAXES_COMPRESSION=slice
tools/caraxes-vm.sh up
tools/caraxes-vm.sh wait          # until ssh answers
tools/caraxes-vm.sh ssh nvidia-smi
tools/caraxes-vm.sh down
```

- `CARAXES_FB_MIB` is the guest's VRAM (the slice); `CARAXES_RESERVE_SLOT=n` places it n
  slices below the top of the host's VRAM, one slot per VM. A reservation that would leave
  the host under its floor is refused.
- `CARAXES_DMA=register` registers guest RAM with the host RM; the launcher prepares a copy
  of the binary with `cap_sys_admin+p` for that (`sudo -n setcap`), and the device raises
  the capability on one thread around three controls only.
- `CARAXES_DISPLAY=1` presents the DisplayPort monitor; the display page needs patch 0007.
- Knobs: `CARAXES_COMPRESSION` (`off` or `slice`), `CARAXES_DOORBELL_POLL` (`hybrid`, `spin`,
  `off`), `CARAXES_BAR1_POOL_MIB` (32; 0 for none), the VRAM size and `--display`. Everything
  else is a default or a diagnostic ([device configuration](../wiki/device-configuration.md)).
  The device prints its effective settings as a block of `config:` lines at the top of
  `gpu.log` and warns once per variable it no longer reads.
- Logs under `$W`: `gpu.log` (the device), `gpu-trace.log` (every trapped access, `CARAXES_TRACE=0`
  turns it off), `ch.log` (Cloud Hypervisor), `serial.log` (guest console). A host Xid line
  in the host kernel log names the device's pid.
- A Wayland desktop with Sunshine: `caraxes/runs/night-2026-09-28/desktop-b-night.sh` is the
  recorded bring-up (GNOME, Sunshine with `encoder = nvenc`, Moonlight on the host).

## 4. Other guests

- **Windows 11 (driver 617.14).** Installed once under QEMU (the installer needs a VGA and a
  GOP), then run under Cloud Hypervisor with the device: `caraxes/runs/win11-2026-09-25/README.md`
  (`run44.py`, the CH build with EDK2 and swtpm) and the test pack in
  [tools/windows](../../tools/windows/README.md) (smoke, D3D11 stages, CUDA, NVENC, Heaven,
  Superposition, Sunshine). The interrupt tree page runs in exact mode there (patch 0007).
- **nova-core.** Pure emulation, no VRAM slot: `caraxes/runs/nova-2026-09-25/nova-boot.sh <dir>`
  with the nova kernel (`CARAXES_KERNEL`) and the FWSEC-0x85 VBIOS (`CARAXES_VBIOS`); the
  device detects the 570 ABI ([nova-guest.md](nova-guest.md)).
- **nouveau with NVK.** `caraxes/runs/openrender-2026-09-29/README.md`: an overlay on the 615
  image with the Linux 7.2.7 kernel and `DRM_NOUVEAU=m`, host execution, rungs R1 to R8
  (probe, kernel channels, the NVK device, an offscreen clear and triangle, Weston with
  vkcube, glmark2 and vkmark).

## 5. Verify

```sh
VM=vm0 tools/caraxes-vm.sh sync-tests    # copy and build the Leandro-Test material in the guest
VM=vm0 tools/caraxes-gate.sh nvml        # the guest's nvidia-smi -q against the host's, 11 checks
VM=vm0 tools/caraxes-gate.sh cuda        # 12 scored tests, one device-trace window each
VM=vm0 tools/caraxes-gate.sh gpu         # v1's guest-side stages: smi, memory, kernel, torch, robustness, encode
VM=vm0 tools/caraxes-gate.sh vdisplay    # the DisplayPort monitor: modes, flips at 60 Hz
VM=vm0 tools/caraxes-gate.sh display     # the same with Weston and Sunshine capture=kms
```

- Verdicts are v1's: one JSON line, exit 0 pass, 1 fail, 2 skip (only for a missing
  precondition). Raw output goes to `caraxes/runs/gate-<name>-<date>/`. Current results:
  `nvml` pass, `cuda` 10 of 12 (managed memory and cuda-gdb open), `display` 11 of 11
  ([testing.md](testing.md)).
- Before a merge every change passes the four-guest gate on one binary: the Linux 615
  kick-smoke, the nova-core probes, the nouveau plus NVK rungs and a Windows session
  (`caraxes/runs/gate-2026-09-30/README.md`, [verification methods](../wiki/verification-methods.md)).
  The gate's rendering check compares a game frame with a reference by colour statistics.
- Benchmarks against native and v1: [benchmark.md](benchmark.md), [workload-gate.md](workload-gate.md);
  the codec matrix: `caraxes/runs/codec-2026-09-30/matrix.md`.

## Host access and VM state

- Keep the host NVIDIA driver bound. The device submits work through the host RM as an
  unprivileged client; never unbind, reset the GPU, reload host modules or write host
  registers. Passthrough of the physical GPU is a separate, prepared procedure
  ([passthrough-procedure.md](passthrough-procedure.md)) that is not part of this path.
- Give each VM its own device process, socket, VRAM slot, overlay disk, tap and address.
  Keep backing images frozen while their overlays are in use; keep 30 GiB free on the VM
  store (a full disk damaged a guest's file system once, [known issues](KNOWN-ISSUES.md) 5).
- Host floors: 1024 MiB of host VRAM and 64 MiB (Linux) or 96 MiB (Windows) of host BAR1
  stay free for the host desktop; a device that would go below refuses the allocation.
- After a host reboot the first launch may fail at the IOVA setup; bring the VM down and
  launch again.
