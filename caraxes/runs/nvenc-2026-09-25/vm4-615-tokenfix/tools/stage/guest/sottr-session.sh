#!/bin/bash
# SPDX-License-Identifier: MIT
# Guest (desktop user): the SotTR stage's session (docs/caraxes/sottr-stage.md section 4).
#   sottr-session.sh gnome [secs]      GNOME on Wayland through gdm3 autologin, as display-e.md
#                                      section 2 (gnome-check.sh start), unless it runs already
#   sottr-session.sh sunshine-start    Sunshine with capture=kms and software x264 (the display
#                                      stream stage, display-e.md section 7), web login and PIN
#                                      as v1 (bench.sh do_stream), listening on 47989/47990
#   sottr-session.sh sunshine-stop     Sunshine stopped, its log kept
#   sottr-session.sh status            session, Steam, Sunshine, games disk, KMS
# Sunshine runs as the user inside the GNOME session (the file capability cap_sys_admin+p set
# by sottr-prepare.sh gives it KMS capture), so it reaches the session's audio; its pairing
# state stays in ~/.config/sunshine across runs. SUN_USER, SUN_PASS come from up.sh.
set -u
cd ~/caraxes-guest || exit 1
DIR=$HOME/.config/sunshine
U=$(id -u)
RUN=/run/user/$U
senv() {
    local wd=wayland-0 c
    for c in "$RUN"/wayland-*; do case $c in *.lock) continue ;; esac; [ -S "$c" ] && { wd=${c##*/}; break; }; done
    echo "XDG_RUNTIME_DIR=$RUN WAYLAND_DISPLAY=$wd DISPLAY=:0 DBUS_SESSION_BUS_ADDRESS=unix:path=$RUN/bus XDG_CURRENT_DESKTOP=GNOME"
}
case ${1:-status} in
gnome)
    if pgrep -x gnome-shell >/dev/null; then echo "gnome-shell runs (pid $(pgrep -x gnome-shell | head -1))"
    else sudo bash gnome-check.sh start "${2:-45}"; fi
    ;;
sunshine-start)
    pgrep -x gnome-shell >/dev/null || { echo "no gnome-shell: run the gnome step first"; exit 1; }
    mkdir -p "$DIR"
    pkill -x sunshine; sleep 1
    cat > "$DIR/sunshine.conf" <<EOF
min_log_level = 1
capture = kms
encoder = ${SUN_ENCODER:-software}
sw_preset = ultrafast
sw_tune = zerolatency
log_path = $DIR/sunshine.log
file_apps = $DIR/apps.json
EOF
    echo '{"env":{},"apps":[{"name":"Desktop"}]}' > "$DIR/apps.json"
    : > "$DIR/sunshine.log"
    sunshine --creds "${SUN_USER:-lea}" "${SUN_PASS:-leastream}" "$DIR/sunshine.conf" >/dev/null 2>&1
    # shellcheck disable=SC2046
    setsid nohup env $(senv) sunshine "$DIR/sunshine.conf" > "$DIR/stdout.log" 2>&1 < /dev/null &
    # Up to 180 s: with encoder = nvenc Sunshine probes nvenc, then vulkan (four device creations
    # of about 15 s each on the device) and vaapi before libx264; 70 s on vm4-615 (2026-09-25).
    for _ in $(seq 180); do (exec 3<>/dev/tcp/127.0.0.1/47989) 2>/dev/null && break; sleep 1; done
    pgrep -x sunshine > /dev/null || { echo 'sunshine did not start'; tail -20 "$DIR/sunshine.log" "$DIR/stdout.log"; exit 1; }
    (exec 3<>/dev/tcp/127.0.0.1/47989) 2>/dev/null || { echo 'sunshine never opened 47989'; tail -20 "$DIR/sunshine.log"; exit 1; }
    echo "sunshine pid $(pgrep -x sunshine | head -1), listening on 47989 (web UI 47990)"
    grep -a -E 'Found H.264 encoder|Creating encoder|Encoder \[|failed' "$DIR/sunshine.log" | head -12
    grep -a -iE 'kms|monitor' "$DIR/sunshine.log" | head -6
    ;;
