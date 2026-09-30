#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Live watch during a Windows run beside Linux B, every stage line-buffered (P18's watch missed
# the first Xid: `cut` buffered). One line per event: host Xid (any pid), new Linux B device
# ERROR lines, the Windows device's exit. Read-only; for the Monitor tool.
W=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain
LB=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b/gpu.log
wpid=$(cat "$W/gpu.pid" 2>/dev/null)
(journalctl -k -f -n 0 --no-pager 2>&1 | grep --line-buffered -E 'NVRM: Xid' \
    | awk '{print "HOST: " substr($0, 1, 260); fflush()}') &
(tail -n 0 -F "$LB" 2>/dev/null | grep --line-buffered -E 'ERROR' \
    | awk '{print "LINUX-B: " substr($0, 1, 240); fflush()}') &
if [[ -n $wpid ]]; then
    (while kill -0 "$wpid" 2>/dev/null; do sleep 5; done; echo "WINDOWS DEVICE $wpid EXITED") &
else
    echo "WATCH: no $W/gpu.pid (Windows device not running)"
fi
wait
