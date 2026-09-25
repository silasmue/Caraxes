<!-- SPDX-License-Identifier: MIT -->
# The 615 guest's display path

The display path of [display-e.md](display-e.md) and [intr-page.md](intr-page.md) §10 (GPU-rendered
Wayland on the emulated DisplayPort monitor, the interrupt page and the display front-end page) with
the 615.71.09 guest driver instead of 610.57.04 (decision D19), on vm0 on the night of 2026-09-24 to
2026-09-25. Evidence: [../../caraxes/runs/display-615-2026-09-24/](../../caraxes/runs/display-615-2026-09-24/)
(one folder per session, `series.sh`, `env.sh`, `prepare-vm0-615.sh`, `guest/`, `abi/`).

## 1. Outcome

- The **display gate passes 11 of 11** on the 615 guest, twice: `s3-gate/` (device of 57693d8) and
  `s4-series/` (device of 11dabe5, this branch). Weston GL at 59.53 and 59.83 Hz, the Vulkan swapchain
  presents at 52.8 and 55.7 FPS, the RT device comes up, the fence median is 0.75 and 0.14 ms.
- **Three GNOME starts** of three: a Wayland session, gnome-shell with the NVIDIA EGL
  (`libnvidia-eglcore` mapped), 21, 29 and 34 GPPut barrier waits (series C: 23 to 36).
- **The per-frame trap tables match series C** (§5): Mutter 138.92 traps per frame (610: 139.86),
  Weston 29.25 (610: 31.08). Every row is within two traps per frame of C; the only row that moves is
  the config-mirror write (NV_PCFG 0x88704), about 2 fewer per frame.
- **Q1, the 615 half** (§4): the 615 DP library sends `DP_SET_WAR_FLAGS` exactly where the plan
  predicted, once at the unplug (before `HDCP_CTRL` DISABLE_AUTHENTICATION) and once at the plug.
  `HDCP_CTRL` comes once, at the unplug, as on 610. They are the only D controls of the window, and
  no control in it is unanswered.
- **The device needed no change to run the 615 display stack.** The display controls of the 615
  NVKMS are the 610 ones plus `DP_SET_WAR_FLAGS`, which the R1 rule table already acknowledged. The
  changes here make the 615 layouts explicit and give the new control a record (§3.2).
- **What the guest needed** is outside the device (§2): the 615 GL, EGL and Vulkan userspace the
  image lacks, and an `nvidia-drm.ko` that loads, because the image's DKMS module needs
  `drm_display_helper.ko`, which the image does not have.
- **Host:** 0 Xid in the device's sessions. Two Xid 31 in the s3 window (23:32:08 and 23:32:25,
  CE2 `FAULT_PTE`) name pid 1131576, which is another VM's device. It was started before this series,
  and this session's device was pid 1185707.

## 2. The guest: vm0-615

`prepare-vm0-615.sh` builds `/mnt/vmstore/caraxes/vm0-615` offline, from local material only:

