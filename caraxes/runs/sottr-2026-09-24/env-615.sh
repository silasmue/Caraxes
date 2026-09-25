# SPDX-License-Identifier: MIT
# The SotTR stage on the 615 guest (docs/caraxes/sottr-stage.md section 8), sourced by env.sh
# when SOTTR_DRIVER=615 (up-615.sh, stream-615.sh and down-615.sh set it). The game VM is
# vm4-615: the same tap, address, MAC, profile and slot as vm4 (so only one of the two runs),
# an overlay on the Caraxes 615 base image (CARAXES_DRIVER=615: the launcher's check_base and
# --guest-abi 615), the v615 device build of main, and the guest driver as Ubuntu installs it:
# NVIDIA's CUDA repository packages of 615.71.09 (nvidia-driver-open with its GL, EGL, Vulkan,
# encode and decode libraries) and linux-modules-extra for nvidia-drm's drm_display_helper.
# No hand-built module, no /opt/nvrm-gl, no ld.so or ICD edits (guest/sottr-prepare.sh, the
# 615 steps).
export VM=vm4-615 CARAXES_DRIVER=615
export CARAXES_SERVER=${SOTTR_SERVER:-/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-22d58534}
export SOTTR_BASE=/mnt/vmstore/caraxes/images/guest-baked-2026-09-24-615.71.09.qcow2
# The exact package version (NVIDIA's CUDA repository for ubuntu2404) and the image's kernel.
export SOTTR_PKG_VERSION=615.71.09-2ubuntu1 SOTTR_GUEST_KERNEL=6.8.0-136-generic
unset SOTTR_OGKM SOTTR_GSP_SHA256
# Sunshine prefers NVENC (libnvidia-encode of the packages); when NVENC does not initialise it
# probes its other encoders and takes libx264 with sw_preset ultrafast and sw_tune zerolatency.
# The device has no NVENC mirror yet (2026-09-25), so the fallback is expected; sunshine.log
# names the encoder it created. SOTTR_SUN_ENCODER=software skips the probe.
export SOTTR_SUN_ENCODER=${SOTTR_SUN_ENCODER:-nvenc}
