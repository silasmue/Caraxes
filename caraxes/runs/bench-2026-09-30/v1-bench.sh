#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Leandro VirtIO (v1) on its 615.71.09 image, the VM of caraxes/tools/bench-v1vm.sh on a fresh
# overlay: the P13 metrics of caraxes/tools/bench-run.sh (v1 only, restart per rep, as in
# docs/vfio/benchmark.md), the night-2026-09-28 workload series, Leandro-Test's lt-all.sh (the
# list of caraxes/runs/ctxcreate-2026-09-29), the v1 smi and cuda gates (v1-gates.sh), glmark2
# off-screen when the image has it, and the showcase text files.
# - Network: the seed puts the guest on the caraxes2 tap (192.168.102.25); it is made from the
#   image's own seed when missing (user-data address and gateway, a new instance-id).
# - The backend gets LEA_VRAM_LIMIT_MIB=4096 (the v2 guests' 4 GiB; the host keeps its VRAM).
# - The image's unattended-upgrades can hold a poweroff for 30 min; it is masked in the
#   overlay after every start, and a hypervisor that outlives the poweroff is stopped by pid.
# Usage (inside vmlock.sh v1): v1-bench.sh OUTDIR [steps...]  steps: p13 series lt gates gl
set -uo pipefail
C=$1; shift
STEPS=${*:-p13 series lt gates gl}
REPO=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd -P)
STORE=${CARAXES_STORE_ROOT:-/mnt/vmstore/caraxes}
GS=${GUEST_SERIES:-$STORE/night-2026-09-28/guest-scripts/guest-series.sh}
export LEA_V1_ROOT=${LEA_V1_ROOT:-$REPO/../Leandro}
export NVRM=${NVRM:-/mnt/vmstore/leandro/l615-host-bin-8c0d2b2/vhost-user-nvrm}
export V=${V:-$STORE/bench-v1-615.71.09-b} LEA_V1_SEED=${LEA_V1_SEED:-$STORE/bench-v1-615.71.09-b.seed.img}
export IP=192.168.102.25 TAP=caraxes2 MAC=02:00:00:00:00:25
export LEA_VRAM_LIMIT_MIB=4096 LEA_MANAGED_COMPAT=1
export LT=${LT:-$REPO/../Leandro-Test}
VM=$REPO/caraxes/tools/bench-v1vm.sh
BR=$REPO/caraxes/tools/bench-run.sh
[[ -e $C ]] && { echo "ERROR: $C exists" >&2; exit 2; }
mkdir -p "$C"
say() { echo "$(date +%T) $*" | tee -a "$C/steps.txt"; }
T0=$(date '+%F %T')
if [[ ! -f $LEA_V1_SEED ]]; then
    src=$STORE/images/guest-v1-2026-09-24-615.71.09.seed.img w=$C/seed-work
    mkdir -p "$w" && cp "$src" "$LEA_V1_SEED" && chmod u+w "$LEA_V1_SEED" &&
        mtype -i "$LEA_V1_SEED" ::/user-data | sed 's|192.168.105.25/24|192.168.102.25/24; s|via: 192.168.105.1|via: 192.168.102.1|' > "$w/user-data" &&
        mtype -i "$LEA_V1_SEED" ::/meta-data | sed 's|^instance-id: .*|instance-id: v1-615-bench-20260930|' > "$w/meta-data" &&
        mcopy -o -i "$LEA_V1_SEED" "$w/user-data" ::/user-data && mcopy -o -i "$LEA_V1_SEED" "$w/meta-data" ::/meta-data &&
        chmod 444 "$LEA_V1_SEED"; rm -rf "$w"
fi
{ head -1 /proc/driver/nvidia/version; sha256sum "$NVRM" "$(readlink -f "$REPO/vendor/cloud-hypervisor/target/release/cloud-hypervisor")"
  git -C "$LEA_V1_ROOT" log --oneline -1; date -Iseconds; cut -d' ' -f1-3 /proc/loadavg
  env | grep -E '^LEA_|^V=|^IP=|^TAP=' | sort; } > "$C/setup.txt"
