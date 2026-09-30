#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Maintenance helper: type a line into the guest over QMP send-key, for a German (QWERTZ)
keyboard layout (the image's WinRE). Usage: qtype.py '<text>' [--enter]"""
import json, os, socket, sys, time

SOCK = os.environ.get('QMP_SOCK', '/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/qmp.sock')
SPECIAL = {' ': ['spc'], ':': ['shift', 'dot'], '\\': ['alt_r', 'minus'], '/': ['shift', '7'],
           '-': ['slash'], '.': ['dot'], '=': ['shift', '0'], '_': ['shift', 'slash'],
           ',': ['comma'], '"': ['shift', '2'], '*': ['shift', 'bracket_right'], '>': ['shift', 'less'],
           '<': ['less'], '|': ['alt_r', 'less'], '?': ['shift', 'minus']}


def keys(c):
    if c in SPECIAL:
        return SPECIAL[c]
    if c.isdigit():
        return [c]
    if c.isalpha() and c.isascii():
        k = {'y': 'z', 'z': 'y'}.get(c.lower(), c.lower())
        return ['shift', k] if c.isupper() else [k]
    raise SystemExit(f'no key for {c!r}')


s = socket.socket(socket.AF_UNIX)
s.connect(SOCK)
f = s.makefile('rw')
f.readline()
def cmd(o):
    f.write(json.dumps(o) + '\n'); f.flush()
    while True:
        r = json.loads(f.readline())
        if 'return' in r or 'error' in r:
            return r
cmd({'execute': 'qmp_capabilities'})
text = sys.argv[1]
seq = [keys(c) for c in text] + ([['ret']] if '--enter' in sys.argv else [])
for ks in seq:
    r = cmd({'execute': 'send-key', 'arguments': {'keys': [{'type': 'qcode', 'data': k} for k in ks]}})
    if 'error' in r:
        raise SystemExit(r)
    time.sleep(0.05)
print('typed', len(text), 'chars')
