<!-- SPDX-License-Identifier: MIT -->
# D0: does the stock stack come up headless on the device?

Run 2026-09-23 on vm0, against the already-running device (`e2f1aec` plus the uncommitted
`crates/vfio-user-nvgpu`, boot model on, FB 4096 MiB, host-rm on; vm0 was not restarted).
Evidence: [../../caraxes/runs/display-d0-2026-09-23/](../../caraxes/runs/display-d0-2026-09-23/).
Changed nothing in the device or in this tree's code; guest-only, per
[display-path.md](display-path.md) section 6 and [kickoff.md](kickoff.md).

## What was run

1. Baseline: `nvidia.ko`/`nvidia-uvm.ko` loaded, no `nvidia-modeset`, no `/dev/dri`.
2. Built `nvidia-modeset.ko` in the guest (from `~/ogkm/open-gpu-kernel-modules`, the same
   tree `nvidia.ko` was built from) and loaded it; built `nvidia-drm.ko` from the same tree.
3. Meant to start Xorg with the NVIDIA driver, headless (`UseDisplayDevice "None"`, a
   1280x720 virtual screen, modules from `/opt/nvrm`), paint a solid colour and read it back.
4. Meant to retry with `ModeValidation`/`ConnectedMonitor "DFP-0"` if step 3 failed.
5. Restore: unload what was loaded, confirm `nvidia-smi -L`.

## What happened

**nvidia-modeset: loads clean, headless, no device traffic.** Building it required
`NV_KERNEL_MODULES="nvidia nvidia-modeset"` (a lone `nvidia-modeset` build fails at modpost
against a stale `Module.symvers`) and, after that, `modprobe video` first (the stock kernel's
ACPI backlight module -- nvidia-modeset references two of its exported symbols and the guest
image does not autoload it). With both of those, `insmod nvidia-modeset.ko` succeeds and logs
only `Loading NVIDIA UNIX Open Kernel Mode Setting Driver ... 610.57.04`. No
"displayless"/"no display" message is printed at load time (nvidia-modeset only checks that
once a client -- nvidia-drm or an X server -- asks). The device-side trace window for the
whole build-and-load sequence (572 lines) is pure BAR0 register reads with zero RPCs; see
"Display RPCs" below for the stronger, whole-trace version of this check.

**nvidia-drm: does not build against this guest kernel.** `make modules` fails inside
`nvidia-drm` with real API mismatches against Ubuntu 24.04.4's `6.8.0-136-generic` DRM
headers under `-Werror`: `drm_mode_create_dp_colorspace_property` and
`drm_mode_create_hdmi_colorspace_property` are called with too few arguments,
`NV_DRM_MODESET_LOCK_ALL_END_ARGUMENT_COUNT` and two
`NV_IS_EXPORT_SYMBOL_PRESENT_drm_gem_prime_*` conftest macros are never defined (read as 0
under `-Werror=undef`), and `drm_aperture_remove_conflicting_pci_framebuffers`/
`..._framebuffers` are called with a signature the installed headers no longer have. Full
error text: `../../caraxes/runs/display-d0-2026-09-23/step2-modeset-drm.txt`. This is a
source/kernel-header compatibility gap in the vendored `nvidia-drm` tree against this guest's
headers, unrelated to the Caraxes device or to the fuse/RPC question; no source was patched
to route around it (kickoff rule 2). Consequence: no `/dev/dri` node, no render node, no
connector count either way -- that part of D0's expectation is untested, not falsified.

