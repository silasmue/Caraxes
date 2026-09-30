#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Name the display methods the device trace lists ("display channel 0xc57X/N kick K: ...
methods m=v ..."), from the 615 class headers clc57d.h (core) and clc57e.h (window).
Usage: dispdecode.py <gpu-trace.log> [channel filter, e.g. 0xc57e/0]"""
import re, sys

H = '/mnt/vmstore/caraxes/ogkm-615.71.09/src/common/sdk/nvidia/inc/class/'


def names(hdr, prefix):
    """Method offset -> name, with (a)/(a,b) arrays expanded for a small range."""
    out = {}
    for line in open(H + hdr):
        m = re.match(rf'#define ({prefix}[A-Z0-9_]+)(\(([a-z,]+)\))?\s+\((0x[0-9A-Fa-f]+)(\s*\+\s*\(a\)\*(0x[0-9A-Fa-f]+))?(\s*\+\s*\(b\)\*(0x[0-9A-Fa-f]+))?\)', line)
        if not m:
            m2 = re.match(rf'#define ({prefix}[A-Z0-9_]+)\s+\((0x[0-9A-Fa-f]+)\)', line)
            if m2 and not m2.group(1).endswith(tuple('0123456789')) or (m2 and '_' in m2.group(1)):
                pass
            continue
        name, base = m.group(1), int(m.group(4), 16)
        sa = int(m.group(6), 16) if m.group(6) else None
        sb = int(m.group(8), 16) if m.group(8) else None
        if sa is None:
            out.setdefault(base, name)
        else:
            for a in range(8):
                if sb is None:
                    out.setdefault(base + a * sa, f'{name}({a})')
                else:
                    for b in range(4):
                        out.setdefault(base + a * sa + b * sb, f'{name}({a},{b})')
    return out


CORE = names('clc57d.h', 'NVC57D_')
WIN = names('clc57e.h', 'NVC57E_')
path = sys.argv[1]
flt = sys.argv[2] if len(sys.argv) > 2 else None
for line in open(path, errors='replace'):
    m = re.search(r'display channel (0xc57[de])/(\d+) kick (\d+): .*?; (core|window) methods (.*?); GET := PUT', line)
    if not m:
        continue
    ch = f'{m.group(1)}/{m.group(2)}'
    if flt and ch != flt:
        continue
    tbl = CORE if m.group(1) == '0xc57d' else WIN
    print(f'== {ch} kick {m.group(3)} ({line.split()[0]} us)')
    for mv in m.group(5).split():
        k, v = mv.split('=')
        print(f'  {int(k, 16):#06x} {tbl.get(int(k, 16), "?"):<48} {v}')
