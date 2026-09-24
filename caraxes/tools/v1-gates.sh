#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The v1 smi and cuda gates against the bench VM of bench-v1vm.sh (up and modules done), as run by
# hand on 2026-09-24 (caraxes/runs/cutover-615-2026-09-24/v1/commands.txt sections 6 and 7).
#
#   v1-gates.sh OUTDIR [smi|cuda|all]
#
# smi: the guest's nvidia-smi against the host's, with the mask of Leandro-Test's gpu gate
#   (../Leandro-Test/scripts/test.sh:207-240, lea_mediated_name_re in scripts/lib/common.sh:371-377).
#   The host's nvidia-smi is run once as the reference (a host read).
# cuda: the Caraxes guest runner installed by `tools/caraxes-vm.sh sync-tests` (the v1 615 image
#   carries it, docs/caraxes/v1-615-image.md), one ssh call per test, rows in OUTDIR/cuda/rows.tsv.
#   gpu-kernel and cuda-managed pass only with the backend started with LEA_MANAGED_COMPAT=1
#   (bench-v1vm.sh's default; OUTDIR/backend.env records what the backend got).
# Environment: as bench-v1vm.sh (LEA_V1_ROOT, V, IP, ...). Exit status: the number of failed rows.
set -uo pipefail
ROOT=$(cd "$(dirname "$(readlink -f "$0")")/../.." && pwd -P)
VMCTL=$ROOT/caraxes/tools/bench-v1vm.sh
out=${1:?usage: v1-gates.sh OUTDIR [smi|cuda|all]}
what=${2:-all}
g() { "$VMCTL" ssh "$@"; }
mkdir -p "$out"
"$VMCTL" config > "$out/config.txt" 2>&1 || { cat "$out/config.txt"; exit 1; }
want=$(awk '$1 == "nvrm_wire.h" { print $2 }' "$out/config.txt")
vdir=$(awk '$1 == "V" { print $2 }' "$out/config.txt")
cp "$vdir/backend.env" "$out/backend.env" 2>/dev/null || echo "no $vdir/backend.env" > "$out/backend.env"
fails=0

if [[ $what == smi || $what == all ]]; then
    mkdir -p "$out/smi"
    g 'export LD_LIBRARY_PATH=/opt/nvrm/lib; timeout 90 /opt/nvrm/bin/nvidia-smi > /tmp/smi-module.txt 2>&1; echo "module=$?"' \
        > "$out/smi/s41.log" 2>&1
    g 'cat /tmp/smi-module.txt' > "$out/smi/smi-module.txt" 2>&1
    nvidia-smi > "$out/smi/smi-host.txt" 2>&1
    gpu=$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1)
    board=$(sed -e 's/^NVIDIA //' -e 's/^GeForce //' -e 's/[][\.*^$+?(){}|/]/\\&/g' <<<"$gpu")
    re="Leandro( VirtIO| VFIO)? $board"
    mask() {
        sed -E -e '1d' -e '/\| Processes:/,$d' -e "s/$re/CARD/g" -e "s/$gpu/CARD/g" \
            -e 's/[0-9][0-9]*/N/g' -e 's/  */ /g' "$1"
    }
    diff <(mask "$out/smi/smi-host.txt") <(mask "$out/smi/smi-module.txt") > "$out/smi/smi.diff"
    mediated=$(grep -oE "$re" "$out/smi/smi-module.txt" | head -1)
    if [[ ! -s $out/smi/smi.diff ]] && grep -q "module=0" "$out/smi/s41.log" \
       && grep -q "$want" "$out/smi/smi-module.txt" && [[ -n $gpu && -n $mediated ]]; then
        echo "smi PASS: nvidia-smi identical to the native host run (masked), driver $want, guest sees $mediated" | tee "$out/smi/verdict.txt"
    else
        { echo "smi FAIL: see smi.diff and s41.log"; cat "$out/smi/s41.log"; head -20 "$out/smi/smi.diff"; } | tee "$out/smi/verdict.txt"
        fails=$((fails + 1))
    fi
fi

if [[ $what == cuda || $what == all ]]; then
    mkdir -p "$out/cuda"
    name=v1-$(date +%Y%m%d-%H%M%S)
    date -Is > "$out/cuda/start.txt"
    : > "$out/cuda/rows.tsv"
    for t in cuda-init cuda-context gpu-memory gpu-kernel cuda-core cuda-jit cuda-launch cuda-managed \
             cuda-hostreg cuda-torch rlprobe convburn gpu-torch gpu-own; do
        g "CARAXES_TEST_TIMEOUT=120 ~/caraxes-tests/caraxes/run-tests.sh ~/caraxes-tests/out/$name $t" \
            | grep '^CXROW' | tee -a "$out/cuda/rows.tsv"
    done
    mkdir -p "$out/cuda/guest"
    g "tar -C ~/caraxes-tests/out -cf - $name" | tar -C "$out/cuda/guest" -xf - || true
    n=$(awk -F'\t' '$3 == "fail"' "$out/cuda/rows.tsv" | wc -l)
    echo "cuda: $(awk -F'\t' '$3 == "pass"' "$out/cuda/rows.tsv" | wc -l) pass, $n fail (rows.tsv)"
    fails=$((fails + n))
fi
exit "$fails"
