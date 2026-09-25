#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# serial.log and gpu.log are ignored as *.log; keep what the records need from them in files
# the repository takes: guest-check.txt (the guest check's lines: NVRM, the markers, the exit
# codes, lspci, nvidia-smi, deduplicated in order) and device-log.txt (the device log without
# the vfio-user connection lines). Usage: keep.sh (every record under this directory)
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
for d in $(find "$HERE" -name serial.log -printf "%h\n" | sort); do
    if [[ -f $d/serial.log ]]; then
        { echo "# SPDX-License-Identifier: MIT"
          echo "# The guest check lines of serial.log (ignored as *.log), deduplicated in order."
          tr -d '\r' < "$d/serial.log" \
            | grep -aE "NVRM|CARAXES-CHECK|rc=|10de:|Subsystem|Region|Expansion ROM|Capabilities: \[|Product Name|VBIOS Version|Total  |, [0-9.]+, 0x|UVM_|Xid" \
            | grep -av "NVRM: VM:" | awk '!seen[$0]++'
        } > "$d/guest-check.txt"
    fi
    if [[ -f $d/gpu.log ]]; then
        { echo "# SPDX-License-Identifier: MIT"
          echo "# The device log gpu.log (ignored as *.log) without the vfio-user connection lines."
          grep -av "INFO  vfio_user" "$d/gpu.log"
        } > "$d/device-log.txt"
    fi
done
