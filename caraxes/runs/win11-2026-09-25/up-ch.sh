#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Windows 11 under Cloud Hypervisor (README.md): the EDK2 CLOUDHV.fd, swtpm over --tpm (the
# state install-qemu.sh created), the installed virtio-blk disk, virtio-net on the tap caraxes4
# (192.168.104.1/24, NAT to the uplink like the SotTR stage), serial into a log. Headless: reach
# the guest by RDP or Sunshine over the tap. CARAXES_WIN_GPU=1 adds the vfio-user device from
# $W/gpu.sock, which is started separately (tools/caraxes-vm.sh's device environment).
# Usage: up-ch.sh      (WIN_MEM default 8G, WIN_CPUS default 8, WIN_TAP default caraxes4)
set -euo pipefail
W=/mnt/vmstore/caraxes/win11
CH=${CARAXES_CH:-/mnt/vmstore/caraxes/vm1/cloud-hypervisor-0006}
FW=$W/CLOUDHV.fd
TAP=${WIN_TAP:-caraxes4} NET=192.168.104 MAC=02:00:00:00:00:44
MEM=${WIN_MEM:-8G} CPUS=${WIN_CPUS:-8}
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
[[ -x $CH ]] || die "no Cloud Hypervisor at $CH"
[[ -s $FW ]] || die "no $FW (the edk2 build is still running, or failed: target/win11/edk2.log)"
[[ -s $W/win11.qcow2 ]] || die "no $W/win11.qcow2: run install-qemu.sh first"
[[ -s $W/ch.pid ]] && kill -0 "$(cat "$W/ch.pid")" 2>/dev/null && die "already running (pid $(cat "$W/ch.pid"))"
command -v swtpm >/dev/null || die "swtpm missing"
# The tap and the NAT (sudo -n, as the SotTR stage's up.sh does it).
if ! ip link show "$TAP" >/dev/null 2>&1; then
    sudo -n ip tuntap add dev "$TAP" mode tap user "$USER"
    sudo -n ip addr add "$NET.1/24" dev "$TAP"
    sudo -n ip link set "$TAP" up
    echo "tap $TAP created ($NET.1/24)"
fi
up=$(ip -4 route show default | awk '{print $5; exit}')
sudo -n sysctl -q -w net.ipv4.ip_forward=1
: > "$W/nat.rules"
for r in "-t nat -A POSTROUTING -s $NET.0/24 -o $up -j MASQUERADE" "-A FORWARD -s $NET.0/24 -j ACCEPT" "-A FORWARD -d $NET.0/24 -j ACCEPT"; do
    # shellcheck disable=SC2086
    sudo -n iptables $r && echo "$r" >> "$W/nat.rules"
done
# DHCP and DNS on the tap (dnsmasq, one instance per tap), so the guest needs no static address.
if ! pgrep -f "dnsmasq.*interface=$TAP" >/dev/null; then
    sudo -n dnsmasq --interface="$TAP" --bind-interfaces --except-interface=lo \
        --dhcp-range="$NET.10,$NET.50,12h" --dhcp-option=option:router,"$NET.1" \
        --dhcp-option=option:dns-server,"$NET.1" --server=1.1.1.1 \
        --dhcp-leasefile="$W/dnsmasq.leases" --pid-file="$W/dnsmasq.pid" --log-facility="$W/dnsmasq.log"
    echo "dhcp on $TAP: $NET.10-50, leases in $W/dnsmasq.leases"
fi
if ! pgrep -f "swtpm socket.*$W/swtpm" >/dev/null; then
    swtpm socket --tpm2 --tpmstate "dir=$W/swtpm" --ctrl "type=unixio,path=$W/swtpm/sock" \
        --daemon --log "file=$W/swtpm/swtpm.log"
    sleep 0.5
fi
gpu=()
if [[ ${CARAXES_WIN_GPU:-0} == 1 ]]; then
    [[ -S $W/gpu.sock ]] || die "CARAXES_WIN_GPU=1 but no device socket $W/gpu.sock"
    gpu=(--user-device "socket=$W/gpu.sock")
fi
rm -f "$W/ch.sock"
setsid nohup "$CH" --api-socket "$W/ch.sock" --cpus "boot=$CPUS" --memory "size=$MEM,shared=on" \
    --kernel "$FW" --disk "path=$W/win11.qcow2" --net "tap=$TAP,mac=$MAC" --rng src=/dev/urandom \
    --tpm "socket=$W/swtpm/sock" --serial "file=$W/serial.log" --console off "${gpu[@]}" \
    > "$W/ch.log" 2>&1 < /dev/null &
echo $! > "$W/ch.pid"
sleep 2
kill -0 "$(cat "$W/ch.pid")" 2>/dev/null || { tail -20 "$W/ch.log"; die "Cloud Hypervisor exited at once"; }
echo "windows 11 up: pid $(cat "$W/ch.pid"), $CPUS vCPUs, $MEM, tap $TAP ($NET.1), serial $W/serial.log, log $W/ch.log"
echo "guest address: see $W/dnsmasq.leases (DHCP on the tap); RDP: xfreerdp /v:<address> /u:<user>"
