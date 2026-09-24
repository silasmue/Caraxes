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
| `NV2080_CTRL_FB_FLUSH_GPU_CACHE_PARAMS` `flags` | APERTURE 1:0, WB 2, INV 3, FULL_CACHE 4 | APERTURE 2:0, WB 3, INV 4, FULL_CACHE 5 | the guest's L2 operations (`boot.rs` `l2_op`); the struct is 4024 bytes on both, the flag bits moved |
| GSS legacy pmgr 0x18 (0x2080A618) | 20000 | 20512 | nvidia-smi power (`nvml.rs` `host_sized_legacy`); no layout in any open header, forwarded only when the guest's size is the host's |
| GSS legacy thermal 0x36 (0x20808536) | 25552 | 24532 | nvidia-smi thermal (the same rule) |
| GSS legacy perf 0xf1 (0x2080A0F1) | not sent | 9216 | nvidia-smi of a 615 guest (the same rule; NOT_SUPPORTED on the 610 host) |

The other §3.4 rows of blackwell-gb206.md (FIFO device-info table, KGR SM order and
floorsweeping, PCIe P2P caps, NVLink device info) are guest-facing: the device builds them for
the guest and never sends them to the host, so they stay on the guest's 610 layout.

Also changed: `HostRmClient::open` and `HostMemory::open` no longer call `RmClient::new`, whose
`assert_driver_version` would panic on 615 because the build's `DRIVER_VERSION` is still
610.57.04. They open only after the selection succeeded (`hostabi::selected`).

The line to look for in `$W/gpu.log` (`W=/mnt/vmstore/caraxes/<vm>`), printed once when the device
starts with `CARAXES_HOST_RM=1`, `CARAXES_RESERVE_VRAM=1` or `CARAXES_DMA=register`:

```
vfio-user-nvgpu: host ABI 615.71.09: TSG params 28 bytes, GPU_GET_PES_INFO 76 (tpcToPesMap[15]), GR_GET_GLOBAL_SM_ORDER 10264 (SM entry 20), channel params 376, GSS legacy [pmgr 0x18 20512, thermal 0x36 24532, perf 0xf1 9216] (chosen by running driver 615.71.09 (/proc/driver/nvidia/version)); guest ABI 610.57.04
```

Before the upgrade the same line says `host ABI 610.57.04: TSG params 20 bytes, GPU_GET_PES_INFO
56 (tpcToPesMap[10]), GR_GET_GLOBAL_SM_ORDER 9240 (SM entry 18), channel params 376, GSS legacy
[pmgr 0x18 20000, thermal 0x36 25552]`. A device binary built before the join of section (d)
prints the line without the `GSS legacy [...]` part.

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
   - `kgr global SM order: 36 SMs from host` (TU106 has 36 SMs), followed by `(host 615.71.09
     entry 20 to guest entry 18, dropped virtualDpcId)` in a binary with the join of (d);
     `zeros (host status ...)` means the 10264-byte call was refused;
   - `host TSG ... [host]` and `inv-tsg` without `KEPLER_CHANNEL_GROUP_A: ...` errors: the
     28-byte TSG parameters are accepted;
   - no `unanswered` control that the 610 run answered (compare with the last 610 `gpu.log`);
   - the guest's L2 operations (from the module load on, and in every CUDA and display
     gate): `l2 FlushDirty: FB_FLUSH_GPU_CACHE flags 0x29 NV_OK in ... us [host GPU]` and
     `l2 SysmemInvalidate: FB_FLUSH_GPU_CACHE flags 0x31 NV_OK in ... us [host GPU]`, and no
     `FB_FLUSH_GPU_CACHE flags ... failed: status 0x1f`. On the 610 host the words are 0x15
     and 0x19. The host kernel log of the run (`sudo dmesg`, read only) must be free of
     `kmemsysFlushGpuCache_IMPL: Invalid aperture` and `Invalid array size`. Those two came
     in pairs, one per forwarded flush, in the first 615 run
     (`caraxes/runs/cutover-615-2026-09-24/v2-610guest/`, every flush of both operations
     refused with 0x1f), because a device built before the fix sent the 610 flag
     word: 615 widens `_FLAGS_APERTURE` from 1:0 to 2:0 and moves WRITE_BACK, INVALIDATE and
     FLUSH_MODE up by one bit (610 `ctrl2080fb.h:681-696`, 615 `ctrl2080fb.h:682-699`). The
     handler's checks are unchanged (610 `kern_mem_sys_ctrl.c:1464-1547`, 615
     `kern_mem_sys_ctrl.c:1375-1466`), so 0x15 read as aperture 5 and 0x19 as an evict in
     ADDRESS_ARRAY mode with no addresses. The device now takes the words from the host
     layouts (`hostabi.rs` `HostLayouts::flush_gpu_cache`, test `flush_gpu_cache_per_host`).
