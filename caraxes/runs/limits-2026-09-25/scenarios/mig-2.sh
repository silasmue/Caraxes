# SPDX-License-Identifier: MIT
# Sourced by limits-check.sh (vm8).
# mig-2: the boot after mig-1: the mode as the new boot sees it, the profiles, then (after a
# second -mig 1, since a resetless GH100's kernel enables MIG only on request) the GPU and
# compute instances, a CUDA context on the MIG device, and the teardown.
Q="--query-gpu=mig.mode.current,mig.mode.pending --format=csv"
cc -O2 -o /tmp/cuda-probe /usr/local/share/caraxes/cuda-probe.c -ldl; echo "cc rc=$?"
step q-mig $SMI $Q
step lgip-boot $SMI mig -lgip
step mig1-again $SMI -mig 1
step q-mig-after $SMI $Q
step lgip $SMI mig -lgip
step lgipp $SMI mig -lgipp
step cgi $SMI mig -cgi 19,19 -C
step lgi $SMI mig -lgi
step lci $SMI mig -lci
step lcip $SMI mig -lcip
step list $SMI -L
MIG=$($SMI -L | sed -n 's/.*(UUID: \(MIG-[^)]*\)).*/\1/p' | head -n 1)
echo "MIG device: $MIG"
modprobe nvidia-uvm; echo "modprobe nvidia-uvm rc=$?"
[ -e /dev/nvidia-uvm ] || mknod -m 666 /dev/nvidia-uvm c "$(grep nvidia-uvm /proc/devices | cut -d' ' -f1)" 0
[ -e /dev/nvidia-caps ] || true
step cuda-mig env CUDA_VISIBLE_DEVICES="$MIG" /tmp/cuda-probe
step dci $SMI mig -dci
step dgi $SMI mig -dgi
step mig0 $SMI -mig 0
step q-mig-end $SMI $Q
