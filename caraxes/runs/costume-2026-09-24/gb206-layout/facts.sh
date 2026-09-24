#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The costume's identity as the guest sees it, in a second short session after run.sh (the
# first session's fact block called nvidia-smi and torch without the 610 guest's library
# path, /opt/nvrm; tools/lib/driver.sh): same vm1 settings, up, wait, probe, the facts, down.
set -uo pipefail
TREE=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd -P)
source "$TREE/caraxes/runs/doorbell-2026-09-24/env-vm1.sh"
R=$TREE; C=$R/caraxes/runs/costume-2026-09-24/facts; mkdir -p "$C"; cd "$R" || exit 1
export CARAXES_SERVER=$R/target/costume/release/vfio-user-nvgpu LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid CARAXES_COSTUME=rtx5060ti
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_DRIVER CARAXES_GUEST_ABI
W=/mnt/vmstore/caraxes/$VM; T0=$(date '+%F %T'); VMSH=$R/tools/caraxes-vm.sh
L='sudo LD_LIBRARY_PATH=/opt/nvrm/lib'; SMI="$L /opt/nvrm/bin/nvidia-smi"
"$VMSH" up > "$C/up.txt" 2>&1 && "$VMSH" wait >> "$C/up.txt" 2>&1 || { tail -5 "$C/up.txt"; exit 1; }
"$VMSH" probe costume-facts > "$C/probe.txt" 2>&1
{ caraxes/tools/bench-run.sh v2-nodes; "$VMSH" ssh 'ls -la /dev/nvidia*'; } > "$C/nodes.txt" 2>&1
"$VMSH" ssh 'lspci -nn -d 10de:; echo; sudo lspci -vvv -d 10de:; echo; cat /proc/driver/nvidia/gpus/*/information' > "$C/guest-lspci.txt" 2>&1
"$VMSH" ssh "$SMI --query-gpu=name,compute_cap,pci.device_id,pci.sub_device_id,vbios_version,memory.total --format=csv; $SMI; $SMI -q" > "$C/guest-nvidia-smi.txt" 2>&1
"$VMSH" ssh "LD_LIBRARY_PATH=/opt/nvrm/lib /home/leandro/gpu/venv/bin/python -c 'import torch; print(torch.__version__, torch.version.cuda); print(torch.cuda.get_device_name(0)); print(torch.cuda.get_device_capability(0)); print(torch.cuda.get_device_properties(0))'" > "$C/guest-torch.txt" 2>&1
"$VMSH" ssh 'sudo dmesg | grep -iE "nvrm|xid" | tail -60' > "$C/guest-dmesg.txt" 2>&1
DEVPID=$(cat "$W/gpu.pid" 2>/dev/null); echo "device pid $DEVPID" > "$C/device-pid.txt"
"$VMSH" down > "$C/down.txt" 2>&1
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
echo "facts done $(date -Iseconds); Xid lines $(grep -c Xid "$C/host-xid.txt"), device pid $DEVPID"
