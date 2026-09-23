#!/bin/sh
# SPDX-License-Identifier: MIT
# Build the display probes in the guest: kmsflip, and wlframes (needs libwayland-dev,
# libwayland-bin and wayland-protocols; offline .debs in ~/caraxes-debs2 are installed
# first when present).
set -e
cd "$(dirname "$0")"
if ! command -v wayland-scanner >/dev/null && ls ../caraxes-debs2/*.deb >/dev/null 2>&1; then
    sudo dpkg -i ../caraxes-debs2/*.deb >/dev/null
fi
P=/usr/share/wayland-protocols
wayland-scanner client-header $P/stable/xdg-shell/xdg-shell.xml xdg-shell-client-protocol.h
wayland-scanner private-code $P/stable/xdg-shell/xdg-shell.xml xdg-shell-protocol.c
wayland-scanner client-header $P/stable/presentation-time/presentation-time.xml presentation-time-client-protocol.h
wayland-scanner private-code $P/stable/presentation-time/presentation-time.xml presentation-time-protocol.c
cc -O2 -Wall -I. -o wlframes wlframes.c xdg-shell-protocol.c presentation-time-protocol.c -lwayland-client -lm
cc -O2 -o kmsflip kmsflip.c -lm
cc -O2 -o kmsgrab kmsgrab.c
sync
echo built wlframes kmsflip kmsgrab
