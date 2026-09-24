#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Series F, the STREAM stage on vm0 (docs/caraxes/display-e.md §7): Sunshine in the guest with
# capture=kms and software x264 (guest/sunshine-stream.sh), Moonlight on this host over the
# tap network, Moonlight's session summary as the verdict, parsed with the patterns of
# ../Leandro-Test/scripts/bench.sh do_stream. Moonlight opens a window on the host desktop.
# Usage (worktree root): stream.sh [label] [seconds] [bitrate kbps] [WxH] [fps]
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
label=${1:-f7-stream} secs=${2:-60} bitrate=${3:-20000} res=${4:-1920x1080} fps=${5:-60}
D=$here/$label
VMSH=("$here/vm.sh")
IP=192.168.100.20
mkdir -p "$D"
tar -C "$here/guest" -cf - . | "${VMSH[@]}" ssh 'mkdir -p caraxes-f && tar -C caraxes-f -xf -'
t0=$(date '+%F %T')
"${VMSH[@]}" ssh 'bash caraxes-f/sunshine-stream.sh start' > "$D/guest-start.txt" 2>&1 || { cat "$D/guest-start.txt"; exit 1; }
cat "$D/guest-start.txt"
found=$(grep -m1 'Found H.264 encoder' "$D/guest-start.txt")
if ! timeout 25 moonlight list "$IP" > /dev/null 2>&1; then
    echo pairing
    ( timeout 60 moonlight pair "$IP" --pin 4321 > "$D/pair.log" 2>&1 ) &
    pp=$!
    # The PIN goes in while Moonlight's /pair request is pending; retried, each answer kept
    # in its own file (Moonlight writes pair.log at its own offset).
    for i in 1 2 3 4 5 6; do
        sleep 4
        curl -sk -m 10 -u lea:leastream -H 'Content-Type: application/json' \
            -d '{"pin":"4321","name":"host"}' "https://$IP:47990/api/pin" > "$D/pin-$i.txt" 2>&1
        echo " (http $(curl -sk -m 10 -o /dev/null -w '%{http_code}' -u lea:leastream "https://$IP:47990/api/clients/list"))" >> "$D/pin-$i.txt"
        grep -q '"status":true' "$D/pin-$i.txt" && break
    done
    wait $pp
    timeout 25 moonlight list "$IP" > /dev/null 2>&1 || { echo "pairing failed, $D/pair.log"; }
fi
vcpus=$("${VMSH[@]}" ssh nproc | tr -dc '0-9')
cpu0=$("${VMSH[@]}" ssh "grep '^cpu ' /proc/stat")
b0=$(stat -c %s /mnt/vmstore/caraxes/vm0/gpu-trace.log)
echo "streaming ${secs}s ${res}@${fps} ${bitrate} kbps"
( timeout $((secs + 30)) moonlight stream "$IP" Desktop --resolution "$res" --fps "$fps" --bitrate "$bitrate" \
    --display-mode windowed --no-vsync --no-quit-after > "$D/moonlight.log" 2>&1 ) &
ml=$!
sleep "$secs"
cpu1=$("${VMSH[@]}" ssh "grep '^cpu ' /proc/stat")
timeout 25 moonlight quit "$IP" >> "$D/moonlight.log" 2>&1
wait $ml 2>/dev/null
"${VMSH[@]}" ssh 'bash caraxes-f/sunshine-stream.sh stop' > "$D/guest-stop.txt" 2>&1
"${VMSH[@]}" ssh 'cat /tmp/caraxes-sunshine/sunshine.log' > "$D/sunshine.log" 2>&1
tail -c +$((b0 + 1)) /mnt/vmstore/caraxes/vm0/gpu-trace.log | grep -a ' E ' | xz -T0 -3 > "$D/device-events.txt.xz"
{
    echo '# SPDX-License-Identifier: MIT'
    echo "# host kernel log, Xid lines since $t0 (device pid $(cat /mnt/vmstore/caraxes/vm0/gpu.pid))"
    journalctl -k --since "$t0" --no-pager -g 'Xid' 2>/dev/null | grep -v '^-- No entries'
} > "$D/host-xid.txt"
cat "$D/guest-stop.txt"
alive=0; grep -q 'sunshine_alive_at_end=1' "$D/guest-stop.txt" && alive=1
python3 - "$D" "$found" "$bitrate" "$res" "$fps" "$secs" "$alive" "$cpu0" "$cpu1" "${vcpus:-8}" <<'PY'
import re, sys, json, pathlib
out, found, bitrate, res, fps, secs, alive, cpu0, cpu1, vcpus = sys.argv[1:11]
log = pathlib.Path(out, "moonlight.log").read_text(errors="replace")
def grab(p):
    m = re.search(p, log)
    return list(m.groups()) if m else None
stats = {
    "incoming_fps":  grab(r"Incoming frame rate from network: ([\d.]+) FPS"),
    "decoding_fps":  grab(r"Decoding frame rate: ([\d.]+) FPS"),
    "rendering_fps": grab(r"Rendering frame rate: ([\d.]+) FPS"),
    "host_latency":  grab(r"Host processing latency min/max/average: ([\d.]+)/([\d.]+)/([\d.]+) ms"),
    "decode_time":   grab(r"Average decoding time: ([\d.]+) ms"),
    "net_latency":   grab(r"Average network latency: (\d+) ms"),
    "drop_network":  grab(r"Frames dropped by your network connection: ([\d.]+)%"),
    "drop_jitter":   grab(r"Frames dropped due to network jitter: ([\d.]+)%"),
    "bitrate_mbps":  grab(r"(?i)(?:video )?bitrate[^\d]*([\d.]+) ?Mbps"),
}
a = [int(x) for x in cpu0.split()[1:]]; b = [int(x) for x in cpu1.split()[1:]]
d = [y - x for x, y in zip(a, b)]; tot = sum(d); busy = (tot - d[3] - d[4]) / tot if tot else 0
enc = "software" if "libx264" in found else ("nvenc" if "nvenc" in found else "unknown")
inc = float(stats["incoming_fps"][0]) if stats["incoming_fps"] else None
valid = alive == "1" and inc is not None and inc > 0
stats.update(capture="kms", encoder=enc, encoder_line=found.strip(), bitrate_kbps=int(bitrate), resolution=res,
             fps=int(fps), seconds=int(secs), sunshine_alive_at_end=(alive == "1"), valid=valid,
             guest_cpu_percent=round(busy * 100, 2), guest_cpu_cores=round(busy * int(vcpus), 3))
pathlib.Path(out, "stats.json").write_text(json.dumps(stats, indent=2))
print(f"== desktop (GNOME) / kms + {enc}, {res}@{fps}, {bitrate} kbps, {secs}s ==")
for k in ("incoming_fps", "decoding_fps", "rendering_fps", "host_latency", "decode_time", "net_latency",
          "drop_network", "drop_jitter", "bitrate_mbps"):
    v = stats[k]
    print(f"  {k:<16} {'-' if not v else '/'.join(v)}")
print(f"  guest CPU        {busy * 100:.1f} % of {vcpus} = {busy * int(vcpus):.2f} cores")
if not valid:
    print("  RUN INVALID (Sunshine gone or no frames arrived)")
    sys.exit(3)
PY
