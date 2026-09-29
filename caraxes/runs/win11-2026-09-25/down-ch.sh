#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Stop the Windows 11 guest of up-ch.sh: the ACPI power button through the API socket first
# (Windows shuts down cleanly), a kill after 90 s, then the NAT rules go; the tap and the TPM
# state stay. Usage: down-ch.sh
set -uo pipefail
W=/mnt/vmstore/caraxes/win11
if [[ -s $W/ch.pid ]] && kill -0 "$(cat "$W/ch.pid")" 2>/dev/null; then
    pid=$(cat "$W/ch.pid")
    curl -s --unix-socket "$W/ch.sock" -X PUT http://localhost/api/v1/vm.power-button >/dev/null 2>&1 && echo "power button sent"
    for _ in $(seq 90); do kill -0 "$pid" 2>/dev/null || break; sleep 1; done
    if kill -0 "$pid" 2>/dev/null; then kill "$pid"; echo "cloud-hypervisor $pid killed after 90 s"; else echo "windows shut down"; fi
fi
rm -f "$W/ch.pid"
if [[ -s $W/nat.rules ]]; then
    while IFS= read -r r; do
        # shellcheck disable=SC2086
        sudo -n iptables ${r/-A /-D } && echo "nat: removed $r"
    done < "$W/nat.rules"
    rm -f "$W/nat.rules"
fi
[[ -s $W/dnsmasq.pid ]] && sudo -n kill "$(cat "$W/dnsmasq.pid")" 2>/dev/null && echo "dnsmasq stopped"; sudo -n rm -f "$W/dnsmasq.pid"
pkill -f "swtpm socket.*$W/swtpm" 2>/dev/null && echo "swtpm stopped"
exit 0
