# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# guest script: find or build nvenc-probe, then 3 encode/decode runs (1080p H.264, 300 frames)
cd ~
P=$(ls ~/nvenc/nvenc-probe ~/bench/nvenc-probe ~/nvenc-probe 2>/dev/null | head -1)
if [ -z "$P" ]; then
  H=$(dirname "$(find / -xdev -name nvEncodeAPI.h 2>/dev/null | head -1)")
  echo "no probe; headers at: ${H:-none}"; ls ~/nvenc ~/bench 2>&1 | head
  exit 0
fi
echo "probe: $P $(sha256sum "$P" | cut -c1-16)"
for i in 1 2 3; do "$P" -n 300 2>&1 | grep -E "^ENC|^DEC|RESULT|ERROR|error" ; done
nvidia-smi --query-gpu=memory.used --format=csv,noheader
sudo dmesg | grep -E "Xid" | tail -3
