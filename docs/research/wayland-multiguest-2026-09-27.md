<!-- SPDX-License-Identifier: MIT -->
# Stock 615 Wayland and concurrent video checks

Cloud Hypervisor remains the preferred Caraxes VMM. QEMU is used for Windows
kernel debugging and a bounded Linux comparison; its Linux run is still pending.
Evidence: `/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-checks/`.

## Reproducible configuration

The `desktop-a` and `desktop-b` test disks are independent children of the stopped
`/mnt/vmstore/caraxes/vm4-615/rootfs.qcow2`. They use the distribution-packaged
NVIDIA 615.71.09 driver and matching GL/Vulkan/encode/decode userspace, Ubuntu
24.04, GNOME Wayland, and Sunshine KMS capture with `h264_nvenc`. No staged ICD
or library override is needed on this prepared desktop image.

Each VM has 4 GiB RAM, four vCPUs, and 2 GiB reserved VRAM. Reservation slots 0
and 1 do not overlap. Each has a 32 MiB BAR1 pool and two 16 MiB mapping windows;
1 GiB host VRAM and 64 MiB host BAR1 floors remain enabled. Backend SHA-256:
`dbf1c6e271554f15a2f6fdd623a62a0f7f430b907abba58e82168b4e34ffa348`.

Both use registered guest RAM, `CARAXES_GUEST_DMA_GPA=1`, GR/CE shadow mappings,
64 doorbell tokens, hybrid polling, interrupt/display pages, BAR2 memfd,
MSI re-arm ioeventfd and lazy allocation. The guest has no loaded
`caraxes_dma_range`; an extra modprobe blacklist/install refusal in the test
copies prevents accidental loading. Stock NVIDIA modules remain unmodified.

The old desktop image explicitly gated `nvidia_modeset` and `nvidia_drm` loading
on the Caraxes helper. Removing that dependency was necessary. Commit
`b72bdfc2` makes GPA-mode provisioning migrate old prepared overlays despite their
existing done marker; helper mode remains selectable. The new provisioning step
was run twice in desktop-a: first migrated, second reported already configured.
A networkd/NetworkManager ownership conflict in the copied image also required
an explicit MAC-matched networkd configuration. This is image setup, not a GPU fix.

## Results

- GNOME reports `Type=wayland`; Vulkan and GL identify NVIDIA / Leandro VFIO RTX
  2070-2G. The desktop user reported that interaction felt good.
- A native Wayland `vkcube-wayland --c 180` completes before streaming and across
  six subsequent short runs around/during stream startup. Three runs with the
  stream established took 10.97 / 4.92 / 6.18 seconds including device creation.
  These are complete-process durations, not per-frame latency measurements.
- `glmark2-wayland`, 800x600, build scene for ten seconds: NVIDIA renderer,
  331 FPS reported during streaming. This synthetic scene is not a native-speed
  or game-performance comparison.
- Own `nvenc-probe` without streaming: 120 1080p H.264 frames encoded at 116.1 FPS
  and decoded at 115.4 FPS, with the luma compared to the generated pattern.
  During streaming the same check passes at 98.0 / 94.6 FPS.
- Two guests, three concurrent rounds, while the first Sunshine stream remains
  active: A encodes 300 1080p H.264 frames per round; B encodes 450 720p HEVC frames.
  Both are paced to 60 FPS and achieve about 60.1 FPS. A decodes at 110.9–116.9 FPS;
  B at 226.8–274.6 FPS. All six round trips pass bitstream and decoded-luma checks.
  Different lengths deliberately overlap encoding and decoding across guests;
  differing codecs/resolutions distinguish the streams.
- With two connected Sunshine/Moonlight desktops, both guests additionally
  complete `vkcube-wayland --c 300` (7.45 / 8.71 seconds) and another video round
  trip. A decodes its 300 frames at 90.4 FPS; B its 450 at 203.3 FPS. Both pass.
- No new host Xids. The two historical Xids preceding this work are retained in
  captured dmesg and must not be attributed to these trials.

## Moonlight session summaries

These are whole-session aggregates, including startup and the changing test
workloads, not a controlled A/B benchmark or an end-to-end input-latency test.
Moonlight decoded in software on the host; guest NVDEC was tested separately by
the round-trip probes above. Both streams requested 1920x1080 at 60 FPS.

| Metric | Guest A | Guest B |
|---|---:|---:|
| Incoming FPS | 58.49 | 58.20 |
| Rendered FPS | 58.45 | 58.02 |
| Sunshine processing mean | 11.3 ms | 14.2 ms |
| Sunshine processing maximum | 749.6 ms | 115.3 ms |
| Network frame drops | 0.00% | 0.00% |
| Jitter frame drops | 0.01% | 0.04% |
| Client decode mean | 0.96 ms | 1.24 ms |

A 15-second host sample during the single-stream graphics trials recorded
70.93% of one CPU for the backend, 103.67% for Cloud Hypervisor and 20.80% for
Moonlight. This is not idle CPU. Raw guest pidstat samples are also archived.

## Remaining gates

The user reported vkcube instability; the observed short tests intentionally
close after a frame limit, so distinguish expected closure from stalls or visual
corruption. No crash was recorded in these runs. Long processing outliers during
graphics startup remain actionable, despite the much better observed desktop.

Still needed: long repeated connection/restart soak; hardware-decoded clients;
input-to-visible-frame measurement; per-guest CPU and pacing under stable matched
loads; resource exhaustion/recovery; full SotTR and Windows rendering. Short
successful concurrency is not a supported-alpha or hostile-tenant isolation proof.

The historical C5C0 allocation failure with live CUDA/NVENC was already fixed by
commit `583d995b` (doorbell token collision handling). Do not attribute that fix
to GPA mode or to this session's provisioning changes.