4. **nvml gate**: `tools/caraxes-gate.sh nvml`. It compares the guest's nvidia-smi (610) with
   the host's nvidia-smi (now 615). The name and 4096 MiB come from the device; temperature,
   power and utilization are forwarded. Watch the BINAPI path (`nvml.rs`, `CMD_BINAPI_0108`):
   it forwards an undocumented control and is the most likely place for a 615 difference.
   Expected loss with a binary that has the join of (d): the 610 guest's pmgr 0x18 (20000
   bytes) and thermal 0x36 (25552) do not match the 615 host's 20512 and 24532, no open header
   defines either layout, so the device answers them NOT_SUPPORTED instead of forwarding
   (`ctrl 0x2080a618: GSS legacy pmgr 0x18: guest params 20000 bytes, host 615.71.09 takes
   20512; not sent -> status 0x56 [synthesized (NOT_SUPPORTED: GSS legacy params differ ...)]`).
   The guest's power draw then reads N/A, and the gate's `power-number` check fails for this
   combination only; that is the documented loss, not a regression. An older binary forwards
   them and the host refuses with 0x1f (`ERR!` in nvidia-smi), with the same verdict.
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
host-ABI selection (`hostabi.rs`) are both on main now, merged from two branches.

Join (evening, this section's checks below): the host-facing items of open item 1 of
[gsp-transport-615.md](gsp-transport-615.md) are done in code, for every combination of a 610
or 615 guest (`ver::active`, build time) on a 610 or 615 host (`hostabi::current()`, start-up):

- **SM order.** One path, `kgr.rs` `global_sm_order`: the host call is sized by the host
  (`HostLayouts::sm_order`, 9240 or 10264 bytes), the reply is translated field by name into the
  guest's element (`ver::active::SM_ORDER_GUEST`, nine u16 per SM on 610, ten on 615;
  `hostabi::translate_sm_order`). `virtualDpcId` is dropped for a 610 guest and 0 for a 615
  guest on a 610 host. The widened path (`sm_order_widened`, 610 host constants) is gone. Tests:
  `hostabi::tests::sm_order_all_four_combinations`, `kgr::tests::global_sm_order_all_four_combinations`
  (all four from the generated `nvrm_sys::v610`/`v615` sizes and offsets, in either build).
  The guest's floorsweeping `tpcToPesMap` length follows the guest ABI too (10 or 15,
  `FS_TPC_TO_PES_MAP_LEN`).
- **pmgr 0x18 and thermal 0x36.** No layout for either in the open headers of 610 or 615
  (matrix/catalog-610.57.04.md:384, :419; no definition in the 615 tree), so a field-by-field
  translation is not possible. `HostLayouts::legacy` carries each host driver's size (610 from
  the 610 host's own nvidia-smi, e4 trace; 615 from the 615 guest's nvidia-smi, transport-615
  run). Same size: forwarded as before. Different size: NOT_SUPPORTED, not sent (`nvml.rs`
  `host_sized_legacy`). The loss: power draw (pmgr 0x18) and the thermal 0x36 reading are N/A
  whenever guest and host driver differ; both work when they are the same version.
