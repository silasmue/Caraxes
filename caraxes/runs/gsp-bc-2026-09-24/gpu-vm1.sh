#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The nvml and cuda gates of the 610 guest on vm1 (1 GiB, slot 4) with main 7621cfb (B and C merged,
# resets withheld), because the B and C agent could not reserve slot 6. Derived from
# caraxes/runs/costume-2026-09-24/run.sh without the costume and the bench series.
# SPDX-License-Identifier: MIT
# The costume (docs/caraxes/costume-5060.md) on the GPU: vm1, 1 GiB profile at reservation
# slot 4 (caraxes/runs/doorbell-2026-09-24/env-vm1.sh), the 610 guest image, the device of
# the costume worktree with, Cloud Hypervisor 0001-0006, token
# pool with the hybrid reactor, no BAR1 area. up, wait, probe, sync-tests, the nvml and cuda
# gates, the identity as the guest sees it (lspci, nvidia-smi, torch; the 610 guest's
# nvidia-smi and libraries live in /opt/nvrm, tools/lib/driver.sh), then the short-kernel
# and rlprobe metrics (2 reps). Output goes to $OUT (default: final/).
set -uo pipefail
TREE=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd -P)
source "$TREE/caraxes/runs/doorbell-2026-09-24/env-vm1.sh"
# env-vm1.sh sets R to the main checkout; this run uses the costume tree throughout.
R=$TREE; C=$R/caraxes/runs/gsp-bc-2026-09-24/${OUT:-gpu-vm1}; mkdir -p "$C"; cd "$R" || exit 1
export CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-main-7621cfb LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
# The test material sits beside the main checkout, not beside the worktree.
export CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test LT=/home/silas/git/Leandro-Test
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_DRIVER CARAXES_GUEST_ABI
W=/mnt/vmstore/caraxes/$VM; T0=$(date '+%F %T')
VMSH=$R/tools/caraxes-vm.sh
L='sudo LD_LIBRARY_PATH=/opt/nvrm/lib'; SMI="$L /opt/nvrm/bin/nvidia-smi"
{ head -1 /proc/driver/nvidia/version; git -C "$R" log --oneline -1; sha256sum "$CARAXES_SERVER"; env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; } > "$C/setup.txt"
caraxes/tools/bench-hostwatch.sh start "$C"
"$VMSH" up > "$C/up.txt" 2>&1 || { tail -5 "$C/up.txt"; exit 1; }
"$VMSH" wait >> "$C/up.txt" 2>&1 || { tail -5 "$C/up.txt"; exit 1; }
"$VMSH" probe gspbc > "$C/probe.txt" 2>&1; echo "probe rc=$?"
grep -E 'guest name|TU106|host ABI|unanswered|ERROR' "$W/gpu.log" | cut -c1-400 > "$C/device-log-first.txt"
"$VMSH" sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
tools/caraxes-gate.sh nvml > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"; tail -3 "$C/gate-nvml.txt" | cut -c1-200
tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -3 "$C/gate-cuda.txt" | cut -c1-200
# The identity as the guest sees it (the gates left nvidia-uvm and the nodes in place).
"$VMSH" ssh 'lspci -nn -d 10de:; echo; sudo lspci -vvv -d 10de:; echo; cat /proc/driver/nvidia/gpus/*/information' > "$C/guest-lspci.txt" 2>&1
"$VMSH" ssh "$SMI --query-gpu=name,compute_cap,pci.device_id,pci.sub_device_id,vbios_version,memory.total --format=csv; $SMI; $SMI -q" > "$C/guest-nvidia-smi.txt" 2>&1
"$VMSH" ssh "LD_LIBRARY_PATH=/opt/nvrm/lib /home/leandro/gpu/venv/bin/python -c 'import torch; print(torch.__version__, torch.version.cuda); print(torch.cuda.get_device_name(0)); print(torch.cuda.get_device_capability(0)); print(torch.cuda.get_device_properties(0))'" > "$C/guest-torch.txt" 2>&1
"$VMSH" ssh 'sudo dmesg | grep -iE "nvrm|xid" | tail -60' > "$C/guest-dmesg.txt" 2>&1
caraxes/tools/bench-hostwatch.sh stop "$C"
grep -E 'unanswered|ERROR|refused' "$W/gpu.log" | sort | uniq -c | sort -rn | head -12 | cut -c1-250 > "$C/device-log-issues.txt"
DEVPID=$(cat "$W/gpu.pid" 2>/dev/null)
echo "device pid $DEVPID" > "$C/device-pid.txt"
"$VMSH" down > "$C/down.txt" 2>&1
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
# The cuda gate's run directory, into this record.
# The gate directory of THIS run only (the earlier loop moved every gate-cuda-2026-09-24* directory,
# other agents' committed records included; those are restored at the top level).
latest=$(ls -td "$R"/caraxes/runs/gate-cuda-2026-09-24* 2>/dev/null | head -1); [[ -n $latest && $latest -nt "$C/setup.txt" ]] && mv "$latest" "$C/gate-cuda-run"
echo "gsp-bc gpu-vm1 done $(date -Iseconds); Xid lines $(grep -c Xid "$C/host-xid.txt"), device pid $DEVPID"
