#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# One run of the desktop series on vm0 (docs/caraxes/intr-page.md §11; run.sh of display-page-2026-09-24): run a guest command over ssh and keep
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
# Host load over the window (the other VMs share the CPUs and the GPU): the host's busy CPU
# fraction from /proc/stat and the CPU of every other device and VMM process.
others() {
    local p
    for p in $(pgrep -f 'vfio-user-nvgpu --socket|vhost-user-nvrm|cloud-hypervisor'); do
        grep -qa "$W/" "/proc/$p/cmdline" 2>/dev/null && continue
        awk -v p="$p" '{print p, $14 + $15}' "/proc/$p/stat" 2>/dev/null
    done
}
read -r _ u0 n0 s0 i0 w0 q0 sq0 _ < /proc/stat
o0=$(others)
printf '# SPDX-License-Identifier: MIT\n# %s guest command:\n%s\n' "$t0" "$*" > "$out/command.txt"
"${VMSH[@]}" ssh "$*" > "$out/guest.txt" 2>&1
rc=$?
echo "rc=$rc" >> "$out/command.txt"
read -r _ u1 n1 s1 i1 w1 q1 sq1 _ < /proc/stat
o1=$(others)
{
    echo '# SPDX-License-Identifier: MIT'
    busy=$(( (u1 + n1 + s1 + q1 + sq1) - (u0 + n0 + s0 + q0 + sq0) ))
    all=$(( busy + (i1 + w1) - (i0 + w0) ))
    echo "host CPU busy over the window: $(( all > 0 ? 100 * busy / all : 0 )) % of $(nproc) CPUs"
    tick=$(getconf CLK_TCK)
    join <(sort <<< "$o0") <(sort <<< "$o1") | while read -r p a b; do
        echo "other VM process $p: $(( (b - a) * 100 / tick )) centiseconds of CPU in the window: $(tr '\0' ' ' < "/proc/$p/cmdline" 2>/dev/null | cut -c1-90)"
    done
} > "$out/host-load.txt"
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