- **perf 0xf1 (0x2080A0F1, 9216 bytes).** In the 615 host's table: forwarded on a 615 host,
  NOT_SUPPORTED with the label `synthesized (NOT_SUPPORTED: no such GSS legacy control measured
  for the host driver)` on a 610 host.
- **FB_GET_REMAPPED_BANKS (0x20801361, 8208 bytes).** Not new in 615: 610.57.04 has the same
  control and layout (610 `ctrl2080fb.h:2990-3014`, `nvrm_sys::v610` and `v615` both 8208,
  NON_PRIVILEGED and ROUTE_TO_PHYSICAL in both `g_subdevice_nvoc.c`). It is therefore in the
  `FORWARD` table for both hosts (decision D9: forward where the layout is the same on both
  sides); whatever the host answers goes back to the guest.
- **0x20800A36.** Dispatched by the guest ABI (`ver::Id0A36`, `CTRL_0A36`, `ctrl.rs`
  `id_0a36`): chip info for a 610 guest; for a 615 guest `INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO`
  (16 bytes), answered NV_OK as an echo (no NVLink fabric on TU106, nothing to the host).

### Checks on the 615 host

For the 615 guest on the 615 host (vm4 below, `CARAXES_DRIVER=615`, `CARAXES_HOST_RM=1`, and for
the cuda gate `CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register CARAXES_HOST_EXEC=1`). Every `ctrl`
line is the device's per-control line, `ctrl <cmd>: <note> -> status <st> [<label>]`
(`boot.rs:929`), in `$W/gpu.log`.

| What | Gate | Line that verifies it |
|---|---|---|
| selection | start-up, before any gate | `host ABI 615.71.09: ... GR_GET_GLOBAL_SM_ORDER 10264 (SM entry 20), channel params 376, GSS legacy [pmgr 0x18 20512, thermal 0x36 24532, perf 0xf1 9216] (chosen by running driver 615.71.09 ...); guest ABI 615.71.09` |
| SM order, 615 on 615 | module load, then `tools/caraxes-gate.sh cuda` (the first CUDA context reads it) | `ctrl 0x20800a22: kgr global SM order: 36 SMs from host (host 615.71.09 entry 20 to guest entry 20) -> status 0x0 [host (GR_GET_GLOBAL_SM_ORDER, translated field by field from the host's layout)]`; no `dropped` and no `0 (not in the host's)` in it; `zeros (host status ...)` is a failure |
| floorsweeping map | module load, cuda gate | `kgr floorsweeping masks from host: gpcMask 0x7, tpc mask/count per GPC [...]` with three GPCs, as in (c) 3 |
| pmgr 0x18 | `tools/caraxes-gate.sh nvml`, check `power-number` passes | `ctrl 0x2080a618: GSS legacy pmgr 0x18 (20512 bytes, host 615.71.09): host status 0x0 -> status 0x0 [host (GSS legacy, forwarded opaquely; the running host driver's size)]` |
| thermal 0x36 | nvml gate (`nvidia-smi -q`) | `ctrl 0x20808536: GSS legacy thermal 0x36 (24532 bytes, host 615.71.09): host status 0x0 -> ...` |
| perf 0xf1 | nvml gate | `ctrl 0x2080a0f1: GSS legacy perf 0xf1 (9216 bytes, host 615.71.09): host status 0x...`; whatever the status, the label is `host (...)`, not `unanswered` |
| FB_GET_REMAPPED_BANKS | nvml gate | `ctrl 0x20801361: FB_GET_REMAPPED_BANKS: host status 0x... -> ... [host (forwarded as-is, public control)]`, not `unanswered` |
| 0x20800A36 | module load (only sent on NVLink-fabric systems; the transport-615 run never sent it) | if present: `ctrl 0x20800a36: sync fabric probe info (16 bytes): nothing to apply, no NVLink fabric -> status 0x0`; `grep 'ctrl 0x20800a36' $W/gpu.log \| grep -c 'chip info'` prints 0 |
| nothing new unanswered | after the gates | `grep 'unanswered' $W/gpu.log` names none of 0x2080a618, 0x20808536, 0x2080a0f1, 0x20801361 |

