# SPDX-License-Identifier: MIT
# Sourced by limits-check.sh (vm8).
# survey: every command of the three families once, read side first, setters last.
step smi $SMI
step q $SMI -q
step q-ecc $SMI -q -d ECC
step q-rows $SMI -q -d ROW_REMAPPER
step remapped-rows $SMI --query-remapped-rows=gpu_bus_id,gpu_uuid,remapped_rows.correctable,remapped_rows.uncorrectable,remapped_rows.pending,remapped_rows.failure --format=csv
step q-ecc-csv $SMI --query-gpu=ecc.mode.current,ecc.mode.pending,ecc.errors.corrected.volatile.total,ecc.errors.uncorrected.aggregate.total --format=csv
step q-mig $SMI --query-gpu=mig.mode.current,mig.mode.pending --format=csv
step lgip $SMI mig -lgip
step vgpu $SMI vgpu
step q-virt $SMI -q -d VIRTUALIZATION
step e1 $SMI -e 1
step e0 $SMI -e 0
step p0 $SMI -p 0
step mig1 $SMI -mig 1
step q-mig-after $SMI --query-gpu=mig.mode.current,mig.mode.pending,ecc.mode.current,ecc.mode.pending --format=csv
step reset $SMI -r
