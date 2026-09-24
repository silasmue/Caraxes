#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The vm5-615 state folder for the GSP plan round 2 (category C) runs: an overlay on the
# Caraxes 615 base image, prepared as caraxes/runs/gsp-rpc-2026-09-24/prepare-vm3-615.sh
# prepares vm3-615: autoload of nvidia_modeset, nvidia_drm and nvidia_peermem blocked,
# nvidia-persistenced masked. Offline edit, no boot.
set -euo pipefail
W=/mnt/vmstore/caraxes/vm5-615
BASE=/mnt/vmstore/caraxes/images/guest-baked-2026-09-24-615.71.09.qcow2
[[ -e $W/rootfs.qcow2 ]] && { echo "ERROR: $W/rootfs.qcow2 exists" >&2; exit 2; }
mkdir -p "$W"
qemu-img create -f qcow2 -F qcow2 -b "$BASE" "$W/rootfs.qcow2"
conf=$(mktemp)
cat > "$conf" <<'EOF'
# Caraxes: keep the 615 DKMS display modules from loading at boot, so that the launcher's
# probe step can install the DMA table and reload nvidia; the CUDA gates need no display module.
blacklist nvidia_modeset
blacklist nvidia_drm
blacklist nvidia_peermem
install nvidia_modeset /bin/false
install nvidia_drm /bin/false
install nvidia_peermem /bin/false
EOF
guestfish -a "$W/rootfs.qcow2" -i <<EOF
upload $conf /etc/modprobe.d/caraxes-615-autoload.conf
chmod 0644 /etc/modprobe.d/caraxes-615-autoload.conf
ln-sf /dev/null /etc/systemd/system/nvidia-persistenced.service
EOF
rm -f "$conf"
guestfish --ro -a "$W/rootfs.qcow2" -i sh 'cat /etc/modprobe.d/caraxes-615-autoload.conf; ls -la /etc/systemd/system/nvidia-persistenced.service'
