#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Host-side test of caraxes-dmem-cap against a fake cgroup and DRM tree and a fake nvidia-smi
# (no root, no GPU, no VM). The real interfaces are exercised in the guest
# (docs/research/dmem-cap-2026-09-29.md, caraxes/runs/dmem-2026-09-29/README.md).
# Usage: test.sh [SCRATCH_DIR]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
T=${1:-$(mktemp -d)}
rm -rf "$T/cg" "$T/drm" "$T/bin" "$T/conf"
uid=$(id -u)
self=$(awk -F: '$1 == "0" { print $3 }' /proc/self/cgroup)
if [[ $self =~ ^(.*/user@${uid}\.service)(/|$) ]]; then mgr=${BASH_REMATCH[1]}; else mgr=/user.slice/user-$uid.slice/user@$uid.service; fi
CG=$T/cg DRM=$T/drm
REG="nvidia/00000000:00:05.0/vidmem"
fail=0
check() {  # NAME EXPECTED ACTUAL
    if [[ $2 == "$3" ]]; then echo "ok   $1"; else echo "FAIL $1: expected '$2', got '$3'"; fail=1; fi
}

mkdir -p "$CG$mgr/app.slice" "$CG$mgr/session.slice" "$T/bin" "$DRM/pci/0000:00:05.0" "$DRM/card0-DP-1"
echo "cpu io memory pids dmem" > "$CG/cgroup.controllers"
echo "cpu memory pids" > "$CG/cgroup.subtree_control"
d=$CG
for part in user.slice "user-$uid.slice"; do d=$d/$part; mkdir -p "$d"; echo "cpu memory pids" > "$d/cgroup.subtree_control"; done
echo "$REG 4294967296" > "$CG/dmem.capacity"
echo "cpu memory pids dmem" > "$CG$mgr/cgroup.controllers"
echo "cpu memory pids" > "$CG$mgr/cgroup.subtree_control"
echo "$REG 104857600" > "$CG$mgr/app.slice/dmem.current"
echo "$REG max" > "$CG$mgr/app.slice/dmem.max"
mkdir -p "$DRM/card0"; ln -s "$DRM/pci/0000:00:05.0" "$DRM/card0/device"
echo connected > "$DRM/card0-DP-1/status"; echo enabled > "$DRM/card0-DP-1/enabled"
printf '1920x1080\n1280x1024\n640x480\n' > "$DRM/card0-DP-1/modes"
cat > "$T/bin/nvidia-smi" <<'N'
#!/bin/bash
case "$*" in
    -L) echo "GPU 0: Leandro VFIO RTX 2070-4G (UUID: GPU-0)" ;;
    *pci.bus_id,memory.free*) echo "00000000:00:05.0, ${FAKE_FREE_MIB:-3710}" ;;
    *) echo "fake nvidia-smi" ;;
esac
N
chmod +x "$T/bin/nvidia-smi"
run() { env PATH="$T/bin:$PATH" CARAXES_DMEM_CGROOT="$CG" CARAXES_DMEM_DRM="$DRM" CARAXES_DMEM_CONF="$T/conf" "$here/caraxes-dmem-cap" "$@"; }
maxval() { awk -v r="$REG" '$1 == r { print $2 }' "$CG$mgr/app.slice/dmem.max"; }

# delegate: +dmem down to user-UID.slice, idempotent
run delegate "$uid" > "$T/out" 2>&1
check "delegate root" yes "$(grep -qw dmem "$CG/cgroup.subtree_control" && echo yes)"
check "delegate user-UID.slice" yes "$(grep -qw dmem "$CG/user.slice/user-$uid.slice/cgroup.subtree_control" && echo yes)"
run delegate "$uid" > "$T/out" 2>&1
check "delegate twice" 3 "$(grep -c 'already enabled' "$T/out")"
check "delegate rejects a bad uid" 1 "$(run delegate 'x;y' > /dev/null 2>&1; echo $?)"