| Piece | What | Why |
|---|---|---|
| Overlay | `rootfs.qcow2` on `guest-baked-2026-09-24-615.71.09.qcow2` | `CARAXES_DRIVER=615` checks the backing chain |
| Seed | vm0's `user-data` and `network-config` (192.168.100.20, tap caraxes0, MAC 02:00:00:00:00:20), new instance id `caraxes-vm0-615-<time>` | the recipe of `gsp-rpc-2026-09-24/seed-vm3-615.sh` |
| `/etc/modprobe.d/caraxes-615-display.conf` | `nvidia_modeset` and `nvidia_drm` blacklisted, and their `install` rules load them only while `caraxes_dma_range` is loaded. `nvidia_peermem` is `/bin/false`. The persistenced unit is masked, and the default target is multi-user | nothing loads nvidia at boot, so `tools/caraxes-vm.sh probe` can still install the DMA table and reload nvidia. vm2-615 and vm3-615 block the display modules outright |
| `~/caraxes-debs`, `-debs2`, `-debs3` | the .debs vm0 installed in display-cd and display-e (Weston 13, edid-decode, libwayland-dev, wayland-protocols, libvulkan-dev), copied read-only out of vm0's overlay | the 615 image has no Weston and no development files. gdm3, gnome-shell, Mutter, Sunshine and libx264 are in the image |
| `/opt/nvrm-gl` | the NVIDIA 615.71.09 GL, EGL, GLES and Vulkan libraries (`libEGL_nvidia`, `libGLX_nvidia`, `libGLESv*_nvidia`, `libnvidia-{eglcore,glcore,glsi,tls,glvkspirv,rtcore,allocator,present,gpucomp}`), copied from the host's own `nvidia-utils 615.71.09`. `SHA256SUMS` and `README.caraxes` sit next to them. The image's 610.57.04 copy moves to `/opt/nvrm-gl-610.57.04`, and its vendor-neutral EGL platform libraries (egl-wayland, egl-gbm, egl-xcb, egl-xlib) are kept. `/etc/ld.so.conf.d/caraxes-gl-615.conf` names `/opt/nvrm-gl/lib` only | the image lacks `libnvidia-gl-615` (guest-driver-versions.md §8), and the 610 copy cannot talk to the 615 kernel module. These are NVIDIA's binaries for every distribution and need at most glibc 2.10. No package came from the network |
| `~/caraxes-guest` | `crates/vfio-user-nvgpu/guest` of this branch | display-up.sh, nvidia-egl-setup.sh, gnome-check.sh, weston-pace.sh, wlframes |

At the first boot (session `s1-setup`), `guest/setup-guest.sh` installs the .debs with dpkg (all
three sets install, rc 0). It then runs `gdm-wayland-setup.sh` and `nvidia-egl-setup.sh on`, and
builds wlframes, kmsflip and kmsgrab.

### 2.1 nvidia-drm 615 does not load from the image

Session `s2-gate`: `modprobe nvidia-drm` fails with `nvidia_drm: Unknown symbol
drm_hdcp_update_content_protection` and `... drm_connector_attach_content_protection_property`
(`s2-gate/display-up/guest-dmesg.txt`). The 615 nvidia-drm gained an HDCP content-protection
connector property (615 `kernel-open/nvidia-drm/nvidia-drm-connector.c:759-779`, `:913-946`). It is
compiled in when the kernel has `CONFIG_DRM_DISPLAY_HDCP_HELPER`
(`nvidia-drm-conftest.h:103-110`), and 6.8.0-136-generic has it, with the helpers in
`drm_display_helper.ko`. `modinfo` lists that module as a dependency, but the image has no such
file: it belongs to `linux-modules-extra-6.8.0-136-generic`, which the image lacks. `modules.dep`
does not list it either. The 610 guest's nvidia-drm was built in the guest from the 610 source,
which has no such property, so it depends on `nvidia-modeset` only.

The missing package was not fetched. `guest/build-nvdrm-615.sh` (session s3) instead builds
`nvidia-drm.ko` from the image's own DKMS source (`/usr/src/nvidia-615.71.09`). The one `#if` that
defines `NV_DRM_SUPPORT_CONTENT_PROTECTION_PROPERTY` (`nvidia-drm-conftest.h:105`) becomes `#if 0`.
The script checks that no HDCP helper symbol is left, installs the module as
`/lib/modules/6.8.0-136-generic/caraxes/nvidia-drm.ko` (sha256 `9f368f5b...8ce1a1`), and points the
`install nvidia_drm` rule at it. `nvidia.ko` and `nvidia-modeset.ko` stay the DKMS modules, so the
DP library, NVKMS and RM that talk to the device are NVIDIA's unchanged build. The only thing lost
is the DRM connector's "Content Protection" property. The emulated sink has no HDCP in any case.

## 3. The device

### 3.1 What changed between the 610 and 615 display controls

