#!/bin/bash
# SPDX-License-Identifier: MIT
# Guest (root), vm0-615 after session s2-gate (docs/caraxes/display-615.md §3.1): the image's
# DKMS nvidia-drm.ko 615.71.09 does not load. Its HDCP content-protection property (new in the
# 615 nvidia-drm, kernel-open/nvidia-drm/nvidia-drm-connector.c:759-779, :913-946) calls
# drm_hdcp_update_content_protection and drm_connector_attach_content_protection_property of
# drm_display_helper.ko, and the image has no such module (6.8.0-136-generic's lives in
# linux-modules-extra, which the image lacks): "nvidia_drm: Unknown symbol ... (err -2)".
# Instead of that package (not local), this builds nvidia-drm.ko from the image's own DKMS
# source (/usr/src/nvidia-615.71.09) with that property configured out: the one #if of
# nvidia-drm/nvidia-drm-conftest.h:105 that defines NV_DRM_SUPPORT_CONTENT_PROTECTION_PROPERTY
# becomes #if 0. nvidia.ko and nvidia-modeset.ko stay the DKMS ones (nvidia-drm is built
# against the same source, so the symbol CRCs match). The module goes to
# /lib/modules/<kernel>/caraxes/nvidia-drm.ko, and the nvidia_drm install rule of
# /etc/modprobe.d/caraxes-615-display.conf loads that file (still only after caraxes_dma_range).
set -eu
V=615.71.09
K=$(uname -r)
B=/root/nvdrm-615
rm -rf "$B"
cp -a "/usr/src/nvidia-$V" "$B"
cd "$B"
H=nvidia-drm/nvidia-drm-conftest.h
grep -q '^#if (IS_ENABLED(CONFIG_DRM_DISPLAY_HDCP_HELPER))$' "$H" || { echo "ERROR: $H has no HDCP helper #if"; exit 1; }
sed -i 's|^#if (IS_ENABLED(CONFIG_DRM_DISPLAY_HDCP_HELPER))$|#if 0 /* Caraxes display-615: no drm_display_helper.ko in the image */|' "$H"
grep -n 'Caraxes display-615' "$H"
make -j"$(nproc)" SYSSRC="/lib/modules/$K/build" NV_KERNEL_MODULES="nvidia nvidia-modeset nvidia-drm" modules > build.log 2>&1 \
    || { tail -30 build.log; exit 1; }
echo "built: $(modinfo -F version nvidia-drm.ko), depends: $(modinfo -F depends nvidia-drm.ko), vermagic: $(modinfo -F vermagic nvidia-drm.ko)"
if nm -u nvidia-drm.ko | grep -E 'drm_hdcp|content_protection'; then echo "ERROR: still HDCP helper symbols"; exit 1; fi
mkdir -p "/lib/modules/$K/caraxes"
install -m 0644 nvidia-drm.ko "/lib/modules/$K/caraxes/nvidia-drm.ko"
sha256sum "/lib/modules/$K/caraxes/nvidia-drm.ko"
C=/etc/modprobe.d/caraxes-615-display.conf
sed -i "s|^install nvidia_drm .*|install nvidia_drm grep -q '^caraxes_dma_range ' /proc/modules \&\& /sbin/modprobe nvidia_modeset \&\& /sbin/insmod /lib/modules/$K/caraxes/nvidia-drm.ko \$CMDLINE_OPTS|" "$C"
grep '^install nvidia_drm' "$C"
sync
