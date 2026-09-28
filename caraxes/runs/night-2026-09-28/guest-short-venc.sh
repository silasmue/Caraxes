# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# guest script: the night's full Linux series (run by lean-run.sh or over desktop-b ssh)
set +e
export PYTHONDONTWRITEBYTECODE=1
mkdir -p ~/nvenc && base64 -d > ~/nvenc/nvenc-probe <<"B64"
# (base64 of target/nvenc/nvenc-probe, built from caraxes/tools/nvenc-probe.c; the full
# script with the blob is /mnt/vmstore/caraxes/night-2026-09-28/guest-scripts/guest-short-venc.sh)
B64
chmod +x ~/nvenc/nvenc-probe
x() { echo "--- xid: $(sudo dmesg | grep -c 'NVRM: Xid')"; }
echo "=== short"; cd ~/bench; for i in 1 2 3; do NVPROBE_PTX=$HOME/bench/kernels.ptx NVPROBE_ITERS=1000 NVPROBE_SCHED=auto NVPROBE_CYCLES=1000 ./nvprobe 4 2>&1 | grep MSPERLAUNCH; done; x
echo "=== venc"; for i in 1 2 3; do ~/nvenc/nvenc-probe -n 300 2>&1 | grep -E "^ENC|^DEC|RESULT"; done; x
