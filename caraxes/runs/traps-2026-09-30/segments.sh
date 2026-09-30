#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Per-workload census of a session directory (census-linux.sh / census-windows.sh): for each
# "NAME=A:B" pair, profile.B minus profile.A through trap-census.py (NAME.md, NAME.json) and
# the thread CPU of Cloud Hypervisor and the device between marks A and B (NAME-cpu.txt).
# Usage: segments.sh DIR NAME=A:B ...
set -uo pipefail
D=$1; shift
H=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
TC=$(cd "$H/../../tools" && pwd -P)/trap-census.py
for seg in "$@"; do
    name=${seg%%=*} ab=${seg#*=} a=${ab%:*} b=${ab#*:}
    pa=$D/profile.$a.tsv pb=$D/profile.$b.tsv
    [[ -f $pa || -f $pa.xz ]] && [[ -f $pb || -f $pb.xz ]] || { echo "$name: no dumps $a/$b"; continue; }
    python3 "$TC" "$pb" --base "$pa" --name "$name" --regs 40 --json "$D/$name.json" > "$D/$name.md"
    { echo "Cloud Hypervisor:"; python3 "$H/taskstat.py" diff "$D/ch.$a.json" "$D/ch.$b.json"
      echo "device:"; python3 "$H/taskstat.py" diff "$D/dev.$a.json" "$D/dev.$b.json"; } > "$D/$name-cpu.txt" 2>&1
    head -1 "$D/$name.md"
done
