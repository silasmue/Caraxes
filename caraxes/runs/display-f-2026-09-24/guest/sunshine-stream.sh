#!/bin/bash
# SPDX-License-Identifier: MIT
# Series F (guest, desktop user), the STREAM stage of ../Leandro-Test/scripts/bench.sh on the
# Caraxes display: Sunshine with capture=kms and software x264 on the GNOME session, glxgears
# (NVIDIA GLX through Xwayland) as the content. Like bench.sh it borrows the desktop's
# Sunshine config (~/.config/sunshine/sunshine.conf) and puts it back, and restarts a Sunshine
# that was running before. Sunshine runs as root here (KMS capture needs CAP_SYS_ADMIN), so its
# login and pairing state live in root's ~/.config/sunshine.
#   sunshine-stream.sh start   GNOME up if needed, content, Sunshine listening on 47989
#   sunshine-stream.sh stop    logs, Sunshine stopped, config restored, content stopped
set -u
cd ~/caraxes-guest || exit 1
DIR=$HOME/.config/sunshine
S=/tmp/caraxes-sunshine
U=$(id -u)
xa() { ls /run/user/"$U"/.mutter-Xwaylandauth.* 2>/dev/null | head -1; }
case ${1:-start} in
start)
    pgrep -x gnome-shell >/dev/null || sudo bash gnome-check.sh start 60 | grep -E "Type=wayland|gbm renderer|page_flip"
    mkdir -p "$DIR" "$S"
    # What was there, recorded before anything is touched (bench.sh lea_stream_sunshine_save).
    p=$(pgrep -x sunshine | head -1)
    if [ -n "$p" ]; then echo "was_up=1 cmdline=$(tr '\0' ' ' < /proc/$p/cmdline)" > $S/saved; else echo was_up=0 > $S/saved; fi
    [ -f "$DIR/sunshine.conf" ] && cp -a "$DIR/sunshine.conf" "$DIR/sunshine.conf.caraxes-saved"
    cat $S/saved
    sudo pkill -x sunshine; sleep 1
    cat > "$DIR/sunshine.conf" <<EOF
min_log_level = 1
capture = kms
encoder = software
log_path = $S/sunshine.log
file_apps = $S/apps.json
EOF
    echo '{"env":{},"apps":[{"name":"Desktop"}]}' > $S/apps.json
    : > $S/sunshine.log
    # Content: glxgears on the GNOME session's Xwayland, NVIDIA GLX.
    pkill -x glxgears
    setsid nohup env XDG_RUNTIME_DIR=/run/user/"$U" DISPLAY=:0 XAUTHORITY="$(xa)" stdbuf -oL glxgears > $S/gears.txt 2>&1 < /dev/null &
    sudo sunshine --creds lea leastream "$DIR/sunshine.conf" > /dev/null 2>&1
    sudo setsid nohup sunshine "$DIR/sunshine.conf" > $S/stdout.log 2>&1 < /dev/null &
    for _ in $(seq 60); do (exec 3<>/dev/tcp/127.0.0.1/47989) 2>/dev/null && break; sleep 1; done
    pgrep -x sunshine > /dev/null || { echo 'sunshine did not start'; tail -20 $S/sunshine.log; exit 1; }
    grep -a -m1 'Found H.264 encoder' $S/sunshine.log
    grep -a -iE 'kms|monitor' $S/sunshine.log | head -4
    ;;
stop)
    alive=0; pgrep -x sunshine > /dev/null && alive=1
    echo "sunshine_alive_at_end=$alive"
    sudo pkill -x sunshine; sleep 2
    grep -a -iE 'fatal|error|couldn.t|failed' $S/sunshine.log | tail -8
    if [ -f "$DIR/sunshine.conf.caraxes-saved" ]; then mv -f "$DIR/sunshine.conf.caraxes-saved" "$DIR/sunshine.conf"; else rm -f "$DIR/sunshine.conf"; fi
    if grep -q was_up=1 $S/saved; then
        echo "restarting the Sunshine that ran before"
        cmd=$(sed -n 's/^was_up=1 cmdline=//p' $S/saved)
        setsid nohup $cmd > /tmp/caraxes-sunshine-restored.out 2>&1 < /dev/null &
    fi
    pkill -x glxgears; sleep 1
    echo "== glxgears (content)"; tail -4 $S/gears.txt
    ;;
esac
