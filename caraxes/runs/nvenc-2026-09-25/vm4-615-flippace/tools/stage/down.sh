#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The SotTR stage, down (docs/caraxes/sottr-stage.md): the Moonlight that stream.sh started
# (`moonlight quit` first, so that it prints its session summary, then its recorded pid only),
# Sunshine's log, the guest's state synced, the VM and device stopped through the launcher, the
# device's summary lines, host Xid lines, host watch and sampler stopped, the NAT rules up.sh
# added removed. The games overlay $W/games.qcow2 and the root overlay stay for the next start.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
# shellcheck source=caraxes/runs/sottr-2026-09-24/env.sh
source "$here/env.sh"
W=${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM
VMSH=("$LEA_TOOLS/caraxes-vm.sh")
ROOT=$(cd "$LEA_TOOLS/.." && pwd -P)
OUT=$(cat "$W/sottr-session" 2>/dev/null) || OUT=$here/session-down-$(date +%Y%m%d-%H%M%S)
mkdir -p "$OUT"
T0=$(cat "$OUT/t0" 2>/dev/null || date '+%F %T')
ML=${MOONLIGHT:-/usr/bin/moonlight}
up=0; [[ -s $W/ch.pid ]] && kill -0 "$(cat "$W/ch.pid")" 2>/dev/null && up=1

# Moonlight: quit the app on the host side (the running client then prints its summary and
# exits), then end the recorded instance if it is still there. Never another Moonlight.
if [[ -s $W/moonlight.pid ]]; then
    mp=$(cat "$W/moonlight.pid")
    if kill -0 "$mp" 2>/dev/null && [[ $(cat "/proc/$mp/comm" 2>/dev/null) == moonlight ]]; then
        timeout 25 "$ML" quit "$CARAXES_IP" >> "$OUT/moonlight-quit.txt" 2>&1
        for _ in $(seq 20); do kill -0 "$mp" 2>/dev/null || break; sleep 0.5; done
        if kill -0 "$mp" 2>/dev/null; then kill "$mp"; echo "moonlight pid $mp ended (SIGTERM)"; else echo "moonlight pid $mp quit"; fi
    fi
    rm -f "$W/moonlight.pid"
fi
grep -E 'frame rate|latency|dropped|decoding time' "$OUT/moonlight.log" 2>/dev/null | tail -12

if ((up)); then
    "${VMSH[@]}" ssh 'bash caraxes-sottr/sottr-session.sh status' > "$OUT/status-end.txt" 2>&1
    "${VMSH[@]}" ssh 'bash caraxes-sottr/sottr-session.sh sunshine-stop' > "$OUT/sunshine-stop.txt" 2>&1
    "${VMSH[@]}" ssh 'cat ~/.config/sunshine/sunshine.log' > "$OUT/sunshine.log" 2>&1
    "${VMSH[@]}" ssh 'sudo dmesg' > "$OUT/guest-dmesg.txt" 2>&1
    "${VMSH[@]}" ssh 'journalctl -b --no-pager -o short-monotonic 2>/dev/null | grep -iE "steam|gnome-shell|mutter|sunshine" | tail -400' > "$OUT/guest-journal.txt" 2>&1
    # The launcher stops the VM without a guest shutdown; sync first (display D0 lost an
    # unsynced module build that way, experiments.md).
    "${VMSH[@]}" ssh 'sync; sudo sync' > /dev/null 2>&1
fi
"${VMSH[@]}" down > "$OUT/down.txt" 2>&1; cat "$OUT/down.txt"
if [[ -f $W/gpu.log ]]; then
    grep -E '^intr page:|^display page:|doorbell poll \(|^vfio-user-nvgpu: (intr|display) page|reserv' "$W/gpu.log" | cut -c1-1500 > "$OUT/device-summary.txt"
    grep -E 'unanswered|ERROR|refused|NOT_SUPPORTED' "$W/gpu.log" | sort | uniq -c | sort -rn | head -20 | cut -c1-250 > "$OUT/device-log-issues.txt"
    tail -n 200 "$W/gpu.log" | cut -c1-600 > "$OUT/device-log-tail.txt"
fi
{
    echo '# SPDX-License-Identifier: MIT'
    echo "# host kernel log, Xid lines since $T0"
    journalctl -k --since "$T0" --no-pager -g 'Xid' 2>/dev/null | grep -v '^-- No entries'
} > "$OUT/host-xid.txt"
[[ -s $OUT/hostwatch.pid ]] && "$ROOT/caraxes/tools/bench-hostwatch.sh" stop "$OUT"
[[ -s $OUT/sampler.pid ]] && { kill "$(cat "$OUT/sampler.pid")" 2>/dev/null; rm -f "$OUT/sampler.pid"; }
if [[ -s $W/sottr-nat ]]; then
    while read -r line; do
        # shellcheck disable=SC2086
        if [[ $line == -t\ nat* ]]; then sudo -n iptables ${line/POSTROUTING/-D POSTROUTING}
        else sudo -n iptables ${line/FORWARD/-D FORWARD}; fi && echo "nat: removed $line"
    done < "$W/sottr-nat"
    rm -f "$W/sottr-nat"
fi
# Pairing material out of the kept logs (client certificate, challenges, pairing secret).
for f in "$OUT"/pair.log "$OUT"/moonlight*.log "$OUT"/moonlight-*.txt "$OUT"/sunshine.log; do
    [[ -f $f ]] || continue
    sed -i -E 's/(Executing request: "[^?"]*)\?[^"]*"/\1?<query omitted>"/; s/(Debug: [a-z]+ -- )[0-9a-f]{32,}/\1<omitted>/' "$f"
done
echo "host Xid lines since $T0:$(grep -vc '^#' "$OUT/host-xid.txt"); evidence in $OUT"
rm -f "$W/sottr-session"
