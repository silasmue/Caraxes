# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
cd ~/bench-llama && GGML_CUDA_ENABLE_UNIFIED_MEMORY=1 LD_LIBRARY_PATH=$HOME/bench-llama/lib timeout 120 ./llama-bench -m model.gguf -ngl 99 -p 64 -n 16 -r 1 2>&1 | grep -vE "^#[0-9]|^warning|^\[New|^\[Thread|^\[Inferior|^[0-9]+\s" | tail -8
echo "rc=${PIPESTATUS[0]}"
echo "--- xid: $(sudo dmesg | grep -c 'NVRM: Xid')"
