#!/bin/bash
# SPDX-License-Identifier: MIT
# Guest (root), first boot of vm0-615 (docs/caraxes/display-615.md §2): the display setup vm0
# got in display-cd phase D and display-e, from the material prepare-vm0-615.sh put into the
# overlay, nothing from the network:
#  1. the local .debs vm0 installed (~/caraxes-debs: Weston 13, edid-decode and their
#     libraries; ~/caraxes-debs2: the Wayland development files; ~/caraxes-debs3:
#     libvulkan-dev), with dpkg -i; a missing dependency is reported and stops here;
#  2. gdm-wayland-setup.sh (Wayland on, autologin session ubuntu-wayland, gdm's NVIDIA udev
#     rule masked), then nvidia-egl-setup.sh on (EGL and GBM on the NVIDIA driver);
#  3. the display probes of ~/caraxes-guest (build-wl.sh: wlframes, kmsflip, kmsgrab).
set -u
H=/home/leandro
cd "$H" || exit 1
echo "== dpkg"
for d in caraxes-debs caraxes-debs2 caraxes-debs3; do
    dpkg -i "$d"/*.deb > "/root/dpkg-$d.log" 2>&1
    echo "$d: dpkg rc=$? ($(ls "$d"/*.deb | wc -l) debs)"
    grep -E "dependency problems|depends on .* however|not installed" "/root/dpkg-$d.log" | head -8
done
dpkg -l weston libweston-13-0 edid-decode libwayland-dev wayland-protocols libvulkan-dev sunshine gnome-shell 2>/dev/null \
    | awk '/^[a-z][a-z] /{print $1, $2, $3}'
command -v weston >/dev/null || { echo "ERROR: weston not installed"; exit 1; }
echo "== gdm-wayland-setup"
bash "$H/caraxes-guest/gdm-wayland-setup.sh" 2>&1 | tail -6
echo "== nvidia-egl-setup on"
bash "$H/caraxes-guest/nvidia-egl-setup.sh" on 2>&1 | tail -4
echo "== build-wl"
sudo -u leandro bash "$H/caraxes-guest/build-wl.sh" 2>&1 | tail -3
echo "== GL userspace"
cat /opt/nvrm-gl/README.caraxes
ldconfig -p | grep -E "libEGL_nvidia|libGLX_nvidia|libnvidia-eglcore|libcuda.so.1"
sync
