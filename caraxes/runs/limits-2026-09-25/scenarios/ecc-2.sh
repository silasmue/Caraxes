# SPDX-License-Identifier: MIT
# Sourced by limits-check.sh (vm8).
# ecc-2: the boot after ecc-1's -e 0: the mode now off; -e 1, then a GPU reset (-r), which
# applies the pending mode without a reboot.
Q="--query-gpu=ecc.mode.current,ecc.mode.pending --format=csv"
step q-ecc-csv $SMI $Q
step q-ecc $SMI -q -d ECC
step e1 $SMI -e 1
step q-ecc-csv-after-e1 $SMI $Q
step reset $SMI -r
sleep 3
step q-ecc-csv-after-reset $SMI $Q
step e1-again $SMI -e 1
step bad-e $SMI -e 2