sunshine-stop)
    alive=0; pgrep -x sunshine > /dev/null && alive=1
    echo "sunshine_alive_at_end=$alive"
    pkill -x sunshine; sleep 2
    grep -a -iE 'fatal|error|couldn.t|failed' "$DIR/sunshine.log" | tail -8
    ;;
status)
    echo "== session"; loginctl list-sessions --no-legend
    for p in gnome-shell Xwayland steam steamwebhelper sunshine ShadowOfTheTomb; do
        printf '%-16s %s\n' "$p" "$(pgrep -f "$p" | head -3 | tr '\n' ' ')"
    done
    echo "== games"; findmnt /games; ls /games/SteamLibrary/steamapps/common 2>&1
    echo "== kms"; [ -x ./kmsgrab ] && sudo ./kmsgrab info 2>&1 | head -8
    ;;
checks-driver)
    # 615 (vm4-615): the packaged driver after display-up.sh, before GNOME.
    echo "== nvidia-smi"; nvidia-smi; echo "rc=$?"
    echo "== vulkaninfo --summary"; vulkaninfo --summary 2>&1; echo "rc=$?"
    echo "== icds"; ls -la /usr/share/vulkan/icd.d/ /etc/vulkan/icd.d/ 2>&1
    ;;
checks-session)
    # 615 (vm4-615): vkcube on the GNOME session's Wayland socket (300 frames), and Steam's
    # sniper container: steam-runtime-system-info inside it must list the NVIDIA ICD.
    pgrep -x gnome-shell >/dev/null || { echo "no gnome-shell: run the gnome step first"; exit 1; }
    echo "== vkcube"
    # shellcheck disable=SC2046
    env $(senv) timeout 60 vkcube --c 300 2>&1 | tail -8; echo "rc=${PIPESTATUS[0]}"
    echo "== sniper"
    R=""
    for d in /games/SteamLibrary/steamapps/common/SteamLinuxRuntime_sniper "$HOME"/.steam/debian-installation/steamapps/common/SteamLinuxRuntime_sniper; do
        [ -x "$d/run-in-sniper" ] && { R=$d; break; }
    done
    [ -n "$R" ] || { echo "no SteamLinuxRuntime_sniper/run-in-sniper"; exit 0; }
    echo "runtime: $R ($(cat "$R/VERSIONS.txt" 2>/dev/null | grep -m1 -E '^sniper|^depot' | tr '\t' ' '))"
    # shellcheck disable=SC2046
    env $(senv) timeout 300 "$R/run-in-sniper" -- steam-runtime-system-info > "$HOME/sniper-system-info.json" 2> "$HOME/sniper-system-info.err"
    echo "rc=$? ($(wc -c < "$HOME/sniper-system-info.json") bytes, ~/sniper-system-info.json)"
    python3 - "$HOME/sniper-system-info.json" <<'PY'
import json, sys
try:
    d = json.load(open(sys.argv[1]))
except Exception as e:
    print("no JSON:", e); sys.exit(0)
print("overall issues:", d.get("overall-issues", d.get("issues")))
g = d.get("graphics-details", {})
for k, v in g.items():
    print("graphics", k, "renderer:", v.get("renderer"), "version:", v.get("version"), "issues:", v.get("issues"))
for icd in d.get("vulkan", {}).get("icds", []):
    print("vulkan icd:", icd.get("json_path"), icd.get("library_path"), icd.get("dlopen", ""), "issues:", icd.get("issues"))
for icd in d.get("egl", {}).get("icds", []):
    print("egl icd:", icd.get("json_path"), icd.get("library_path"), "issues:", icd.get("issues"))
c = d.get("container", {})
print("container:", c.get("type"), (c.get("host") or {}).get("path"))
PY
    ;;
*) echo "usage: $0 gnome [secs]|sunshine-start|sunshine-stop|status|checks-driver|checks-session" >&2; exit 2 ;;
esac
