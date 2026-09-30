#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Live host-Xid watch for the running Windows device (run44.py start): reads the host kernel log
# read-only (`sudo -n dmesg`) once a second from the moment it starts. At the first NVRM Xid line
# naming the device's pid (gpu.pid) it stops the device by pid (TERM, then KILL after 2 s) so a
# fault storm cannot flood the host (C80: 54 Xid 31). Xid lines of other processes are only
# reported. Log: <current run>/xid-watch.log. Ends when the device exits.
set -u
S=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain
pid=$(cat "$S/gpu.pid")
run=$S/$(cat "$S/current")
log=$run/xid-watch.log
t0=$(cut -d' ' -f1 /proc/uptime)
echo "$(date +%T) watching device pid $pid (run $(basename "$run")) from host uptime $t0" | tee -a "$log"
while kill -0 "$pid" 2>/dev/null; do
    new=$(sudo -n dmesg | awk -v t="$t0" '
        match($0, /^\[ *[0-9]+\.[0-9]+\]/) {
            ts = substr($0, 2, RLENGTH - 2) + 0
            if (ts > t && /NVRM: Xid/) print
        }')
    if [[ -n $new ]]; then
        echo "$new" | tee -a "$log"
        t0=$(echo "$new" | tail -1 | sed -E 's/^\[ *([0-9]+\.[0-9]+)\].*/\1/')
        if grep -q "pid=$pid," <<<"$new"; then
            echo "$(date +%T) Xid of the device: stopping pid $pid" | tee -a "$log"
            kill -TERM "$pid" 2>/dev/null
            sleep 2
            kill -0 "$pid" 2>/dev/null && kill -KILL "$pid"
            exit 1
        fi
    fi
    sleep 1
done
echo "$(date +%T) device pid $pid exited; watch ends" | tee -a "$log"
