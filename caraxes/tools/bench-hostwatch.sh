#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Host watch for runs that stress the host RM (docs/caraxes/mapping.md, the P20 lesson:
# half a million mappings made host nvidia-smi time out). Every INTERVAL seconds it times
# one read-only `nvidia-smi --query-gpu` (an NVML client of the host RM) and counts the
# host kernel log lines of that interval (this host logs every RM ioctl, so the count is
# a load signal, not an error count), plus Xid lines. A query slower than SLOW seconds is
# an ALERT; with KILL=<pidfile> the watched device process is sent SIGTERM at the first
# alert (its own teardown then runs), so a run that slows the host is stopped and recorded.
#
#   bench-hostwatch.sh start <dir> [INTERVAL=5] [SLOW=2] [KILL=<pidfile>]
#   bench-hostwatch.sh stop <dir>        stop it and print the summary line
set -uo pipefail
cmd=${1:-}; dir=${2:-}
[[ -n $dir ]] || { echo "usage: $0 start|stop <dir>" >&2; exit 2; }
mkdir -p "$dir"
out=$dir/hostwatch.txt
case $cmd in
start)
    INTERVAL=${INTERVAL:-5} SLOW=${SLOW:-2} KILL=${KILL:-}
    (
        echo "# SPDX-License-Identifier: MIT"
        echo "# host watch from $(date -Iseconds): interval ${INTERVAL}s, slow above ${SLOW}s, kill ${KILL:-none}"
        echo "# time smi_s used_mib klog_lines_interval xid_lines_interval"
        last=$(date '+%Y-%m-%d %H:%M:%S')
        while :; do
            t0=$(date +%s%N)
            used=$(timeout 30 nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits 2>&1 | head -1)
            t1=$(date +%s%N)
            s=$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.3f", (b-a)/1e9}')
            now=$(date '+%Y-%m-%d %H:%M:%S')
            kl=$(journalctl -k --no-pager -q --since "$last" --until "$now" 2>/dev/null | wc -l)
            xid=$(journalctl -k --no-pager -q --since "$last" --until "$now" -g 'Xid' 2>/dev/null | wc -l)
            last=$now
            echo "$(date +%T) $s ${used// /} $kl $xid"
            if awk -v s="$s" -v m="$SLOW" 'BEGIN{exit !(s>m)}'; then
                echo "$(date +%T) ALERT host nvidia-smi took ${s}s"
                if [[ -n $KILL && -s $KILL ]]; then
                    echo "$(date +%T) ALERT SIGTERM to the device $(cat "$KILL")"
                    kill -TERM "$(cat "$KILL")" 2>/dev/null
                    KILL=
                fi
            fi
            sleep "$INTERVAL"
        done
    ) >> "$out" 2>&1 &
    echo $! > "$dir/hostwatch.pid"
    echo "host watch pid $(cat "$dir/hostwatch.pid") -> $out"
    ;;
stop)
    [[ -s $dir/hostwatch.pid ]] && kill "$(cat "$dir/hostwatch.pid")" 2>/dev/null
    rm -f "$dir/hostwatch.pid"
    awk '!/^#/ && $2 ~ /^[0-9.]+$/ {n++; if ($2>max) max=$2; sum+=$2; kl+=$4; x+=$5}
         /ALERT/ {a++}
         END {printf "host watch: %d samples, nvidia-smi max %.3fs mean %.3fs, %d kernel log lines, %d Xid lines, %d alerts\n", n, max, (n?sum/n:0), kl, x, a}' "$out" | tee -a "$out"
    ;;
*) echo "usage: $0 start|stop <dir>" >&2; exit 2 ;;
esac
