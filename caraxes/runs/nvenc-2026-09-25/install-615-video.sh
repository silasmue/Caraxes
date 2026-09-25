#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Install NVIDIA's 615.71.09 encode and decode libraries (libnvidia-encode, libnvidia-decode,
# the CUDA repository packages that sottr-prepare.sh installs on vm4-615) into vm2-615's
# persistent overlay, through a NAT for vm2's tap as sottr-2026-09-24/up.sh sets it up
# (sudo -n; exactly the rules added here are removed at the end). vm2-615 goes up and down.
#   install-615-video.sh <out dir>
set -uo pipefail
OUT=${1:?out dir}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
cd "$ROOT" || exit 1
. caraxes/runs/night-2026-09-24/env.sh
night_guest 615
export CARAXES_SERVER=${CARAXES_SERVER_615:?the v615 device binary}
export CARAXES_MEM=2G
V=615.71.09-2ubuntu1
mkdir -p "$OUT"
tools/caraxes-vm.sh up > "$OUT/up.txt" 2>&1 && tools/caraxes-vm.sh wait >> "$OUT/up.txt" 2>&1 || { tail -5 "$OUT/up.txt"; tools/caraxes-vm.sh down; exit 1; }
IP=$(tools/caraxes-vm.sh ssh 'hostname -I' | awk '{print $1}')
net=${IP%.*}.0/24
uplink=$(ip route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<NF;i++) if($i=="dev") print $(i+1); exit}')
added=()
sudo -n sysctl -q net.ipv4.ip_forward=1
if ! sudo -n iptables -t nat -C POSTROUTING -s "$net" -o "$uplink" -j MASQUERADE 2>/dev/null; then
    sudo -n iptables -t nat -A POSTROUTING -s "$net" -o "$uplink" -j MASQUERADE && added+=("-t nat -D POSTROUTING -s $net -o $uplink -j MASQUERADE")
fi
for dir in -s -d; do
    if ! sudo -n iptables -C FORWARD "$dir" "$net" -j ACCEPT 2>/dev/null; then
        sudo -n iptables -I FORWARD 1 "$dir" "$net" -j ACCEPT && added+=("-D FORWARD $dir $net -j ACCEPT")
    fi
done
echo "nat: $net out of $uplink, ${#added[@]} rule(s) added" | tee "$OUT/nat.txt"
tools/caraxes-vm.sh ssh "sudo -n env DEBIAN_FRONTEND=noninteractive sh -c 'apt-get update -q && apt-get install -y -q nvidia-driver-pinning-615.71.09=$V && apt-get install -y -q --no-install-recommends libnvidia-encode=$V libnvidia-decode=$V' 2>&1; ls -la /usr/lib/x86_64-linux-gnu/libnvidia-encode.so.1 /usr/lib/x86_64-linux-gnu/libnvcuvid.so.1; dpkg -l 'libnvidia-*' | grep '^ii' | awk '{print \$2, \$3}'; sync" > "$OUT/install.txt" 2>&1
tail -12 "$OUT/install.txt"
for r in "${added[@]}"; do
    # shellcheck disable=SC2086
    sudo -n iptables $r && echo "nat: removed $r" >> "$OUT/nat.txt"
done
tools/caraxes-vm.sh down > "$OUT/down.txt" 2>&1
cat "$OUT/nat.txt"
