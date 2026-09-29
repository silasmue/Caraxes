#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Guest half of run.sh (the coverage follow-up of 2026-09-25): nvidia-smi timed per process,
# run from ~/cov beside covguest.sh of the coverage map, which serves every other workload.
# Every workload prints its "COV" line last.
set -uo pipefail
cd ~/cov || exit 2
LIB=/opt/nvrm/lib
SMI="sudo LD_LIBRARY_PATH=$LIB /opt/nvrm/bin/nvidia-smi"
W=${1:?workload}
ms() { echo $(( ($(date +%s%N) - $1) / 1000000 )); }
case $W in
    smi-q-timed)  # 10 x nvidia-smi -q, one process each: wall ms per process
        t=()
        for i in $(seq 10); do
            t0=$(date +%s%N); $SMI -q > smi-q-$i.txt || exit 1; t+=("$(ms "$t0")")
        done
        echo "smi-q ms: ${t[*]}"
        echo "COV smi-q-timed ok 10 x nvidia-smi -q, ms per process: ${t[*]}" ;;
    smi-query-timed)  # 20 x a static-field query, one process each
        t=()
        for i in $(seq 20); do
            t0=$(date +%s%N)
            $SMI --query-gpu=name,pci.bus_id,driver_version,vbios_version,inforom.oem,clocks.max.sm,power.default_limit,pstate,temperature.gpu --format=csv,noheader > smi-query-$i.txt || exit 1
            t+=("$(ms "$t0")")
        done
        tail -1 smi-query-20.txt
        echo "COV smi-query-timed ok 20 x nvidia-smi --query-gpu, ms per process: ${t[*]}" ;;
    smi-dmon)  # as the coverage map's covguest.sh smi-dmon, with the wall time of each tool
        t0=$(date +%s%N); $SMI dmon -c 20 -s pucvmet | tail -3; a=$(ms "$t0")
        t0=$(date +%s%N); $SMI pmon -c 5 | tail -2; b=$(ms "$t0")
        t0=$(date +%s%N)
        timeout 6 $SMI --query-gpu=utilization.gpu,clocks.sm,clocks.mem,power.draw,temperature.gpu,pstate,memory.used --format=csv -lms 200 > smi-loop.txt
        c=$(ms "$t0"); tail -2 smi-loop.txt; lines=$(grep -c . smi-loop.txt)
        echo "COV smi-dmon ok dmon 20 s ($a ms), pmon 5 s ($b ms), query loop 6 s at 200 ms ($c ms, $lines lines)" ;;
    *) bash ~/cov/covguest.sh "$W" ;;
esac
