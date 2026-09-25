# SPDX-License-Identifier: MIT
# Sourced by limits-check.sh (vm8).
# ga100-2: the boot after ga100-1: which pending settings the power-on applied (MIG through
# the devinit scratch bit the kernel reads at load), then a GPU instance from a MIG mode
# that was on at load.
Q="--query-gpu=ecc.mode.current,ecc.mode.pending,mig.mode.current,mig.mode.pending --format=csv"
step q $SMI $Q
step mig1 $SMI -mig 1
step q-mig1 $SMI $Q
step reset $SMI -r
sleep 3
step q-after-reset $SMI $Q
step cgi $SMI mig -cgi 19
step lgi $SMI mig -lgi
step dgi $SMI mig -dgi
step mig0 $SMI -mig 0
step q-end $SMI $Q
