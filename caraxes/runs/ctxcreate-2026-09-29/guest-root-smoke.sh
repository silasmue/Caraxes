# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# guest script (lean-run.sh): CUDA as root and as the user, the copy channels of a root process
# mirrored since 83bfe733 (docs/research/ctxcreate-2026-09-29.md); Xid count after each stage.
set +e
export PYTHONDONTWRITEBYTECODE=1
x() { echo "--- xid: $(sudo dmesg | grep -c 'NVRM: Xid')"; }
cd ~/bench
echo "=== user bench-cu copy"; timeout 200 ./bench-cu copy 3 2>&1 | grep -E "^BW|^CHECK|ERROR"; x
echo "=== root bench-cu copy"; sudo timeout 200 ./bench-cu copy 3 2>&1 | grep -E "^BW|^CHECK|ERROR"; x
echo "=== root bench-cu init"; for i in 1 2 3; do sudo timeout 60 ./bench-cu init 2>&1; done; x
echo "=== root bench-cu churn"; sudo timeout 200 ./bench-cu churn 10 2>&1; x
echo "=== root transformer"; sudo timeout 200 ~/gpu/venv/bin/python ./transformer.py --seconds 30 2>&1 | grep -E "^# TRANSFORMER|Error|error" | cut -c1-200; x
echo "=== root convnet"; sudo timeout 200 ~/gpu/venv/bin/python ./convnet.py --seconds 30 2>&1 | grep -E "^# CONVNET|Error|error" | cut -c1-200; x
echo "=== user convnet"; timeout 200 ~/gpu/venv/bin/python ./convnet.py --seconds 30 2>&1 | grep -E "^# CONVNET|Error|error" | cut -c1-200; x
L="env LD_LIBRARY_PATH=$HOME/bench-llama/lib"
echo "=== root llama1b"; cd ~/bench-llama; sudo timeout 300 $L ./llama-bench -m model.gguf -ngl 99 -p 512 -n 128 -r 2 2>&1 | grep -E "^\| llama|error"; x
echo "=== guest xid lines"; sudo dmesg | grep 'NVRM: Xid' | tail -5 | cut -c1-200
