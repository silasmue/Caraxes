<!-- SPDX-License-Identifier: MIT -->
# Display phase E: a GPU-rendered Wayland desktop on the shadow GR path

Phase E of [display-connector.md](display-connector.md): the guest's compositor and clients
render on the physical RTX 2070 through the device's host execution (the shadow GR path of
[execution.md](execution.md) and [mapping.md](mapping.md)), and the emulated DisplayPort
monitor of phases A to D ([display-ab.md](display-ab.md), [display-cd.md](display-cd.md))
scans the result out on the device clock. Two agents ran it on vm0 on 2026-09-24:

- **Series E** (e1 to e31, 08:39 to 10:33, the first display agent, stopped by the operator at
  10:46): the NVIDIA EGL/GL/Vulkan stack on the device, Weston and GNOME on it, the display
  gate's GPU stages, the classic demos, the rerun after the mapping worker. Evidence
  [../../caraxes/runs/display-e-2026-09-24/](../../caraxes/runs/display-e-2026-09-24/) (per run:
  `command.txt` with the guest command, `guest.txt`, the device trace window or its event lines,
  `guest-dmesg.txt`, `host-xid.txt`, `host-check.txt`), the gate runs
  `caraxes/runs/gate-display-2026-09-24-5/` to `-7/`. This section is reconstructed from that
  evidence and the branch's commit messages (e6e9fff, d5987e9, ea7d773, b11234d, 587e276);
  the agent's own notes did not survive.
- **Series F** (f0 to f7, 13:06 to 13:52, this package): the demos on the current main, the
  per-frame trap tables of Mutter and Weston with and without the doorbell ioeventfd, the stream
  stage, a pointer device. Evidence
  [../../caraxes/runs/display-f-2026-09-24/](../../caraxes/runs/display-f-2026-09-24/): `env.sh`
  (the configuration), `vm.sh` and `run.sh` (how each run was taken), `guest/` (the guest
  scripts), one directory per run.

Every host Xid of both series is on the device's own pid (`name=vfio-user-nvgpu`); the host
desktop was not affected. Host GPU reads outside the device: `nvidia-smi` memory queries (13:05
before `up`, and after each run in `host-check.txt`), the host kernel log for Xid lines.

## 1. Outcome

