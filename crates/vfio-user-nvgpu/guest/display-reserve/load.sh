#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
#
# Load the display reserve build of nvidia.ko (build.sh) for a game test, as root:
#
#   load.sh REGISTRY_DWORDS      e.g. load.sh RmDisplayReserve=1
#
# Unloads the NVIDIA modules if loaded (they must not be in use), then loads
# /opt/caraxes/display-reserve/<kernel>/nvidia.ko with the options modprobe.d gives nvidia and
# REGISTRY_DWORDS appended to their NVreg_RegistryDwords. nvidia-modeset, nvidia-drm and
# nvidia-uvm stay the installed modules; the caller modprobes them as usual. Nothing is installed:
# the next boot loads the stock driver again.
set -euo pipefail

regs=${1:?usage: load.sh REGISTRY_DWORDS}
ko=/opt/caraxes/display-reserve/$(uname -r)/nvidia.ko

die() { echo "load.sh: $*" >&2; exit 1; }
(( EUID == 0 )) || die "run as root"
[[ -f $ko ]] || die "$ko missing (build.sh)"

for m in nvidia_drm nvidia_modeset nvidia_uvm nvidia; do
    if [[ -d /sys/module/$m ]]; then
        rmmod "$m" || die "cannot unload $m (in use?)"
    fi
done

# The options modprobe would give nvidia, with REGISTRY_DWORDS added to NVreg_RegistryDwords.
opts=() found=0
while read -r opt; do
    if [[ $opt == NVreg_RegistryDwords=* ]]; then
        val=${opt#NVreg_RegistryDwords=}
        val=${val#\"}
        val=${val%\"}
        opts+=("NVreg_RegistryDwords=${val:+$val;}$regs")
        found=1
    else
        opts+=("$opt")
    fi
done < <(modprobe -c | awk '$1 == "options" && $2 == "nvidia" { for (i = 3; i <= NF; i++) print $i }')
(( found )) || opts+=("NVreg_RegistryDwords=$regs")

insmod "$ko" "${opts[@]}"
echo "loaded $ko: $(grep -h '^RegistryDwords' /proc/driver/nvidia/params)"
