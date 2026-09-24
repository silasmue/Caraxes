<!-- SPDX-License-Identifier: MIT -->
<!-- Written 2026-09-24 for the host driver upgrade 610.57.04 -> 615.71.09 (decision D19 step 3,
     repo-split.md step W). Nothing in this file was run against the host GPU by its author. -->
# Cut-over to host driver 615.71.09: the checklist for tonight

The operator upgrades the host NVIDIA driver on this machine from 610.57.04 to 615.71.09 tonight.
From then on 615 is the target ABI (README.md D19). This page is the operator's and the
coordinator's list, in order. Every step says who does it and what counts as done. Agents do not
touch host GPU state and do not start VMs unless the coordinator assigns one.

What is prepared (2026-09-24):

| Piece | Where | State |
|---|---|---|
| Host-side layouts follow the running host driver | `crates/vfio-user-nvgpu/src/hostabi.rs` | done; see §0 |
| The 615 driver sources for the tools | `CARAXES_OGKM=/mnt/vmstore/caraxes/ogkm-615.71.09` | done; gsp-surface.md "615.71.09" |
| A standalone 615 guest base image | `/mnt/vmstore/caraxes/images/guest-baked-2026-09-24-615.71.09.qcow2` | done: sha256 `08b2680c329483a637461b7dec963cff7a255447834480f62ac5e673b9e9bdb9`, no backing file, `qemu-img check` clean; GL, encode, decode and ffmpeg missing (guest-driver-versions.md §8) |
| Launcher switch | `CARAXES_DRIVER=610\|615` in `tools/caraxes-vm.sh` | done: `tools/lib/driver.sh` maps it to the version, base image and `CARAXES_GUEST_ABI`, and the launcher's `check_base` refuses an overlay on the other driver's image; the default stays 610 (`DRIVER_VERSION`) |
| The device's 615 GUEST transport (msgq v2, GMCAPI init) | `ver/v615` | not done (another agent); a 615 guest does not boot until it lands |

## 0. What changed in the device, and what to look for in its log

The device is a client of two drivers: the guest's (layouts the guest reads and writes, fixed at
build time by `ver/v610`) and the host's (layouts of the calls the device makes as an
unprivileged RM client). Only the second changes tonight. `hostabi.rs` picks the host layouts at
start-up, before the first host RM client opens:

- `CARAXES_HOST_ABI=610|615` if set (an explicit override; the log names the running driver
  beside it and says OVERRIDES when they differ);
- otherwise the version in `/proc/driver/nvidia/version`, matched exactly: `610.57.04` or
  `615.71.09`;
- anything else, or an unreadable file, ends start-up with a message naming the version found
  and the two supported ones. Nothing is approximated.

What differs on the host side, measured from the generated bindings (`nvrm_sys::v610`,
`nvrm_sys::v615`; `hostabi.rs` tests pin both):

| Struct | 610.57.04 | 615.71.09 | Device call |
|---|---:|---:|---|
| `NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS` | 20 | 28 | host TSG for the invalidator and every mirrored TSG (`invalidate.rs`, `mirror.rs`) |
| `NV2080_CTRL_GPU_GET_PES_INFO_PARAMS` | 56 | 76 (`tpcToPesMap[15]`) | floorsweeping masks (`kgr.rs` `floorsweeping`) |
| `NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS` | 9240 | 10264 (`virtualDpcId` per SM) | global SM order (`kgr.rs` `global_sm_order`, translated field by field) |
| `NV_CHANNEL_ALLOC_PARAMS` | 376 | 376 | unchanged; equality asserted at compile time |

The other §3.4 rows of blackwell-gb206.md (FIFO device-info table, KGR SM order and
floorsweeping, PCIe P2P caps, NVLink device info) are guest-facing: the device builds them for
the guest and never sends them to the host, so they stay on the guest's 610 layout.

Also changed: `HostRmClient::open` and `HostMemory::open` no longer call `RmClient::new`, whose
`assert_driver_version` would panic on 615 because the build's `DRIVER_VERSION` is still
610.57.04. They open only after the selection succeeded (`hostabi::selected`).

The line to look for in `$W/gpu.log` (`W=/mnt/vmstore/caraxes/<vm>`), printed once when the device
starts with `CARAXES_HOST_RM=1`, `CARAXES_RESERVE_VRAM=1` or `CARAXES_DMA=register`:

```
vfio-user-nvgpu: host ABI 615.71.09: TSG params 28 bytes, GPU_GET_PES_INFO 76 (tpcToPesMap[15]), GR_GET_GLOBAL_SM_ORDER 10264 (SM entry 20), channel params 376 (chosen by running driver 615.71.09 (/proc/driver/nvidia/version)); guest ABI 610.57.04
```

Before the upgrade the same line says `host ABI 610.57.04: TSG params 20 bytes, GPU_GET_PES_INFO
56 (tpcToPesMap[10]), GR_GET_GLOBAL_SM_ORDER 9240 (SM entry 18)`.

## (a) Before the upgrade (coordinator, then operator)

1. **Freeze the 610 state.** Every agent commits or parks its work; note the Caraxes commit
   (`git rev-parse HEAD`) in the run directory below. Push Caraxes `main` (memory: push after
   every checkpoint).
2. **The quiet 610 closing benchmark** (coordinator; the last 610 numbers, benchmark.md §2
   method). With exactly one v2 VM up on the 610 image and no other VM, no desktop GPU load:

   ```
   OUT=caraxes/runs/bench-610-closing-2026-09-24
   V2_RESTART=1 caraxes/tools/bench-run.sh series --targets "native v2" --out $OUT --reps 5
   caraxes/tools/bench-summary.py $OUT/native $OUT/v2 > $OUT/summary.md
   ```

   Add `v1` to `--targets` only if the v1 VM is also up (then nothing else). Record
   `/proc/driver/nvidia/version` in `$OUT/driver.txt`. The P13 numbers before and after the
   window are not comparable (repo-split.md step W); every later run records its driver.
3. **All VMs down.** `tools/caraxes-vm.sh down` for every Caraxes VM (`VM=vm0 ... vm3`, including
   `vm3/g615`), the v1 VM (`caraxes/tools/bench-v1vm.sh down`), and
   check: `pgrep -a cloud-hypervisor; pgrep -a vfio-user-nvgpu; pgrep -a vhost-user-nvrm` print
   nothing. No process may hold `/dev/nvidia*` except the desktop
   (`sudo fuser -v /dev/nvidia*`, operator).
4. **Frozen binaries and their sha256** (coordinator). Build and copy, then hash:

   ```
   F=/mnt/vmstore/caraxes/frozen-610-2026-09-24
   mkdir -p $F
   cargo build --release -p vfio-user-nvgpu            # the device, with hostabi.rs
   cp target/release/vfio-user-nvgpu $F/
   cp "${LEA_CH:-/mnt/vmstore/leandro/cloud-hypervisor/bin/cloud-hypervisor}" $F/  # caraxes-vm.sh:59
   cp -r target/bench/bin $F/bench-bin
   git rev-parse HEAD > $F/caraxes-commit.txt
   (cd $F && find . -type f ! -name SHA256SUMS -exec sha256sum {} + | sort -k2 > SHA256SUMS)
   ```

   The device binary serves both host drivers, so the same file is the rollback binary and the
   first 615 binary. v1's `vhost-user-nvrm` and `libnvrm_trace.so` are built for one version
   (`assert_driver_version`); freeze the 610 ones next to them (repo-split.md step L2 builds
   the L615 set).
5. **Host facts before** (operator; read-only): `cat /proc/driver/nvidia/version`,
   `nvidia-smi -q > $F/nvidia-smi-610.txt`, `sha256sum /usr/lib/firmware/nvidia/610.57.04/*`,
   `dmesg | grep -iE 'NVRM|nvidia|Xid' | tail -50`. Keep them in `$F`.

## (b) The operator's upgrade

The operator installs 615.71.09 and reboots. Afterwards, read-only:

1. `cat /proc/driver/nvidia/version` names `615.71.09` (open kernel modules).
2. `modinfo nvidia | grep -E '^(version|license|firmware)'`: version 615.71.09, licence
   `Dual MIT/GPL`, firmware `nvidia/615.71.09/gsp_*.bin`.
3. `nvidia-smi`: driver 615.71.09, the RTX 2070 at its usual bus address, the desktop's
   processes only. `nvidia-smi -q > $F/nvidia-smi-615.txt`.
4. Firmware files: `ls -l /usr/lib/firmware/nvidia/615.71.09/` has `gsp_tu10x.bin`,
   `gsp_ga10x.bin`, `ucodes_tu10x.bin`, `ucodes_ga10x.bin`; `sha256sum` them into `$F`. The
   lab capture's 615 set is in blackwell-gb206.md §1.5 (sizes 22897536 for `gsp_tu10x.bin`);
   the host's package may differ in build, so compare sizes, not hashes.
