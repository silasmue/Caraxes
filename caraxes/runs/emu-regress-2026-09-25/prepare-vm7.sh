#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The state folders of the pure-emulation regression runs (README.md next to this script),
# offline edits, no boot:
#   vm7      a copy of the costume ladder's vm6 overlay (610 image with the guest modules of
#            vm0, the 610.57.04 gsp_tu10x.bin and gsp_ga10x.bin, gdm, the serial getty and the
#            network waits masked; docs/caraxes/costume-ladder.md section 2) and its seed
#   vm7-615  a fresh overlay on the Caraxes 615 base image, prepared as vm3-615
#            (caraxes/runs/gsp-rpc-2026-09-24/prepare-vm3-615.sh) plus vm6's masks, with vm3's
#            seed; the base image carries the 615.71.09 firmware
# Both get this directory's guest-check.sh as caraxes-check.service and uvm-register.c.
# CARAXES_GUEST_CONF=1 (the second step of README.md) also gives vm7 the guest module options
# of docs/caraxes/quickstart.md section 2 step 3 (/etc/modprobe.d/caraxes.conf, the registry
# keys RmDisableGlobalCeUtils=1, RMDisableScrubOnFree=1, RmRcWatchdog=0), which vm0 to vm5
# carry and the 615 base image ships, but which vm6's fresh overlay on the 610 image lacked.
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
SRC=$(cd "$HERE/../../.." && pwd -P)
S=/mnt/vmstore/caraxes
BASE615=$S/images/guest-baked-2026-09-24-615.71.09.qcow2
if [[ ! -e $S/vm7/rootfs.qcow2 ]]; then
    mkdir -p "$S/vm7"
    cp "$S/vm6/rootfs.qcow2" "$S/vm6/seed.img" "$S/vm7/"
fi
if [[ ! -e $S/vm7-615/rootfs.qcow2 ]]; then
    mkdir -p "$S/vm7-615"
    qemu-img create -f qcow2 -F qcow2 -b "$BASE615" "$S/vm7-615/rootfs.qcow2"
    cp "$S/vm3-615/seed.img" "$S/vm7-615/seed.img"
    conf=$(mktemp)
    printf '%s\n' "# Caraxes: no display modules at boot (prepare-vm3-615.sh)" \
        "blacklist nvidia_modeset" "blacklist nvidia_drm" "blacklist nvidia_peermem" \
        "install nvidia_modeset /bin/false" "install nvidia_drm /bin/false" \
        "install nvidia_peermem /bin/false" > "$conf"
    guestfish -a "$S/vm7-615/rootfs.qcow2" -i <<GF
upload $conf /etc/modprobe.d/caraxes-615-autoload.conf
chmod 0644 /etc/modprobe.d/caraxes-615-autoload.conf
ln-sf /dev/null /etc/systemd/system/nvidia-persistenced.service
ln-sf /dev/null /etc/systemd/system/gdm.service
ln-sf /dev/null /etc/systemd/system/serial-getty@ttyS0.service
ln-sf /dev/null /etc/systemd/system/NetworkManager-wait-online.service
ln-sf /dev/null /etc/systemd/system/systemd-networkd-wait-online.service
GF
    rm -f "$conf"
fi
unit=$(mktemp)
printf '%s\n' "[Unit]" "Description=Caraxes pure-emulation boot check" "After=multi-user.target" "" \
    "[Service]" "Type=oneshot" "ExecStart=/usr/local/sbin/caraxes-check.sh" "" \
    "[Install]" "WantedBy=multi-user.target" > "$unit"
for vm in vm7 vm7-615; do
    guestfish -a "$S/$vm/rootfs.qcow2" -i <<GF
upload $unit /etc/systemd/system/caraxes-check.service
upload $HERE/guest-check.sh /usr/local/sbin/caraxes-check.sh
chmod 0755 /usr/local/sbin/caraxes-check.sh
mkdir-p /usr/local/share/caraxes
upload $SRC/crates/vfio-user-nvgpu/guest/uvm-register.c /usr/local/share/caraxes/uvm-register.c
ln-sf /etc/systemd/system/caraxes-check.service /etc/systemd/system/multi-user.target.wants/caraxes-check.service
GF
done
rm -f "$unit"
if [[ ${CARAXES_GUEST_CONF:-0} == 1 ]]; then
    conf=$(mktemp)
    printf '%s\n' \
        'options nvidia NVreg_GpuInitOnProbe=1 NVreg_ResmanDebugLevel=0 NVreg_RegistryDwords="RmDisableGlobalCeUtils=1;RMDisableScrubOnFree=1;RmRcWatchdog=0"' \
        'options nvidia-uvm uvm_disable_hmm=1' 'blacklist nvidia' > "$conf"
    guestfish -a "$S/vm7/rootfs.qcow2" -i <<GF
upload $conf /etc/modprobe.d/caraxes.conf
chmod 0644 /etc/modprobe.d/caraxes.conf
GF
    rm -f "$conf"
fi
for vm in vm7 vm7-615; do
    echo "== $vm"
    guestfish --ro -a "$S/$vm/rootfs.qcow2" -i sh 'ls -la /etc/systemd/system/multi-user.target.wants/caraxes-check.service /usr/local/sbin/caraxes-check.sh; ls /lib/firmware/nvidia/*/; cat /etc/modprobe.d/caraxes.conf 2>/dev/null; true'
done
