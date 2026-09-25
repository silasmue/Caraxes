#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The SotTR stage, up (docs/caraxes/sottr-stage.md): vm4 with env.sh, prepared on its first start,
# the display up, GNOME on Wayland with Steam in the session, Sunshine with KMS capture; prints
# what to do next (stream.sh opens Moonlight on this desktop). Evidence goes to a session folder
# beside this script (session-<time>/, or $SOTTR_OUT); down.sh closes it.
#   up.sh                        the game VM on the host GPU (vm0 and vm2 must be down)
#   SOTTR_EMULATION=1 up.sh      the same VM in pure emulation (no host RM, no GPU memory)
# SOTTR_NAT=1 (the default on the GPU run) lets the guest reach the internet for Steam's
# installer, updates and login: MASQUERADE for 192.168.103.0/24 out of the uplink plus two
# FORWARD accepts, as v1's `rig.sh net up` (Leandro-Test scripts/lib/rig.sh:355-392); down.sh
# removes exactly these rules. SOTTR_NAT=0 keeps the guest offline.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
# shellcheck source=caraxes/runs/sottr-2026-09-24/env.sh
source "$here/env.sh"
VMSH=("$LEA_TOOLS/caraxes-vm.sh")
ROOT=$(cd "$LEA_TOOLS/.." && pwd -P)
STORE=${CARAXES_STORE:-/mnt/vmstore/caraxes}
W=$STORE/$VM
EMU=${SOTTR_EMULATION:-0}
NAT=${SOTTR_NAT:-$((1 - EMU))}
OUT=${SOTTR_OUT:-$here/session-$(date +%Y%m%d-%H%M%S)}
BASE610=/mnt/vmstore/leandro/vm/guest-baked-20260801-610.57.04-20260820-105733.qcow2
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
alive() { [[ -f $1 ]] && kill -0 "$(cat "$1")" 2>/dev/null; }
gsh() { "${VMSH[@]}" ssh "$@"; }
mkdir -p "$W" "$OUT"
echo "$OUT" > "$W/sottr-session"

# 1. Preconditions. The 3 GiB reservation at slot 0 is the top 3 GiB of the host FB, where vm0
#    (slot 1 of 2 GiB) and vm2 (slot 0 of 2 GiB) sit.
#    2026-09-25: the check reads every running device's profile and slot from its command line
#    (`--fb-mib`, `--reserve-slot`: n profile sizes below the top) and refuses only a reservation
#    that overlaps the game VM's top 3072 MiB; a 1 GiB VM at slot 3 or below may stay up.
if [[ $EMU != 1 ]]; then
    for v in "$STORE"/*/; do
        [[ $v == "$W/" ]] && continue
        alive "$v/gpu.pid" || continue
        cmd=$(tr '\0' ' ' < "/proc/$(cat "$v/gpu.pid")/cmdline" 2>/dev/null) || continue
        grep -q -- '--reserve-vram' <<<"$cmd" || continue   # pure emulation reserves no host FB
        fb=$(sed -n 's/.*--fb-mib \([0-9]*\).*/\1/p' <<<"$cmd"); slot=$(sed -n 's/.*--reserve-slot \([0-9]*\).*/\1/p' <<<"$cmd")
        [[ -n $fb ]] || continue
        lo=$(( ${slot:-0} * fb ))
        (( lo < CARAXES_FB_MIB )) && die "$(basename "$v") runs with $fb MiB at slot ${slot:-0} (top $lo..$((lo + fb)) MiB), which overlaps vm4's top $CARAXES_FB_MIB MiB; wait until it is down"
    done
fi
alive "$W/ch.pid" && die "vm4 runs already (pid $(cat "$W/ch.pid")); down.sh first"
for t in "$LEA_TOOLS/caraxes-vm.sh" "$CARAXES_SERVER" "$SOTTR_OGKM" "$CARAXES_GAMES_DISK" "$BASE610"; do
    [[ -e $t ]] || die "$t missing"
