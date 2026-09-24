#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The cloud-init seed of vm5-615: vm5's seed (same user, tap, address) with a new
# instance id, as a FAT image labelled cidata like vm5's seed.img (as seed-vm3-615.sh of
# caraxes/runs/gsp-rpc-2026-09-24 does for vm3-615).
set -euo pipefail
S=/mnt/vmstore/caraxes/vm5/seed
W=/mnt/vmstore/caraxes/vm5-615
[[ -e $W/seed.img ]] && { echo "ERROR: $W/seed.img exists" >&2; exit 2; }
mkdir -p "$W/seed"
cp "$S/user-data" "$S/network-config" "$W/seed/"
printf 'instance-id: caraxes-vm5-615-%s\nlocal-hostname: caraxes-vm5\n' "$(date +%s)" > "$W/seed/meta-data"
mkfs.vfat -C -n cidata "$W/seed.img" 1440 >/dev/null
mcopy -i "$W/seed.img" "$W/seed/user-data" "$W/seed/meta-data" "$W/seed/network-config" ::
mdir -i "$W/seed.img" ::
