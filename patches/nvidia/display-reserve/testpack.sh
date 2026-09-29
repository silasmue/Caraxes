#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
#
# Display reserve test pack: one self-contained pack a person runs on a bare-metal Linux machine
# with an NVIDIA GPU (Turing or newer, open kernel modules), and one tarball of results.
#
#   testpack.sh pack [-o DIR] [--ogkm GITDIR] [--tag TAG]   at home: build the pack
#   ./testpack.sh run OUTDIR [--steps LIST]                  on the test machine, as root, from a
#                                                            text console, display manager stopped
#
# The machine must run the NVIDIA driver release the pack was made for: the pack builds the
# patched modules from that release's source against the running kernel, and the driver's
# user-space libraries must match them.
#
# Steps, in this order (default: check build stock load repro modes control unload):
#   check    root, no display manager or compositor, driver release, kernel headers, Secure Boot
#   build    unpack the source, apply the patch, build the modules and scanout-oom
#   stock    scanout-oom with the installed modules: a refused buffer is expected
#   load     replace the installed modules with the patched ones (display_reserve_mib=-1, and
#            NVreg_RmMsg=display_reserve.c for the reserve's state lines)
#   repro    scanout-oom on every NVIDIA card: every buffer granted is expected
#   modes    walk the modes (preferred, smallest, off): the reserve follows the display
#   control  nvidia-modeset with display_reserve_mib=0: a refused buffer is expected
#   suspend  suspend to RAM for about 30 s with the display lit and the reserve held
#   rtd3     if a GPU supports runtime D3 and shows no display: let it suspend, then light it
#   hotplug  asks you to unplug and replug the display within 60 s
#   unload   leave the reserve holding a taken-back run, then unload the patched modules
# suspend, rtd3 and hotplug run only when named: --steps all, or e.g. --steps check,build,load,suspend
# (check, build and load are added when a later step needs them). At the end the installed
# modules are loaded again; reboot afterwards to be sure.
#
# Result: OUTDIR/display-reserve-testpack-<stamp>.tar.gz and its .sha256, holding run.log, one
# log per step, the kernel log lines of the NVIDIA modules and a description of driver and GPUs.
# The host name and the user name are replaced; GPU UUIDs and serial numbers are not collected.
# The pack calls no network tool and sends nothing anywhere.
set -uo pipefail
export LC_ALL=C

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
usage() { sed -n '5,36p' "$0"; }
die() { echo "testpack: $*" >&2; exit 2; }
have() { command -v "$1" >/dev/null 2>&1; }

# ===========================================================================
# pack (at home)
# ===========================================================================
do_pack() {
    local outdir=target/display-reserve-testpack ogkm=${CARAXES_OGKM:-} tag=615.71.09
    local stage patch commit
    while [ $# -gt 0 ]; do
        case $1 in
            -o) outdir=$2; shift 2 ;;
            --ogkm) ogkm=$2; shift 2 ;;
            --tag) tag=$2; shift 2 ;;
            *) usage; exit 2 ;;
        esac
    done
    [ -n "$ogkm" ] || die "give the open-gpu-kernel-modules git tree with --ogkm or CARAXES_OGKM"
    commit=$(git -C "$ogkm" rev-parse --verify "$tag^{commit}") || die "no tag $tag in $ogkm"
    patch=$(ls "$here"/../0001-*.patch) || die "no patch next to $here"
    stage=$(mktemp -d "${TMPDIR:-/var/tmp}/testpack.XXXXXX")
    mkdir -p "$stage/display-reserve-testpack" "$outdir"
    stage=$stage/display-reserve-testpack
    git -C "$ogkm" archive --format=tar.gz --prefix="open-gpu-kernel-modules-$tag/" "$commit" \
        > "$stage/source.tar.gz" || die "git archive failed"
    cp "$patch" "$stage/display-reserve.patch" &&
        cp "$here/testpack.sh" "$here/scanout-oom.c" "$here/README.md" "$stage/" ||
        die "cannot copy the pack's files"
    printf 'release %s\ncommit %s\npatch %s\n' "$tag" "$commit" "$(basename "$patch")" \
        > "$stage/MANIFEST"
    (cd "$stage" && sha256sum source.tar.gz display-reserve.patch testpack.sh scanout-oom.c \
        README.md MANIFEST > SHA256SUMS) || die "sha256sum failed"
    tar -C "$(dirname "$stage")" -czf "$outdir/display-reserve-testpack-$tag.tar.gz" \
        display-reserve-testpack || die "tar failed"
    (cd "$outdir" && sha256sum "display-reserve-testpack-$tag.tar.gz" \
        > "display-reserve-testpack-$tag.tar.gz.sha256")
    rm -rf "$(dirname "$stage")"
    echo "$outdir/display-reserve-testpack-$tag.tar.gz"
}

