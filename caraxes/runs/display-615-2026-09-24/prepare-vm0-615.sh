#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The vm0-615 state folder for the 615 display runs (docs/caraxes/display-615.md §2): an
# overlay on the Caraxes 615 base image, prepared offline (no boot, nothing from the network):
#  - the cloud-init seed of vm0 (192.168.100.20, tap caraxes0, MAC 02:00:00:00:00:20) with a
#    new instance id (seed-vm3-615.sh's recipe);
#  - nvidia-persistenced masked; nvidia_peermem blocked; nvidia_modeset and nvidia_drm allowed,
#    but only once caraxes_dma_range is loaded (the launcher's DMA table step), so that nothing
#    loads them at boot and `tools/caraxes-vm.sh probe` can still reload nvidia; default target
#    multi-user.target (gdm3 is started by hand after display-up.sh, as on vm0);
#  - the display userspace vm0 got in display-cd and display-e, from the local copies only:
#    the .debs in vm0's overlay (~/caraxes-debs: Weston 13 and edid-decode, ~/caraxes-debs2: the
#    Wayland development files, ~/caraxes-debs3: libvulkan-dev), copied into ~/caraxes-debs*
#    of the new overlay and installed at the first boot by setup-guest.sh;
#  - the 615.71.09 GL, EGL, GLES and Vulkan userspace the image lacks (guest-driver-versions.md
#    §8: no libnvidia-gl-615): the NVIDIA libraries of the host's own 615.71.09 install
#    (nvidia-utils 615.71.09, the same binaries NVIDIA ships for every distribution, glibc
#    2.10 at most), in /opt/nvrm-gl/lib; the image's 610.57.04 copy moves to
#    /opt/nvrm-gl-610.57.04; the vendor-neutral EGL platform libraries (egl-wayland, egl-gbm,
#    egl-xcb, egl-xlib) are kept from that copy; /etc/ld.so.conf.d/caraxes-gl-615.conf names
#    /opt/nvrm-gl/lib only (not /opt/nvrm/lib, which still holds 610.57.04 CUDA);
#  - crates/vfio-user-nvgpu/guest as ~/caraxes-guest.
# Usage (from the worktree root): prepare-vm0-615.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
ROOT=$(cd "$here/../../.." && pwd -P)
W=/mnt/vmstore/caraxes/vm0-615
BASE=/mnt/vmstore/caraxes/images/guest-baked-2026-09-24-615.71.09.qcow2
VM0=/mnt/vmstore/caraxes/vm0
V=615.71.09
[[ -e $W/rootfs.qcow2 ]] && { echo "ERROR: $W/rootfs.qcow2 exists" >&2; exit 2; }
pgrep -f "$VM0/" >/dev/null && { echo "ERROR: a vm0 process runs; vm0's overlay is read below" >&2; exit 2; }
[[ -f /usr/lib/libnvidia-glcore.so.$V ]] || { echo "ERROR: the host has no $V GL userspace" >&2; exit 2; }
mkdir -p "$W/seed"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

# ---- seed: vm0's user-data and network-config, a new instance id
S=$VM0/seed
cp "$S/user-data" "$S/network-config" "$W/seed/"
printf 'instance-id: caraxes-vm0-615-%s\nlocal-hostname: caraxes-vm0\n' "$(date +%s)" > "$W/seed/meta-data"
mkfs.vfat -C -n cidata "$W/seed.img" 1440 >/dev/null
mcopy -i "$W/seed.img" "$W/seed/user-data" "$W/seed/meta-data" "$W/seed/network-config" ::

# ---- the overlay
qemu-img create -f qcow2 -F qcow2 -b "$BASE" "$W/rootfs.qcow2"

# ---- the local .debs of vm0 (read-only)
mkdir -p "$stage/debs"
guestfish --ro -a "$VM0/rootfs.qcow2" -i <<EOF
copy-out /home/leandro/caraxes-debs $stage/debs
copy-out /home/leandro/caraxes-debs2 $stage/debs
copy-out /home/leandro/caraxes-debs3 $stage/debs
EOF

# ---- the 615.71.09 GL userspace of the host
mkdir -p "$stage/gl/lib"
for l in libEGL_nvidia libGLX_nvidia libGLESv2_nvidia libGLESv1_CM_nvidia libnvidia-eglcore \
         libnvidia-glcore libnvidia-glsi libnvidia-tls libnvidia-glvkspirv libnvidia-rtcore \
         libnvidia-allocator libnvidia-present libnvidia-gpucomp; do
    cp "/usr/lib/$l.so.$V" "$stage/gl/lib/"
