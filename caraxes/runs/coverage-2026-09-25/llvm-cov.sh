#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Function coverage of the device crate: the instrumented device of `run.sh map` against its
# unit tests (the map and extra parts; both built with -C instrument-coverage; llvm-profdata and llvm-cov of the rustup
# llvm-tools component, LLVM 20, the same major as rustc 1.89's). Writes llvm-functions.md and
# llvm-functions.tsv next to this script.
#   llvm-cov.sh tests   build and run the unit tests instrumented (target/cov-test)
#   llvm-cov.sh report  merge both profiles and write the tables
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
LLVM=${LLVM_BIN:-$HOME/.rustup/toolchains/1.88.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin}
STORE=/mnt/vmstore/caraxes/coverage-2026-09-25
T=$ROOT/target/cov-test
cd "$ROOT"
case ${1:-} in
    tests)
        mkdir -p "$T/prof"; rm -f "$T"/prof/*.profraw
        RUSTFLAGS="-C instrument-coverage" CARGO_TARGET_DIR=$T LLVM_PROFILE_FILE=$T/prof/t-%p-%m.profraw \
            cargo test --release -p vfio-user-nvgpu --lib ;;
    report)
        "$LLVM/llvm-profdata" merge -sparse "$STORE"/llvm-map/*.profraw "$STORE"/llvm-extra/*.profraw -o "$STORE/map.profdata"
        "$LLVM/llvm-profdata" merge -sparse "$T"/prof/*.profraw -o "$STORE/tests.profdata"
        TBIN=$(find "$T/release/deps" -maxdepth 1 -type f -executable -name 'vfio_user_nvgpu-*' | head -1)
        "$LLVM/llvm-cov" export -format=text -skip-expansions "$ROOT/target/cov-instr/release/vfio-user-nvgpu" \
            -instr-profile="$STORE/map.profdata" > "$STORE/map-cov.json"
        "$LLVM/llvm-cov" export -format=text -skip-expansions "$TBIN" -instr-profile="$STORE/tests.profdata" > "$STORE/tests-cov.json"
        python3 "$HERE/llvm-functions.py" "$STORE/map-cov.json" "$STORE/tests-cov.json" \
            --tsv "$HERE/llvm-functions.tsv" > "$HERE/llvm-functions.md"
        "$LLVM/llvm-cov" report "$ROOT/target/cov-instr/release/vfio-user-nvgpu" -instr-profile="$STORE/map.profdata" \
            -ignore-filename-regex='(\.cargo|/rustc/|crates/(nvrm|dp-sink|vfio-user-rs)|vendor/)' | tail -3 > "$HERE/llvm-map-totals.txt"
        cat "$HERE/llvm-functions.md" | tail -3 ;;
    *) echo "usage: llvm-cov.sh tests|report" >&2; exit 2 ;;
esac