# ===========================================================================
# run (on the test machine)
# ===========================================================================
OUT= WORK= KVER= TAG= STAMP_LINE= PATCHED=0
MODS="nvidia_drm nvidia_modeset nvidia_uvm nvidia"

kmark() { echo "display-reserve-testpack: $*" > /dev/kmsg; }
log_step() { echo; echo "== $(date +%T) $*"; kmark "$*"; }
reserve_lines() { dmesg | grep -E "display reserve|Failed to allocate NVKMS|Xid" | tail -n "${1:-12}"; }

# NVIDIA cards: /dev/dri/cardN bound to nvidia-drm
nvidia_cards() {
    local c drv
    for c in /sys/class/drm/card[0-9]*; do
        [ -e "$c/device/driver" ] || continue
        case $(basename "$c") in *-*) continue ;; esac
        drv=$(basename "$(readlink -f "$c/device/driver")")
        [ "$drv" = nvidia ] && echo "/dev/dri/$(basename "$c")"
    done
}

# options modprobe.d gives a module, one per line
modopts() {
    modprobe -c 2>/dev/null | awk -v m="$1" '$1 == "options" { n = $2; gsub("-", "_", n)
        if (n == m) for (i = 3; i <= NF; i++) print $i }'
}

unload_all() {
    local m
    for m in nvidia_peermem $MODS; do
        [ -d "/sys/module/$m" ] || continue
        rmmod "$m" || {
            echo "cannot unload $m: $(fuser /dev/nvidia* /dev/dri/* 2>/dev/null | wc -w) processes" \
                 "hold the devices (see fuser -v /dev/nvidia* /dev/dri/*)"
            return 1
        }
    done
}

load_stock() {
    unload_all || return 1
    modprobe nvidia_drm && modprobe nvidia_uvm && PATCHED=0
    [ "$(cat /sys/module/nvidia_drm/parameters/modeset 2>/dev/null)" = Y ] || {
        rmmod nvidia_drm && modprobe nvidia_drm modeset=1
    }
}

# insmod one patched module with its modprobe.d options and extra ones
insmod_patched() {
    local ko=$WORK/src/kernel-open/$1.ko mod=${1//-/_} d
    shift
    for d in $(modinfo -F depends "$ko" | tr ',-' ' _'); do
        case " $MODS " in *" $d "*) ;; *) modprobe "$d" || return 1 ;; esac
    done
    # shellcheck disable=SC2046
    insmod "$ko" $(modopts "$mod") "$@"
}

load_patched() {
    local mib=$1
    unload_all || return 1
    insmod_patched nvidia NVreg_RmMsg=display_reserve.c || return 1
    grep -qF "$STAMP_LINE" /proc/driver/nvidia/version || {
        echo "the loaded nvidia is not the patched build"; return 1; }
    insmod_patched nvidia-modeset "display_reserve_mib=$mib" || return 1
    insmod_patched nvidia-uvm || return 1
    insmod_patched nvidia-drm modeset=1 || return 1
    [ "$(cat /sys/module/nvidia_modeset/parameters/display_reserve_mib)" = "$mib" ] || return 1
    PATCHED=1
    echo "patched modules loaded, display_reserve_mib=$mib"
}

reload_modeset() {
    local mib=$1
    rmmod nvidia_drm nvidia_modeset || return 1
    insmod_patched nvidia-modeset "display_reserve_mib=$mib" || return 1
    insmod_patched nvidia-drm modeset=1 || return 1
    echo "nvidia-modeset reloaded, display_reserve_mib=$mib"
}

# scanout-oom on every card; EXPECT is the exit status expected (0 granted, 1 refused)
repro_all() {
    local expect=$1 card rc bad=0
    shift
    for card in $(nvidia_cards); do
        "$WORK/scanout-oom" -d "$card" "$@"
        rc=$?
        echo "$card: scanout-oom exit $rc (expected $expect)"
        [ "$rc" = "$expect" ] || bad=1
        reserve_lines 16
    done
    return $bad
}

