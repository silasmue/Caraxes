#!/bin/sh
# SPDX-License-Identifier: MIT
# The boot-only check of the pure-emulation regression runs (README.md next to this script),
# extended from caraxes/runs/costume-ladder-2026-09-24/guest-check.sh: runs once per boot in
# the tapless VMs vm7 (610 guest) and vm7-615 (615 guest) as caraxes-check.service, writes
# everything to the serial console and powers the VM off. It covers the Blackwell N gate's
# guest side too (docs/caraxes/blackwell-gate.md section 1): N1 lspci, N2 modprobe and dmesg,
# N3 nvidia-smi -q, and on a GB206 (10de:2d04) N4, UVM registration with uvm-register.c.
exec > /dev/ttyS0 2>&1
echo "CARAXES-CHECK begin $(date -Is)"
# Force the power-off even when a step hangs in the kernel (a UVM push that never completes).
echo 1 > /proc/sys/kernel/sysrq
( sleep 420; echo "CARAXES-CHECK watchdog: forced power-off"; sync; echo o > /proc/sysrq-trigger ) &
lspci -nn -d 10de:
echo "CARAXES-CHECK lspci-vvv"
lspci -vvv -d 10de:
depmod -a
modprobe nvidia; echo "modprobe rc=$?"
[ -e /dev/nvidiactl ] || mknod -m 666 /dev/nvidiactl c 195 255
[ -e /dev/nvidia0 ] || mknod -m 666 /dev/nvidia0 c 195 0
ls -la /dev/nvidia0
dmesg | grep -E "NVRM|nvidia" | grep -v "NVRM: VM:"
SMI=/opt/nvrm/bin/nvidia-smi
case $(modinfo -F version nvidia 2>/dev/null) in 615.*) SMI=/usr/bin/nvidia-smi ;; esac
echo "CARAXES-CHECK information"
cat /proc/driver/nvidia/gpus/*/information
echo "CARAXES-CHECK smi"
timeout 90 $SMI; echo "nvidia-smi rc=$?"
timeout 90 $SMI --query-gpu=name,compute_cap,pci.device_id,vbios_version,memory.total --format=csv; echo "query rc=$?"
echo "CARAXES-CHECK smi-q"
timeout 120 $SMI -q; echo "smi-q rc=$?"
if lspci -n -d 10de:2d04 | grep -q .; then
    echo "CARAXES-CHECK uvm"
    modprobe nvidia-uvm uvm_debug_prints=1; echo "modprobe nvidia-uvm rc=$?"
    rm -f /dev/nvidia-uvm
    mknod -m 666 /dev/nvidia-uvm c "$(grep nvidia-uvm /proc/devices | cut -d' ' -f1)" 0
    cc -O2 -o /tmp/uvm-register /usr/local/share/caraxes/uvm-register.c; echo "cc rc=$?"
    ( timeout -s KILL 60 /tmp/uvm-register --flags 0; echo "uvm-register rc=$?" ) &
    sleep 70
    dmesg | grep -E "nvidia-uvm|UVM" | tail -n 30
fi
echo "CARAXES-CHECK dmesg-after"
dmesg | grep -E "NVRM|nvidia|Xid" | grep -v "NVRM: VM:" | tail -n 60
echo "CARAXES-CHECK end"
sync
systemctl poweroff