done
grep -q CARAXES_GAMES_DISK "$LEA_TOOLS/caraxes-vm.sh" || die "$LEA_TOOLS/caraxes-vm.sh has no CARAXES_GAMES_DISK (merge the stage's launcher change)"
ip link show "$CARAXES_TAP" >/dev/null 2>&1 || die "tap $CARAXES_TAP missing"
for v in "$STORE"/*/; do
    [[ $v == "$W/" ]] && continue
    if alive "$v/ch.pid" && tr '\0' ' ' < "/proc/$(cat "$v/ch.pid")/cmdline" | grep -q "tap=$CARAXES_TAP,"; then
        die "$(basename "$v") uses the tap $CARAXES_TAP"
    fi
done

# 2. State: an overlay on the 610.57.04 base, a seed with vm4's identity, the GSP firmware.
if [[ ! -f $W/rootfs.qcow2 ]]; then
    qemu-img create -q -f qcow2 -F qcow2 -b "$BASE610" "$W/rootfs.qcow2" || die "overlay"
    echo "new overlay $W/rootfs.qcow2 on $BASE610"
fi
if [[ ! -f $W/seed.img ]]; then
    mkdir -p "$W/seed"
    key=$(cat "${CARAXES_SSH_KEY:-/mnt/vmstore/leandro/vm/id_leandro}.pub") || die "no ssh public key"
    # The host's own IPv4 resolvers for the guest (v1's _lea_guest_dns, rig.sh:323-338).
    dns=$(grep -E '^nameserver [0-9.]+$' /etc/resolv.conf | awk '$2 !~ /^127\./ {printf "%s%s", s, $2; s=", "}')
    printf 'instance-id: caraxes-%s-%s\nlocal-hostname: caraxes-%s\n' "$VM" "$(date +%s)" "$VM" > "$W/seed/meta-data"
    printf 'version: 2\nethernets: {}\n' > "$W/seed/network-config"
    cat > "$W/seed/user-data" <<EOF
#cloud-config
hostname: caraxes-$VM
users:
  - name: leandro
    sudo: "ALL=(ALL) NOPASSWD:ALL"
    shell: /bin/bash
    lock_passwd: false
    plain_text_passwd: leandro
    ssh_authorized_keys:
      - $key
ssh_pwauth: true
write_files:
  - path: /etc/netplan/60-leandro.yaml
    permissions: "0600"
    content: |
      network:
        version: 2
        ethernets:
          eth0:
            match: { name: eth0 }
            addresses: [$CARAXES_IP/24]
            nameservers: { addresses: [${dns:-1.1.1.1}] }
            routes:
              - to: default
                via: ${CARAXES_IP%.*}.1
  - path: /etc/hosts
    append: true
    content: "127.0.1.1 caraxes-$VM\n"
runcmd:
  - [ netplan, apply ]
  - [ systemctl, disable, --now, ssh.socket ]
  - [ systemctl, enable, --now, ssh.service ]
EOF
    mkfs.fat -C -n cidata "$W/seed.img" 1440 >/dev/null && mcopy -i "$W/seed.img" "$W/seed/user-data" "$W/seed/meta-data" "$W/seed/network-config" :: \
        || die "seed.img"
    echo "new seed $W/seed.img (caraxes-$VM, $CARAXES_IP)"
fi
FW=$W/gsp_tu10x-610.57.04.bin
if [[ ! -f $FW ]]; then
    alive "$STORE/vm3/ch.pid" && die "$FW missing and vm3 runs (its overlay is the source)"
    guestfish --ro --format=qcow2 -a "$STORE/vm3/rootfs.qcow2" -i download /lib/firmware/nvidia/610.57.04/gsp_tu10x.bin "$FW" || die "firmware from vm3"
fi
[[ $(sha256sum < "$FW" | cut -d' ' -f1) == "$SOTTR_GSP_SHA256" ]] || die "$FW: sha256 differs from $SOTTR_GSP_SHA256"

# 3. Record, host watch (the device is stopped with SIGTERM when host nvidia-smi takes longer
#    than 3 s: a game must not slow the host desktop), host sampler, NAT.
{
    echo "# SPDX-License-Identifier: MIT"
    echo "# SotTR stage session, up at $(date -Iseconds), emulation=$EMU nat=$NAT"
    echo "host driver: $(head -1 /proc/driver/nvidia/version 2>/dev/null)"
    sha256sum "$CARAXES_SERVER" "$(readlink -f "$LEA_CH" 2>/dev/null || echo "$LEA_CH")" 2>/dev/null
    grep -o 'cloud-hypervisor-[0-9]*' "$LEA_CH" | head -1
    echo "caraxes $(git -C "$ROOT" describe --always --dirty --abbrev=12 2>/dev/null)"
    env | grep -E '^(CARAXES_|LEA_CH|LEA_TOOLS|SOTTR_(EMULATION|GAMES|SERVER))' | grep -v -E 'PASS|PIN' | sort
} > "$OUT/setup.txt"
T0=$(date '+%F %T'); echo "$T0" > "$OUT/t0"
if [[ $EMU != 1 ]]; then
    KILL=$W/gpu.pid SLOW=3 "$ROOT/caraxes/tools/bench-hostwatch.sh" start "$OUT"
fi
SAMPLER_OUT=$OUT/host-1hz.csv setsid nohup bash "$here/host-sampler.sh" "$W" > /dev/null 2>&1 < /dev/null &
echo $! > "$OUT/sampler.pid"
if [[ $NAT == 1 ]]; then
    uplink=$(ip route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<NF;i++) if($i=="dev") print $(i+1); exit}')
    net=${CARAXES_IP%.*}.0/24
    [[ -n $uplink ]] || die "no uplink for NAT (SOTTR_NAT=0 keeps the guest offline)"
    sudo -n sysctl -q net.ipv4.ip_forward=1
    : > "$W/sottr-nat"
    if ! sudo -n iptables -t nat -C POSTROUTING -s "$net" -o "$uplink" -j MASQUERADE 2>/dev/null; then
        sudo -n iptables -t nat -A POSTROUTING -s "$net" -o "$uplink" -j MASQUERADE && echo "-t nat POSTROUTING -s $net -o $uplink -j MASQUERADE" >> "$W/sottr-nat"
    fi
    for dir in -s -d; do
        if ! sudo -n iptables -C FORWARD "$dir" "$net" -j ACCEPT 2>/dev/null; then
            sudo -n iptables -I FORWARD 1 "$dir" "$net" -j ACCEPT && echo "FORWARD $dir $net -j ACCEPT" >> "$W/sottr-nat"
        fi
    done
    echo "nat: $net out of $uplink ($(wc -l < "$W/sottr-nat") rule(s) added, down.sh removes them)"
fi

# 4. Up.
"${VMSH[@]}" up > "$OUT/up.txt" 2>&1 || { tail -8 "$OUT/up.txt"; die "up failed ($OUT/up.txt)"; }
cat "$OUT/up.txt"
"${VMSH[@]}" wait 240 >> "$OUT/up.txt" 2>&1 || { tail -3 "$OUT/up.txt"; die "no ssh"; }
grep -iE 'intr page|display page|doorbell tokens|doorbell poll|reserv|fb ' "$W/gpu.log" | cut -c1-400 > "$OUT/device-start.txt"

# 5. The guest's material and its one-time preparation (the driver build takes minutes once).
rsh="ssh -i ${CARAXES_SSH_KEY:-/mnt/vmstore/leandro/vm/id_leandro} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"
gsh 'test -f ogkm-610.57.04.tar.zst' || rsync -a -e "$rsh" "$SOTTR_OGKM" "leandro@$CARAXES_IP:ogkm-610.57.04.tar.zst"
rsync -a -e "$rsh" "$FW" "leandro@$CARAXES_IP:gsp_tu10x-610.57.04.bin"
rsync -a --delete -e "$rsh" "$ROOT/crates/vfio-user-nvgpu/guest/" "leandro@$CARAXES_IP:caraxes-guest/" --exclude kmsgrab
rsync -a -e "$rsh" "$here/guest/" "leandro@$CARAXES_IP:caraxes-sottr/"
gsh 'sudo bash caraxes-sottr/sottr-prepare.sh' > "$OUT/prepare.txt" 2>&1 || { tail -20 "$OUT/prepare.txt"; die "guest preparation failed ($OUT/prepare.txt)"; }
grep -E '^==|prepared|total|LEAGAMES|cap_sys' "$OUT/prepare.txt"

# 6. The driver (DMA table and nvidia.ko through the launcher's probe), the display modules,
#    EGL on the NVIDIA driver with host execution, on Mesa kms_swrast in emulation.
"${VMSH[@]}" probe sottr > "$OUT/probe.txt" 2>&1; tail -4 "$OUT/probe.txt" | cut -c1-200
egl=on; [[ $EMU == 1 ]] && egl=off
gsh "cd caraxes-guest && sudo bash display-up.sh && sudo bash nvidia-egl-setup.sh $egl && lsmod | grep -E '^nvidia'" > "$OUT/display-up.txt" 2>&1
cat "$OUT/display-up.txt"

# 7. GNOME (gdm3 autologin, Wayland; Steam starts from the session's autostart), Sunshine.
gsh 'bash caraxes-sottr/sottr-session.sh gnome 45' > "$OUT/gnome.txt" 2>&1
grep -E 'Type=wayland|gbm renderer|page_flip|CRTC|fb ' "$OUT/gnome.txt" | head -8
gsh "SUN_USER=$SOTTR_SUN_USER SUN_PASS=$SOTTR_SUN_PASS bash caraxes-sottr/sottr-session.sh sunshine-start" > "$OUT/sunshine-start.txt" 2>&1
cat "$OUT/sunshine-start.txt"
gsh 'bash caraxes-sottr/sottr-session.sh status' > "$OUT/status.txt" 2>&1
cat "$OUT/status.txt"

cat <<EOF

vm4 is up ($CARAXES_IP on $CARAXES_TAP), evidence in $OUT
Next, on this desktop:
  $here/stream.sh        pairs Moonlight with the guest's Sunshine if needed (PIN $SOTTR_SUN_PIN
                         posted to https://$CARAXES_IP:47990/api/pin with the web login) and
                         opens 'moonlight stream $CARAXES_IP Desktop' in a window
  Sunshine web UI:       https://$CARAXES_IP:47990  (login $SOTTR_SUN_USER / $SOTTR_SUN_PASS)
  Manual pairing:        moonlight pair $CARAXES_IP --pin $SOTTR_SUN_PIN, then enter the PIN in
                         the web UI's PIN page (or: curl -sk -u $SOTTR_SUN_USER:$SOTTR_SUN_PASS
                         -H 'Content-Type: application/json' -d '{"pin":"$SOTTR_SUN_PIN","name":"host"}'
                         https://$CARAXES_IP:47990/api/pin)
  When done:             $here/down.sh
EOF