step_check() {
    local installed license
    [ "$(id -u)" = 0 ] || die "run as root"
    if systemctl is-active --quiet display-manager 2>/dev/null; then
        die "stop the display manager first (systemctl stop display-manager) and run from a text console"
    fi
    if pgrep -x 'Xorg|Xwayland|gnome-shell|kwin_wayland|sway|weston|Hyprland|mutter|cosmic-comp' >/dev/null; then
        die "a display server or compositor is running; stop it and run from a text console"
    fi
    if pgrep -x nvidia-persistenced >/dev/null; then
        die "nvidia-persistenced holds the GPU; stop it first (systemctl stop nvidia-persistenced)"
    fi
    installed=$(modinfo -F version nvidia 2>/dev/null)
    [ "$installed" = "$TAG" ] || die "this pack is for NVIDIA $TAG; the installed driver is ${installed:-none}"
    license=$(modinfo -F license nvidia)
    [ "$license" = "Dual MIT/GPL" ] || echo "note: the installed nvidia module is not the open flavour ($license); the patched open modules need a Turing or newer GPU"
    [ -d "/lib/modules/$KVER/build" ] || die "no kernel headers at /lib/modules/$KVER/build"
    if have mokutil && mokutil --sb-state 2>/dev/null | grep -q "SecureBoot enabled"; then
        die "Secure Boot is enabled: the unsigned patched modules would not load"
    fi
    for t in cc make patch tar sha256sum modinfo insmod rmmod; do have "$t" || die "missing $t"; done
    ldconfig -p | grep -q 'libcuda\.so\.1 ' || die "libcuda.so.1 (the driver's CUDA library) is missing"
    [ -n "$(nvidia_cards)" ] || die "no nvidia-drm card; load nvidia_drm with modeset=1"
    echo "kernel $KVER, NVIDIA $TAG ($license)"
    cat /proc/driver/nvidia/version 2>/dev/null
    lspci -nn -d 10de: 2>/dev/null
    have nvidia-smi && nvidia-smi --query-gpu=name,pci.bus_id,driver_version,memory.total,vbios_version --format=csv
    echo "nvidia_drm modeset: $(cat /sys/module/nvidia_drm/parameters/modeset 2>/dev/null)," \
         "fbdev: $(cat /sys/module/nvidia_drm/parameters/fbdev 2>/dev/null)"
    echo "cards: $(nvidia_cards | tr '\n' ' ')"
    echo "modprobe.d options:"; for m in $MODS; do echo "  $m: $(modopts "$m" | tr '\n' ' ')"; done
}

step_build() {
    rm -rf "$WORK/src" && mkdir -p "$WORK/src" || return 1
    tar -xzf "$here/source.tar.gz" -C "$WORK/src" --strip-components=1 || return 1
    (cd "$WORK/src" && patch -p1 --quiet < "$here/display-reserve.patch") || return 1
    echo "building the patched modules for $KVER"
    nice make -C "$WORK/src" modules -j"$(nproc)" SYSSRC="/lib/modules/$KVER/build" \
        > "$OUT/build.log" 2>&1 || { tail -n 30 "$OUT/build.log"; return 1; }
    (cd "$WORK/src/kernel-open" && sha256sum nvidia.ko nvidia-modeset.ko nvidia-uvm.ko nvidia-drm.ko)
    cc -O2 -Wall -o "$WORK/scanout-oom" "$here/scanout-oom.c" -ldl || return 1
    stamp_line
    echo "build: $STAMP_LINE"
}

stamp_line() {
    STAMP_LINE=$(tr -c '[:print:]' '\n' < "$WORK/src/kernel-open/nvidia.ko" |
                 sed -n 's/^nvidia id: *//p' | head -1)
}

step_stock() {
    load_stock || return 1
    repro_all 1
}

step_load() { load_patched -1; }
step_repro() { repro_all 0; }

step_modes() {
    local card
    for card in $(nvidia_cards); do
        "$WORK/scanout-oom" -d "$card" -M
        dmesg | grep -E "display reserve: (declared|[0-9]+ KiB for)" | tail -n 8
    done
}

step_control() {
    reload_modeset 0 || return 1
    repro_all 1
    local rc=$?
    reload_modeset -1 || return 1
    return $rc
}

step_suspend() {
    local card hold
    have rtcwake || { echo "skipped: no rtcwake"; return 0; }
    grep -qw mem /sys/power/state || { echo "skipped: no suspend to RAM"; return 0; }
    card=$(nvidia_cards | head -1)
    "$WORK/scanout-oom" -d "$card" -H 90 & hold=$!
    sleep 5
    reserve_lines 6
    echo "suspending for about 30 s"
    rtcwake -m no -s 30 >/dev/null && systemctl suspend
    sleep 45
    kill "$hold" 2>/dev/null; wait "$hold" 2>/dev/null
    dmesg | grep -E "PM: suspend (entry|exit)" | tail -n 2
    reserve_lines 10
    repro_all 0 -r 1
}