done
( cd "$stage/gl/lib"
  ln -s libEGL_nvidia.so.$V libEGL_nvidia.so.0
  ln -s libGLX_nvidia.so.$V libGLX_nvidia.so.0
  ln -s libGLESv2_nvidia.so.$V libGLESv2_nvidia.so.2
  ln -s libGLESv1_CM_nvidia.so.$V libGLESv1_CM_nvidia.so.1
  ln -s libnvidia-allocator.so.$V libnvidia-allocator.so.1
  sha256sum ./*.so.$V > ../SHA256SUMS )
cp /usr/share/vulkan/icd.d/nvidia_icd.json "$stage/gl/nvidia_icd.json"
cat > "$stage/gl/README.caraxes" <<EOF
/opt/nvrm-gl: the NVIDIA $V GL, EGL, GLES and Vulkan libraries, copied from the host's
nvidia-utils $V on $(date -Iseconds) by caraxes/runs/display-615-2026-09-24/prepare-vm0-615.sh
(docs/caraxes/display-615.md). The image's 610.57.04 copy is /opt/nvrm-gl-610.57.04; the EGL
platform libraries (egl-wayland, egl-gbm, egl-xcb, egl-xlib) are its vendor-neutral ones.
EOF
cat > "$stage/caraxes-gl-615.conf" <<'EOF'
# Caraxes display-615: the 615.71.09 GL userspace (README.caraxes there)
/opt/nvrm-gl/lib
EOF
cat > "$stage/caraxes-615-display.conf" <<'EOF'
# Caraxes display-615 (docs/caraxes/display-615.md): nvidia_modeset and nvidia_drm load only
# after the launcher installed its DMA table (caraxes_dma_range), never at boot, so that
# `tools/caraxes-vm.sh probe` can reload nvidia; nvidia_peermem never.
blacklist nvidia_modeset
blacklist nvidia_drm
blacklist nvidia_peermem
install nvidia_modeset grep -q '^caraxes_dma_range ' /proc/modules && /sbin/modprobe --ignore-install nvidia_modeset $CMDLINE_OPTS
install nvidia_drm grep -q '^caraxes_dma_range ' /proc/modules && /sbin/modprobe --ignore-install nvidia_drm $CMDLINE_OPTS
install nvidia_peermem /bin/false
EOF

# ---- the offline edits
guestfish -a "$W/rootfs.qcow2" -i <<EOF
upload $stage/caraxes-615-display.conf /etc/modprobe.d/caraxes-615-display.conf
chmod 0644 /etc/modprobe.d/caraxes-615-display.conf
ln-sf /dev/null /etc/systemd/system/nvidia-persistenced.service
rm-f /etc/systemd/system/multi-user.target.wants/nvidia-persistenced.service
ln-sf /lib/systemd/system/multi-user.target /etc/systemd/system/default.target
mv /opt/nvrm-gl /opt/nvrm-gl-610.57.04
mkdir-p /opt/nvrm-gl
copy-in $stage/gl/lib /opt/nvrm-gl
cp-a /opt/nvrm-gl-610.57.04/egl_vendor.d /opt/nvrm-gl/
glob cp-a /opt/nvrm-gl-610.57.04/lib/libnvidia-egl-* /opt/nvrm-gl/lib/
upload $stage/gl/SHA256SUMS /opt/nvrm-gl/SHA256SUMS
upload $stage/gl/README.caraxes /opt/nvrm-gl/README.caraxes
upload $stage/gl/nvidia_icd.json /usr/share/vulkan/icd.d/nvidia_icd.json
upload $stage/caraxes-gl-615.conf /etc/ld.so.conf.d/caraxes-gl-615.conf
copy-in $stage/debs/caraxes-debs /home/leandro
copy-in $stage/debs/caraxes-debs2 /home/leandro
copy-in $stage/debs/caraxes-debs3 /home/leandro
copy-in $ROOT/crates/vfio-user-nvgpu/guest /home/leandro
mv /home/leandro/guest /home/leandro/caraxes-guest
sh "chown -R leandro:leandro /home/leandro/caraxes-debs /home/leandro/caraxes-debs2 /home/leandro/caraxes-debs3 /home/leandro/caraxes-guest"
sh "ldconfig"
EOF
guestfish --ro -a "$W/rootfs.qcow2" -i sh 'cat /etc/modprobe.d/caraxes-615-display.conf; ls -la /etc/systemd/system/nvidia-persistenced.service /etc/systemd/system/default.target; ls -la /opt/nvrm-gl/lib; cat /opt/nvrm-gl/egl_vendor.d/10_nvidia.json; ls /home/leandro/caraxes-debs* | wc -l; ls /home/leandro/caraxes-guest | wc -l; ldconfig -p | grep -E "libEGL_nvidia|libGLX_nvidia|libcuda.so.1"'
