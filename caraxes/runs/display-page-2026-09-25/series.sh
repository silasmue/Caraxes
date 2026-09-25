#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The desktop series of 2026-09-25 (docs/caraxes/intr-page.md §11): vm0 in env.sh's environment
# (the recommended configuration at slot 0). Configurations:
#   D  the recommended configuration on main (the memfd's and the MSI re-arm's effect on the desktop)
#   E  D plus the BAR2 flush deferral (CARAXES_BAR2_FLUSH_DEFER_US=200) and the display clock's
#      timing counters (binary vfio-user-nvgpu-desk-e)
#   F  the deferral with a 2 ms bound (E's 200 us paid most debts in the middle of a PTE batch)
#      and the MMU invalidate page (CARAXES_INVALIDATE_PAGE=1, binary vfio-user-nvgpu-desk-f)
#   D2 D again in the same session as F, as the load reference (other agents' VMs share the card
#      and the CPUs; run.sh records the host load per window in host-load.txt)
#   G  F with the display delivery counters (binary vfio-user-nvgpu-desk-g) and the reactor
#      spinning from 1.5 ms before each display tick (CARAXES_INTR_PAGE_VBLANK_SPIN_US=1500)
#   H  G without the spin window (the A/B for it)
#   I  H with the pool poll after a page trigger (binary vfio-user-nvgpu-desk-i): the windows of
#      this branch as they are recommended (deferral on = 2 ms, invalidate page, no spin)
#   I2 I again, with the guest's interrupt placement recorded (guest/irq-info.sh); binary
#      vfio-user-nvgpu-desk-i2, the same code built at the successor's start
#   P1 K's configuration with the BAR2 flush page as memory (CARAXES_BAR2_FLUSH_PAGE=memory,
#      binary vfio-user-nvgpu-desk-p1, intr-page.md §12.1)
#   U1 P1 with the usermode page as memory (CARAXES_USERMODE_PAGE=1, binary
#      vfio-user-nvgpu-desk-u1, intr-page.md §12.2), the hybrid reactor
#   U2 U1 with the reactor spinning (CARAXES_DOORBELL_POLL=spin)
#   W1 U1 with the display kick page (CARAXES_DISPLAY_KICK_PAGE=1, intr-page.md §12.3): every
#      window of this branch; W2 the kick page with the token pool (no usermode page)
#   U1, U2, T1, W1 and W2 run vfio-user-nvgpu-desk-w1
#   T1 P1 again with the binary of U1 (the token pool's doorbell), the reference for U1 and U2
#   CUDA=1 adds the launch rows after the trap windows: nvprobe 4 (100 launches, blocking
#   sync) with 1000-cycle and 30M-cycle kernels, as bench-run.sh short-blocking and
#   launch-blocking run them, from the guest's ~/bench
#   J1 J2  the A/B for the pool poll after a page trigger (f37b354), interleaved with K1 K2 in
#      one session: J is H's binary and configuration (no poll after a trigger), K this
#      branch's binary with the watchdog counters (vfio-user-nvgpu-desk-k) and I's
#      configuration; one GNOME start each and no gate (GNOME_STARTS=1 SKIP_GATE=1)
# Per configuration, with the host watch around it: up (a refused reservation is retried every
# 5 minutes), probe, display-up, GNOME_STARTS fresh GNOME starts, the trap windows (run.sh --full,
# trap-table.py --pages --regs): Mutter with wlframes, the clients vkcube and glmark2 under Mutter
# and a browser if the image has one, then Weston with wlframes; the display gate; down; the
# device's summary lines; host Xid lines with the device pid.
# Run from the worktree root: caraxes/runs/display-page-2026-09-25/series.sh D [E F]
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
# shellcheck disable=SC1091
source "$here/env.sh"
W=${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM
VMSH=("$LEA_TOOLS/caraxes-vm.sh")
STARTS=${GNOME_STARTS:-3}
# The image's firefox is the snap stub (D: "snap install firefox"), so no browser by default.
APPS=${APPS:-vkcube glmark2}
G24=$here/../display-page-2026-09-24/guest
[[ $# -eq 0 ]] && set -- D
for cfg in "$@"; do
    source "$here/env.sh"
    case $cfg in
        D) name=D-recommended ;;
        E) export CARAXES_SERVER=${CARAXES_SERVER_E:-/mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-desk-e}
           export CARAXES_BAR2_FLUSH_DEFER_US=200; name=E-flush-defer ;;
        F) export CARAXES_SERVER=${CARAXES_SERVER_F:-/mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-desk-f}
           export CARAXES_BAR2_FLUSH_DEFER_US=2000 CARAXES_INVALIDATE_PAGE=1; name=F-invalidate-page ;;
        D2) name=D2-recommended-rerun ;;
        I|I2) export CARAXES_SERVER=${CARAXES_SERVER_I:-/mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-desk-i}
           export CARAXES_BAR2_FLUSH_DEFER_US=on CARAXES_INVALIDATE_PAGE=1
           name=I-windows; [[ $cfg == I2 ]] && name=I2-windows-rerun ;;
        J1|J2) export CARAXES_SERVER=${CARAXES_SERVER_G:-/mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-desk-g}
           export CARAXES_BAR2_FLUSH_DEFER_US=2000 CARAXES_INVALIDATE_PAGE=1
           unset CARAXES_INTR_PAGE_VBLANK_SPIN_US; name=$cfg-no-poll-after-trigger ;;
        K1|K2) export CARAXES_SERVER=${CARAXES_SERVER_K:-/mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-desk-k}
           export CARAXES_BAR2_FLUSH_DEFER_US=on CARAXES_INVALIDATE_PAGE=1
           unset CARAXES_INTR_PAGE_VBLANK_SPIN_US; name=$cfg-poll-after-trigger ;;
        P1) export CARAXES_SERVER=${CARAXES_SERVER_P1:-/mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-desk-p1}
           export CARAXES_BAR2_FLUSH_DEFER_US=on CARAXES_INVALIDATE_PAGE=1 CARAXES_BAR2_FLUSH_PAGE=memory
           unset CARAXES_INTR_PAGE_VBLANK_SPIN_US; name=P1-flush-page-memory ;;
        U1|U2|T1|W1|W2) export CARAXES_SERVER=${CARAXES_SERVER_U:-/mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-desk-w1}
           export CARAXES_BAR2_FLUSH_DEFER_US=on CARAXES_INVALIDATE_PAGE=1 CARAXES_BAR2_FLUSH_PAGE=memory
           unset CARAXES_INTR_PAGE_VBLANK_SPIN_US
           case $cfg in
               U1) export CARAXES_USERMODE_PAGE=1; name=U1-usermode-page ;;
               U2) export CARAXES_USERMODE_PAGE=1 CARAXES_DOORBELL_POLL=spin; unset CARAXES_DOORBELL_POLL_IDLE_US
                   name=U2-usermode-page-spin ;;
               T1) name=T1-tokens-reference ;;
               W1) export CARAXES_USERMODE_PAGE=1 CARAXES_DISPLAY_KICK_PAGE=1; name=W1-all-windows ;;
               W2) export CARAXES_DISPLAY_KICK_PAGE=1; name=W2-kick-page-tokens ;;
           esac ;;
        G|H) export CARAXES_SERVER=${CARAXES_SERVER_G:-/mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-desk-g}
           export CARAXES_BAR2_FLUSH_DEFER_US=2000 CARAXES_INVALIDATE_PAGE=1
           if [[ $cfg == G ]]; then
               export CARAXES_INTR_PAGE_VBLANK_SPIN_US=1500; name=G-vblank-spin
           else
               unset CARAXES_INTR_PAGE_VBLANK_SPIN_US; name=H-no-spin
           fi ;;
        *) echo "unknown configuration $cfg"; continue ;;
    esac
    D=$here/$name
    [[ -e $D ]] && { echo "ERROR: $D exists; never overwrite a run" >&2; continue; }
    if pgrep -f "/mnt/vmstore/caraxes/$VM/" > /dev/null; then
        echo "ERROR: a $VM process runs; not starting" >&2; pgrep -af "/mnt/vmstore/caraxes/$VM/" >&2; exit 2
    fi
    mkdir -p "$D"
    { head -1 /proc/driver/nvidia/version; sha256sum "$CARAXES_SERVER"; date -Iseconds; cut -d' ' -f1-3 /proc/loadavg
      env | grep -E '^(CARAXES_|LEA_CH|VM=)' | sort; } > "$D/setup.txt"
    pgrep -af 'vfio-user-nvgpu --socket|vhost-user-nvrm|cloud-hypervisor' | grep -v pgrep | cut -c1-160 > "$D/other-vms-at-start.txt"
    t0=$(date '+%F %T')
    caraxes/tools/bench-hostwatch.sh start "$D"
    echo "== $name up at $t0"
    up=0
    for try in $(seq 1 13); do
        if "${VMSH[@]}" up >> "$D/up.txt" 2>&1 && "${VMSH[@]}" wait 180 >> "$D/up.txt" 2>&1; then up=1; break; fi
        tail -3 "$D/up.txt"
        "${VMSH[@]}" down >> "$D/up.txt" 2>&1
        if grep -qE 'headroom|NO_MEMORY|did not give|refused: reserving|MiB floor' "$D/up.txt" "$W/gpu.log" 2>/dev/null && ((try < 13)); then
            echo "try $try $(date -Iseconds): reservation refused, next try in 5 min" | tee -a "$D/retries.txt"
            sleep 300; continue
        fi
        break
    done
    if ((up)); then
        pgrep -af "vfio-user-nvgpu.*$W/" | cut -c1-200 > "$D/device-pid.txt"
        grep -iE 'intr page|display page|bar2 memfd|flush deferral|bar1 pool|msi|lazy|doorbell tokens|doorbell poll|invalidate page' "$W/gpu.log" | cut -c1-500 > "$D/device-start.txt"
        "${VMSH[@]}" probe "desk-$name" > "$D/probe.txt" 2>&1
        for f in "$G24"/*.sh "$here"/guest/*.sh; do
            "${VMSH[@]}" ssh "mkdir -p caraxes-f && cat > caraxes-f/$(basename "$f")" < "$f"
        done
        "$here/run.sh" "$name/display-up" 'cd caraxes-guest && sudo bash display-up.sh && sudo bash nvidia-egl-setup.sh on && lsmod | grep nvidia'
        for i in $(seq 1 "$STARTS"); do
            "$here/run.sh" "$name/gnome-start-$i" bash caraxes-f/gnome-restart.sh 20
            echo "gnome-start-$i: GPPut barrier waits $(xz -dcf "$D/gnome-start-$i"/device-events.txt* 2>/dev/null | grep -c 'GPPut of a mirrored channel held')"
        done
        "$here/run.sh" "$name/irq-info" bash caraxes-f/irq-info.sh
        wins=(traps-mutter)
        "$here/run.sh" "$name/traps-mutter" --full bash caraxes-f/trap-mutter.sh 14
        for app in $APPS; do
            "$here/run.sh" "$name/traps-$app" --full bash caraxes-f/trap-app.sh "$app" 16
            wins+=("traps-$app")
        done
        "$here/run.sh" "$name/traps-weston" --full bash caraxes-f/trap-weston.sh 14
        wins+=(traps-weston)
        if [[ -n ${CUDA:-} ]]; then
            pre='sudo modprobe nvidia-uvm; sudo nvidia-modprobe -u -c=0 2>/dev/null; cd ~/bench && export LD_LIBRARY_PATH=/opt/nvrm/lib NVPROBE_PTX=$HOME/bench/kernels.ptx &&'
            "$here/run.sh" "$name/cuda-short" "$pre NVPROBE_ITERS=100 NVPROBE_CYCLES=1000 NVPROBE_SCHED=blocking ./nvprobe 4"
            "$here/run.sh" "$name/cuda-launch" "$pre NVPROBE_ITERS=100 NVPROBE_SCHED=blocking ./nvprobe 4"
            "$here/run.sh" "$name/cuda-short-2" "$pre NVPROBE_ITERS=100 NVPROBE_CYCLES=1000 NVPROBE_SCHED=blocking ./nvprobe 4"
            for r in cuda-short cuda-launch cuda-short-2; do echo "-- $r"; tail -4 "$D/$r/guest.txt"; done
        fi
        for r in "${wins[@]}"; do
            python3 caraxes/tools/trap-table.py "$D/$r/device-trace.txt.xz" --pages --regs > "$D/$r/trap-table.txt" 2>&1
            echo "-- $r"
            grep -E "FRAMES|PRESENTED|^APP" "$D/$r/guest.txt"
            grep -E "all trapped accesses|latch intervals|per-frame base" "$D/$r/trap-table.txt"
        done
        [[ -n ${SKIP_GATE:-} ]] || CARAXES_GATE_RUNS=$D "$LEA_TOOLS/caraxes-gate.sh" display > "$D/gate.json" 2> "$D/gate.err"
        echo "gate rc=$?" >> "$D/gate.json"
        grep -o '"result": *"[^"]*"' "$D/gate.json" | head -1
    else
        echo "VM did not come up (up.txt, retries.txt)"
    fi
    pid=$(cat "$W/gpu.pid" 2>/dev/null)
    "${VMSH[@]}" down > "$D/down.txt" 2>&1
    grep -E '^intr page:|^bar1 pool:|^display page|^invalidate page:|^display engine:|^bar2 flush deferral|^bar2 flush page|^usermode page:|^display kick page:|^reactor wake-up|bar2 memfd|doorbell poll \(|msi re-arm|^vfio-user-nvgpu: (intr|display|invalidate) page' "$W/gpu.log" | cut -c1-1600 > "$D/device-summary.txt"
    caraxes/tools/bench-hostwatch.sh stop "$D"
    { echo "# host kernel log, Xid lines since $t0 (device pid $pid)"
      journalctl -k --since "$t0" --no-pager -g 'Xid' 2>/dev/null | grep -v '^-- No entries'; } > "$D/host-xid.txt"
    echo "$name: Xid lines $(grep -vc '^#' "$D/host-xid.txt"), device pid was $pid, $VM processes left $(pgrep -fc "/mnt/vmstore/caraxes/$VM/")"
    cat "$D/device-summary.txt"
done
