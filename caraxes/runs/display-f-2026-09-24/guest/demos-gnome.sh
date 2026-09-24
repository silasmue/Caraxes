#!/bin/bash
# SPDX-License-Identifier: MIT
# Series F (guest, desktop user): start GNOME (gdm3, Wayland, autologin), then the classic demos
# in that session, 20 s each; the demo outputs go to ~/demo-out-<label>/. GNOME stays up.
cd ~/caraxes-guest || exit 1
unset DISPLAY
sudo bash gnome-check.sh start 60 | grep -E "Type=wayland|gbm renderer|page_flip"
SECS=${SECS:-20} bash demos.sh gnome
rm -rf ~/demo-out-"${1:-gnome}"; mkdir -p ~/demo-out-"${1:-gnome}"; cp /tmp/demo-*.txt ~/demo-out-"${1:-gnome}"/
