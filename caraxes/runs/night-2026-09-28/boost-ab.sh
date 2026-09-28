#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Night 2026-09-28: the host boost A/B in the lean VM (1 GiB, slot 5, FAST=2): short kernel and
# NVENC/NVDEC with CARAXES_HOST_BOOST=1 and =0, host clocks sampled every 100 ms (read-only).
N=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
B=${1:-/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-hostboost-0a068b0d}
GS=${2:-/mnt/vmstore/caraxes/night-2026-09-28/guest-scripts/guest-short-venc.sh}; SFX=${3:-}
V=vm2-615; LOG=/mnt/vmstore/caraxes/$V/gpu.log
cd "$N"
for base in boost-on boost-off; do
    arm=$base$SFX
    x=CARAXES_HOST_BOOST=1; [[ $base == boost-off ]] && x=CARAXES_HOST_BOOST=0
    nvidia-smi --query-gpu=timestamp,pstate,clocks.sm,clocks.video,clocks.mem,pcie.link.gen.current --format=csv -lms 100 > "clocks-$arm.csv" 2>&1 & sp=$!
    ./lean-run.sh "$arm" "$B" "$GS" CARAXES_RESERVE_SLOT=5 CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_LAUNCH_FAST=2 "$x" > "$arm.log" 2>&1
    kill "$sp"
    grep -h "host boost" "$LOG" > "$arm/host-boost.txt"
    echo "== $arm: $(tail -1 "$arm.log" | cut -c1-100)"
    grep -E "MSPERLAUNCH|^ENC|^DEC|xid" "$arm/guest.txt" | sed -E 's/ (bytes|fill_ms_total|kbit|psnr).*//'
    cut -c1-200 "$arm/host-boost.txt" | tail -3
    awk -F', ' 'NR>1 {c[$2" vid="$4" gen="$6]++} END {for (k in c) print "  " c[k], k}' "clocks-$arm.csv" | sort -rn | head -4
done