[[ -f $V/rootfs.qcow2 ]] || "$VM" prepare >> "$C/setup.txt" 2>&1
"$VM" config > "$C/config.txt" 2>&1
quiet_apt() { "$VM" ssh 'sudo systemctl kill -s KILL unattended-upgrades.service apt-daily.service apt-daily-upgrade.service 2>/dev/null; sudo systemctl mask unattended-upgrades.service apt-daily.timer apt-daily-upgrade.timer >/dev/null 2>&1; echo apt-quiet' >> "$C/up.txt" 2>&1; }
up() { "$VM" up >> "$C/up.txt" 2>&1 && "$VM" modules >> "$C/up.txt" 2>&1 && quiet_apt && say "up: $(grep -a 'backend pid' "$C/up.txt" | tail -1 | cut -c1-160)"; }
restart() {
    local hp bp
    hp=$(cat "$V/hypervisor.pid" 2>/dev/null) bp=$(cat "$V/backend.pid" 2>/dev/null)
    if ! "$VM" down >> "$C/up.txt" 2>&1; then
        say "down failed: stopping hypervisor $hp and backend $bp by pid"
        [[ -n $hp ]] && kill "$hp" 2>/dev/null; sleep 5; [[ -n $bp ]] && kill "$bp" 2>/dev/null; sleep 2
    fi
    sleep 3; up
}
trap '"$VM" down >> "$C/down.txt" 2>&1; say "down"' EXIT
up || { say "up failed"; tail -20 "$C/up.txt"; exit 1; }
"$BR" stage v1 > "$C/stage.txt" 2>&1
"$BR" stage-llama v1 >> "$C/stage.txt" 2>&1
for st in $STEPS; do
    say "=== $st"
    case $st in
    p13)
        V1_RESTART=1 "$BR" series --targets v1 --reps 3 --out "$C/p13" \
            --metrics "short-auto short-blocking launch-auto launch-blocking copy init churn ioctl ctrl rl mm" > "$C/p13.txt" 2>&1
        restart ;;
    series)
        # The workload series with libcuda and the NVIDIA user space from /opt/nvrm/lib.
        sed -e '2a export LD_LIBRARY_PATH=/opt/nvrm/lib' \
            -e 's|^L="env LD_LIBRARY_PATH=$HOME/bench-llama/lib"|L="env LD_LIBRARY_PATH=$HOME/bench-llama/lib:/opt/nvrm/lib"|' "$GS" > "$C/guest-series-v1.sh"
        "$VM" ssh 'bash -s' < "$C/guest-series-v1.sh" > "$C/guest-series.txt" 2>&1
        rm -f "$C/guest-series-v1.sh"
        grep -E "MSPERLAUNCH|\| llama|TRANSFORMER|CONVNET|^ENC|^DEC|RESULT|xid" "$C/guest-series.txt" | cut -c1-200 | tee -a "$C/steps.txt"
        restart ;;
    lt)
        "$VM" ssh 'test -r ~/caraxes-tests/caraxes/driver.env || printf "# SPDX-License-Identifier: MIT\nCARAXES_GUEST_LIBDIR=/opt/nvrm/lib\nCARAXES_GUEST_SMI=/opt/nvrm/bin/nvidia-smi\n" > ~/caraxes-tests/caraxes/driver.env; cat ~/caraxes-tests/caraxes/driver.env' > "$C/lt-driver-env.txt" 2>&1
        "$VM" ssh 'mkdir -p ~/caraxes-tests/caraxes && cat > ~/caraxes-tests/caraxes/lt-all.sh && chmod +x ~/caraxes-tests/caraxes/lt-all.sh' < "$REPO/caraxes/runs/ctxcreate-2026-09-29/lt-all.sh"
        "$VM" ssh 'export LD_LIBRARY_PATH=/opt/nvrm/lib; ~/caraxes-tests/caraxes/lt-all.sh ~/caraxes-tests/out/lt-all-v1' > "$C/lt-all.txt" 2>&1
        say "lt-all: $(tail -1 "$C/lt-all.txt")"
        "$VM" ssh 'cd ~/caraxes-tests/out && tar czf - lt-all-v1' > "$C/lt-all.tgz" 2>/dev/null
        grep '^LTROW' "$C/lt-all.txt" > "$C/rows.tsv"
        restart ;;
    gates)
        "$REPO/caraxes/tools/v1-gates.sh" "$C/gates" all > "$C/gates.txt" 2>&1
        say "gates rc=$?: $(grep -h 'PASS\|FAIL' "$C/gates/smi/verdict.txt" 2>/dev/null | cut -c1-120) $(tail -1 "$C/gates.txt")" ;;
    gl)
        "$VM" ssh 'export LD_LIBRARY_PATH=/opt/nvrm/lib; for p in glmark2 Xvfb vkmark; do printf "%s=%s " $p "$(command -v $p || echo none)"; done; echo; ls /opt/nvrm-gl/env.sh 2>&1' > "$C/gl.txt" 2>&1
        if grep -q "glmark2=/" "$C/gl.txt" && grep -q "Xvfb=/" "$C/gl.txt" && grep -q "^/opt/nvrm-gl/env.sh" "$C/gl.txt"; then
            "$VM" ssh 'unset DISPLAY; (setsid nohup Xvfb :1 -screen 0 1920x1080x24 +extension GLX >/tmp/xvfb.log 2>&1 </dev/null &); sleep 3; . /opt/nvrm-gl/env.sh
                export DISPLAY=:1 __NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia
                timeout 600 glmark2 --off-screen --size 1920x1080 2>&1 | tail -60' >> "$C/gl.txt" 2>&1
        else
            echo "glmark2 off-screen not available in this image (needs glmark2, Xvfb and /opt/nvrm-gl)" >> "$C/gl.txt"
        fi
        say "gl: $(grep -E 'Score|not available' "$C/gl.txt" | tail -1)" ;;
    *) say "unknown step $st" ;;
    esac
done
# Showcase text files (not committed): what the guest sees, hardware identifiers redacted.
SC=$C/showcase; mkdir -p "$SC"
red() { sed -E 's/(Serial Number|GPU UUID|UUID|Board Part Number|GPU Part Number)( *:).*/\1\2 (redacted)/; s/GPU-[0-9a-f-]{36}/GPU-(redacted)/g'; }
"$VM" ssh 'export LD_LIBRARY_PATH=/opt/nvrm/lib; timeout 30 /opt/nvrm/bin/nvidia-smi' 2>&1 | red > "$SC/nvidia-smi.txt"
"$VM" ssh 'export LD_LIBRARY_PATH=/opt/nvrm/lib; timeout 30 /opt/nvrm/bin/nvidia-smi -q | head -40' 2>&1 | red > "$SC/nvidia-smi-q.txt"
"$VM" ssh 'lspci -nnk | grep -iA3 -E "virtio|nvidia|1af4"; uname -r; lsmod | grep -E "^(virtio_nvrm|nvrm_nodes) "' > "$SC/lspci-uname.txt" 2>&1
{ echo "# host kernel log, Xid lines since $T0"; journalctl -k --since "$T0" --no-pager -g Xid 2>/dev/null | grep -v '^-- No entries'; } > "$C/host-xid.txt"
say "done: host Xid lines $(grep -vc '^#' "$C/host-xid.txt")"
