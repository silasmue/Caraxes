#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# One window of the 615 display series on vm0-615 (docs/caraxes/display-615.md; run.sh of the
# display page series, display-page-2026-09-24/run.sh, unchanged but for this env.sh): run a guest command over ssh and keep
# the evidence of that window beside it, the way the e-runs were kept:
#   command.txt       the guest command, start time, exit code
#   guest.txt         its output
#   device-events.txt the device trace's event lines (E) of the window (.xz when large);
#                     with --full also device-trace.txt.xz, every line of the window
#   device-log.txt    what the device printed to gpu.log in the window
#   guest-dmesg.txt   the guest kernel log lines added in the window
#   host-xid.txt      the host kernel log's Xid lines since the start (with pid and name)
#   host-check.txt    host kernel log volume and nvidia-smi memory after the run
# Usage (from the worktree root): run.sh <label> [--full] <guest command...>
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
# shellcheck disable=SC1091
source "$here/env.sh"
W=${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM
label=$1; shift
full=0
[[ ${1:-} == --full ]] && { full=1; shift; }
out=$here/$label
mkdir -p "$out"
VMSH=("$LEA_TOOLS/caraxes-vm.sh")
t0=$(date '+%F %T')
b0=$(stat -c %s "$W/gpu-trace.log")
l0=$(stat -c %s "$W/gpu.log")
g0=$("${VMSH[@]}" ssh 'sudo dmesg | wc -l' 2>/dev/null || echo 0)
printf '# SPDX-License-Identifier: MIT\n# %s guest command:\n%s\n' "$t0" "$*" > "$out/command.txt"
"${VMSH[@]}" ssh "$*" > "$out/guest.txt" 2>&1
rc=$?
echo "rc=$rc" >> "$out/command.txt"
sleep 1
tail -c +$((b0 + 1)) "$W/gpu-trace.log" > "$out/.trace"
n=$(wc -l < "$out/.trace")
grep -a '^ *[0-9]* E ' "$out/.trace" > "$out/device-events.txt"
if (( full )); then
    xz -T0 -3 -c "$out/.trace" > "$out/device-trace.txt.xz"
    echo "full trace $n lines kept (device-trace.txt.xz)" >> "$out/command.txt"
else
    echo "full trace $n lines, events only kept" >> "$out/command.txt"
fi
rm -f "$out/.trace"
(( $(stat -c %s "$out/device-events.txt") > 20000000 )) && xz -T0 -3 -f "$out/device-events.txt"
tail -c +$((l0 + 1)) "$W/gpu.log" > "$out/device-log.txt"
"${VMSH[@]}" ssh 'sudo dmesg' 2>/dev/null | tail -n +$((g0 + 1)) > "$out/guest-dmesg.txt"
{
    echo '# SPDX-License-Identifier: MIT'
    echo "# host kernel log, Xid lines since $t0 (device pid $(cat "$W/gpu.pid" 2>/dev/null))"
    journalctl -k --since "$t0" --no-pager -g 'Xid' 2>/dev/null | grep -v '^-- No entries'
} > "$out/host-xid.txt"
{
    echo '# SPDX-License-Identifier: MIT'
    echo "since $t0: host kernel log lines $(journalctl -k --since "$t0" --no-pager 2>/dev/null | wc -l)"
    echo "nvidia-smi: $(nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader 2>/dev/null)"
} > "$out/host-check.txt"
echo "$label: rc=$rc, trace $n lines, host Xid lines $(grep -vc '^#' "$out/host-xid.txt")"