The other combinations, when they are run: a 610 guest on the 615 host is (c), with the SM-order
note `(host 615.71.09 entry 20 to guest entry 18, dropped virtualDpcId)` and pmgr 0x18 and
thermal 0x36 NOT_SUPPORTED (c) 4. A 615 guest on the 610 host (after a rollback, (f)) logs
`(host 610.57.04 entry 18 to guest entry 20, virtualDpcId 0 (not in the host's))`, pmgr 0x18 and
thermal 0x36 NOT_SUPPORTED (`guest params 20512 bytes, host 610.57.04 takes 20000; not sent`),
and perf 0xf1 NOT_SUPPORTED (`not in host 610.57.04's own nvidia-smi; not sent`).

Offline, without VM or GPU, both builds: `CARGO_TARGET_DIR=target/h615 cargo test -p
vfio-user-nvgpu` and the same with `--features v615` run the combination tests above
(`hostabi::`, `kgr::`, `nvml::tests::host_sized_legacy_follows_the_host`,
`ctrl::tests::id_0a36_follows_the_guest_abi`). In the `v615` build six `msgq::tests` fail by
construction: they drive the message queue v0 with the 615 element header size that `msgq.rs`
imports from `ver::active` (24 instead of 16), unchanged by this work.

### Bringing up the 615 guest

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

### The 615 guest on the 615 host: runs 4 to 6

