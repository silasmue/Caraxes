#!/bin/sh
# SPDX-License-Identifier: MIT
# The boot-only check of the costume ladder (docs/caraxes/costume-ladder.md): runs once per
# boot in the tapless VM vm6 (systemd unit caraxes-check.service), writes everything to the
# serial console and powers the VM off, so the device exits with its access summary.
exec > /dev/ttyS0 2>&1
echo "CARAXES-CHECK begin $(date -Is)"
lspci -nn -d 10de:
depmod -a
modprobe nvidia; echo "modprobe rc=$?"
[ -e /dev/nvidiactl ] || mknod -m 666 /dev/nvidiactl c 195 255
[ -e /dev/nvidia0 ] || mknod -m 666 /dev/nvidia0 c 195 0
dmesg | grep -E "NVRM|nvidia" | grep -v "NVRM: VM:"
echo "CARAXES-CHECK information"
cat /proc/driver/nvidia/gpus/*/information
echo "CARAXES-CHECK smi"
timeout 90 /opt/nvrm/bin/nvidia-smi; echo "nvidia-smi rc=$?"
timeout 90 /opt/nvrm/bin/nvidia-smi --query-gpu=name,compute_cap,pci.device_id,vbios_version,memory.total --format=csv; echo "query rc=$?"
echo "CARAXES-CHECK smi-q"
timeout 120 /opt/nvrm/bin/nvidia-smi -q; echo "smi-q rc=$?"
echo "CARAXES-CHECK dmesg-after"
dmesg | grep -E "NVRM|nvidia|Xid" | grep -v "NVRM: VM:" | tail -n 60
echo "CARAXES-CHECK end"
sync
systemctl poweroff
