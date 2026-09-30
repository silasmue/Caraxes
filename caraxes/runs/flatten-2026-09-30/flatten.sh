#!/usr/bin/env bash
# Step 1 (user, 2026-09-29 23:10): flatten the Windows qcow2 chain (win11/ <- codex-win <- windows/
# <- windows-plain/, 80 GiB) into one sparse raw image, check it byte for byte against the chain,
# grow it to 160 GiB, and make the old top layer read-only so no VM boots the stale chain.
# Runs under vmlock.sh windows (no Windows VM may run). Exit 75 = not now (space or in use).
set -uo pipefail
W=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain
SRC=$W/win11.qcow2 TMP=$W/win11.raw.part DST=$W/win11.raw
LOG=/mnt/vmstore/caraxes/runs-2026-09-30/flatten/flatten.log
log() { echo "$(date +%T) $*" | tee -a "$LOG"; }
[[ -e $DST ]] && { log "raw already there"; exit 0; }
avail=$(df --output=avail -B1G /mnt/vmstore | tail -1 | tr -d ' ')
(( avail >= 115 )) || { log "only ${avail}G free, need 115"; exit 75; }
log "convert start, ${avail}G free"
ionice -c2 -n7 qemu-img convert -f qcow2 -O raw -m 8 -W "$SRC" "$TMP" >> "$LOG" 2>&1 || { log "convert FAILED (partial $TMP left)"; exit 1; }
log "convert done, compare start"
qemu-img compare -f qcow2 -F raw "$SRC" "$TMP" >> "$LOG" 2>&1 || { log "compare FAILED (kept $TMP)"; exit 1; }
mv "$TMP" "$DST" && qemu-img resize -f raw "$DST" 160G >> "$LOG" 2>&1 || { log "resize FAILED"; exit 1; }
chmod a-w "$SRC" && log "old top layer read-only: $SRC"
log "done: $(du -h "$DST" | cut -f1) allocated, $(qemu-img info -f raw "$DST" | grep 'virtual size'); free $(df -h --output=avail /mnt/vmstore | tail -1)"