State folder `vm2-615` (an overlay on the 615 base image, autoload of nvidia_modeset, nvidia_drm
and nvidia_peermem blocked, `nvidia-persistenced` masked), the v615-feature device binary, token
pool and hybrid reactor, no BAR1 area. Records in `caraxes/runs/cutover-615-2026-09-24/`: run 4
is `v2-615guest.sh` with `v2-615guest/`; runs 5 and 6 are `v2-615guest-run.sh r5|r6` with
`v2-615guest-r5/` and `v2-615guest-r6/`. The run script is run 4's with the device binary of the
checkout it lives in (`setup.txt` names its sha256 and commit) and the per-control lines extracted
from `gpu-trace.log` (`device-log-lce-shim.txt`, `device-trace-issues.txt`; `gpu.log` has no
`ctrl` lines, so run 4's `device-log-issues.txt` is empty).

**Profile change from run 5 on.** Runs 5 and 6 use a 1 GiB profile at reserve slot 3
(`CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=3`, the range
[top-4 GiB, top-3 GiB)), not 2 GiB at slot 0 as run 4 and the 610 guest run, because a 3 GiB VM
takes slot 0 and vm1 holds 1 GiB at slot 4. The cuda gate passed on 1 GiB profiles before (P20,
vm2 at 1 GiB slot 4), so the comparison with the 610 guest stays valid for the scored rows. The
unscored `suite-test_async_streams` asks for 256 MiB more than a 1 GiB card has free and fails
with `torch.OutOfMemoryError`; that is the profile, not the device.

**Run 4 (device 61e0b6a).** `RmInitAdapter succeeded!`, the nvml gate passes, and every cuda stage
fails at `cuInit: 100 no CUDA-capable device is detected` (0 of 12 scored pass,
`v2-615guest/gate-cuda.txt:1-3`). The device refused two controls with 0x56:
`NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO` (0x20802A0C) and `NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS`
(0x2080012F). Which one breaks cuInit:

- `GPU_QUERY_ECC_STATUS` is not the cause. It is in `FORWARD` (`ver/v610.rs`) and the 0x56 is the
  host's answer (`host status 0x56 -> status 0x56 [host (forwarded as-is, public control)]` in
  run 5's `device-trace-issues.txt`): the card has no ECC. The 610 guest gets the same 0x56 and its
  cuInit passes (`v2-610guest/gate-cuda.txt:49`). The struct is the same in both SDKs
  (`ctrl2080gpu.h:1207-1213` in 610, `:1208-1214` in 615, 41 units), so forwarding it is sound on
  either host.
- `CE_GET_LCE_SHIM_INFO` is the cause. The struct is the same 12 bytes in both SDKs
  (`ctrl2080ce.h:351-374`: ceEngineType in, shimInstance and shimLocalLceIdx out), and both route
  it to GSP, but 610 exports it PRIVILEGED (flags 0x145, 610 `src/nvidia/generated/g_subdevice_nvoc.c:11827`)
  and 615 NON_PRIVILEGED (0x149, 615 `g_subdevice_nvoc.c:12008`). The 610 libcuda therefore never
  asked it; the 615 libcuda asks it after `CE_GET_CAPS_V2` for every CE, and on the real host RM it
  succeeds (`v1/cuda/guest/v1-615/cuda-managed.rm.tsv:214-228`, status 0x0, len=12). Its
  implementation is GSP-RM code, not in the open tree; the kernel side keeps every KernelCE in
  shim 0 before Blackwell (615 `src/kernel/gpu/ce/kernel_ce.c:53-56`, the default no-op HAL of
  `g_kernel_ce_nvoc.c:352-362`; only `kceSetShimInstance_GB100`,
  `kernel_ce_gb100.c:105-112`, divides the public id by a stride).

**The answer** (`cuda.rs` `lce_shim_info`): forwarded to the host subdevice when the host driver
exports it NON_PRIVILEGED (`hostabi::HostLayouts::lce_shim_info_public`, 615 true, 610 false,
because the device's client is unprivileged); otherwise from the chip's shim table
(`chips/tu106/chip.toml` `[ce] lce_shim`, one shim: shim 0, local index = LCE index). The device
log line names both, so a disagreement is visible. A non-copy engine type is
NV_ERR_INVALID_ARGUMENT. Tests: `cuda::tests::lce_shim_info_and_ecc_layouts`,
`cuda::tests::lce_shim_info_answer`. After the merge of the E and A rules, `surface.rs` was
regenerated: 0x20802A0C has a handler now (`other: true`), A_INVENTORY 60 to 59 on both ABIs
(gsp-surface.md "After E and A").

| Run | Device | Profile | nvml | cuda gate (scored) | Unscored suites | Refused controls | Host Xid |
|---|---|---|---|---|---|---|---|
| 610 guest (`v2-610guest/`) | e0b3dc9 | 2 GiB slot 0 | pass | 9 pass, 2 fail (cuda-managed 801, robustness 2 of 3), 1 unsupported | 5 pass, 3 skip | ECC_STATUS (host 0x56) | 0 |
| 4 | 61e0b6a | 2 GiB slot 0 | pass | 0 pass, 11 fail (cuInit 100), 1 unsupported | 5 fail, 3 skip | LCE_SHIM_INFO (device 0x56), ECC_STATUS (host 0x56) | 0 |
| 5 | 68a09e7 | 1 GiB slot 3 | pass | 10 pass, 1 fail (cuda-managed 801), 1 unsupported | 4 pass, 1 fail (async_streams, OOM on 1 GiB), 3 skip | ECC_STATUS (host 0x56) | 0 (device pid 143264) |
| 6 | 7f37529 (after the merge of the E and A rules, `surface.rs` regenerated) | 1 GiB slot 3 | pass | 10 pass, 1 fail (cuda-managed 801), 1 unsupported | 4 pass, 1 fail (async_streams, OOM on 1 GiB), 3 skip | ECC_STATUS (host 0x56); no `unanswered`, no family refusal | 0 (device pid 220569) |

Run 5's host answers for CE_GET_LCE_SHIM_INFO: LCE 2, 3 and 4 (ceEngineType 0xB to 0xD), each
shimInstance 0, shimLocalLceIdx = the LCE, 21 times each, `chip table agrees`
(`v2-615guest-r5/device-log-lce-shim.txt`). cuda-managed fails as on the 610 guest (`AllocManaged
a: 801 operation not supported`, the known exception on this host); robustness passed 3 of 3
where the 610 guest run had 2 of 3. rlprobe `mean10=56.9`, convburn `acc=3.041450977e+00`.
Run 6 repeats run 5 with the merged device (the chipctl family hook sits after the CUDA rows, so
it never sees 0x20802A0C): the same 10 of 12, the same host answers, cuda gate 391 s (run 5
426 s, the 610 guest 388 s). During run 6 the host kernel logged 22 pairs of
`kmemsysFlushGpuCache_IMPL: Invalid aperture.` / `Invalid array size (0x0)` (18:34:52 to 18:45);
they are not vm2's: its device sent the 615 flag words (0x29, 0x31) with NV_OK every time, while
vm0's device (pid 253268, another agent's) logs `FB_FLUSH_GPU_CACHE flags 0x8 failed: status 0x1f`,
the 610 word on the 615 host.

**Closed: the bench series** (commit a0bf4ef). `bench-run.sh series` (init, short-auto) failed in
runs 4 to 6 with `cuInit: 803 system has unsupported display driver / cuda driver combination`
within 10 to 35 ms, while the gate's cuInit in the same VM passed. Cause, read from the base
images with `guestfish --ro`: the 615 image's `/opt/nvrm/lib` still holds the 610.57.04
userspace (`libcuda.so.1 -> libcuda.so.610.57.04`), its `nvrm.conf` is renamed
`nvrm.conf.disabled`, and the 615.71.09 libraries sit in `/usr/lib/x86_64-linux-gnu`
(guest-driver-versions.md section 8). The gate's probes run without `LD_LIBRARY_PATH`
(`tools/caraxes-guest-tests.sh:32` sets it only for nvidia-smi), so the linker cache hands them
the 615 libcuda; bench-run.sh exported `LD_LIBRARY_PATH=/opt/nvrm/lib` for every guest
target, so every bench binary loaded the 610 libcuda against the 615 kernel module. The 610
image has the same `/opt/nvrm/lib` and an active `nvrm.conf`, so both paths agree there. Fix:
bench-run.sh takes the v2 directory from `tools/lib/driver.sh` (`CARAXES_GUEST_LIBDIR`, 610
`/opt/nvrm/lib`, 615 `/usr/lib/x86_64-linux-gnu`) like the gates, keeps `/opt/nvrm/lib` for v1
(its 615 image repoints it, (e) below), records the directory in `series.txt`, and
`bench-run.sh libdir v2` prints it; `tools/check.sh`'s bash step checks the three cases.
Verified in `caraxes/runs/bench-615-2026-09-24/615/libcuda.txt` (the harness resolves
`/usr/lib/x86_64-linux-gnu/libcuda.so.1`, the cache `libcuda.so.615.71.09`) and by the first 615
series (benchmark.md "First 615 series"), every run rc 0.

## (e) v1 (Leandro)

v1 forwards ioctls verbatim, so its guest module and user-space libraries must be the host's
version. After the upgrade the 610 v1 guest must not be started against the 615 host: v1 stays
down until Leandro's L615 step (repo-split.md step L2: `DRIVER_VERSION` 615.71.09,
`nvrm_wire.h` regenerated, `vhost-user-nvrm` built with `v615`) and a v1 615 image (step G,
`nix build .#guest-image` at the L615 pin) exist. The v1 benchmark path
(`caraxes/tools/bench-v1vm.sh`) is off until then.

