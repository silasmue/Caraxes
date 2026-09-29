#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
#
# Load the display reserve build of nvidia.ko (build.sh) for a game test, as root:
#
#   load.sh REGISTRY_DWORDS [PARAM=VALUE...]
#
# e.g. load.sh RmDisplayReserve=1 NVreg_RmMsg=display_reserve.c: the reserve on, and its state
# messages (NV_PRINTF LEVEL_INFO, not printed by default) in the kernel log through NVIDIA's
# RmMsg filter (kernel-open/nvidia/nv-reg.h, nvlog_printf.c nvDbgRmMsgCheck).
#
# Unloads the NVIDIA modules if loaded (they must not be in use), loads the modules nvidia.ko
# depends on, then loads /opt/caraxes/display-reserve/<kernel>/nvidia.ko with the options
# modprobe.d gives nvidia and REGISTRY_DWORDS appended to their NVreg_RegistryDwords, and checks
# that /proc/driver/nvidia/version reports the version line of that build (MANIFEST). Fails
# otherwise. nvidia-modeset, nvidia-drm and nvidia-uvm stay the installed modules; the caller
# modprobes them as usual and they bind to the loaded nvidia. Nothing is installed: the next boot
# loads the stock driver again.
set -euo pipefail

regs=${1:?usage: load.sh REGISTRY_DWORDS [PARAM=VALUE...]}
shift
dir=/opt/caraxes/display-reserve/$(uname -r)
ko=$dir/nvidia.ko

die() { echo "load.sh: $*" >&2; exit 1; }
(( EUID == 0 )) || die "run as root"
[[ -f $ko ]] || die "$ko missing (build.sh)"
want=$(sed -n 's/^version //p' "$dir/MANIFEST")
[[ -n $want ]] || die "$dir/MANIFEST has no version line (build.sh)"

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

# insmod does not resolve dependencies (nvidia.ko needs ecc, for example).
IFS=, read -r -a deps <<< "$(modinfo -F depends "$ko")"
for d in "${deps[@]}"; do
    [[ -n $d ]] && { modprobe "$d" || die "cannot load $d, which nvidia.ko depends on"; }
done
insmod "$ko" "${opts[@]}" "$@" || die "insmod $ko failed"

got=$(sed -n 's/^NVRM version: *//p' /proc/driver/nvidia/version)
[[ $got == "$want" ]] || die "loaded nvidia is '$got', not the display reserve build '$want'"
echo "loaded $ko ($got): $(grep -h '^RegistryDwords' /proc/driver/nvidia/params)"
