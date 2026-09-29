#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
#
# Load the display reserve build (build.sh) for a game test, as root:
#
#   load.sh DISPLAY_RESERVE_MIB [NVIDIA_PARAM=VALUE...]
#
# e.g. load.sh -1 NVreg_RmMsg=display_reserve.c: NVKMS declares five surfaces and four cursors per
# active head, and RM's state messages for the reserve (NV_PRINTF LEVEL_INFO, not printed by
# default) reach the kernel log through NVIDIA's RmMsg filter (kernel-open/nvidia/nv-reg.h,
# nvlog_printf.c nvDbgRmMsgCheck).
#
# Unloads the NVIDIA modules if loaded (they must not be in use), loads the modules nvidia.ko
# depends on (insmod does not), then loads the build's nvidia.ko with the options modprobe.d gives
# nvidia plus NVIDIA_PARAM=VALUE, and its nvidia-modeset.ko with the options modprobe.d gives
# nvidia_modeset plus display_reserve_mib. Fails unless /proc/driver/nvidia/version reports the
# build's version line (MANIFEST) and nvidia_modeset has the display_reserve_mib asked for.
# nvidia-drm and nvidia-uvm stay the installed modules; the caller modprobes them as usual and
# they bind to the loaded ones. Nothing is installed: the next boot loads the stock driver again.
set -euo pipefail

mib=${1:?usage: load.sh DISPLAY_RESERVE_MIB [NVIDIA_PARAM=VALUE...]}
shift
dir=/opt/caraxes/display-reserve-v2/$(uname -r)

die() { echo "load.sh: $*" >&2; exit 1; }
(( EUID == 0 )) || die "run as root"
[[ $mib =~ ^-?[0-9]+$ ]] || die "DISPLAY_RESERVE_MIB '$mib' is not a number"
[[ -f $dir/nvidia.ko && -f $dir/nvidia-modeset.ko ]] || die "$dir has no build (build.sh)"
want=$(sed -n 's/^version //p' "$dir/MANIFEST")
[[ -n $want ]] || die "$dir/MANIFEST has no version line (build.sh)"

for m in nvidia_drm nvidia_modeset nvidia_uvm nvidia; do
    if [[ -d /sys/module/$m ]]; then
        rmmod "$m" || die "cannot unload $m (in use?)"
    fi
done

# The options modprobe.d gives module $1 (written with - or _), one per line.
modprobe_opts() {
    modprobe -c | awk -v m="$1" '$1 == "options" { n = $2; gsub(/-/, "_", n); if (n == m)
        for (i = 3; i <= NF; i++) print $i }'
}

IFS=, read -r -a deps <<< "$(modinfo -F depends "$dir/nvidia.ko")"
for d in "${deps[@]}"; do
    [[ -n $d ]] && { modprobe "$d" || die "cannot load $d, which nvidia.ko depends on"; }
done

mapfile -t opts < <(modprobe_opts nvidia)
insmod "$dir/nvidia.ko" "${opts[@]}" "$@" || die "insmod $dir/nvidia.ko failed"
got=$(sed -n 's/^NVRM version: *//p' /proc/driver/nvidia/version)
[[ $got == "$want" ]] || die "loaded nvidia is '$got', not the display reserve build '$want'"

mapfile -t opts < <(modprobe_opts nvidia_modeset)
insmod "$dir/nvidia-modeset.ko" "${opts[@]}" display_reserve_mib="$mib" \
    || die "insmod $dir/nvidia-modeset.ko failed"
got=$(cat /sys/module/nvidia_modeset/parameters/display_reserve_mib 2>/dev/null || true)
[[ $got == "$mib" ]] || die "loaded nvidia_modeset has display_reserve_mib '$got', not $mib"

echo "loaded $dir: nvidia ($want), nvidia_modeset display_reserve_mib=$mib; $(grep -h '^RegistryDwords:' /proc/driver/nvidia/params)"