### Result on the 615 host (2026-09-24, 17:28 to 17:40)

v1 runs on 615.71.09 and every gate run passes, with two conditions: `nvidia-smi` needs a guest image fix (below), and
managed memory needs the backend started with `LEA_MANAGED_COMPAT=1`, as the v1 gate starts it. Record, with the
commands, the raw outputs and the harness files: `caraxes/runs/cutover-615-2026-09-24/v1/`
(`commands.txt` first).

What ran:

- **Host binaries.** The frozen L615 set, `/mnt/vmstore/leandro/l615-host-bin-8c0d2b2/`
  (`leandro-commit.txt` 8c0d2b2, `SHA256SUMS` verified). The **615-only** `vhost-user-nvrm` was used. It reported
  `driver 615.71.09 ABI`, tables checksum 0x255c7423. The dual binary was not needed. Nothing was rebuilt on the host.
  The `port/615` worktree is at `/mnt/vmstore/leandro/wt-615` (51c0884, which is 8c0d2b2 plus the record).
- **Guest image.** An overlay on the Caraxes 615 base image (guest-driver-versions.md section 8, sha256 `08b2680c...`):
  `/mnt/vmstore/caraxes/bench-v1-615/rootfs.qcow2`, with its own seed (192.168.105.25, instance id
  `v1-615-1790262142`). The overlay was edited offline and in the guest, nothing else:
  - the guest's own NVIDIA modules are blacklisted and set to `install ... /bin/false`;
  - `nvidia-persistenced` is disabled;
  - `/opt/nvrm/lib` points to the 615 system libraries (`bench-run.sh` sets `LD_LIBRARY_PATH=/opt/nvrm/lib`), and the
    610 copies moved to `/opt/nvrm-610.57.04`;
  - `/usr/bin/nvidia-modprobe` was renamed (see smi).

  The overlay's sha256 after shutdown is in `image-sha256.txt`. The guest modules were built inside the guest from the
  `port/615` sources against 6.8.0-136-generic. `virtio_nvrm.ko` carries 615.71.09. `bench-v1vm.sh` was run unchanged
  with `V` and `NVRM` set. Only its `modules` step was repeated by hand with the worktree's sources, because it tars
  Caraxes' `guest-module`, whose `nvrm_wire.h` still says 610.57.04.

