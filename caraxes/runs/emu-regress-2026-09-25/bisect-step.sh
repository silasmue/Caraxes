#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# One `git bisect run` step of the pure-emulation regressions (README.md next to this script):
# builds the checked-out commit's device with CARGO_TARGET_DIR=target/bisect, boots it with
# emu-check.sh through the frozen HEAD launcher (CARAXES_RIG=target/rig) and decides by the
# serial log: good when nvidia-smi returns 0 and no `RmInitAdapter failed` precedes it, bad
# otherwise, skip (125) when the build fails. Usage: bisect-step.sh tu106|gb206
# Records go to <BISECT_RECORDS, default bisect>/<mode>-<sha>/ next to this script; an existing
# record is kept and its verdict reused (no second boot of the same commit).
set -uo pipefail
MODE=${1:?usage: bisect-step.sh tu106|gb206}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
R=$(cd "$HERE/../../.." && pwd -P)
sha=$(git -C "$R" rev-parse --short=7 HEAD)
rec=$HERE/${BISECT_RECORDS:-bisect}
out=$rec/$MODE-$sha
mkdir -p "$rec"
if [[ -f $out/verdict.txt ]]; then
    echo "$sha $(cat "$out/verdict.txt") (recorded)"; grep -q '^good' "$out/verdict.txt"; exit $?
fi
rm -rf "$out"
if ! (cd "$R" && CARGO_TARGET_DIR=target/bisect cargo build --release -p vfio-user-nvgpu) > "$out.build.txt" 2>&1; then
    echo "$sha build failed: skip"; exit 125
fi
rm -f "$out.build.txt"
env=(CARAXES_SERVER="$R/target/bisect/release/vfio-user-nvgpu")
[[ $MODE == gb206 ]] && env+=(CARAXES_CHIP=gb206 CARAXES_FB_MIB=4096)
CARAXES_RIG=$R/target/rig "$HERE/emu-check.sh" "$out" "${env[@]}" > "$out.oracle.txt" 2>&1
mv "$out.oracle.txt" "$out/oracle.txt"
# The serial log is ignored as *.log; keep the NVRM lines of the first load.
grep -aE "NVRM|CARAXES-CHECK|rc=" "$out/serial.log" | head -n 60 > "$out/serial-nvrm.txt"
if grep -q "nvidia-smi rc=0" "$out/serial.log" && ! grep -q "RmInitAdapter failed" "$out/serial.log"; then
    echo "$sha good"; echo good > "$out/verdict.txt"; exit 0
fi
first=$(grep -aoE "Check failed: numGpcs > 0|memmgrMemSet\(pMemoryManager, &vidSurface|Assertion failed: [^@]*@ [a-z_0-9.]+:[0-9]+|RmInitAdapter failed[^\r]*" "$out/serial.log" | head -n 1)
echo "$sha bad: $first"; echo "bad: $first" > "$out/verdict.txt"; exit 1
