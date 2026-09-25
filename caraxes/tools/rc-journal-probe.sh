#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Run caraxes/tools/rc-journal-probe.c in the running guest (tools/caraxes-vm.sh with the
# caller's VM environment, from the main checkout): build it there and read the device's
# RC journal back as root (RC_GET_ERROR_COUNT, RC_GET_ERROR_V2, RC_SET_CLEAN_ERROR_HISTORY).
# The records come from the device: start it with CARAXES_RC_SELFTEST=<seconds>.
# Usage: rc-journal-probe.sh [seconds]; exit status of the probe (0 round trip, 3 no record).
set -uo pipefail
SECONDS_WAIT=${1:-60}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
R=${CARAXES_MAIN:-/home/silas/git/Caraxes}
cd "$R" || exit 1
SRC=$(base64 -w0 "$HERE/rc-journal-probe.c")
# The device nodes, as nvidia-modprobe makes them (major 195: minor 255 control, 0 the GPU),
# on a guest where nothing has opened the GPU yet (the 610 image has no nvidia-smi on PATH).
tools/caraxes-vm.sh ssh 'for n in nvidiactl:255 nvidia0:0; do [ -e /dev/${n%:*} ] || sudo mknod -m 666 /dev/${n%:*} c 195 ${n#*:}; done; ls -l /dev/nvidiactl /dev/nvidia0' || exit 1
tools/caraxes-vm.sh ssh "echo $SRC | base64 -d > /tmp/rc-journal-probe.c && gcc -O2 -Wall -o /tmp/rc-journal-probe /tmp/rc-journal-probe.c && echo built" || exit 1
tools/caraxes-vm.sh ssh "sudo /tmp/rc-journal-probe $SECONDS_WAIT"
rc=$?
tools/caraxes-vm.sh ssh 'sudo dmesg | grep -E "NVRM: Xid|SIMULATED RC" | tail -10'
echo "rc-journal-probe rc=$rc"
exit $rc
