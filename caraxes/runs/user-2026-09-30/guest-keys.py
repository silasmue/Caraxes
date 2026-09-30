#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Key presses into the guest's session through a throwaway uinput keyboard (root).
# Used for docs/vfio/showcase.md (2026-09-30): Super+Left/Right tile the windows, Enter answers a dialog.
# Usage: guest-keys.py KEY...   (ENTER TAB ESC DOWN UP LEFT RIGHT SPACE; S-, A-, M- for Shift, Alt, Super)
import fcntl, os, struct, sys, time
K = dict(ESC=1, TAB=15, ENTER=28, SPACE=57, UP=103, DOWN=108, LEFT=105, RIGHT=106, LEFTSHIFT=42, LEFTALT=56, LEFTMETA=125)
MOD = {'S': 42, 'A': 56, 'M': 125}
fd = os.open('/dev/uinput', os.O_WRONLY | os.O_NONBLOCK)
fcntl.ioctl(fd, 0x40045564, 1)  # UI_SET_EVBIT EV_KEY
for c in K.values():
    fcntl.ioctl(fd, 0x40045565, c)  # UI_SET_KEYBIT
os.write(fd, struct.pack('80sHHHHi', b'caraxes-keys', 3, 1, 1, 1, 0) + b'\0' * (4 * 64 * 4))
fcntl.ioctl(fd, 0x5501)  # UI_DEV_CREATE
time.sleep(1.5)
def ev(t, c, v):
    os.write(fd, struct.pack('llHHi', 0, 0, t, c, v))
def tap(c):
    ev(1, c, 1); ev(0, 0, 0); ev(1, c, 0); ev(0, 0, 0); time.sleep(0.25)
for k in sys.argv[1:]:
    if len(k) > 2 and k[1] == '-':
        m = MOD[k[0]]
        ev(1, m, 1); ev(0, 0, 0); tap(K[k[2:]]); ev(1, m, 0); ev(0, 0, 0); time.sleep(0.5)
    else:
        tap(K[k])
time.sleep(0.5)
fcntl.ioctl(fd, 0x5502)  # UI_DEV_DESTROY
os.close(fd)
