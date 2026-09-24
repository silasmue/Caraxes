#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Guest (root), display phase E of docs/caraxes/display-e.md: send EGL and GBM to the
# NVIDIA driver instead of Mesa's kms_swrast (the override gdm-wayland-setup.sh put in
# /etc/environment for the device without host execution).
#  - /etc/environment: the Mesa EGL vendor pin and MESA_LOADER_DRIVER_OVERRIDE go; glvnd
#    then tries 10_nvidia.json first (the image's NVIDIA stack lives in /opt/nvrm-gl);
#  - GBM: libgbm looks for <drm driver>_gbm.so in /usr/lib/x86_64-linux-gnu/gbm; the image
#    lacks nvidia-drm_gbm.so, the .run installer's link to libnvidia-allocator.so.1.
# Usage: nvidia-egl-setup.sh [on|off]  (off restores the Mesa kms_swrast setting)
set -eu
MODE=${1:-on}
GBM=/usr/lib/x86_64-linux-gnu/gbm/nvidia-drm_gbm.so
[ -f /etc/environment.caraxes-orig ] || cp /etc/environment /etc/environment.caraxes-orig
sed -i '/^# CARAXES display/d; /^__EGL_VENDOR_LIBRARY_FILENAMES=/d; /^MESA_LOADER_DRIVER_OVERRIDE=/d' /etc/environment
case $MODE in
on)
    echo '# CARAXES display-e: EGL/GBM on the NVIDIA driver (host execution on)' >> /etc/environment
    ln -sfn /opt/nvrm-gl/lib/libnvidia-allocator.so.1 "$GBM"
    ;;
off)
    echo '# CARAXES display-cd phase D: Mesa EGL/GBM with kms_swrast (no GPU execution on vm0)' >> /etc/environment
    echo '__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json' >> /etc/environment
    echo 'MESA_LOADER_DRIVER_OVERRIDE=kms_swrast' >> /etc/environment
    rm -f "$GBM"
    ;;
*) echo "usage: $0 [on|off]" >&2; exit 2 ;;
esac
ldconfig
sync
tail -3 /etc/environment
ls -la "$GBM" 2>/dev/null || echo "no $GBM"