`diff -r` of the SDK control headers of the two trees (610.57.04: `vendor/open-gpu-kernel-modules`,
615.71.09: `/home/silas/git/Leandro/vendor/open-gpu-kernel-modules`): among the display headers,
only `ctrl0073/ctrl0073dp.h`, `ctrl0073specific.h` and `ctrl0073system.h` differ. `ctrl5070`,
`ctrlc370` and `ctrlc372` are identical. Of the display classes, `clc57d.h` gains
`HEAD_SET_LOCK_OFFSET` (0x2040) and `clc573.h` gains `HEAD_CLK_CAP` (0x5e8). NVKMS already defined
both itself in 610 (`nvkms-evo3.c:2244-2253`, `:5242-5259`), so the core channel methods and the
capability reads are unchanged. The ids the 615 `src/nvidia-modeset`, `src/common/displayport`,
`src/common/modeset` and `kernel-open/nvidia-drm` name are the 610 set plus
`NV0073_CTRL_CMD_DP_SET_WAR_FLAGS`. Three NV0080 names are gone, and none of them reaches the device.

gcc measured the structs of both trees (`abi/gen.py`, the 610 list of
`display-ab-2026-09-23/abi/names.txt` plus the changed and new structs; `abi/layouts-*.txt`). The
615 table is `crates/vfio-user-nvgpu/src/ver/v615/display-layouts.txt`. Differences:

| Struct | 610 | 615 | For the device |
|---|---|---|---|
| `NV0073_CTRL_SYSTEM_GET_ACTIVE_PARAMS` | 16 | 20: `staleSorIndex` at 16, output flag `GOP_STALE_HEAD_NEEDS_CLEANUP` (flags bit 1) (ctrl0073system.h:638-678) | answered: no stale head (§3.2) |
| `NV0073_CTRL_CMD_DP_GET_CAPS_PARAMS` | 64 | 64: `bIsInternalDpTunnelingSupported` at 33, former padding (ctrl0073dp.h:1743) | the handler already zeroes bytes 20 to 35, so it reads 0 |
| `NV2080_CTRL_INTERNAL_INIT_BRIGHTC_STATE_LOAD_PARAMS` | 4104 | 4104: `bNitsPwmFallbackEnabled` inserted at 4, the backlight data moves to 8 | acknowledged without reading it |
| `NV0073_CTRL_SPECIFIC_DISPLAY_BRIGHTNESS_LTM_PARAMS` | 52 | 72: five fields appended (ctrl0073specific.h:3167-3171) | not sent (no panel) |
| `NV0073_CTRL_CMD_DP_SET_WAR_FLAGS_PARAMS` | none | 16: `{subDeviceInstance, displayId, warId, bEnable}` (ctrl0073dp.h:3734-3773) | the R1 ACK row, now recorded (§3.2) |
| `NV0073_CTRL_SPECIFIC_HDCP_KEY_DECRYPTION_PARAMS` | none | 8 (ctrl0073specific.h:3208-3228) | REFUSE row (ver/v615/dmodel.rs) |

### 3.2 The changes (commit 11dabe5)

- `src/ver/v615/display.rs` (new): the 615 display vocabulary, the 610 one
  (`pub use super::super::v610::display::*`) with `GET_ACTIVE_SIZE` 20,
  `GET_ACTIVE_STALE_SOR_INDEX` 16, `GET_ACTIVE_FLAGS_GOP_STALE_HEAD`,
  `DP_CAPS_INTERNAL_DP_TUNNELING` 33 and `CMD_DP_SET_WAR_FLAGS` overridden. `src/ver/v615.rs:45`
  declares it, and it shadows the glob import. `src/ver/v610/display.rs` carries the 610 values
  (none, 0 or 16). Two tests: `display_layouts_differ_only_where_overridden` compares every number
  of the 610 table with the 615 one, and fails for a difference that is neither overridden nor
  listed with its reason. `the_615_display_fields_match_the_headers` pins the new fields and the
  generated D row's id and size.
- `src/display/mod.rs:674` `SYSTEM_GET_ACTIVE`: no firmware-lit head, so no stale head either. The
  output flag is cleared, `staleSorIndex` is 0 and the input flag `CLIENT` is kept (test
  `get_active_reports_no_head_and_no_stale_head`).
