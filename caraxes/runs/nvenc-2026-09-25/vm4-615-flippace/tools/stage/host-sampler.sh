#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# 1 Hz host sampler of the SotTR stage, the columns of v1's sottr-4q sampler
# (../Leandro/docs/measurements/sottr-4q/sample.sh) for one VM: the card (read-only
# nvidia-smi --query-gpu), the device process's own VRAM (--query-compute-apps), host RAM, and
# the CPU time of the device and Cloud Hypervisor processes. Without the host RM
# (SOTTR_EMULATION=1) the nvidia-smi columns stay empty and nvidia-smi is not run.
# Usage: SAMPLER_OUT=<csv> host-sampler.sh <state dir>   (up.sh starts it, down.sh stops it)
set -u
W=$1 O=${SAMPLER_OUT:?}
echo "ts,used_mib,free_mib,util_gpu,util_mem,sm_clk,mem_clk,temp_c,power_w,enc_util,dec_util,ram_avail_mib,device_vram_mib,device_cpu_s,ch_cpu_s" > "$O"
tck=$(getconf CLK_TCK)
cpu_s() { [[ -r /proc/$1/stat ]] && awk -v t="$tck" '{printf "%.1f", ($14 + $15) / t}' "/proc/$1/stat"; }
while :; do
    dp=$(cat "$W/gpu.pid" 2>/dev/null) cp=$(cat "$W/ch.pid" 2>/dev/null)
    g=",,,,,,,,," v=""
    if [[ ${SOTTR_EMULATION:-0} != 1 ]]; then
        g=$(timeout 10 nvidia-smi --query-gpu=memory.used,memory.free,utilization.gpu,utilization.memory,clocks.sm,clocks.mem,temperature.gpu,power.draw,utilization.encoder,utilization.decoder --format=csv,noheader,nounits 2>/dev/null | head -1 | tr -d ' ')
        v=$(timeout 10 nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader,nounits 2>/dev/null | tr -d ' ' | awk -F, -v p="${dp:-0}" '$1==p{print $2}')
    fi
    r=$(free -m | awk '/^Mem:/{print $7}')
    echo "$(date +%H:%M:%S),${g:-,,,,,,,,,},$r,${v:-},$(cpu_s "${dp:-0}"),$(cpu_s "${cp:-0}")" >> "$O"
    sleep 1
done
