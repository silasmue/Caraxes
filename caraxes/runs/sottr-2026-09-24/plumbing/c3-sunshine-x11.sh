# SPDX-License-Identifier: MIT
# Guest (desktop user), plumbing check 3. Pure emulation has no KMS device: nvidia-drm refuses
# with "Failed to allocate NvKmsKapiDevice" (display-up.txt), so there is no GNOME and no
# capture=kms. The stage's Sunshine (sottr-session.sh sunshine-start) therefore refuses first.
# Then, for the pairing and the Moonlight launch of stream.sh only, a stand-in Sunshine with
# capture=x11 on an Xvfb with openbox, the same web login and the same state directory
# (~/.config/sunshine), so the pairing made here holds for the GPU run.
bash caraxes-sottr/sottr-session.sh sunshine-start; echo "stage sunshine-start rc=$? (expected 1: no gnome-shell)"
DIR=$HOME/.config/sunshine
mkdir -p "$DIR"
pgrep -x Xvfb >/dev/null || (setsid nohup Xvfb :1 -screen 0 1920x1080x24 > /tmp/xvfb.log 2>&1 < /dev/null &)
sleep 2
pgrep -x openbox >/dev/null || (DISPLAY=:1 setsid nohup openbox > /tmp/openbox.log 2>&1 < /dev/null &)
(DISPLAY=:1 setsid nohup xterm > /dev/null 2>&1 < /dev/null &) 2>/dev/null
pkill -x sunshine; sleep 1
cat > "$DIR/sunshine-x11.conf" <<EOF
min_log_level = 1
capture = x11
encoder = software
log_path = $DIR/sunshine.log
file_apps = $DIR/apps.json
EOF
echo '{"env":{},"apps":[{"name":"Desktop"}]}' > "$DIR/apps.json"
: > "$DIR/sunshine.log"
sunshine --creds lea leastream "$DIR/sunshine-x11.conf" > /dev/null 2>&1
setsid nohup env -u WAYLAND_DISPLAY DISPLAY=:1 XDG_RUNTIME_DIR=/run/user/$(id -u) sunshine "$DIR/sunshine-x11.conf" > "$DIR/stdout.log" 2>&1 < /dev/null &
for _ in $(seq 60); do (exec 3<>/dev/tcp/127.0.0.1/47989) 2>/dev/null && break; sleep 1; done
pgrep -x sunshine > /dev/null && echo "stand-in sunshine pid $(pgrep -x sunshine)" || { echo "sunshine did not start"; tail -20 "$DIR/sunshine.log"; }
grep -a -m1 'Found H.264 encoder' "$DIR/sunshine.log"
grep -a -iE 'x11|screen|error|fatal' "$DIR/sunshine.log" | head -6
