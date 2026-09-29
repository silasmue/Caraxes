#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""The dependency closure of Debian/Ubuntu packages from Packages.xz indexes (fetch-weston-debs.sh).

Usage: apt-closure.py INDEX.xz[,INDEX.xz...] PACKAGE[,PACKAGE...] > closure.txt
The newest version of each package over all indexes; Depends and Pre-Depends, the first
alternative that exists (virtual packages through Provides). One line per package:
name version filename size sha256; warnings and the total on stderr."""
import lzma, re, sys
def vercmp(a, b):
    def split(v):
        e, _, r = v.partition(':') if ':' in v else ('0', '', v)
        if ':' not in v: e, r = '0', v
        u, _, d = r.rpartition('-') if '-' in r else (r, '', '0')
        return int(e), u, d
    def order(c):
        if c == '~': return -1
        if c.isdigit(): return 0
        if c.isalpha(): return ord(c)
        return ord(c) + 256
    def cmp_part(x, y):
        while x or y:
            nx = re.match(r'^[^0-9]*', x).group(0); ny = re.match(r'^[^0-9]*', y).group(0)
            for i in range(max(len(nx), len(ny))):
                cx = order(nx[i]) if i < len(nx) else 0; cy = order(ny[i]) if i < len(ny) else 0
                if cx != cy: return cx - cy
            x, y = x[len(nx):], y[len(ny):]
            dx = re.match(r'^[0-9]*', x).group(0); dy = re.match(r'^[0-9]*', y).group(0)
            ix, iy = int(dx or 0), int(dy or 0)
            if ix != iy: return ix - iy
            x, y = x[len(dx):], y[len(dy):]
        return 0
    ea, ua, da = split(a); eb, ub, db = split(b)
    if ea != eb: return ea - eb
    c = cmp_part(ua, ub)
    return c if c else cmp_part(da, db)
pkgs, provides = {}, {}
for f in sys.argv[1].split(','):
    for block in lzma.open(f, 'rt', errors='replace').read().split('\n\n'):
        d = {}
        key = None
        for line in block.split('\n'):
            if line.startswith(' ') and key: d[key] += '\n' + line; continue
            if ':' in line:
                key, _, val = line.partition(':'); d[key] = val.strip()
        if 'Package' not in d: continue
        n = d['Package']
        if n not in pkgs or vercmp(d['Version'], pkgs[n]['Version']) > 0:
            pkgs[n] = d
for n, d in pkgs.items():
    for p in d.get('Provides', '').split(','):
        p = p.strip().split(' ')[0]
        if p: provides.setdefault(p, []).append(n)
want = sys.argv[2].split(',')
seen, order = set(), []
def add(n):
    if n in seen: return
    if n not in pkgs:
        if n in provides: n = provides[n][0]
        else: print('MISSING', n, file=sys.stderr); return
        if n in seen: return
    seen.add(n); order.append(n)
    d = pkgs[n]
    for field in ('Pre-Depends', 'Depends'):
        for dep in d.get(field, '').split(','):
            dep = dep.strip()
            if not dep: continue
            alts = [a.strip().split(' ')[0].split(':')[0] for a in dep.split('|')]
            for a in alts:
                if a in pkgs or a in provides: add(a); break
            else: print('UNRESOLVED', n, dep, file=sys.stderr)
for w in want: add(w)
total = 0
for n in order:
    d = pkgs[n]; total += int(d.get('Size', 0))
    print(n, d['Version'], d['Filename'], d['Size'], d['SHA256'])
print('TOTAL', len(order), total, file=sys.stderr)
