#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# The leak watch from outside the device (docs/caraxes/host-resources.md): once a second,
# read-only, the host's VRAM and BAR1 (one `nvidia-smi -q -d MEMORY`), the resident set of
# every vfio-user-nvgpu process, the host-wide pinned pages (foll_pin) and MemAvailable,
# into a CSV. The device prints the same figures from inside (`soak` lines, every 60 s).
#
#   soak-watch.sh <csv> [SECONDS]    sample until SECONDS passed, or until Ctrl-C
#   soak-watch.sh --summary <csv>    means of the first and the last minute, the deltas, and
#                                    the host BAR1 peak of the run (a 1 Hz sample can still
#                                    miss a sub-second spike)
#
# A leak is a delta that grows with the session's length at the same guest load. VM
# starts and stops move vram_used in steps of the reservation size; read the rows around
# them, not the delta alone.
set -uo pipefail

summary() {
    local csv=$1
    [[ -s $csv ]] || { echo "no samples in $csv" >&2; exit 1; }
    awk -F, '
        NR == 1 { for (i = 1; i <= NF; i++) name[i] = $i; n = NF; next }
        $1 !~ /^[0-9]+$/ { next }
        { t[++rows] = $1; tm[rows] = $2; for (i = 3; i <= n; i++) v[rows, i] = $i }
        END {
            if (rows == 0) { print "no samples"; exit 1 }
            t0 = t[1]; t1 = t[rows]
            printf "soak-watch: %d samples over %d s (epoch %d to %d)\n", rows, t1 - t0, t0, t1
            printf "%-18s %14s %14s %14s\n", "column", "first minute", "last minute", "delta"
            for (i = 3; i <= n; i++) {
                a = 0; na = 0; b = 0; nb = 0
                for (r = 1; r <= rows; r++) {
                    if (v[r, i] == "") continue
                    if (t[r] < t0 + 60) { a += v[r, i]; na++ }
                    if (t[r] > t1 - 60) { b += v[r, i]; nb++ }
                }
                if (na == 0 || nb == 0) { printf "%-18s %14s %14s %14s\n", name[i], "-", "-", "-"; continue }
                printf "%-18s %14.1f %14.1f %+14.1f\n", name[i], a / na, b / nb, b / nb - a / na
            }
            # The BAR1 peak: the host aperture runs out in transients the minute means hide.
            for (i = 3; i <= n; i++) { if (name[i] == "bar1_used_mib") bu = i; if (name[i] == "bar1_free_mib") bf = i }
            if (bu) {
                peak = -1; low = -1
                for (r = 1; r <= rows; r++) {
                    if (v[r, bu] != "" && v[r, bu] + 0 > peak) { peak = v[r, bu] + 0; pt = tm[r] }
                    if (v[r, bf] != "" && (low < 0 || v[r, bf] + 0 < low)) { low = v[r, bf] + 0; lt = tm[r] }
                }
                if (peak >= 0) printf "BAR1 peak: %d MiB used at %s, %d MiB free at the lowest (%s)\n", peak, pt, low, lt
            }
        }' "$csv"
}

if [[ ${1:-} == --summary ]]; then
    [[ -n ${2:-} ]] || { echo "usage: $0 --summary <csv>" >&2; exit 2; }
    summary "$2"
    exit
fi
csv=${1:-}
[[ -n $csv ]] || { echo "usage: $0 <csv> [SECONDS] | --summary <csv>" >&2; exit 2; }
limit=${2:-0}

foll_pin() {
    awk '/^nr_foll_pin_acquired /{a=$2} /^nr_foll_pin_released /{r=$2} END{print a-r}' /proc/vmstat
}

device_rss() {
    local pid kib sum=0 n=0 max=0
    for pid in $(pgrep -x vfio-user-nvgpu 2>/dev/null); do
        kib=$(awk '/^VmRSS:/{print $2}' "/proc/$pid/status" 2>/dev/null)
        [[ -n $kib ]] || continue
        sum=$((sum + kib)); n=$((n + 1)); ((kib > max)) && max=$kib
    done
    echo "$n,$sum,$max"
}

if [[ ! -s $csv ]]; then
    echo "epoch,time,vram_used_mib,vram_free_mib,bar1_used_mib,bar1_free_mib,devices,device_rss_kib,device_rss_max_kib,foll_pin,mem_available_kib,smi_ms" > "$csv"
fi
echo "soak-watch: sampling to $csv every second$( ((limit > 0)) && echo " for ${limit}s") (Ctrl-C to stop; --summary $csv afterwards)" >&2
trap 'echo; summary "$csv"; exit 0' INT TERM
start=$(date +%s)
while :; do
    t0=$(date +%s%N)
    mem=$(timeout 30 nvidia-smi -i 0 -q -d MEMORY 2>/dev/null | awk '
        /FB Memory Usage/ { s = "fb" } /BAR1 Memory Usage/ { s = "bar1" } /Conf Compute/ { s = "" }
        s != "" && $1 == "Used" { u[s] = $3 } s != "" && $1 == "Free" { f[s] = $3 }
        END { printf "%s,%s,%s,%s", u["fb"], f["fb"], u["bar1"], f["bar1"] }')
    t1=$(date +%s%N)
    now=$(date +%s)
    avail=$(awk '/^MemAvailable:/{print $2}' /proc/meminfo)
    echo "$now,$(date +%T),${mem:-,,,},$(device_rss),$(foll_pin),$avail,$(((t1 - t0) / 1000000))" >> "$csv"
    if ((limit > 0 && now - start >= limit)); then
        summary "$csv"
        exit 0
    fi
    sleep "$(awk -v a="$t0" -v b="$(date +%s%N)" 'BEGIN{d = 1 - (b - a) / 1e9; print (d > 0.05 ? d : 0.05)}')"
done