5. `dmesg`: no `NVRM:` errors, no Xid, GSP booted (`nvidia-smi -q` shows the GSP firmware
   version 615.71.09). The dmesg flood of 610 came from `NVreg_ResmanDebugLevel=0` on the kernel
   command line (memory: host-dmesg-flood-cause); check whether it is still wanted.
6. `tools/build.sh check-driver`: running 615.71.09. The target (`DRIVER_VERSION`) and the
   vendored tree still say 610.57.04 until the Caraxes pin moves (repo-split.md C3); that
   mismatch is expected tonight and does not affect the device (§0).

## (c) First test against the real 615 driver: the 610 guest on the 615 host

Coordinator, one VM, on the frozen device binary. The guest is unchanged (610 image,
`CARAXES_DRIVER=610`, the default).

1. **Device.** `cargo build --release -p vfio-user-nvgpu` (or `CARAXES_SERVER=$F/vfio-user-nvgpu`).
   No rebuild is needed for the host side; rebuild only if the tree moved since (a).
2. **Up**: `VM=vm0 CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 tools/caraxes-vm.sh up` and `wait`,
   with the same flags as the last 610 gate run (`CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
   CARAXES_HOST_EXEC=1` for the CUDA gate). Then run `tools/caraxes-vm.sh sync-tests`, so
   that the guest has the current test material.
3. **Log first**: the §0 line in `$W/gpu.log` says `host ABI 615.71.09` and `chosen by running
   driver 615.71.09`. Then, from the guest's module load:
   - `kgr floorsweeping masks from host: gpcMask 0x7, tpc mask/count per GPC [...]`: three
     GPCs as on 610 (TU106); a `host status 0x...` here means a PES or GPC control failed;
   - `kgr global SM order: 36 SMs from host` (TU106 has 36 SMs); `zeros (host status ...)` means
     the 10264-byte call was refused;
   - `host TSG ... [host]` and `inv-tsg` without `KEPLER_CHANNEL_GROUP_A: ...` errors: the
     28-byte TSG parameters are accepted;
   - no `unanswered` control that the 610 run answered (compare with the last 610 `gpu.log`).
4. **nvml gate**: `tools/caraxes-gate.sh nvml`. It compares the guest's nvidia-smi (610) with
   the host's nvidia-smi (now 615). The name and 4096 MiB come from the device; temperature,
   power and utilization are forwarded. Watch the BINAPI path (`nvml.rs`, `CMD_BINAPI_0108`):
   it forwards an undocumented control and is the most likely place for a 615 difference.
5. **cuda gate**: `tools/caraxes-gate.sh cuda`. The first CUDA context exercises the TSG and
   channel allocations, the SM order and the floorsweeping masks.
6. **display gate**: restart with `CARAXES_DISPLAY=1`, then `tools/caraxes-gate.sh vdisplay`
   and `display`. The display is emulated; the host driver is only reached through the FB and
   host execution, so a failure here that the 610 run did not have points at host execution.
7. **Explicit override test** (optional): `CARAXES_HOST_ABI=610` must log `OVERRIDES the
   running driver 615.71.09` and is expected to fail at the first TSG allocation; it shows the
   selection is what makes the difference. Do not keep it.
8. Record every verdict with the driver in `caraxes/runs/cutover-615-2026-09-24/`.

## (d) The 615 guest, when the transport lands

Merge note (13:10): the 615 guest transport (msgq v2, GMCAPI init, `ver/v615`) and the
host-ABI selection (`hostabi.rs`) are both on main now, merged from two branches. Their
SM-order paths in kgr.rs meet as follows: a 615 guest on a 610 host takes the widened path
(guest entry 20, host entry 18), a 610 guest on a 615 host takes `HostLayouts`; a 615 guest
on a 615 host still uses the widened path with 610 host constants and needs the two paths
joined (open item 1 of [gsp-transport-615.md](gsp-transport-615.md)). Tonight's first test
is the 610 guest, which is unaffected.

Only after `ver/v615` (msgq v2 with register-held pointers, MCTP-framed elements,
`GMCAPI_CMD_GSP_INIT` with NVKV static info; guest-driver-versions.md §5.4) is merged:

