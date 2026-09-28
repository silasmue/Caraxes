# SPDX-License-Identifier: MIT
# Sourced by limits-check.sh (vm8).
# ecc-1: the ECC read side, then -e 0 (pending until the next boot) and the counter resets.
Q="--query-gpu=ecc.mode.current,ecc.mode.pending,ecc.errors.corrected.volatile.total,ecc.errors.uncorrected.volatile.total,ecc.errors.corrected.aggregate.total,ecc.errors.uncorrected.aggregate.total --format=csv"
step q-ecc $SMI -q -d ECC
step q-ecc-csv $SMI $Q
step q-rows $SMI -q -d ROW_REMAPPER
step remapped-rows $SMI --query-remapped-rows=gpu_bus_id,gpu_uuid,remapped_rows.correctable,remapped_rows.uncorrectable,remapped_rows.pending,remapped_rows.failure --format=csv
step q-page-retirement $SMI -q -d PAGE_RETIREMENT
step e0 $SMI -e 0
step q-ecc-csv-after-e0 $SMI $Q
step p0 $SMI -p 0
step p1 $SMI -p 1
step fb-info $SMI --query-gpu=memory.total,memory.reserved --format=csv
