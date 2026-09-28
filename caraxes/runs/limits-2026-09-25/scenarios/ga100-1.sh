# SPDX-License-Identifier: MIT
# Sourced by limits-check.sh (vm8).
# ga100-1: the GA100 rung from a fresh card (FRESH=1): the ECC read side and -e 0, -mig 1
# (pending on GA100: no resetless MIG), then a GPU reset (-r), which applies both pending
# settings; after it the MIG profiles, a GPU instance, a compute instance, the teardown and
# -mig 0 (pending again). The next boot (ga100-2) shows what the power-on applies.
Q="--query-gpu=ecc.mode.current,ecc.mode.pending,mig.mode.current,mig.mode.pending --format=csv"
step q $SMI $Q
step q-ecc $SMI -q -d ECC
step remapped-rows $SMI --query-remapped-rows=gpu_bus_id,remapped_rows.correctable,remapped_rows.uncorrectable,remapped_rows.pending,remapped_rows.failure --format=csv
step e0 $SMI -e 0
step mig1 $SMI -mig 1
step q-pending $SMI $Q
step lgip-pending $SMI mig -lgip
step reset $SMI -r
sleep 3
step q-after-reset $SMI $Q
step lgip $SMI mig -lgip
step lgipp $SMI mig -lgipp
step cgi $SMI mig -cgi 19 -C
step lgi $SMI mig -lgi
step lci $SMI mig -lci
step list $SMI -L
step dci $SMI mig -dci
step dgi $SMI mig -dgi
step mig0 $SMI -mig 0
step e1 $SMI -e 1
step q-end $SMI $Q
