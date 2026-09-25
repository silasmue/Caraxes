#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The state folder of the limits runs (README.md next to this script), offline edits, no
# boot: vm8, a copy of vm7's overlay (caraxes/runs/emu-regress-2026-09-25/prepare-vm7.sh: the
# 610 image with the guest modules of vm0, the 610.57.04 gsp_tu10x.bin and gsp_ga10x.bin,
# gdm, the serial getty and the network waits masked, and /etc/modprobe.d/caraxes.conf with
# the registry keys RmDisableGlobalCeUtils=1, RMDisableScrubOnFree=1, RmRcWatchdog=0), made
# the way prepare-vm7.sh makes vm7 from vm6: the overlay and seed copied. Its
# caraxes-check.service runs limits-check.sh, which sources the scenario run.sh uploads before
# each boot; cuda-probe.c goes to /usr/local/share/caraxes.
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
S=/mnt/vmstore/caraxes
if [[ ! -e $S/vm8/rootfs.qcow2 ]]; then
    mkdir -p "$S/vm8"
    cp "$S/vm7/rootfs.qcow2" "$S/vm7/seed.img" "$S/vm8/"
fi
guestfish -a "$S/vm8/rootfs.qcow2" -i <<GF
upload $HERE/limits-check.sh /usr/local/sbin/caraxes-check.sh
chmod 0755 /usr/local/sbin/caraxes-check.sh
mkdir-p /usr/local/share/caraxes
upload $HERE/cuda-probe.c /usr/local/share/caraxes/cuda-probe.c
GF
guestfish --ro -a "$S/vm8/rootfs.qcow2" -i sh 'ls -la /etc/systemd/system/multi-user.target.wants/caraxes-check.service /usr/local/sbin/caraxes-check.sh /usr/local/share/caraxes; cat /etc/modprobe.d/caraxes.conf'
