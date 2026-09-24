#!/bin/bash
# SPDX-License-Identifier: MIT
# Guest (root), phase D of docs/caraxes/display-cd.md: prepare vm0 for a GNOME Wayland
# session on the Caraxes display. Every change is recorded in docs/caraxes/experiments.md.
#  - gdm3 enabled again (it was disabled in phase B); the default target stays
#    multi-user.target, so gdm3 is started by hand after the display modules are loaded
#    (nvidia is blacklisted from autoloading on this image);
#  - WaylandEnable=true (the image had false);
#  - gdm's udev rule that turns Wayland off on NVIDIA without
#    NVreg_PreserveVideoMemoryAllocations is masked (/etc/udev/rules.d/61-gdm.rules -> /dev/null);
#  - the user's session (AccountsService) is ubuntu-wayland (was ubuntu-xorg), and
#    accounts-daemon is restarted so gdm sees it;
#  - Mesa's EGL and GBM with kms_swrast (llvmpipe rendering into KMS dumb buffers) for the
#    compositor: the device runs without host execution, so NVIDIA's EGL has no GPU to
#    render with. Run d6 showed that LIBGL_ALWAYS_SOFTWARE does not work here: Mutter
#    selects the EGL device explicitly, Mesa refuses to force software rendering and
#    gnome-shell crashes in libgallium; naming kms_swrast works on any KMS device.
#    (/etc/environment is read by gdm and the session.)
# Revert: see the "undo" line printed at the end.
set -eu
[ -f /etc/gdm3/custom.conf.caraxes-orig ] || cp /etc/gdm3/custom.conf /etc/gdm3/custom.conf.caraxes-orig
sed -i 's/^WaylandEnable=false/WaylandEnable=true/' /etc/gdm3/custom.conf
grep -q '^WaylandEnable=true' /etc/gdm3/custom.conf || sed -i 's/^\[daemon\]/[daemon]\nWaylandEnable=true/' /etc/gdm3/custom.conf
ln -sf /dev/null /etc/udev/rules.d/61-gdm.rules
[ -f /etc/environment.caraxes-orig ] || cp /etc/environment /etc/environment.caraxes-orig
sed -i '/^LIBGL_ALWAYS_SOFTWARE=1$/d; /^GBM_ALWAYS_SOFTWARE=1$/d' /etc/environment
grep -q CARAXES /etc/environment || echo '# CARAXES display-cd phase D: Mesa EGL/GBM with kms_swrast (no GPU execution on vm0)' >> /etc/environment
grep -q '^__EGL_VENDOR_LIBRARY_FILENAMES=' /etc/environment || echo '__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json' >> /etc/environment
grep -q '^MESA_LOADER_DRIVER_OVERRIDE=' /etc/environment || echo 'MESA_LOADER_DRIVER_OVERRIDE=kms_swrast' >> /etc/environment
U=/var/lib/AccountsService/users/leandro
[ -f $U.caraxes-orig ] || cp $U $U.caraxes-orig
sed -i 's/^Session=ubuntu-xorg/Session=ubuntu-wayland/; /^XSession=/d' $U
systemctl restart accounts-daemon
systemctl enable gdm3 2>/dev/null || true
systemctl get-default
grep -v '^#' /etc/gdm3/custom.conf | grep .
tail -4 /etc/environment
sync
echo "undo: cp /etc/gdm3/custom.conf.caraxes-orig /etc/gdm3/custom.conf; cp /etc/environment.caraxes-orig /etc/environment; rm /etc/udev/rules.d/61-gdm.rules; cp $U.caraxes-orig $U; systemctl disable gdm3"
