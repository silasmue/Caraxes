#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# In the guest (desktop-b-dmem on the HWE kernel, modules loaded, GNOME and Sunshine up), as
# the ssh user: the checks before the game (docs/research/dmem-cap-2026-09-29.md, test plan
# step 1). Read-only except for two short-lived transient scopes in app.slice (vulkaninfo and
# nvidia-smi inside the cap). Run: desktop-b-dmem.sh ssh 'bash -s' < guest-dmem-check.sh
set -u
export XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
U=/sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service
step() { echo "== $*"; }
step "kernel $(uname -r); $(systemctl --version | head -n 1); nvidia $(cat /sys/module/nvidia/version 2>/dev/null)"
step "root controllers: $(cat /sys/fs/cgroup/cgroup.controllers)"
step "dmem.capacity"; cat /sys/fs/cgroup/dmem.capacity
step "user@1000.service: $(systemctl show user@1000.service -p Delegate -p DelegateSubgroup | tr '\n' ' ')"
step "root half (journal of user@1000.service)"
journalctl -b -u user@1000.service --no-pager -o cat | grep caraxes-dmem-cap
step "user half"
systemctl --user --no-pager status caraxes-dmem-cap.service | head -n 5
journalctl --user -b -u caraxes-dmem-cap.service --no-pager -o cat
/usr/local/libexec/caraxes/caraxes-dmem-cap status
step "unit ordering"
systemctl --user show -p After -p Before graphical-session.target caraxes-dmem-cap.service | cut -c1-400
step "cgroups"
for p in gnome-shell Xwayland sunshine steam steamwebhelper; do
    for i in $(pgrep -x "$p" | head -n 2); do echo "$p $i $(cat /proc/"$i"/cgroup)"; done
done
step "connectors"
for c in /sys/class/drm/card*-*; do echo "${c##*/} $(cat "$c"/status) $(cat "$c"/enabled 2>/dev/null) $(tr '\n' ' ' < "$c"/modes)"; done
step "NVML and Vulkan budget outside the cap (ssh session scope)"
nvidia-smi --query-gpu=memory.total,memory.reserved,memory.used,memory.free --format=csv
vulkaninfo 2>/dev/null | grep -E "heapBudget|heapUsage" | head -n 4
step "NVML and Vulkan budget inside app.slice (does NVIDIA's userspace read the ancestor's dmem.max?)"
systemd-run --user --scope --slice=app.slice --collect -q -- sh -c 'cat /proc/self/cgroup; nvidia-smi --query-gpu=memory.total,memory.reserved,memory.used,memory.free --format=csv; vulkaninfo 2>/dev/null | grep -E "heapBudget|heapUsage" | head -n 4'
step "dmesg"
sudo dmesg | grep -E "NVKMS|NVRM: Xid|dmem|cgroup" | tail -n 10