- `src/display/mod.rs:1032`, `:1154` `DP_SET_WAR_FLAGS`: the status and label stay the D rule
  table's ACK (`dmodel.rs` checks every display row against it, and that file belongs to the
  control-table work). The connector records `SW_AUTO_READ_ENABLE`
  (`Connector::sw_auto_read_war`), and the note names the WAR and its previous state. A request
  the header calls `NV_ERR_INVALID_ARGUMENT` (not a connector, or an unknown WAR id) is named in the
  note and still acknowledged, as the row says. The WAR switches the hardware DPCD auto-read off on
  a DP-tunneling port (bug 6102109). The AUX model reads in software, so nothing changes. Tests
  `dp_set_war_flags_is_recorded_per_connector` and `dp_caps_claim_no_internal_dp_tunneling`.

`cargo test -p vfio-user-nvgpu --lib`: 286 passed on 610, and 290 passed (1 ignored) with
`--features v615`.

## 4. Q1: DP unplug and plug on the 615 guest

`s4-series/q1-unplug-plug/`: `unplug` was written into the device's `display-ctl` at 00:06:58 and
`plug` at 00:07:12, while `hotplug-watch.sh 40` watched the connector (four DRM change uevents).
`dscan.py` (from gsp-d-2026-09-24) finds 181 control answers, 2 D ids and 0 unanswered. The device
trace in order:

```
472599089 ctrl 0x731391: dp war flags: SW_AUTO_READ_ENABLE 0 on 0x100 (was 0; ...) -> status 0x0 [acknowledged]
472599458 ctrl 0x730282: hdcp ctrl of 0x100: cmd DISABLE_AUTHENTICATION (8): nothing to do, not authenticated -> status 0x0
486548611 ctrl 0x731391: dp war flags: SW_AUTO_READ_ENABLE 0 on 0x100 (was 0; ...) -> status 0x0 [acknowledged]
```

- **Unplug:** `setDpWarFlag(SW_AUTO_READ_ENABLE, false)` (615 `dp_connectorimpl.cpp:8475`, in
  `notifyLongPulseInternal`), then `configureHDCPDisableAuthentication` (`:8491`). The 610 guest
  sent only the second.
- **Plug:** `setDpWarFlag(..., false)` after the OUI handshake (`:8048`). The WAR is only ever set
  to true for one Realtek sink behind a USB4 tunnel (`dp_wardatabase.cpp:130-140`), so it is
  cleared every time.
- The plan's answer to Q1 holds: `DP_SET_WAR_FLAGS` and `HDCP_CTRL` are the only D controls of the
  DP path on 615. `HDCP_KEY_DECRYPTION`, the other new 615 id, was not sent. Nothing in the window
  is unanswered.

Over a whole session, the display gate's window (`s4-series/gate-dscan.txt`) has 3138 control answers
and 2 D ids: `DP_SET_WAR_FLAGS` 7 times (one per modeset plug cycle) and `SET_ZBC_COLOR_CLEAR` 3
times (unanswered, as on 610). It has 813 unanswered answers in 2 ids (§6).

## 5. Trap tables: 615 against series C (610)

The same environment as series C (`display-page-2026-09-24/env.sh`, slot 0, 2 GiB profile, host
execution, token pool, hybrid reactor, `CARAXES_INTR_PAGE=1 CARAXES_DISPLAY_PAGE=1`), and the same
guest scripts (`display-page-2026-09-24/guest/trap-*.sh`, 14 s of wlframes, a 10 s window). Per
presented frame (`s4-series/trap-compare.txt`, `compare.py`):

| Row | Mutter 615 | Mutter 610 C | Weston 615 | Weston 610 C |
|---|---:|---:|---:|---:|
| presented frames in the window | 594 (59.40 Hz) | 597 (59.70 Hz) | 600 (60.00 Hz) | 570 (57.00 Hz) |
| frame callbacks (wlframes) | 58.62 Hz, sd 7.70 ms, max 188 ms | 59.86 Hz, sd 2.90 ms | 59.79 Hz, sd 1.98 ms | 57.41 Hz, sd 4.11 ms |
| **all trapped accesses** | **138.92** | **139.86** | **29.25** | **31.08** |
| BAR0 writes: NV_PCFG config mirror 0x88704 | 10.21 | 12.17 | 8.89 | 10.65 |
| BAR0 writes: MMU invalidate | 6.08 | 6.02 | 0 | 0 |
| BAR0 writes: 0x696000 (display channel kick) | 1.11 | 1.11 | 1.11 | 1.11 |
| BAR0 reads: VF TIME_0/1 | 17.53 | 16.69 | 17.02 | 17.09 |
| BAR0 reads: VF PRIV_MMU_INVALIDATE | 4.05 | 4.01 | 0 | 0 |
| BAR1 writes: USERD GPPut | 3.00 | 3.00 | 2.00 | 2.00 |
| BAR2 writes (pages 0x23000, 0x39000, 0x25000) | 73.63 (32, 30, 5) | 73.60 (32, 30, 5) | 0 | 0 |
| BAR2 reads | 23.05 | 23.02 | 0 | 0 |
| host notifier events to MSI | 4.66 | 6.61 | | |

