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
encoder = software
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
    for _ in $(seq 60); do (exec 3<>/dev/tcp/127.0.0.1/47989) 2>/dev/null && break; sleep 1; done
    pgrep -x sunshine > /dev/null || { echo 'sunshine did not start'; tail -20 "$DIR/sunshine.log" "$DIR/stdout.log"; exit 1; }
    (exec 3<>/dev/tcp/127.0.0.1/47989) 2>/dev/null || { echo 'sunshine never opened 47989'; tail -20 "$DIR/sunshine.log"; exit 1; }
    echo "sunshine pid $(pgrep -x sunshine | head -1), listening on 47989 (web UI 47990)"
    grep -a -m1 'Found H.264 encoder' "$DIR/sunshine.log"
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
*) echo "usage: $0 gnome [secs]|sunshine-start|sunshine-stop|status" >&2; exit 2 ;;
esac
