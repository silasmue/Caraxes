#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Guest half of run.sh: one workload of the coverage map per call, run from ~/cov, where run.sh
# copied covwork.c, covtorch.py and this script. `covguest.sh build` compiles covwork in the
# guest (gcc against the venv's cuda.h, -lcuda from the driver's libdir); `covguest.sh inventory`
# records what the image has. Every workload prints its "COV" line last.
set -uo pipefail
cd ~/cov || exit 2
LIB=/opt/nvrm/lib
PY=$HOME/gpu/venv/bin/python
export LD_LIBRARY_PATH=$LIB
SMI="sudo LD_LIBRARY_PATH=$LIB /opt/nvrm/bin/nvidia-smi"
W=${1:?workload}
case $W in
    build)
        I=$(dirname "$(find ~/gpu/venv -name cuda.h -path '*cu13*' | head -1)")
        gcc -O2 -Wall -o covwork covwork.c -I"$I" -L$LIB -lcuda -ldl && ls -la covwork ;;
    inventory)
        for t in nvcc ffmpeg cuda-gdb clinfo vulkaninfo vkcube glxinfo; do printf '%s %s\n' $t "$(command -v $t || echo missing)"; done
        echo "nvrtc: $(find ~/gpu/venv -name 'libnvrtc.so*' | head -3 | tr '\n' ' ')"
        echo "opencl: $(ls $LIB/libnvidia-opencl* 2>/dev/null | tr '\n' ' ') icd: $(ls /etc/OpenCL/vendors 2>/dev/null | tr '\n' ' ')"
        echo "vulkan icd: $(ls /usr/share/vulkan/icd.d /etc/vulkan/icd.d 2>/dev/null | tr '\n' ' '); libGLX_nvidia: $(ls $LIB/libGLX_nvidia.so* 2>/dev/null | head -1)"
        echo "nvenc: $(ls $LIB/libnvidia-encode* 2>/dev/null | head -1) nvcuvid: $(ls $LIB/libnvcuvid* 2>/dev/null | head -1)" ;;
    smi-q) for i in $(seq 10); do $SMI -q > /dev/null || exit 1; done; echo "COV smi-q ok 10 x nvidia-smi -q" ;;
    smi-dmon)
        $SMI dmon -c 20 -s pucvmet | tail -3
        $SMI pmon -c 5 | tail -2
        timeout 6 $SMI --query-gpu=utilization.gpu,clocks.sm,clocks.mem,power.draw,temperature.gpu,pstate,memory.used --format=csv -lms 200 | tail -2
        echo "COV smi-dmon ok dmon 20 s, pmon 5 s, query loop 6 s at 200 ms" ;;
    graphs) ./covwork graphs 200 ;;
    streams) ./covwork streams 300 ;;
    managed) ./covwork managed ;;
    texsurf) ./covwork texsurf ;;
    zerocopy) ./covwork zerocopy ;;
    query) ./covwork query ;;
    oom) ./covwork oom ;;
    illegal) ./covwork illegal ;;
    nvrtc)  # NVRTC loads libnvrtc-builtins from the library path: the venv's lib directory too.
        lib=$(find ~/gpu/venv -name 'libnvrtc.so.*' | grep -v builtins | head -1)
        LD_LIBRARY_PATH=$LIB:$(dirname "$lib") ./covwork nvrtc "$lib" ;;
    ctlsweep)  # The control sweep (caraxes/tools/ctlsweep.c), as root for the priv rows.
        sudo ./ctlsweep > ctlsweep.txt 2>&1; rc=$?
        grep -c . ctlsweep.txt; tail -1 ctlsweep.txt; [[ $rc == 0 ]] ;;
    build-sweep) gcc -O2 -Wall -o ctlsweep ctlsweep.c && ls -la ctlsweep ;;
    torch-matmul) $PY covtorch.py matmul 15 ;;
    torch-conv) $PY covtorch.py conv 15 ;;
    torch-attention) $PY covtorch.py attention 15 ;;
    multiproc)
        ./covwork streams 300 > mp1.txt 2>&1 & p1=$!
        ./covwork graphs 300 > mp2.txt 2>&1 & p2=$!
        wait $p1; r1=$?; wait $p2; r2=$?
        tail -1 mp1.txt; tail -1 mp2.txt
        [[ $r1 == 0 && $r2 == 0 ]] && echo "COV multiproc ok two processes (streams, graphs) at once" || echo "COV multiproc fail rc $r1 $r2" ;;
    kill9)
        ./covwork spin 8000 > spin.txt 2>&1 & p=$!
        for _ in $(seq 50); do grep -q SPINNING spin.txt && break; sleep 0.1; done
        sleep 1; kill -9 $p; wait $p; rc=$?
        cat spin.txt
        sleep 2
        ./covwork query | tail -1
        echo "COV kill9 ok spin kernel's process killed with SIGKILL mid-kernel (rc $rc); a new context works after it" ;;
    vulkan)
        icd=$(ls /usr/share/vulkan/icd.d/nvidia_icd*.json /etc/vulkan/icd.d/nvidia_icd*.json 2>/dev/null | head -1)
        if [[ -z $icd ]]; then
            glx=$(ls $LIB/libGLX_nvidia.so.0 2>/dev/null)
            [[ -n $glx ]] || { echo "COV vulkan skip no nvidia ICD and no libGLX_nvidia in $LIB"; exit 0; }
            printf '{"file_format_version":"1.0.1","ICD":{"library_path":"%s","api_version":"1.4.0"}}\n' "$glx" > nvidia_icd.json
            icd=$PWD/nvidia_icd.json
        fi
        VK_ICD_FILENAMES=$icd timeout 60 vulkaninfo --summary > vk.txt 2>&1; rc=$?
        grep -E 'deviceName|driverName|apiVersion|ERROR' vk.txt | head -6
        grep -q 'deviceName' vk.txt && echo "COV vulkan ok vulkaninfo --summary with $icd (rc $rc)" || echo "COV vulkan fail vulkaninfo rc $rc: $(grep -m1 -iE 'error|fail' vk.txt)" ;;
    idle) sleep "${IDLE_SECONDS:-300}"; echo "COV idle ok ${IDLE_SECONDS:-300} s with nothing running" ;;
    *) echo "COV $W fail unknown workload"; exit 2 ;;
esac
