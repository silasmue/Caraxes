#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Runs the cuda gate on one P20 VM (argument vm1 or vm2) into this directory.
set -u
ROOT=/home/silas/git/Caraxes; R=$(cd "$(dirname "$0")" && pwd); v=$1
cd "$ROOT"
source <(sed -n '/^vm_env()/,/^}/p' caraxes/tools/p20.sh)
vm_env "$v"
export CARAXES_GATE_RUNS=$R/gate-$v CARAXES_TEST_TIMEOUT=300
mkdir -p "$R/gate-$v"
t0=$(date +%s)
tools/caraxes-gate.sh cuda > "$R/gate-$v/verdict.json" 2> "$R/gate-$v/transcript.log"
echo "exit=$? dur_s=$(( $(date +%s) - t0 )) start=$(date -d @$t0 +%T)" > "$R/gate-$v/exit.txt"