**Xorg: not reached.** The guest has `Xorg` (1.21.1.11) but no NVIDIA X driver at all:
`nvidia_drv.so` and the GLX server module (`libglxserver_nvidia.so`) are absent everywhere
in the guest filesystem. Only the CUDA/NVML/GL client runtime was ever staged into
`/opt/nvrm`/`/opt/nvrm-gl`; the Xorg-side binary components were never part of that (matches
quickstart.md's own "scope is nvidia.ko only until CUDA is working"). The matching
610.57.04 files exist on the host (read-only `ls`/`find`/`sha256sum`, not host GPU state);
copying them into the guest -- the same pattern quickstart.md already uses for
`gsp_tu10x.bin` -- was attempted and was blocked by the session's own safety classifier
("Untrusted Code Integration" on a host-to-guest binary copy). That block was not worked
around; it is reported here as a decision for the operator. Steps 3 and 4 (the pixel
readback, the `ModeValidation`/`ConnectedMonitor` fallback) did not run.

**Restore: clean.** `rmmod nvidia_modeset` (nvidia-drm was never loaded) and `rmmod video`
brought the guest back to exactly the step-1 baseline; `nvidia-smi -L` still prints
`Leandro RTX 2070-4G`. vm0 was left running throughout, never restarted or rebuilt.

## Display RPCs (expected: none)

Checked two ways: the 8211-line device-trace window for this whole D0 run, and every one of
the 267459 lines the device has logged today (E0 through E5c, every gate run) -- the server
has run continuously, so the wider window is strictly more evidence for the same claim, not a
different measurement. Both give the same answer: **zero.** Neither window contains a single
display-class allocation (`NV04_DISPLAY_COMMON` 0x73, `NVC370/1/2/3`, `NVC57A/B/D/E`,
`NVA083_GRID_DISPLAYLESS`) or a single display-related RPC/control (`GET_IP_VERSION`
0x20800a4b, `INTERNAL_DISPLAY_GET_STATIC_INFO` 0x20800a01, `..._WRITE_INST_MEM` 0x20800a49,
`..._SET_IMP_INIT_INFO` 0x20800a54, `..._CHANNEL_PUSHBUFFER` 0x20800a58,
`INTERNAL_INIT_BRIGHTC_STATE_LOAD` 0x20800ac6, `INTERNAL_SET_STATIC_EDID_DATA` 0x20800adf,
`NV0073_CTRL_CMD_SYSTEM_MAP_SHARED_DATA` 0x730151). The display fuse
(`NV_FUSE_STATUS_OPT_DISPLAY`, BAR0 0x21C04) was read 3 times today, always `0x1` (no
display). Full accounting: `../../caraxes/runs/display-d0-2026-09-23/whole-trace-display-check.txt`.
This confirms `gpuStatePreInit` is deleting `ENG_KERNEL_DISPLAY` before any display RPC can
fire, exactly as display-path.md section 2.1 describes, and that neither `nvidia.ko` alone
nor `nvidia.ko` + `nvidia-modeset.ko` ever asks the device about display, matching the
expectation.

## Verdict against display-path.md's D0 expectations

| Expectation | Result |
|---|---|
| nvidia-drm registers a DRM device, zero connectors, a render node | untested: nvidia-drm does not build against this guest kernel |
| dmesg shows KAPI's "falling back to displayless mode" | untested: no nvidia-drm to log it; nvidia-modeset alone logs nothing display-related at load |
| the device trace shows no display RPC | **confirmed**, on both the D0 window and the whole day's trace |
| Xorg starts headless, paintprobe/XGetImage read back the colour | untested: no NVIDIA X driver in the guest image |
| falsifier: nvidia-drm fails to load | not reached this way -- it fails to **build**, earlier and harder than a load-time refusal |
| falsifier: the NVIDIA X driver refuses a GPU without display hardware | untested |

D0 is **inconclusive on the headless-stack question**, not falsified: nothing observed
contradicts display-path.md's prediction, but two independent guest-image gaps (nvidia-drm's
build against this kernel, and the missing X driver component) stopped the run before either
could be exercised. The one thing D0 does settle outright is the RPC question: the device
sees no display traffic under `nvidia.ko` + `nvidia-modeset.ko`, across the device's entire
history today, which is the strongest form of "none" this method can produce.

## What D1 needs

D1 (GRID displayless, [display-path.md](display-path.md) section 6.3) needs a
`GRID_BUILD=1` guest `nvidia.ko` and does not depend on `nvidia-drm` or Xorg the same way --
it changes what `NV04_DISPLAY_COMMON` resolves to (`NVA083_GRID_DISPLAYLESS` instead of
`NO_HARDWARE`) and is checked through `nvidia-smi`/DRM connector state, not necessarily
through X. Two things from D0 carry over and should be resolved first if D1 or a later X-based
check is wanted:

1. **nvidia-drm needs a kernel/headers combination it actually compiles against**, or the
   specific API deltas above need to be understood well enough to know whether they are
   cosmetic (a conftest gap for this particular 6.8.0-136-generic build) or a real version
   skew. This is independent of GRID and of the display fuse.
2. **The guest image has no NVIDIA X driver component.** If any experiment past D0 needs
   Xorg on the NVIDIA GPU, `nvidia_drv.so` and `libglxserver_nvidia.so` (version-matched,
   610.57.04, present on the host at `/usr/lib/xorg/modules/drivers/nvidia_drv.so` and
   `/usr/lib/nvidia/xorg/libglxserver_nvidia.so.610.57.04`) need to be added to the guest
   image or copied in for the run -- an operator decision, since the copy step tripped this
   session's own safety classifier.

D1 itself does not strictly need either fix (it is measured through `nvidia-smi`/DRM state
per display-path.md), but the capture/swapchain/present/stream gate stages that D0 was meant
to open up (display-path.md's table, row "headless stock") stay closed until both are
resolved.
