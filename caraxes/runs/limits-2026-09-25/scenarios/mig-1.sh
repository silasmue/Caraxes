# SPDX-License-Identifier: MIT
# Sourced by limits-check.sh (vm8).
# mig-1: the MIG mode on (-mig 1) and what nvidia-smi then shows; the next boot is mig-2.
Q="--query-gpu=mig.mode.current,mig.mode.pending --format=csv"
step q-mig $SMI $Q
step lgip-before $SMI mig -lgip
step mig1 $SMI -mig 1
step q-mig-after $SMI $Q
step q-d-mig $SMI -q -d MIG