step_rtd3() {
    local card bdf st i
    for card in $(nvidia_cards); do
        bdf=$(basename "$(readlink -f "/sys/class/drm/$(basename "$card")/device")")
        if ! grep -q "Runtime D3 status: *Enabled" "/proc/driver/nvidia/gpus/$bdf/power" 2>/dev/null; then
            echo "$card: skipped, runtime D3 is not enabled"; continue
        fi
        if grep -qx enabled /sys/class/drm/"$(basename "$card")"-*/enabled 2>/dev/null; then
            echo "$card: skipped, a display is lit on it (the console?)"; continue
        fi
        for i in $(seq 60); do
            st=$(cat "/sys/bus/pci/devices/$bdf/power/runtime_status")
            [ "$st" = suspended ] && break
            sleep 1
        done
        echo "$card: runtime status $st after ${i} s"
        "$WORK/scanout-oom" -d "$card" -r 1
        echo "$card: scanout-oom exit $?"
        reserve_lines 10
    done
}

step_hotplug() {
    echo "Unplug the display now, wait 5 s, plug it back in; press Enter when done (60 s at most)."
    read -r -t 60 _ || true
    sleep 5
    dmesg | grep -E "display reserve: declared" | tail -n 6
}

step_unload() {
    local card
    for card in $(nvidia_cards); do "$WORK/scanout-oom" -d "$card" -r 1 -b 1; done
    reserve_lines 8
    kmark "unloading the patched modules"
    unload_all || return 1
    PATCHED=0
    if dmesg | sed -n '/display-reserve-testpack: unloading/,$p' |
        grep -E "Scrubber object is not valid|Assertion failed|display reserve.*(cannot|fail)|BUG|WARNING|Oops"; then
        return 1
    fi
    echo "unloaded cleanly"
}

finish() {
    local host user f result
    [ "$PATCHED" = 1 ] && { echo "loading the installed modules again"; load_stock; }
    dmesg | grep -E "NVRM|nvidia|nvkms|drm|display reserve|display-reserve-testpack|Xid|PM: |BUG|WARNING|Oops|Call Trace|RIP:" \
        > "$OUT/kernel.log"
    host=$(hostname 2>/dev/null) user=${SUDO_USER:-}
    for f in "$OUT"/*.log; do
        [ -n "$host" ] && sed -i "s/$host/HOST/g" "$f"
        [ -n "$user" ] && [ "$user" != root ] && sed -i "s/\b$user\b/USER/g" "$f"
    done
    result=$OUT/display-reserve-testpack-$(date +%Y%m%d-%H%M%S).tar.gz
    tar -C "$OUT" -czf "$result" --exclude=work --exclude='*.tar.gz' --exclude='*.sha256' .
    sha256sum "$result" > "$result.sha256"
    echo "result: $result"
}

do_run() {
    local steps="check build stock load repro modes control unload" s rc failed=""
    OUT=${1:?usage: testpack.sh run OUTDIR [--steps LIST]}
    shift
    while [ $# -gt 0 ]; do
        case $1 in
            --steps) steps=${2//,/ }; shift 2 ;;
            *) usage; exit 2 ;;
        esac
    done
    [ "$steps" = all ] && steps="check build stock load repro modes control suspend rtd3 hotplug unload"
    case " $steps " in *" check "*) ;; *) steps="check $steps" ;; esac
    for s in stock load repro modes control suspend rtd3 hotplug unload; do
        case " $steps " in *" $s "*) case " $steps " in *" build "*) ;; *) steps="${steps/check/check build}" ;; esac ;; esac
    done
    for s in repro modes control suspend rtd3 hotplug unload; do
        case " $steps " in *" $s "*) case " $steps " in *" load "*) ;; *) steps="${steps/ $s/ load $s}" ;; esac ;; esac
    done

    mkdir -p "$OUT" && OUT=$(cd "$OUT" && pwd) || die "cannot create $OUT"
    WORK=$OUT/work KVER=$(uname -r)
    TAG=$(sed -n 's/^release //p' "$here/MANIFEST")
    (cd "$here" && sha256sum -c --quiet SHA256SUMS) || die "the pack's checksums do not match"
    exec > >(tee -a "$OUT/run.log") 2>&1
    trap finish EXIT
    echo "display reserve test pack, NVIDIA $TAG, steps: $steps"
    for s in $steps; do
        log_step "$s"
        "step_$s" > >(tee "$OUT/$s.log") 2>&1
        rc=$?
        sleep 1
        echo "-- $s: $([ $rc = 0 ] && echo pass || echo FAIL)"
        [ $rc = 0 ] || failed="$failed $s"
        case $s in check|build|load) [ $rc = 0 ] || { echo "stopping: $s failed"; break; } ;; esac
    done
    echo "failed steps:${failed:- none}"
}

case ${1:-} in
    pack) shift; do_pack "$@" ;;
    run) shift; do_run "$@" ;;
    *) usage; exit 2 ;;
esac
