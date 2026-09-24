#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Run caraxes/tools/goe-probe.c in the running 615 guest (tools/caraxes-vm.sh with the
# caller's VM environment, from the main checkout): build it there, bind a GOE
# subscription as root and print the operational events that reach the guest kernel's
# log within SECONDS (default 45). nvidia-smi runs meanwhile, so the guest keeps talking
# to the device (the device emits its events at a GSP exchange). For the device's
# self-test events start the device with CARAXES_GOE_SELFTEST=<period in s>.
# Usage: goe-probe.sh [seconds]; exit status of the probe (0 records seen, 3 none).
set -uo pipefail
SECONDS_WAIT=${1:-45}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
R=${CARAXES_MAIN:-/home/silas/git/Caraxes}
cd "$R" || exit 1
SRC=$(base64 -w0 "$HERE/goe-probe.c")
tools/caraxes-vm.sh ssh "echo $SRC | base64 -d > /tmp/goe-probe.c && gcc -O2 -Wall -o /tmp/goe-probe /tmp/goe-probe.c && echo built" || exit 1
tools/caraxes-vm.sh ssh "sudo /tmp/goe-probe $SECONDS_WAIT" &
probe=$!
for _ in $(seq 1 $((SECONDS_WAIT / 5))); do
    sleep 5
    tools/caraxes-vm.sh ssh 'nvidia-smi --query-gpu=name,temperature.gpu --format=csv,noheader' >/dev/null 2>&1
done
wait "$probe"
rc=$?
tools/caraxes-vm.sh ssh 'sudo dmesg | grep -iE "goeimport|GpuOpEventImport|OpEventLog" | tail -20'
echo "goe-probe rc=$rc"
exit $rc
