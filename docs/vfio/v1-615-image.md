<!-- SPDX-License-Identifier: MIT -->
# The v1 guest image for driver 615.71.09

The Leandro v1 (API forwarding) guest for the 615.71.09 host. It replaces the hand-made overlay of the first v1 run
on 615 ([cutover-615.md](../history/caraxes-cutover-615-2026-09-24.md) section (e), record `caraxes/runs/cutover-615-2026-09-24/v1/`). Made on
2026-09-24 without booting it, from that overlay. `caraxes/tools/bench-v1vm.sh` uses it by default when the driver
selection says 615.

## 1. The image

| Item | Value |
|---|---|
| Path | `/mnt/vmstore/caraxes/images/guest-v1-2026-09-24-615.71.09.qcow2` (mode 0444) |
| Source chain | `bench-v1-615/rootfs.qcow2` (the run's overlay, sha256 `67787853...c2fb` as in the record's `image-sha256.txt`, unchanged afterwards) on `guest-baked-2026-09-24-615.71.09.qcow2` (the Caraxes 615 base image, [guest-driver-versions.md](guest-driver-versions.md) section 8) |
| Format | qcow2, compat 1.1, no backing file, virtual size 40 GiB |
| Size | 16794648576 bytes (15.6 GiB, uncompressed clusters) |
| `qemu-img check` | no errors, 256201/655360 clusters allocated |
| sha256 | `cba6720844d74a2ec15bff89d6df89ba31a1d082bb4986fb49e28d7d2bf71f5d` (also in `.sha256` next to the image) |
| Seed | `guest-v1-2026-09-24-615.71.09.seed.img` next to it: the run's seed (sha256 `cce93340...c2ca`), instance id `v1-615-1790262142`, hostname `v1-615`, 192.168.105.25, key `id_leandro`. The image's cloud-init state is that instance, so a VM with this seed skips the per-instance steps. With another seed, cloud-init runs them again, as on the base image |

How it was made (no VM had either file open):

```
qemu-img create -f qcow2 -F qcow2 -b /mnt/vmstore/caraxes/bench-v1-615/rootfs.qcow2 work.qcow2
guestfish -a work.qcow2 -i -f fix.gf        # the edits of section 2, in the work overlay only
qemu-img convert -O qcow2 work.qcow2 /mnt/vmstore/caraxes/images/guest-v1-2026-09-24-615.71.09.qcow2
qemu-img check ...; sha256sum ...; rm work.qcow2
```

A nix-built image (Leandro `nix build .#guest-image` at the L615 pin) was not possible offline. At the L615 pin,
`nix build --offline --dry-run path:/mnt/vmstore/leandro/wt-615#guest-image` lists 3038 derivations to build,
qemu and nixos-install among them, and their sources are not in the store. It is also a different guest: a NixOS
system on kernel 6.12 whose identity comes from the kernel command line and has no seed disk
(`nix/guest-image.nix:41-64` in Leandro), and whose NVIDIA userspace the rig stages at `up` time
(`nix/guest-image.nix:66-70`). The bench and the Caraxes gates use the Ubuntu 6.8.0-136 guests, so this image
follows the qemu-img route of the base image.

## 2. What differs from the Caraxes 615 base image

Read with `guestfish --ro` from both images.

