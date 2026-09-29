#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# R4 host switch (docs/caraxes/experiments.md, R4): moves the RTX 2070, all four functions of
# IOMMU group 28, between the host drivers and vfio-pci. Never run automatically: the operator
# runs it by hand from an SSH session in tmux on another machine, with the Wayland session
# ended. Every step prints what it does before it does it.
#
#   pt-host.sh status              read-only; as root it also sees other users' open files
#   pt-host.sh check               read-only preconditions of to-vfio; exit 1 names each blocker
#   sudo pt-host.sh to-vfio        check, then: fbcon unbind, nvidia stack unload, .1-.3 unbind,
#                                  vfio-pci bind of all four, /dev/vfio/28 to the operator
#   sudo pt-host.sh restore        vfio-pci unbind, override cleared, .1-.3 probed, nvidia stack
#                                  loaded, fbcon rebound, kernel log checked
#   sudo pt-host.sh reset flr|bus  with no VM and no host driver bound: FLR of .0, or a secondary
#                                  bus reset below 00:03.1 (all four functions); restore's retry
#
# Idempotent: a step whose target state already holds prints "ok (already)". Each kernel-facing
# step runs in the background and is abandoned after PT_STEP_TIMEOUT s (default 30, module loads
# 120): a step still running then sits in the kernel, and the script stops with the reboot path.
# PT_FORCE=1 skips only the SSH/tmux check. Root runs append to /var/log/caraxes-pt-host.log.
set -uo pipefail

FUNCS=(0000:2d:00.0 0000:2d:00.1 0000:2d:00.2 0000:2d:00.3)
IDS=(10de:1f02 10de:10f9 10de:1ada 10de:1adb)
HOST_DRV=(nvidia snd_hda_intel xhci_hcd nvidia-gpu)
GPU=${FUNCS[0]} GROUP=28 BRIDGE=0000:00:03.1
NV_MODS=(nvidia_drm nvidia_modeset nvidia_uvm nvidia)
T=${PT_STEP_TIMEOUT:-30}
SYS=/sys/bus/pci/devices
OPERATOR=${SUDO_USER:-$(id -un)}
STATE=/run/caraxes-pt
ME=$(readlink -f "$0")
CMD=${1:-status}

say() { printf '%s %s\n' "$(date +%T)" "$*"; }
step() { local n=$1; shift; say "STEP $n: $*"; }
already() { say "  ok (already): $*"; }
need_root() { ((EUID == 0)) || { echo "needs root: sudo $ME $CMD" >&2; exit 1; }; }
drv_of() { local l; l=$(readlink "$SYS/$1/driver" 2>/dev/null) && basename "$l"; }
override_of() { cat "$SYS/$1/driver_override" 2>/dev/null; }
id_of() { printf '%s:%s' "$(sed 's/^0x//' "$SYS/$1/vendor")" "$(sed 's/^0x//' "$SYS/$1/device")"; }
sysfs_write() { printf '%s\n' "$2" > "$1"; }
state_of() { awk '/^State:/{print $2}' "/proc/$1/status" 2>/dev/null; }

recovery() {
    cat <<EOF
RECOVERY (a kernel step did not finish; do not repeat it):
  1. sudo $ME status                      what is bound now
  2. sudo systemctl reboot                give it 3 minutes (the stuck step can block the shutdown)
  3. host still up after that:
     sudo sh -c 'echo 1 > /proc/sys/kernel/sysrq; echo s > /proc/sysrq-trigger; sleep 5; echo u > /proc/sysrq-trigger; sleep 5; echo b > /proc/sysrq-trigger'
  4. no SSH at all: the reset button.
After the reboot: log in on tty1 and run start-hyprland; recreate the taps caraxes6/caraxes7
before the next Caraxes run (the first GPA-mode launch after a reboot fails once).
EOF
}

# run_k SECONDS WHAT CMD...: CMD in the background; after SECONDS it is abandoned (STUCK).
run_k() {
    local t=$1 what=$2 pid rc i
    shift 2
    say "  do: $what"
    "$@" &
    pid=$!
    for ((i = 0; i < t * 10; i++)); do
        if ! kill -0 "$pid" 2>/dev/null || [[ $(state_of "$pid") == Z ]]; then
            wait "$pid"
            rc=$?
            if ((rc == 0)); then say "  ok: $what"; else say "  FAILED (exit $rc): $what"; fi
            return "$rc"
        fi
        sleep 0.1
    done
    say "  STUCK: '$what' still runs after ${t}s (pid $pid, state $(state_of "$pid"))"
    [[ -r /proc/$pid/stack ]] && sed 's/^/    /' "/proc/$pid/stack"
    recovery
    exit 3
}