| Gate | Result |
|---|---|
| tables | pass: the guest's checksum equals the host's (0x255c7423), protocol v6 |
| smi | **fail, then pass after an image fix.** The first run failed with `Failed to initialize NVML: GPU access blocked by the operating system` (exit 17), and no ioctl reached the device. Cause: NVML finds no `/proc/driver/nvidia/capabilities/mig/*` (this was already so on 610) and then runs `/usr/bin/nvidia-modprobe`. That fails without `/sys/module/nvidia`, and NVML turns the failure into an OS error. The 610 image had no `nvidia-modprobe`. With it renamed in the overlay, the masked diff against the host's `nvidia-smi` is empty: driver 615.71.09, name `Leandro VirtIO RTX 2070` |
| cuda: nvprobe 0-4 | pass: stages 0 to 4. nvprobe 4 took 16.6 ms per launch for the 30M-cycle kernel. The matrix rows core, jit, launch, hostreg and torch (cuDNN) pass |
| cuda: torch | pass: rlprobe `mean10=56.9`, convburn `acc=3.041451216e+00`. These are the values every 610 run recorded (native, v1, v2). The native reference was not re-run, because that would use the host card |
| managed memory | fail with the bench backend (`AllocManaged: invalid argument`, control 0x2080012f status 0x56). **Pass** with `LEA_MANAGED_COMPAT=1`, which Leandro-Test's gpu gate sets and `bench-v1vm.sh` does not: managedprobe reached stage 2 twice, and the kernel stage's accounting ended at 0 KiB and 0 pool pages |
| robustness (v1 form) | pass: two processes in parallel, SIGKILL without a leak, `rmmod virtio_nvrm` refused while an FD is open |
| own | pass: the guest's PID appears in the guest's `nvidia-smi` |
| bench `short-auto rl`, 2 reps | ran. short-auto 0.008 and 0.021 ms/launch (610: 0.007 [0.007-0.008]). rl `mean10` 56.9 both times, stage4 2.2 and 1.6 s (610: 1.50 [1.40-1.80]), 1284 and 1791 transfers/s, process wall time 6.0 and 5.1 s. vm2 was running, so these are not a clean series (`bench/series.txt`) |