# apply, automatic reserve: 1920x1080 -> S 0x870000 -> R 52 MiB; 100 MiB used + 3710 free - 52
echo "SESSION_RESERVE_MIB=0" > "$T/conf"
run apply > "$T/out" 2>&1
check "apply enables dmem in user@" yes "$(grep -qw dmem "$CG$mgr/cgroup.subtree_control" && echo yes)"
check "auto 1080p max" $(( (100 + 3710 - 52) << 20 )) "$(maxval)"
check "auto 1080p log" 1 "$(grep -c 'reserve 52.00 MiB (DP-1 1920x1080 S=8.44 MiB R=52.00 MiB; session 0 MiB)' "$T/out")"
for m in "2560x1440 82" "3840x2160 162"; do
    printf '%s\n1920x1080\n' "${m% *}" > "$DRM/card0-DP-1/modes"
    run apply > "$T/out" 2>&1
    check "auto ${m% *} max" $(( (100 + 3710 - ${m#* }) << 20 )) "$(maxval)"
done
printf '1920x1080\n' > "$DRM/card0-DP-1/modes"
rm -f "$T/conf"; run apply > "$T/out" 2>&1
check "default session reserve 32 MiB" $(( (100 + 3710 - 52 - 32) << 20 )) "$(maxval)"
echo "SESSION_RESERVE_MIB=0" > "$T/conf"

# fixed reserve, no cap, not enough free VRAM, a disconnected display
printf 'SESSION_RESERVE_MIB=0\nDISPLAY_RESERVE_MIB=128\n' > "$T/conf"; run apply > "$T/out" 2>&1
check "fixed 128 MiB" $(( (100 + 3710 - 128) << 20 )) "$(maxval)"
echo "DISPLAY_RESERVE_MIB=0" > "$T/conf"; run apply > "$T/out" 2>&1
check "reserve 0 is no cap" max "$(maxval)"
printf 'SESSION_RESERVE_MIB=0\nDISPLAY_RESERVE_MIB=-1\n' > "$T/conf"
FAKE_FREE_MIB=20 run apply > "$T/out" 2>&1
check "free below reserve freezes at current" 104857600 "$(maxval)"
check "free below reserve warns" 1 "$(grep -c 'warning: free VRAM' "$T/out")"
echo disconnected > "$DRM/card0-DP-1/status"; run apply > "$T/out" 2>&1
check "no display is no cap" max "$(maxval)"
echo connected > "$DRM/card0-DP-1/status"
echo "DISPLAY_RESERVE_MIB=abc" > "$T/conf"
check "bad setting fails" 1 "$(run apply > /dev/null 2>&1; echo $?)"
echo "APP_SLICE=../escape.slice" > "$T/conf"
check "bad slice fails" 1 "$(run apply > /dev/null 2>&1; echo $?)"
echo "SESSION_RESERVE_MIB=-5" > "$T/conf"
check "bad session reserve fails" 1 "$(run apply > /dev/null 2>&1; echo $?)"
rm -f "$T/conf"

# clear; a kernel without dmem; a driver without a region
run apply > /dev/null 2>&1; run clear > "$T/out" 2>&1
check "clear" max "$(maxval)"
echo "cpu io memory pids" > "$CG/cgroup.controllers"
check "no dmem: apply succeeds" 0 "$(run apply > "$T/out" 2>&1; echo $?)"
check "no dmem: says so" 1 "$(grep -c 'no dmem controller' "$T/out")"
echo "cpu io memory pids dmem" > "$CG/cgroup.controllers"
: > "$CG/dmem.capacity"
check "no region with a GPU fails" 1 "$(run apply > "$T/out" 2>&1; echo $?)"
echo "$REG 4294967296" > "$CG/dmem.capacity"
run status > "$T/out" 2>&1
check "status lists app.slice" 1 "$(grep -c "app.slice $REG current 104857600 max max" "$T/out")"

(( fail == 0 )) && echo "all passed" || { echo "FAILED (scratch: $T)"; exit 1; }