gpu_dri() { local d; for d in "$SYS/$GPU"/drm/*; do [[ -e $d ]] && basename "$d"; done; }
snd_card() { local c; for c in "$SYS/${FUNCS[1]}"/sound/card*; do [[ -e $c ]] && echo "${c##*card}"; done; }

# holders: "pid comm device" for every open file on this card's device nodes (root: all users)
holders() {
    local pat=(-lname '/dev/nvidia*' -o -lname "/dev/vfio/$GROUP" -o -lname '/dev/vfio/devices/*')
    local d c
    for d in $(gpu_dri); do pat+=(-o -lname "/dev/dri/$d"); done
    for c in $(snd_card); do pat+=(-o -lname "/dev/snd/*C${c}D*" -o -lname "/dev/snd/controlC$c"); done
    find /proc/[0-9]*/fd -maxdepth 1 -type l \( "${pat[@]}" \) -printf '%h %l\n' 2>/dev/null |
        while read -r fddir dev; do
            p=${fddir#/proc/}
            p=${p%/fd}
            printf '%s %s %s\n' "$p" "$(cat "/proc/$p/comm" 2>/dev/null)" "$dev"
        done | sort -u -k1,1n -k3,3
}

vm_procs() { pgrep -a '^(vfio-user-nvgp|vhost-user-nvr|cloud-hypervis|qemu-system-x86|crosvm|sunshine)' | cut -c1-140 || true; }
compositors() { pgrep -a -x 'Hyprland|Xwayland|kwin_wayland|gnome-shell|sway|weston|gamescope|mutter|labwc|niri|river|Xorg' || true; }
graphical_sessions() {
    local s ty
    for s in $(loginctl list-sessions --no-legend 2>/dev/null | awk '{print $1}'); do
        ty=$(loginctl show-session "$s" -p Type --value 2>/dev/null)
        [[ $ty == wayland || $ty == x11 ]] && echo "$s $ty $(loginctl show-session "$s" -p Name -p TTY --value 2>/dev/null | tr '\n' ' ')"
    done
}
in_tmux() {
    local p=$$ c
    while ((p > 1)); do
        c=$(cat "/proc/$p/comm" 2>/dev/null) || return 1
        [[ $c == tmux* ]] && return 0
        p=$(awk '/^PPid:/{print $2}' "/proc/$p/status")
    done
    return 1
}
mem_avail_gib() { awk '/^MemAvailable/{printf "%d", $2 / 1048576}' /proc/meminfo; }

status() {
    say "== status ($(id -un), kernel $(uname -r))"
    local i f
    for i in 0 1 2 3; do
        f=${FUNCS[i]}
        printf '  %s %s driver=%-14s override=%-9s power=%-6s reset_method=%s\n' "$f" "$(id_of "$f")" \
            "$(drv_of "$f" || echo -)" "$(override_of "$f")" "$(cat "$SYS/$f/power_state" 2>/dev/null)" \
            "$(cat "$SYS/$f/reset_method" 2>/dev/null)"
    done
    printf '  group %s: %s\n' "$GROUP" "$(ls /sys/kernel/iommu_groups/$GROUP/devices | tr '\n' ' ')"
    printf '  /dev/vfio: %s\n' "$(ls -l /dev/vfio 2>/dev/null | awk 'NR>1{print $3":"$1":"$NF}' | tr '\n' ' ')"
    local m out=()
    for m in "${NV_MODS[@]}" vfio_pci vfio_pci_core vfio_iommu_type1 vfio snd_hda_intel i2c_nvidia_gpu; do
        [[ -d /sys/module/$m ]] && out+=("$m($(cat /sys/module/$m/refcnt 2>/dev/null || echo b))") || out+=("$m(-)")
    done
    printf '  modules (refcnt): %s\n' "${out[*]}"
    local v
    for v in /sys/class/vtconsole/vtcon*; do printf '  %s: %s bind=%s\n' "${v##*/}" "$(cat "$v/name")" "$(cat "$v/bind")"; done
    printf '  graphical sessions: %s\n' "$(graphical_sessions | tr '\n' ';')"
    printf '  compositors: %s\n' "$(compositors | tr '\n' ';')"
    printf '  VM and GPU processes: %s\n' "$(vm_procs | cut -c1-90 | tr '\n' ';')"
    ((EUID == 0)) || say "  (not root: only $(id -un)'s open files are visible)"
    say "  open files on the card's nodes (dri: $(gpu_dri | tr '\n' ' ')snd card: $(snd_card)):"
    holders | sed 's/^/    /'
    printf '  MemAvailable %s GiB; /mnt/vmstore free %s\n' "$(mem_avail_gib)" "$(df -h --output=avail /mnt/vmstore 2>/dev/null | tail -1 | tr -d ' ')"
    if ((EUID == 0)); then
        say "  kernel log (last NVRM/Xid/vfio lines):"
        dmesg | grep -E 'NVRM|Xid|vfio|2d:00' | tail -n 8 | sed 's/^/    /'
    fi
}

check() {
    local bad=0 i h
    block() { say "  BLOCK: $*"; bad=1; }
    say "== check"
    ((EUID == 0)) || say "  note: not root; other users' open files are not visible (use sudo)"
    if [[ ${PT_FORCE:-0} != 1 ]]; then
        in_tmux || block "not inside tmux (run from tmux in an SSH session; PT_FORCE=1 overrides)"
        pgrep -u "$OPERATOR" -x sshd-session >/dev/null || pgrep -u "$OPERATOR" -x sshd >/dev/null ||
            block "no SSH session of $OPERATOR"
    fi
    while read -r h; do [[ -n $h ]] && block "graphical session: $h -> loginctl terminate-session ${h%% *}"; done < <(graphical_sessions)
    while read -r h; do [[ -n $h ]] && block "compositor: $h"; done < <(compositors)
    systemctl is-active --quiet display-manager.service 2>/dev/null && block "display manager active -> sudo systemctl stop display-manager"
    systemctl is-active --quiet nvidia-persistenced.service 2>/dev/null && block "nvidia-persistenced active -> sudo systemctl stop nvidia-persistenced"
    while read -r h; do [[ -n $h ]] && block "VM or GPU process: $h"; done < <(vm_procs)
    local hs
    hs=$(holders)
    while read -r h; do [[ -n $h ]] && block "open: $h"; done <<< "$hs"
    grep -qE ' (pipewire|wireplumber|pipewire-pulse) ' <<< "$hs" &&
        say "  hint: as $OPERATOR: systemctl --user stop pipewire.socket pipewire-pulse.socket pipewire.service pipewire-pulse.service wireplumber.service"
    local members
    members=$(ls /sys/kernel/iommu_groups/$GROUP/devices | sort | tr '\n' ' ')
    [[ $members == "${FUNCS[*]} " ]] || block "group $GROUP holds '$members', expected the four functions only"
    for i in 0 1 2 3; do
        [[ $(id_of "${FUNCS[i]}") == "${IDS[i]}" ]] || block "${FUNCS[i]} is $(id_of "${FUNCS[i]}"), expected ${IDS[i]}"
    done
    modinfo -n vfio-pci >/dev/null 2>&1 || block "no vfio-pci module for $(uname -r)"
    (($(mem_avail_gib) >= 14)) || say "  WARN: MemAvailable $(mem_avail_gib) GiB (a VM needs 8 + 6)"
    if ((bad)); then say "check: BLOCKED"; else say "check: ok"; fi
    return "$bad"
}

unbind() {  # unbind BDF: from whatever driver it has
    local f=$1 d
    d=$(drv_of "$f") || { already "$f has no driver"; return 0; }
    run_k "$T" "unbind $f from $d" sysfs_write "$SYS/$f/driver/unbind" "$f"
}

to_vfio() {
    need_root
    say "== to-vfio $(date -Is)"
    check || { say "nothing changed"; exit 1; }
    mkdir -p "$STATE"
    step 1/6 "console: unbind fbcon from the frame buffer (nvidia-drm fbdev)"
    local v
    for v in /sys/class/vtconsole/vtcon*; do
        [[ $(cat "$v/name") == *"frame buffer"* ]] || continue
        if [[ $(cat "$v/bind") == 0 ]]; then already "${v##*/} unbound"; continue; fi
        run_k "$T" "${v##*/} bind=0" sysfs_write "$v/bind" 0 || exit 1
        echo "${v##*/}" >> "$STATE/vtcons"
    done
    step 2/6 "nvidia stack: modprobe -r ${NV_MODS[*]}"
    local m
    for m in "${NV_MODS[@]}"; do
        if [[ ! -d /sys/module/$m ]]; then already "$m not loaded"; continue; fi
        say "  $m refcnt $(cat "/sys/module/$m/refcnt")"
        run_k 60 "modprobe -r $m" modprobe -r "$m" || {
            say "  $m stays loaded (still in use):"
            holders | sed 's/^/    /'
            say "  put back what was removed: sudo $ME restore"
            exit 1
        }
    done
    step 3/6 "unbind .1 .2 .3 from the host drivers"
    local i
    for i in 1 2 3; do
        [[ $(drv_of "${FUNCS[i]}") == vfio-pci ]] && { already "${FUNCS[i]} on vfio-pci"; continue; }
        unbind "${FUNCS[i]}" || exit 1
    done
    step 4/6 "vfio-pci module (disable_idle_d3=1: the functions stay in D0 while unused)"
    if [[ -d /sys/module/vfio_pci ]]; then
        already "vfio_pci loaded, disable_idle_d3=$(cat /sys/module/vfio_pci/parameters/disable_idle_d3)"
    else
        run_k 120 "modprobe vfio-pci disable_idle_d3=1" modprobe vfio-pci disable_idle_d3=1 || exit 1
    fi
    step 5/6 "driver_override=vfio-pci and probe, all four functions"
    local f d
    for f in "${FUNCS[@]}"; do
        if [[ $(override_of "$f") != vfio-pci ]]; then
            run_k "$T" "$f driver_override=vfio-pci" sysfs_write "$SYS/$f/driver_override" vfio-pci || exit 1
        else
            already "$f override vfio-pci"
        fi
        d=$(drv_of "$f") || d=
        if [[ $d == vfio-pci ]]; then already "$f on vfio-pci"; continue; fi
        [[ -n $d ]] && { unbind "$f" || exit 1; }
        run_k "$T" "probe $f" sysfs_write /sys/bus/pci/drivers_probe "$f" || exit 1
        [[ $(drv_of "$f") == vfio-pci ]] || { say "  FAILED: $f is on '$(drv_of "$f")', not vfio-pci"; exit 1; }
    done
    step 6/6 "/dev/vfio/$GROUP for $OPERATOR (QEMU runs as $OPERATOR)"
    for ((i = 0; i < 50; i++)); do [[ -e /dev/vfio/$GROUP ]] && break; sleep 0.1; done
    [[ -e /dev/vfio/$GROUP ]] || { say "  FAILED: no /dev/vfio/$GROUP"; exit 1; }
    chown "$OPERATOR" "/dev/vfio/$GROUP" && ls -l "/dev/vfio/$GROUP"
    status
    say "to-vfio done. Next (as $OPERATOR, in this tmux shell once): sudo prlimit --pid \$\$ --memlock=unlimited:unlimited"
}

restore() {
    need_root
    say "== restore $(date -Is)"
    local users
    users=$(holders | grep -E ' /dev/vfio/' || true)
    if [[ -n $users ]]; then
        say "BLOCKED: a VM still holds the card:"
        echo "$users" | sed 's/^/  /'
        say "stop it first (pt-vm.sh stop); nothing changed"
        exit 1
    fi
    local dm0 f d i
    dm0=$(dmesg | wc -l)
    step 1/6 "vfio-pci off and driver_override cleared, all four functions"
    for f in "${FUNCS[@]}"; do
        d=$(drv_of "$f") || d=
        if [[ $d == vfio-pci ]]; then unbind "$f" || exit 1; else already "$f driver ${d:-none}"; fi
        if [[ $(override_of "$f") != "(null)" ]]; then
            run_k "$T" "$f driver_override cleared" sysfs_write "$SYS/$f/driver_override" "" || exit 1
        else
            already "$f no override"
        fi
    done
    step 2/6 "probe .1 .2 .3: the host drivers match again"
    for i in 1 2 3; do
        f=${FUNCS[i]}
        d=$(drv_of "$f") || d=
        if [[ $d == "${HOST_DRV[i]}" ]]; then already "$f on $d"; continue; fi
        run_k "$T" "probe $f" sysfs_write /sys/bus/pci/drivers_probe "$f"
        say "  $f now on '$(drv_of "$f")' (want ${HOST_DRV[i]})"
    done
    step 3/6 "nvidia stack: modprobe nvidia (softdeps: nvidia-uvm, nvidia-drm), modprobe nvidia_drm"
    if [[ $(drv_of "$GPU") == nvidia ]]; then already "$GPU on nvidia"; else
        run_k 120 "modprobe nvidia" modprobe nvidia
    fi
    run_k 120 "modprobe nvidia_drm" modprobe nvidia_drm
    [[ $(drv_of "$GPU") == nvidia ]] || run_k "$T" "probe $GPU" sysfs_write /sys/bus/pci/drivers_probe "$GPU"
    step 4/6 "kernel log since restore started"
    local log bad
    log=$(dmesg | tail -n +"$((dm0 + 1))")
    grep -E 'NVRM|nvidia|Xid|vfio|2d:00|drm' <<< "$log" | tail -n 30 | sed 's/^/    /'
    bad=$(grep -E 'RmInitAdapter failed|Xid|fallen off the bus|GSP.*(fail|timeout|error)|probe of 0000:2d:00\.0 failed' <<< "$log" || true)
    step 5/6 "console: rebind fbcon"
    local v
    for v in /sys/class/vtconsole/vtcon*; do
        [[ $(cat "$v/name") == *"frame buffer"* ]] || continue
        if [[ $(cat "$v/bind") == 1 ]]; then already "${v##*/} bound"; continue; fi
        run_k "$T" "${v##*/} bind=1" sysfs_write "$v/bind" 1
    done
    rm -f "$STATE/vtcons"
    step 6/6 "desktop"
    if systemctl cat display-manager.service >/dev/null 2>&1; then
        run_k 60 "systemctl start display-manager" systemctl start display-manager.service
    else
        say "  no display manager on this host: log in on tty1 and run start-hyprland"
    fi
    say "  as $OPERATOR: systemctl --user start pipewire.socket pipewire-pulse.socket wireplumber.service"
    status
    if [[ -n $bad || $(drv_of "$GPU") != nvidia ]]; then
        say "RESTORE INCOMPLETE: $GPU on '$(drv_of "$GPU")'; kernel log:"
        echo "$bad" | sed 's/^/    /'
        say "retry once: sudo $ME to-vfio && sudo $ME reset bus && sudo $ME restore"
        say "then, if still broken:"
        recovery
        exit 2
    fi
    say "restore done: $GPU on nvidia"
}

reset() {
    need_root
    local how=${1:-} f d
    [[ $how == flr || $how == bus ]] || { echo "usage: sudo $ME reset flr|bus" >&2; exit 1; }
    [[ -z $(holders | grep -E ' /dev/vfio/' || true) ]] || { say "BLOCKED: a VM holds the card"; exit 1; }
    for f in "${FUNCS[@]}"; do
        d=$(drv_of "$f") || d=
        [[ -z $d || $d == vfio-pci ]] || { say "BLOCKED: $f is on $d (run to-vfio first)"; exit 1; }
    done
    if [[ $how == flr ]]; then
        step 1/1 "function-level reset of $GPU (reset_method: $(cat "$SYS/$GPU/reset_method"))"
        run_k "$T" "$GPU reset" sysfs_write "$SYS/$GPU/reset" 1 || exit 1
    else
        step 1/1 "secondary bus reset below $BRIDGE (all four functions)"
        run_k "$T" "$BRIDGE reset_subordinate" sysfs_write "$SYS/$BRIDGE/reset_subordinate" 1 || exit 1
    fi
    status
}

if ((EUID == 0)) && [[ $CMD != status && $CMD != check ]]; then
    exec > >(tee -a /var/log/caraxes-pt-host.log) 2>&1
fi
case $CMD in
    status) status ;;
    check) check ;;
    to-vfio) to_vfio ;;
    restore) restore ;;
    reset) reset "${2:-}" ;;
    *) sed -n '2,/^set -uo/p' "$ME" | sed '$d'; exit 1 ;;
esac