| Piece | `guest-baked-2026-09-24-615.71.09` | `guest-v1-2026-09-24-615.71.09` | Why |
|---|---|---|---|
| `/etc/modprobe.d/leandro-v1.conf` | none (only `caraxes.conf` with `blacklist nvidia`) | `blacklist` and `install <m> /bin/false` for `nvidia`, `nvidia-uvm`, `nvidia-modeset`, `nvidia-drm`, `nvidia-peermem` | `virtio_nvrm` owns `/dev/nvidia*`. The DKMS modules of the base must never load |
| `/usr/bin/nvidia-modprobe` (package `nvidia-modprobe` 615.71.09-2ubuntu1) | present, setuid | **diverted**: a local dpkg diversion to `/usr/bin/nvidia-modprobe.disabled-v1`, recorded in `/var/lib/dpkg/diversions` as `dpkg-divert --local --rename` would do it. The original path is absent | NVML runs it when `/proc/driver/nvidia/capabilities/mig/*` is missing and turns its failure (no `/sys/module/nvidia`) into `GPU access blocked by the operating system`. See cutover-615.md section (e), smi. A package upgrade writes to the diverted name |
| `nvidia-persistenced` | enabled (`multi-user.target.wants` link) | the wants link removed, the unit **masked** (`/etc/systemd/system/nvidia-persistenced.service -> /dev/null`) | It needs the real `nvidia.ko`. Masking survives a package postinst that re-enables the unit |
| `/opt/nvrm/lib` | directory, 610.57.04 (with a stale 610.43.03) | symlink to `/usr/lib/x86_64-linux-gnu` (615.71.09) | `bench-run.sh` sets `LD_LIBRARY_PATH=/opt/nvrm/lib` for every guest target (`caraxes/tools/bench-run.sh:95,260`) |
| `/opt/nvrm/bin/nvidia-smi` | 610.57.04 | symlink to `/usr/bin/nvidia-smi` (615.71.09) | the same, for `v1-gates.sh` and the bench |
| `/opt/nvrm-610.57.04/{lib,bin}` | none | the 610.57.04 copies moved out of `/opt/nvrm` | kept for reference, outside every search path |
| `/opt/nvrm/README.leandro-v1` | none | names the image and these edits | |
| `~/caraxes-tests` | none | `tools/caraxes-vm.sh sync-tests` of 2026-09-24 17:31 (`SYNC.txt`): the Caraxes runner with `driver.env` for 615, Leandro-Test 3edea73 with the probes built against libcuda 615.71.09, the L615 `libnvrm_trace.so` in `leandro-test/lib`. `out/` holds only `build.log` | the gates run without a new sync |
| `~/bench` | `bench-trap` only | plus `bench-cu`, `nvprobe`, `ioctlping`, `ctrlping`, `vrampress`, `kernels.ptx` and the Python workloads of `bench-run.sh stage v1` (hashes in the record's `bench-stage.txt`) | the bench runs without a new stage |
| cloud-init, network, ssh | instance `caraxes-vm3-1790170385`, hostname `caraxes-vm3` | also instance `v1-615-1790262142` (current), hostname `v1-615`, `/etc/netplan/60-leandro.yaml` with 192.168.105.25, new ssh host keys, the `id_leandro` key authorized | from the run's two boots |
| logs | vm3's | plus the journal, `syslog`, `Xorg.0.log` and cloud-init logs of the run's two boots | not cleaned |

Removed from the run's overlay because they belong to one run or to the host, and `bench-v1vm.sh modules`
and `v1-gates.sh` create them again: `~/guest-module` (the modules built at 17:30), `~/gpu/params.txt` (a copy of
the host's `/proc/driver/nvidia/params`), `~/caraxes-tests/out/v1-615*` (the run's results, in the record) and
`~/.nv/ComputeCache`.

Unchanged from the base: the kernel modules (DKMS `nvidia*.ko.zst` stay installed but cannot load), the GSP
firmware, the 615.71.09 CUDA, NVML and OpenCL libraries in the system paths, the renamed `nvrm.conf` and
`nvrm-gl.conf`, the 610 GL userspace in `/opt/nvrm-gl`, the torch venv and the tool set. NVENC, NVDEC, the 615 GL,
EGL and Vulkan userspace, nvcc and ffmpeg are still missing (guest-driver-versions.md section 8). So v1's encode
stage and the display gates are not possible on this image either.

## 3. The bench script and the backend switch

`caraxes/tools/bench-v1vm.sh` now takes v1's material from a Leandro checkout (repo-split.md section 2.3):

- **Driver.** `tools/lib/driver.sh` `caraxes_driver_select` runs on `$LEA_V1_ROOT/DRIVER_VERSION`, and
  `CARAXES_DRIVER=610|615` overrides it (`bench-v1vm.sh:61`). `up` and `modules` refuse when the running host
  driver (a read of `/proc/driver/nvidia/version`, parsed as nvrm-sys `version.rs:17-23`) is not the selected
  version (`check_host`, `bench-v1vm.sh:96-103`).
- **Guest modules.** They come from `$LEA_V1_ROOT/guest-module/{nvrm_nodes,virtio_nvrm}`
  (default `$ROOT/../Leandro`; on 2026-09-24 the worktree `/mnt/vmstore/leandro/wt-615`). `nvrm_wire.h` must
  carry the selected version (`check_sources`, `bench-v1vm.sh:109-116`). `modules` does what was done by hand
  that night, in the order of Leandro-Test's `lea_guest_build_nvrm` (`scripts/lib/provision.sh:1157-1225`):
  1. tar the sources into the guest;
  2. copy the host's params;
  3. build both modules for the running guest kernel;
  4. check the vermagic and the version string in `virtio_nvrm.ko`;
  5. unload any loaded copies;
  6. load `nvrm_nodes` with `create_nodes=0`, provision the params, then load `virtio_nvrm`, which creates
     `/dev/nvidia*`;
  7. check `/dev/nvidiactl`.

  It also refuses when one of the guest's own `nvidia*` modules is loaded (`bench-v1vm.sh:207-241`).
- **Image.** The default is `$CARAXES_IMAGES/guest-v1-2026-09-24-615.71.09.qcow2` and its seed for 615, and
  `caraxes_driver_image` (the baked 610.57.04 image) and the v1check seed for 610 (`bench-v1vm.sh:65-74`).
  `LEA_V1_BASE` and `LEA_V1_SEED` override them. `up` refuses an overlay in `$V` that is not on the selected base
  (`check_overlay`, `bench-v1vm.sh:118-126`). So the run's `bench-v1-615` directory, which is on the base image,
  is refused. The default `V` for 615 is `/mnt/vmstore/caraxes/bench-v1-615.71.09`.
- **Host binary.** `NVRM` (default `$LEA_V1_BIN/vhost-user-nvrm`, with `LEA_V1_BIN` defaulting to
  `$LEA_V1_ROOT/target/release`; on 2026-09-24 `/mnt/vmstore/leandro/l615-host-bin-8c0d2b2`). After the start,
  `check_backend_log` (`bench-v1vm.sh:129-141`) reads the binary's own report. `serve()` prints
  `driver X ABI` after `detect()` (Leandro `crates/vhost-user-nvrm/src/nvrm.rs:1283-1285`), and it exits before
  binding the socket with `driver X is not supported` (`crates/nvrm-sys/src/version.rs:48-59`). Both cases, and an
  ABI that differs from the running driver, stop `up` with the binary's name. `down` stops the binary recorded in
  `$V/backend.bin`.
- **610.** The 610 behaviour is still reachable through the same variables:
  `LEA_V1_ROOT=<Caraxes or a Leandro L610 checkout> CARAXES_DRIVER=610 NVRM=$ROOT/target/bench/release/vhost-user-nvrm`.
  It is refused while the host runs 615.
- **Dry run.** `bench-v1vm.sh -n <step>` (or `BENCH_V1_DRY_RUN=1`) prints the commands. `bench-v1vm.sh config`
  prints the resolved paths and warns about missing files.

**LEA_MANAGED_COMPAT.** Leandro-Test's gpu gate starts a fresh backend for every run with
`LEA_DEBUG=1 LEA_MANAGED_COMPAT=1 lea_rig_up` (`scripts/test.sh:180-187`). `lea_backend_start` passes the switch
explicitly to `vhost-user-nvrm` (`scripts/lib/rig.sh:595-596,687`). The backend reads it once
(`crates/vhost-user-nvrm/src/session.rs:406-410`). It affects only the managed-only UVM commands 42-47 and 51: with
the switch on they complete as no-ops on the external range, and without it they go to the real UVM, which
refuses them with 0x1e (`session.rs:1671-1678`, the `FakeManaged` branch at `:1836` and `:1931`). No other path
reads it. Leandro-Test's suites derive the managed expectation from the backend's `/proc/<pid>/environ`
(`probe/run/suites.sh:76-90`). Its README records the pass and fail pair (`probe/suites/README.md:160-172`).

`bench-v1vm.sh` now starts the backend the same way: `LEA_MANAGED_COMPAT=1` by default, passed explicitly with
`env` so that `suites.sh` sees it in the environment (`bench-v1vm.sh:84,172`), and recorded in `$V/backend.env`.
None of the bench metrics (`bench-cu`, nvprobe, the Python workloads) allocates managed memory, so the switch does
not change what the bench measures. It only makes `gpu-kernel` and `cuda-managed` pass without a restart.
`LEA_MANAGED_COMPAT=` (empty) or `0` gives the old start. The gate's `LEA_DEBUG=1` is not copied, because every
log line in the hot path is a measurement error (`rig.sh:603-605`). Other `LEA_*` host switches pass through from
the caller's environment, as before.

## 4. Boot and gates (run on 2026-09-24, 18:12 to 18:24)

From the main Caraxes checkout (the default `CH` lives there), in bash, with the tap `caraxes5` present:

```
cd /home/silas/git/Caraxes
export LEA_V1_ROOT=/mnt/vmstore/leandro/wt-615 LEA_V1_BIN=/mnt/vmstore/leandro/l615-host-bin-8c0d2b2
caraxes/tools/bench-v1vm.sh config                  # driver 615 (615.71.09), host 615.71.09, base guest-v1-...
caraxes/tools/bench-v1vm.sh prepare                 # V=/mnt/vmstore/caraxes/bench-v1-615.71.09
caraxes/tools/bench-v1vm.sh up                      # "driver 615.71.09 ABI", LEA_MANAGED_COMPAT=1
caraxes/tools/bench-v1vm.sh modules                 # built for 6.8.0-136-generic, carries 615.71.09, /dev/nvidiactl
caraxes/tools/v1-gates.sh caraxes/runs/v1-615-image-2026-09-24 all
caraxes/tools/bench-v1vm.sh down
```

Expected: `smi PASS` without touching `nvidia-modprobe`, and 14 of 14 cuda rows pass, `gpu-kernel` and
`cuda-managed` included, without a backend restart. `v1-gates.sh` exits with the number of failed rows.

Result (`caraxes/runs/v1-615-image-2026-09-24/run.log`, `smi/`, `cuda/rows.tsv`): `smi PASS`, nvidia-smi
identical to the native host run with the masks, driver 615.71.09, the guest sees "Leandro VirtIO RTX 2070";
14 of 14 cuda rows pass, `gpu-kernel` and `cuda-managed` included (managedprobe stage 2, bad=0), rlprobe
mean10 56.9, convburn acc 3.041451216; `v1-gates.sh` exit 0; the VM came down cleanly. The run shared the card
with vm1 (1 GiB, slot 4) and the host desktop.