1. **Fresh state folder.** A 615 VM gets a new state directory on the new image. An existing
   `rootfs.qcow2` is an overlay on the 610 base, and the launcher's `check_base`
   (`tools/caraxes-vm.sh:170`) refuses it. Create the overlay and a seed with a new cloud-init
   instance id, so that cloud-init runs again on the new image:

   ```
   D=/mnt/vmstore/caraxes/vm4
   mkdir -p $D
   qemu-img create -f qcow2 -F qcow2 \
       -b /mnt/vmstore/caraxes/images/guest-baked-2026-09-24-615.71.09.qcow2 $D/rootfs.qcow2
   ```

   Then build `seed.img` as for any new VM, with a new `instance-id` in its meta-data.
2. `VM=vm4 CARAXES_DRIVER=615 tools/caraxes-vm.sh up`. The launcher selects the 615 base image
   through `tools/lib/driver.sh`, checks that the overlay sits on it, and sets
   `CARAXES_GUEST_ABI=615`.
3. **`tools/caraxes-vm.sh sync-tests` in every VM** before any gate. The test material was
   built for 610, and a fresh image has none of it.
4. The same gates as (c). The device log then says `guest ABI 615.71.09`.
5. The image is missing `libnvidia-gl-615`, `libnvidia-encode-615` and
   `libnvidia-decode-615`, and ffmpeg (guest-driver-versions.md §8). Until the operator fetches
   them (the host downloads them with plain GETs, as in
   `caraxes/runs/guest-driver-2026-09-24/commands.txt`), `gpu-encode` and the GL/EGL/Vulkan
   rows fail on 615. That is expected, not a device fault.

Until then `CARAXES_DRIVER=615` boots a guest that stops at the status-queue link
(guest-driver-versions.md §5), exactly as `vm3/g615` did.

## (e) v1 (Leandro)

v1 forwards ioctls verbatim, so its guest module and user-space libraries must be the host's
version. After the upgrade the 610 v1 guest must not be started against the 615 host: v1 stays
down until Leandro's L615 step (repo-split.md step L2: `DRIVER_VERSION` 615.71.09,
`nvrm_wire.h` regenerated, `vhost-user-nvrm` built with `v615`) and a v1 615 image (step G,
`nix build .#guest-image` at the L615 pin) exist. The v1 benchmark path
(`caraxes/tools/bench-v1vm.sh`) is off until then.

## (f) Rollback

Rolling back is the operator's call; no host state was changed by the agents.

1. All VMs down (as in (a) 3).
2. The operator reinstalls 610.57.04 and reboots; check (b) 1 to 5 with 610.57.04.
3. The frozen binaries in `$F` still work: the device selects the 610 host layouts by itself
   (`host ABI 610.57.04` in `gpu.log`); v1 uses its frozen 610 binaries.
4. Guests: `CARAXES_DRIVER=610` (the default) and the unchanged 610 image.

If only the device misbehaves on 615 and the host driver is fine, keep the driver and stop the
VMs; the device refuses nothing on 615 that it accepts on 610, so a device fault is a bug in a
host call, not a selection problem.

## Open points

1. The build default `DRIVER_VERSION` (and `nvrm_sys::DRIVER_VERSION`, `assert_driver_version`)
   still says 610.57.04. Every other host tool that calls `RmClient::new` or
   `assert_driver_version` panics on the 615 host until the pin moves (repo-split.md C3):
   the `nvrm-client` binaries (`fbclients`, `vgpuprofile`, `e1-extmap`, `mmapping`,
   `classlist`, `smipids`) and the RM query tools in `caraxes/tools`. `lab-abi-check.sh`
   against the 615 tree reports `GPU_GET_PES_INFO` 56 to 76 and `FIFO_GET_DEVICE_INFO_TABLE`
   3212 to 3340, so `fifo-host-query.rs` and the tools built on `lab-abi-610.57.04.txt` need
   615 layouts before they run on the new host.
2. The BINAPI control the nvml path forwards (`CMD_BINAPI_0108`) has no generated layout; its
   615 behaviour is only known after (c) 4.
3. `GPU_GET_VMMU_SEGMENT_SIZE` is answered by the 615 kernel itself (gsp-surface.md
   "615.71.09"); the device's host-forward branch (`ctrl.rs:996`) still works, it only no longer
   reaches GSP on the host.
4. The 0x20800A36 id reuse (`INTERNAL_GPU_GET_CHIP_INFO` on 610, `SYNC_FABRIC_PROBE_INFO` on
   615) is a guest-ABI matter for `ver/v615`, not for tonight.
5. The first 615 benchmark (P13 after the window) needs its own series; do not merge it with the
   closing 610 one.
