# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# guest script: the night's full Linux series (run by lean-run.sh or over desktop-b ssh)
set +e
export PYTHONDONTWRITEBYTECODE=1
mkdir -p ~/nvenc && base64 -d > ~/nvenc/nvenc-probe <<"B64"
# (base64 of target/nvenc/nvenc-probe, built from caraxes/tools/nvenc-probe.c; the full
# script with the blob is /mnt/vmstore/caraxes/night-2026-09-28/guest-scripts/guest-series.sh)
B64
chmod +x ~/nvenc/nvenc-probe
x() { echo "--- xid: $(sudo dmesg | grep -c 'NVRM: Xid')"; }
echo "=== short"; cd ~/bench; for i in 1 2 3; do NVPROBE_PTX=$HOME/bench/kernels.ptx NVPROBE_ITERS=1000 NVPROBE_SCHED=auto NVPROBE_CYCLES=1000 ./nvprobe 4 2>&1 | grep MSPERLAUNCH; done; x
L="env LD_LIBRARY_PATH=$HOME/bench-llama/lib"
echo "=== llama1b"; cd ~/bench-llama; $L ./llama-bench -m model.gguf -ngl 99 -p 512 -n 128 -r 3 2>&1 | grep -E "^\| llama|error"; x
if [ -s qwen7b.gguf ]; then echo "=== llama7b-ngl20"; $L ./llama-bench -m qwen7b.gguf -ngl 20 -p 512 -n 128 -r 2 2>&1 | grep -E "^\| qwen|error"; x; fi
cd ~/bench
echo "=== transformer"; timeout 300 ~/gpu/venv/bin/python ./transformer.py --seconds 60 2>&1 | grep -E "^# TRANSFORMER|Error|error" | cut -c1-200; x
for i in 1 2 3; do echo "=== convnet $i"; timeout 300 ~/gpu/venv/bin/python ./convnet.py --seconds 60 2>&1 | grep -E "^# CONVNET|Error|error" | cut -c1-200; x; done
echo "=== venc"; for i in 1 2; do ~/nvenc/nvenc-probe -n 300 2>&1 | grep -E "^ENC|^DEC|RESULT"; done; x
echo "=== guest xid lines"; sudo dmesg | grep 'NVRM: Xid' | tail -5 | cut -c1-200