| Check | Result | Runs |
|---|---|---|
| First GPU-rendered frame (EGL, GLES clear + readback, 1080p) | all 2 073 600 pixels green, `GPU-RENDERED-OK`; before the bare-CE mirror the clear never landed | e9-e13, e17 |
| Weston 13, GL renderer on the NVIDIA EGL/GBM stack, 1080p60 | frame callbacks 56.5 Hz (e15), 59.5 Hz with a 12 ms repaint window after the mapping worker (e30), 59.77 Hz over 30 s (e31), 58.4 Hz in f4 | e8-e17, e30, e31, f4 |
| GNOME 46 on Wayland, Mutter on the NVIDIA EGL stack | 0 page-flip failures, `libnvidia-eglcore` in gnome-shell's maps; frame callbacks 47.1 Hz (e30), 30.0 to 41.5 Hz in series F | e18, e19, e30, f2, f3, f3b |
| Vulkan on Wayland (v1's vkprobe) | swapchain, 120 presented frames 43.6-44.4 FPS (e20, e24), 52.4-55.5 FPS after the worker; RT device (acceleration structures) after `nvidia-uvm` is loaded; empty submit to fence 0.09-1.0 ms | e20-e24, e30 |
| KMS capture of the GPU-rendered scanout | block-linear scanout read correctly (`SAMPLE 320 240 0x00808080`); Sunshine `capture=kms` finds the monitor and libx264 | e25, e30 |
| The display gate with host execution, 11 stages | **pass 11/11** (`gate-display-2026-09-24-7`): compositor 59.87 Hz, Vulkan present 55.5 FPS, fence median 0.09 ms | gates 5-7 |
| Classic demos on Weston, current main | vkcube-wayland 44.1 FPS, vkcube (Xwayland) 37.6, glxgears (NVIDIA GLX, Xwayland) 57-60, weston-simple-egl 59-60, glmark2-wayland score 1804; 0 Xid | f1 |
| Classic demos on GNOME, current main | vkcube-wayland 31.1, vkcube 29.9, glxgears 33-37, weston-simple-egl 22-33, glmark2-wayland 34; 1 Xid 31 at GNOME's start | f2 |
| Per-frame traps (10 s windows) | Weston 157 trapped accesses per frame, Mutter 662-712; 2-3 doorbells and 2-3 GPPut writes per frame, 0 shadow plans; the rest is interrupt servicing and (Mutter) BAR2 page-table writes | f3-f5b |
| Doorbell ioeventfd under Mutter | doorbell traps 3.00 -> 0 per frame; frame callbacks 35.2 / 37.6 Hz against 30.0 / 41.5 Hz without it: no measurable effect | f5, f5b |
| Stream: Sunshine kms + x264 in the guest, Moonlight on the host | valid session: 25.85 FPS incoming and decoded, host processing latency avg 101 ms, 0.00 % network drops, 0.35 % jitter drops, 9.8 Mbit/s actual at 20 Mbit/s asked, guest CPU 2.09 cores; 0 Xid | f7 |
| Pointer device through Cloud Hypervisor | virtio-input over `--generic-vhost-user` (no patch), `CARAXES_INPUT=fifo`; 180 of 180 host events arrived, udev `ID_INPUT_MOUSE=1`, on seat0 of the GNOME session | f6 |

## 2. Configuration

vm0, reservation slot 1, 2 GiB profile, host execution, the display engine
([../../caraxes/runs/display-f-2026-09-24/env.sh](../../caraxes/runs/display-f-2026-09-24/env.sh)):

```
VM=vm0 CARAXES_IP=192.168.100.20 CARAXES_TAP=caraxes0 CARAXES_MAC=02:00:00:00:00:20
CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
CARAXES_MIRROR_GR_SHADOW=1 CARAXES_MIRROR_BARE_CE=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
CARAXES_FB_MIB=2048 CARAXES_RESERVE_SLOT=1 CARAXES_DISPLAY=1
```

Then `tools/caraxes-vm.sh probe <label>` (DMA table, `nvidia.ko`), and in the guest
`crates/vfio-user-nvgpu/guest/display-up.sh` (nvidia-modeset, `nvidia-drm modeset=1 fbdev=0`,
the device nodes, `nvidia-uvm` and its nodes for the Vulkan RT device) and once
`nvidia-egl-setup.sh on` (drops the Mesa `kms_swrast` pin of phase D from `/etc/environment` and
links `nvidia-drm_gbm.so` to `/opt/nvrm-gl/lib/libnvidia-allocator.so.1`, so EGL and GBM go to
the NVIDIA driver). Weston runs through `weston-run.sh` / `weston-pace.sh` (DRM backend, kiosk
shell, GL renderer, `REPAINT_WINDOW=12`), GNOME through `gnome-check.sh start` (gdm3, Wayland,
autologin), the demos through `demos.sh weston|gnome`.

Series F built the device from main `8cca8e6` (`CARGO_TARGET_DIR=target/display`).

## 3. Series E, run by run

| Run (time) | Question | Change | Result | Host Xid |
|---|---|---|---|---|
| e1-e3 (08:39-08:40) | does the NVIDIA EGL answer on the device | none (EGL still pinned to Mesa by phase D) | `eglInitialize failed` on gbm and surfaceless; strace: the Mesa vendor pin and no `nvidia-drm_gbm.so` | 0 |
| e4-e7 (08:40-08:53) | Weston with the GL renderer | `nvidia-egl-setup.sh`; RM trace with `libnvrm_trace.so` | e5 `EGL_BAD_ACCESS`, `failed to create compositor backend`; the RM trace named the refused controls; after `FB_GET_GPU_CACHE_INFO` and `FIFO_GET_ENGINE_CONTEXT_PROPERTIES` forwarded and the channel `BIND` recorded (e6, e7) the GL renderer comes up (`OpenGL ES 3.2 NVIDIA 610.57.04`, renderer `Leandro VFIO RTX 2070-2G`) | 0 |
| e8 (08:53) | frame pacing with the GL renderer | wlframes as the client | 60.00 Hz callbacks and presentation (pixels not checked) | 0 |
| e9-e12 (08:55-09:05) | does a GPU clear land | `eglprobe.c` (GLES clear, `glFinish`, `glReadPixels`); RM traces; `GF100_DISP_SW` allocatable (`ACTIVE_DISPLAY_DEVICES` reports `numHeads`); `ZCULL_BIND` recorded, BINAPI 0x0107 forwarded | clears "finish" but the readback is wrong (60 032 green pixels of 2 073 600 at 1080p, 0 to 15 356 of 307 200 at 640x480) | 0 |
| e13 (09:09) | the GL driver's bare copy-engine channels do the clear's copies | `CARAXES_MIRROR_BARE_CE=1`: a CE channel without a guest TSG gets a host TSG of its own | **`GPU-RENDERED-OK`** at 640x480, 307 200 of 307 200 pixels; the first GPU-rendered frame through the stock NVIDIA EGL | 0 |
| e14 (09:10) | Weston GL at 1080p | none | first frame after 5.1 s, then nothing; MMU fault `PRIV_VIOLATION` by `GPCCLIENT_PROP_0` at VA 0x1_20800000, twice | **2 x Xid 31**, device pid |
| e15-e17 (09:23-09:25) | the global context buffers of the 3D class collide with set A | buffers promoted by VA only (size 0) are excluded from set A with the host's context-buffer size (`FIFO_GET_ENGINE_CONTEXT_PROPERTIES`) | Weston GL 56.45 Hz (e15); repaint window 7 and 12 ms (e16); eglprobe 1080p 120 frames 1.78 ms each, `GPU-RENDERED-OK` on 2 073 600 pixels; full-screen damage 52.35 Hz (e17) | 0 |
| e18 (09:26) | GNOME on the NVIDIA EGL | none | gnome-shell loads `libnvidia-eglcore`, `Type=wayland`; 28 lines of `Xid 13 3D-Z KIND Violation` (the guest's depth/stencil kinds were dropped in the shadow mappings) | **Xid 13**, device pid |
| e19 (09:32) | carry the guest's kind into the shadow | depth/stencil kinds 0x01-0x05 (uncompressed) passed as `NVOS46_FLAGS_PAGE_KIND_OVERRIDE` | GNOME clean: gbm renderer on card0, high-priority EGL context, 0 Xid | 0 |
| e20-e24 (09:38-09:41) | v1's Vulkan stages on Wayland | `vkprobe-wl.c`, `weston-run.sh`; `display-up.sh` loads `nvidia-uvm` | swapchain ok; present 43.6 FPS; `vkCreateDevice` with acceleration structures `VK_ERROR_INITIALIZATION_FAILED` until `/dev/nvidia-uvm` exists (e21-e23); present 44.4 / 43.9 FPS (e24) | 0 |
| e25 (09:51) | KMS capture of a block-linear scanout | `kmsgrab.c` de-tiles block-linear | `SAMPLE 320 240 0x00808080` (the Weston client), Sunshine finds the monitor, connector and cursor plane | 0 |
| e26-e29 (09:54-10:11) | the classic demos | `demos.sh` | Weston: vkcube-wayland 33.9, vkcube 33.1, glxgears 54-56, glmark2 1216; GNOME: vkcube-wayland 22.8, vkcube 19.0, glxgears 14-21, weston-simple-egl 16-28, glmark2 31 (e26 an earlier full Weston pass; e28 a 12 s pass meant for the two EGL demos, lower numbers throughout) | 0 |
| e30 (10:23) | after merging the mapping worker (main `b58cb12`) | merge; the bare CE channel gets its own host TSG on the CE shadow path too | eglprobe 1080p 1.09 ms per frame; Weston 58.49 Hz (7 ms window), 59.53 Hz (12 ms); present 52.4 / 53.2 FPS (48.4 before); Sunshine kms + libx264; Mutter 47.1 Hz (38.6 before) | 0 |
| e31 (10:32) | Weston stalls over 30 s | none | 59.77 Hz, max interval 62.8 ms | 0 |

The device changes of series E (all small, in the files of the mapping package, so listed here
for their owners):

- `ctrl.rs:1004-1021`: `FB_GET_GPU_CACHE_INFO` and `FIFO_GET_ENGINE_CONTEXT_PROPERTIES`
  forwarded, `GR_CTXSW_ZCULL_BIND` recorded; `ver/v610.rs:383`, `ver/v610.rs:850` their
  layouts and the BINAPI 0x0107 size.
- `display/mod.rs:521-526`: `INTERNAL_DISPLAY_GET_ACTIVE_DISPLAY_DEVICES` reports `numHeads`,
  so the GL driver's `GF100_DISP_SW` allocates.
- `mirror.rs:204-208`, `mirror.rs:493`, `mirror.rs:818`, `mirror.rs:1232`: `CARAXES_MIRROR_BARE_CE`.
- `mirror.rs:480`: size-0 global context buffers excluded with the host's size.
- `shadow.rs:134`, `shadow.rs:870`, `hostmem.rs:946`: the depth/stencil kind override.

## 4. The display gate with host execution

`tools/caraxes-gate.sh display` gained v1's swapchain, present, rt and events stages on
Wayland (`vkprobe-wl.c` in Weston, v1's `fencetime.c`) and runs Weston with the GL renderer when
the device executes on the host (commit ea7d773). Runs on vm0:

| Gate run | Build | Verdict | Numbers |
|---|---|---|---|
| gate-display-2026-09-24-5 | d5987e9 | fail: capture (kmsgrab read block-linear as linear, fixed in e25), present | compositor 59.70 Hz, present 48.4 FPS |
| gate-display-2026-09-24-6 | ce97ae7 (after the worker) | fail: compositor 53.89 Hz | present 52.5 FPS |
| gate-display-2026-09-24-7 | ce97ae7 | **pass, 11 of 11** (rig, connector, edid, modeset, compositor, capture, swapchain, present, rt, events, kernel) | compositor 59.87 Hz, present 55.5 FPS, fence median 0.09 ms, Sunshine kms + libx264, 0 dmesg warnings |

## 5. Series F: the demos on the current main

`demos.sh` with `SECS=20` (f1 on Weston with Xwayland, f2 in a fresh GNOME session). vkcube
renders 1200 frames under FIFO, the rate is frames over wall time; glxgears and
weston-simple-egl print every 5 s; glmark2-wayland runs its default scenes at 1280x720.

| Demo | Weston before (e27) | Weston now (f1) | GNOME before (e29) | GNOME now (f2) |
|---|---|---|---|---|
| vkcube-wayland | 33.9 | **44.1** | 22.8 | **31.1** |
| vkcube (XCB, Xwayland) | 33.1 | 37.6 | 19.0 | 29.9 |
| glxgears (NVIDIA GLX on Xwayland) | 54-56 | 57-60 | 14-21 | 33-37 |
| weston-simple-egl | 54-58 (e28) | 59-60 | 16-28 | 22-33 |
| es2gears_wayland | runs 20 s, prints no rate | same | same | same |
| glmark2-wayland score | 1216 | **1804** | 31 | 34 |
| host Xid | 0 | 0 | 0 | 1 (Xid 31, see §9) |

eglgears_wayland is not in `demos.sh` (it prints no rate either). Every info probe reports the
NVIDIA stack: `vulkaninfo` device 0 `Leandro VFIO RTX 2070-2G` with driver 610.57.04 (llvmpipe
second), `eglinfo` vendor NVIDIA, `glxinfo` renderer `Leandro VFIO RTX 2070-2G/PCIe/SSE2`.
GNOME's glmark2 stays near 34 because Mutter itself composites at 30-40 Hz (§6).

## 6. Per-frame traps of Mutter and Weston

Method (`caraxes/tools/trap-table.py`): the device trace with every access (`run.sh --full`),
a client committing once per frame callback (wlframes, 64x64 surface, 8x8 damage) for 14 s,
and a 10 s window ending 1 s before the last scanout latch. A presented frame is one latch of
window 6 with a surface. Trap cost for the time estimates: 23-27 us per trapped access, the
mapping package's c0/c1 measurement ([mapping.md](mapping.md)).

| Per presented frame | Weston GL (f4) | Mutter (f3, after the demos) | Mutter, fresh (f3b) | Mutter + ioeventfd (f5) | Mutter + ioeventfd (f5b) |
|---|---|---|---|---|---|
| frames in 10 s / callback rate | 585 / 58.4 Hz | 300 / 30.0 Hz | 406 / 41.5 Hz | 375 / 35.2 Hz | 387 / 37.6 Hz |
| BAR0 writes | 33.3 | 383.9 | 374.7 | 374.0 | 372.5 |
| of which doorbell (0xbb0090) | 2.00 | 3.01 | 3.00 | **0** | **0** |
| of which MMU invalidate (3 writes each) | 0 | 6.02 | 5.99 | 6.00 | 6.01 |
| of which PMC interrupt enable set/clear (0x160/0x180) | 9.9 | 186.7 | 183.4 | 184.2 | 183.7 |
| of which VF CPU_INTR enable/clear | 13.3 | 175.7 | 171.4 | 172.4 | 171.8 |
| BAR0 reads (interrupt tree, 0x611ec0 display interrupt status, PTIMER) | 121.7 | 230.4 | 190.0 | 200.4 | 192.6 |
| BAR1 writes: USERD GPPut (offset 0x8c) | 2.00 | 3.01 | 3.00 | 3.01 | 2.99 |
| BAR1 writes: GPFIFO entries, push data | 0 | 0 | 0 | 0 | 0 |
| BAR2 writes (PTEs at FB 0x168c50000, PDEs at 0x168dd0000, each written twice) | 0 | 71.2 | 70.9 | 71.0 | 71.0 |
| BAR2 reads | 0 | 23.1 | 23.0 | 23.0 | 23.0 |
| **all trapped accesses** | **157** | **712** | **662** | **672** | **662** |
| forwarded doorbells (host doorbell rung) | 2.00 | 3.01 | 3.00 | 3.00 | 2.99 |
| host notifier events raising an MSI | 3.56 | 5.90 | 4.71 | 4.98 | 4.85 |
| shadow plans applied in the window | 0 | 0 | 0 | 0 | 1 (walk 63 us, apply 375 us) |

**Current numbers (2026-09-25):** with the interrupt page, the display page, the BAR1 pool, the
BAR2 memfd and the MSI re-arm ioeventfd, Mutter traps 50.5 times per frame and Weston 17.1
([intr-page.md](intr-page.md) §11.1). With the flush deferral and the MMU invalidate page, Mutter
traps 40 times per frame (23 cheap flush reads, 16 `TIME` reads, 1.3 display kicks) and Weston
15 to 17 (§11.5). The frame-timing analysis of Weston is in §11.4.

Per-run tables: `f3-traps-mutter/trap-table.txt`, `f3b-traps-mutter-fresh/`, `f4-traps-weston/`,
`f5-traps-mutter-ioeventfd/`, `f5b-traps-mutter-ioeventfd/`. Findings:

- **Submission is no longer the cost.** A frame costs 2 (Weston) or 3 (Mutter) doorbells and as
  many USERD GPPut writes; no GPFIFO entry is written through BAR1 (the entries are in guest
  sysmem), and no shadow plan runs in steady state (one in 50 s of Mutter windows): the mapping package's write-through copies
  and cached walk leave nothing to do per frame. The GR shadow resync per frame that series E
  measured (26 ms walks at 60 Hz under Mutter, mapping.md §4) is gone.
- **Interrupt servicing is the cost.** Weston traps about 157 times per frame, Mutter about 670:
  every MSI makes the guest RM walk the VF interrupt tree and toggle `NV_PMC_INTR_EN_SET/CLEAR`
  (0x160/0x180) and the leaf enables, about 70 trapped accesses per interrupt. At 23-27 us per
  trap that is roughly 4 ms of vCPU exits per Weston frame and 15-19 ms per Mutter frame, which
  is Mutter's whole 16.7 ms budget and matches its 30-41 Hz. Mutter also rewrites 71 BAR2 page
  table entries per frame (PTEs written twice through BAR2, then an MMU invalidate): a per-frame
  mapping the guest RM does for Mutter and not for Weston.
- **The doorbell ioeventfd removes what it should and nothing else**: 0 trapped doorbells, the
  same 3 host doorbells per frame, frame callbacks within Mutter's run-to-run spread (30.0 to
  41.5 Hz without, 35.2 and 37.6 Hz with). With 3 of about 670 traps per frame it cannot move
  the frame rate; it matters for submission-heavy CUDA (mapping.md, the doorbell entry of the
  README), not for a compositor.
- Next levers, in order: fewer interrupts per frame (coalesce the host notifier events of one
  frame into one MSI; the non-MSI duplicates already exist), the interrupt tree and PMC enables
  served without a trap (a memfd-backed page as in P24), and the BAR2 PTE churn of Mutter (read
  which guest mapping is rebuilt per frame).

## 7. The stream stage

Method: `../Leandro-Test/scripts/bench.sh stream` (do_stream), carried over to the Caraxes
display because that script only knows `x11` and `portal` capture:
[stream.sh](../../caraxes/runs/display-f-2026-09-24/stream.sh) and
[guest/sunshine-stream.sh](../../caraxes/runs/display-f-2026-09-24/guest/sunshine-stream.sh).
Sunshine in the guest with `capture = kms` and `encoder = software` (libx264), as root because
KMS capture needs CAP_SYS_ADMIN (its login state therefore in root's `~/.config/sunshine`),
glxgears on GNOME's Xwayland (NVIDIA GLX) as the content; the guest's
`~/.config/sunshine/sunshine.conf` is saved and restored and a Sunshine that ran before is
restarted, as bench.sh does. Moonlight on this host pairs once over the tap network (PIN through
Sunshine's API, the login and PIN of Leandro-Test's config.sh), streams `Desktop` for 60 s at
1920x1080@60 and 20 Mbit/s, and `moonlight quit` makes it print the session summary, which is
the verdict (parsed with bench.sh's patterns into `stats.json`; exit 3 if the run is not a
measurement).

| f7, GNOME / kms + libx264, 1080p60, 20 Mbit/s asked, 60 s | Value |
|---|---|
| verdict | valid (Sunshine alive at the end, frames arrived) |
| incoming / decoding / rendering frame rate | 25.85 / 25.85 / 25.67 FPS |
| host processing latency min / max / avg | 50.6 / 494.9 / 101.3 ms |
| frames dropped by the network / by jitter | 0.00 % / 0.35 % |
| average decode time, network latency | 0.56 ms, 9 ms |
| bitrate actually produced (x264 summary) | 9.78 Mbit/s, 1191 frames (5 I, 1186 P) |
| guest CPU over the session | 26.1 % of 8 vCPUs = 2.09 cores |
| content (glxgears in the session) | 27.7-29.6 FPS, one 5 s dip to 14.1 |
| host Xid | 0 |

Where the time goes: Sunshine imports the NVIDIA scanout buffer at every captured frame, so the
device applied 1263 shadow plans during the session (about one per captured frame, 5710 leaves
mapped and 5198 unmapped, worker apply 2.42 s in total, max 82 ms; the server-thread walks
33 ms in total), on top of Mutter's own 30-40 Hz. The 101 ms average host processing latency and
the 26 FPS are this capture path plus x264 at 1080p on the guest's vCPUs.

What the NVENC mirror would change (expectation, not measured): with `encoder = nvenc` the
frames stay in VRAM and TU106's NVENC encodes them, so the 2.09 guest cores of x264 and most of
the encode share of the 101 ms go away, and the bitrate follows the request. It needs the device
to mirror the guest's NVENC channel (class C5B7) on the physical GPU as it mirrors CE and GR
channels, with the NVENC engine shared with the host desktop's encoder sessions. It does not
remove the per-frame shadow plan of the capture import (that needs the capture buffer's mapping
kept across frames), nor Mutter's interrupt cost of §6.

## 8. A pointer device for the guest desktop

Cloud Hypervisor v53 has no built-in virtio-input, but it has `--generic-vhost-user`, and
Leandro's `vhost-user-input` backend serves virtio-input (device type 18) over vhost-user. The
launcher adds it without a patch (`tools/caraxes-vm.sh`, header and `input_up`):

```
CARAXES_INPUT=fifo tools/caraxes-vm.sh up        # backend on $W/input.sock, FIFO $W/input.fifo
tools/caraxes-vm.sh input '2 0 40' '0 0 0'       # REL_X +40, SYN (Linux input event lines)
tools/caraxes-vm.sh input '1 272 1' '0 0 0' '1 272 0' '0 0 0'   # BTN_LEFT click
```

`CARAXES_INPUT_SERVER` names the backend binary (default `vhost-user-input` from PATH, else
`/mnt/vmstore/leandro/build-refactor-thesis/target/release/vhost-user-input`);
`CARAXES_INPUT=evdev:/dev/input/eventN` forwards a host device verbatim instead (never the
operator's own keyboard or mouse without asking). `down` stops the backend. The device advertises
`EV_KEY` (all keys and buttons) and `EV_REL` (X, Y, wheel), so the guest sees one combined
mouse and keyboard, `caraxes-vm0-pointer`.

f6: GNOME took the device on seat0 (`ID_INPUT_MOUSE=1`, `ID_INPUT_KEYBOARD=1`), and 60 relative
moves from the host arrived as 180 of 180 evdev events. Over the stream, Moonlight's mouse and
keyboard reach the guest through Sunshine's own uinput devices (also on seat0); the virtio
pointer gives the session a pointer from boot, before and without a stream, and lets the host
script the desktop.

Side effect: the virtio-input function takes PCI slot 00:05.0 and the GPU moves to 00:06.0.
Nothing in the tools depends on the GPU's address (the DMA module and `probe` find it by vendor
id), and `RmInitAdapter` succeeded there.

## 9. Open items

- **One GR MMU fault at GNOME's start (f2, 13:20:35)**: Xid 31 `FAULT_PTE`
  `ACCESS_TYPE_VIRT_WRITE` by `GPCCLIENT_PROP_0` at VA 0x1_202c0000, channel 0x32, device pid;
  one host RC event, the session came up and ran the demos. It did not recur at the four later
  GNOME starts (f3b, f5, f6, f7). The same Xid (channel 0x32, root 0xe90f0000, PROP_0
  `FAULT_PTE` at VA 0x1_20281000) came at GNOME's start of run B1 of the interrupt page series
  (intr-page.md §9). **Finding (2026-09-24 evening): the fault comes before the doorbell, while
  the worker is still applying a plan of that root. The doorbell barrier holds. The GPPut store
  that precedes the doorbell does not wait for it.**
  - The order can be read from the trace after all. The worker's plan lines and the event
    thread's RC line both go through `IntrShared::note`, one `Vec` (intr.rs:119-121; worker
    mirror.rs:2063-2065, event thread mirror.rs:2258-2267). The time stamp is the drain time, but
    the order within a drained batch is the real order.
  - f2 (`f2-demos-gnome/device-events.txt.xz`, trace µs):
    - 811261509: doorbell 40 of guest chid 2 (host token 0x32). The doorbell's own resync
      maps 8 leaves.
    - 811263297 and 811266380: two MMU invalidates of pdb 0xe90f0000, which submit plan A
      (19 leaves walked, 8 mapped) and plan B (293 walked, 1 mapped) to the worker.
    - Then 80 ms without a line: the server thread waits in the barrier of doorbell 41
      (mirror.rs:491-494, `ShadowWorker::wait_all`).
    - 811347069: doorbell 41 forwarded.
    - One drained batch follows, in this order: the host RC event (exceptType 0x1f), then
      plan A done ("apply 75719 us after 10 us in the worker's queue"), then plan B done
      ("apply 7796 us after 72804 us in the worker's queue").
    - So the RC came before plan A finished, at about 811339 ms. That is before doorbell 41
      was forwarded.
    - Channel 0x32 is the only GR shadow root at that point.
  - B1 (`B1-page-on-uncoalesced/traps-mutter/device-events.txt`) has the same shape.
    - 159834178: an invalidate submits an 8-leaf plan.
    - 159921544: doorbell 30 forwarded, 87 ms later.
    - 159924437: the RC, noted before that plan's line ("apply 74082 us").
  - A doorbell cannot pass an unapplied plan. Every submit runs on the server thread, and
    `wait_all` (mirror.rs:2099-2109) waits for a single global sequence number. It therefore
    covers every root and every job, the doorbell's own resync included (mirror.rs:491 then
    :494). A leaf of another root is not the cause either: only one shadow root existed at
    either fault, and the barrier is global anyway.
  - **The gap.** The host channel runs on the guest's own USERD (the host RM's clear and the
    restore, mirror.rs:543-560 and :796-805). The guest's GPPut store (USERD + 0x8C, a trapped
    BAR1 write: device.rs:843, boot.rs:384-389 `bar_access`, fb.rs:1481-1512 `bar_access`) reaches VRAM at once,
    with no barrier. The guest's order is: page tables, invalidate (plan queued), GPFIFO entry,
    GPPut, doorbell. A host channel that is still loaded can fetch the new GPPut without a
    doorbell. Doorbells 31 to 40 came in quick succession before 41, so the channel was
    likely loaded. This mechanism is inferred, not observed. The plan lines do not name their
    VAs, so whether plan A maps 0x1_202c0000 is likely (its 8 leaves are the same shape as the
    first-doorbell plans) but not shown.
  - **Fix (in this package, small).** `BootModel::gp_put_barrier` (boot.rs:401-428) runs before a
    trapped BAR1 write, and only while the worker has plans pending
    (`Mirror::shadow_pending`). When the write covers the GPPut dword of a mirrored channel's
    USERD (`Mirror::mirrored_userds`, `stores_gp_put`), it waits for the worker as the
    doorbell does. The wait is traced as an event, `shadow: BAR1 write ...: fb ... (GPPut of a
    mirrored channel held N us until the shadow worker applied its plans)`. In steady state no
    plan is pending (0 per Mutter frame, §6), so it costs nothing there. Test:
    `boot::tests::gp_put_stores_of_mirrored_channels_are_found`. It does not cover a USERD
    that does not trap. The P21 BAR1 pool keeps a mirrored channel's USERD page trapped
    (bar1pool.rs:15-22, :457-458), but the P21 whole-BAR1 passthrough does not. There, the
    safe version is the barrier at the invalidate (a `wait_all` at the end of
    `mirror_shadow_resync`), which costs the invalidate the apply time (13-75 ms in these
    plans).
  - **The BAR2 memfd.** f2 and B1 ran without `CARAXES_BAR2_MEMFD`. The page-table writes were
    trapped (the plans read 0 tables), and the invalidate syncs the BAR2 memfd before its walk
    (boot.rs:718, then the resync at :725-734). With the memfd, the invalidate stays the
    sync point, so the display path does not need
    `CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1`: flips and scanout imports reach the GPU only through
    GR/CE pushes after the guest's invalidate. That flag would not close this gap either,
    because the gap is after the sync.
  - Measurement: five fresh GNOME starts per configuration in
    `caraxes/runs/display-page-2026-09-24/series.sh` (intr-page.md §6a). The fix predicts 0
    Xid. The barrier's events show how often a GPPut store met a pending plan. The reference
    without the fix is f2 and B1, one Xid in six GNOME starts. A negative result needs the
    counter-check: `CARAXES_SHADOW_WORKER=0` (plans synchronous at the invalidate) should also
    give 0 Xid.
- Mutter at 30-41 Hz: interrupt servicing, §6.
- The stream at 26 FPS: the per-frame capture import and x264, §7; NVENC needs an NVENC mirror.
- es2gears_wayland and eglgears_wayland run but print no frame rate.
- `bench.sh stream` itself does not know `capture=kms`; the Caraxes variant lives in the run
  directory. A `--capture kms` in Leandro-Test would make it one method.
