#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The cloud-init seed of vm3-615: vm3's seed (same user, tap, address) with a new
# instance id, as a FAT image labelled cidata like vm3's seed.img.
set -euo pipefail
S=/mnt/vmstore/caraxes/vm3/seed
W=/mnt/vmstore/caraxes/vm3-615
[[ -e $W/seed.img ]] && { echo "ERROR: $W/seed.img exists" >&2; exit 2; }
mkdir -p "$W/seed"
cp "$S/user-data" "$S/network-config" "$W/seed/"
printf 'instance-id: caraxes-vm3-615-%s\nlocal-hostname: caraxes-vm3\n' "$(date +%s)" > "$W/seed/meta-data"
mkfs.vfat -C -n cidata "$W/seed.img" 1440 >/dev/null
mcopy -i "$W/seed.img" "$W/seed/user-data" "$W/seed/meta-data" "$W/seed/network-config" ::
mdir -i "$W/seed.img" ::
