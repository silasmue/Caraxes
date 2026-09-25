#!/bin/sh
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The boot-time check of the limits runs (README.md next to this script): runs once per
# boot in the tapless vm8 as caraxes-check.service, loads nvidia, runs the scenario the host
# uploaded for this boot (/usr/local/sbin/caraxes-scenario.sh, one of scenarios/*.sh; the
# host's run.sh uploads it with guestfish before the boot), writes everything to the serial
# console and powers the VM off. The scenario calls `step <label> <command...>`, which prints
# the command, its output and its exit code between markers.
exec > /dev/ttyS0 2>&1
echo "CARAXES-CHECK begin $(date -Is)"
echo 1 > /proc/sys/kernel/sysrq
( sleep 540; echo "CARAXES-CHECK watchdog: forced power-off"; sync; echo o > /proc/sysrq-trigger ) &
SMI=/opt/nvrm/bin/nvidia-smi
export PATH=/opt/nvrm/bin:$PATH LD_LIBRARY_PATH=/opt/nvrm/lib
step() {
    label=$1; shift
    echo "CARAXES-STEP $label: $*"
    timeout 120 "$@"
    echo "CARAXES-STEP $label rc=$?"
}
lspci -nn -d 10de:
depmod -a
modprobe nvidia; echo "modprobe rc=$?"
[ -e /dev/nvidiactl ] || mknod -m 666 /dev/nvidiactl c 195 255
[ -e /dev/nvidia0 ] || mknod -m 666 /dev/nvidia0 c 195 0
dmesg | grep -E "NVRM|nvidia" | grep -v "NVRM: VM:"
echo "CARAXES-CHECK scenario $(head -n 3 /usr/local/sbin/caraxes-scenario.sh | tail -n 1)"
dmesg -C
. /usr/local/sbin/caraxes-scenario.sh
echo "CARAXES-CHECK dmesg-after"
dmesg | grep -E "NVRM|nvidia|Xid" | grep -v -E "NVRM: VM:|NVRM: ioctl\(|nvidia_close|nvidia_ctl_close|Opening GPU with minor" | tail -n 120
echo "CARAXES-CHECK end"
sync
systemctl poweroff