The 71 BAR2 writes per Mutter frame of optimization-backlog.md and intr-page.md section 6a are another series (series F, 610 guest, neither page on, `caraxes/runs/display-f-2026-09-24/f3b-traps-mutter-fresh/trap-table.txt`); the 73.63 here is the 615 guest with both pages.

- The 615 driver produces the same traps per frame as 610, row for row. The BAR2 page-table
  churn, the GPPut writes, the VF timer reads and the display kicks are identical, so the 615
  kernel RM, NVKMS and GL driver drive the device the same way per frame. The two fewer config
  mirror writes per frame go with fewer MSIs (4.66 against 6.61 per frame for Mutter).
- Weston runs at a full 60.00 Hz with a narrower spread than C (sd 1.98 against 4.11 ms), so the
  wider spread of C was not a property of the display page. Mutter shows one long interval
  (188 ms) in the window and a wider spread (sd 7.7 ms), at the same trap count.
- The display page summary of the session: 31374 scans with work, 24080 EVT_STAT acks,
  7450 awaken markers and 7450 acked by CORE_RESET (longest 15.4 ms), `inconsistencies: none`
  (`s4-series/device-summary.txt`).

## 6. Found in passing

- **`NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY` (0x90960107, category A) 45 times per GL context.**
  The 615 GL driver asks for the ZBC clear table at every context creation: 45 in the Weston window
  and 90 per GNOME start (two contexts), 810 in a display gate. The 610 driver asked 18 times in a
  whole gate and 0 times per GNOME start (series C). The device answers NOT_SUPPORTED
  (`unanswered`), and GL carries on, so the gate passes. This belongs to the ZBC table model of
  gsp-plan 6.2 (R3) and to the control-table work, not to this branch.
- **The flip `WARN_ON(nv_flip == NULL)`** (615 `nvidia-drm-crtc.h:368`, the same statement as 610's
  `:355`, "the known flip one" of the display gate) appears at least 7 times in s4 (`q1-unplug-plug/guest.txt`), at GNOME starts and in the
  Mutter window, without a flip timeout. The display gate's kernel stage knows it only as `:355`
  (`tools/caraxes-gate.sh`, `display_kernel`). In s4 it read 0 warnings, because the NVRM ioctl lines
  (`NVreg_ResmanDebugLevel=0` in the image) push older lines out of the kernel ring. On a 615
  guest the filter should accept `:368` as well.
- **`GPU_QUERY_ECC_STATUS`** forwarded to the host and refused (0x56), 34 times in s4. This is the
  same as every 610 cuda and display gate.

## 7. How to repeat

```
caraxes/runs/display-615-2026-09-24/prepare-vm0-615.sh           # once; refuses an existing overlay
CARGO_TARGET_DIR=target/disp615 cargo build --release -p vfio-user-nvgpu --features v615
caraxes/runs/display-615-2026-09-24/series.sh s1-setup setup       # first boot: debs, gdm, EGL, probes
caraxes/runs/display-615-2026-09-24/series.sh <name> nvdrm display gate          # nvdrm once per overlay
caraxes/runs/display-615-2026-09-24/series.sh <name> display gnome traps q1 gate
```

`series.sh` refuses to start while a vm0 or vm0-615 device or Cloud Hypervisor runs. A refused VRAM
reservation is retried every 5 minutes for up to an hour: s3 waited 10 minutes for 550 MiB of host
headroom. The session always ends with `down`, the host's Xid lines and the device pid.