Host: no Xid since the 16:56 boot. The v1 backend PIDs were 35440, then 46217 for the managed retest. The kernel log
has `kmemsysFlushGpuCache_IMPL: Invalid aperture` pairs from 17:31:19 on. These come from vm2's device
(vfio-user-nvgpu 36468, the v2 cut-over, running concurrently): it logs `FB_FLUSH_GPU_CACHE ... failed: status 0x1f`
at the same times, and the first pairs came before v1's first RM call. They are not v1's. The host was only read:
`nvidia-smi` and `/proc/driver/nvidia/params`.

Not run: Leandro-Test's own `scripts/test.sh gpu`, because it needs its vm0 rig and a nix-built 615 guest image
(step G). The encode stage was also not run, because the image has no NVENC and no ffmpeg at 615. The display gates
were not run.

Follow-ups:

1. The v1 615 image needs `nvidia-modprobe` absent, or disabled, as in this overlay.
2. `bench-v1vm.sh` should take its guest-module source and base image from Leandro at the L615 pin, instead of from
   Caraxes' `guest-module` and the 610 file name.
3. Decide whether the bench backend should run with `LEA_MANAGED_COMPAT=1`, as the gate does.

The VM is down. To bring it back up: `V=/mnt/vmstore/caraxes/bench-v1-615 NVRM=/mnt/vmstore/leandro/l615-host-bin-8c0d2b2/vhost-user-nvrm caraxes/tools/bench-v1vm.sh up`,
then the manual `modules` step from `commands.txt` section 4. With the new script this overlay is refused,
because it sits on the base image. Use the image below.

**Follow-ups done and booted (2026-09-24, 18:24: smi PASS, 14 of 14 cuda rows pass on the 615 image; the record is `caraxes/runs/v1-615-image-2026-09-24/`), in [v1-615-image.md](v1-615-image.md):**

1. There is now a standalone image, `/mnt/vmstore/caraxes/images/guest-v1-2026-09-24-615.71.09.qcow2` (sha256
   `cba67208...71f5d`). It was flattened from this overlay. `nvidia-modprobe` is now a local dpkg diversion,
   `nvidia-persistenced` is masked, and the run's state is removed. Section 2 of that page is the table of
   differences from the base image.
2. `bench-v1vm.sh` now takes the guest modules from `LEA_V1_ROOT` and the host binary from `NVRM`/`LEA_V1_BIN`.
   It picks the image by `tools/lib/driver.sh`. It refuses a host driver, `nvrm_wire.h` version, overlay base or
   backend ABI that does not match. `modules` now does the manual steps.
3. Yes. The backend now starts with `LEA_MANAGED_COMPAT=1` by default, as Leandro-Test's gate does. The switch
   touches only the managed-only UVM commands. `caraxes/tools/v1-gates.sh` runs the smi and cuda gates of this
   run. The boot command is in section 4 of v1-615-image.md.


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
   615) is dispatched by the guest ABI now ((d), `ctrl.rs` `id_0a36`).
5. The first 615 benchmark (P13 after the window) needs its own series; do not merge it with the
   closing 610 one. First series done (benchmark.md "First 615 series",
   `caraxes/runs/bench-615-2026-09-24/`): the 615 guest matches the 610 guest on the same
   device configuration; a like-for-like with the closing series (P21 area, spin reactor, quiet
   host) is still to run.
