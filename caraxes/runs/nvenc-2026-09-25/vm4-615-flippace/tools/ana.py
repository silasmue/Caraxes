#!/usr/bin/env python3
# ana.py <trace-window.txt.xz>: gaps from a "host notifier ... fired" note to the next MSI re-arm
# (eventfd E line or trapped W bar0 0x00088704), and kick interval stats. Note timestamps are
# the flush time of the note (the next traced event), not the event itself.
import lzma, sys, re
t_notes=[]; rearm=[]; kicks=[]
for l in lzma.open(sys.argv[1], 'rt', errors='replace'):
    p=l.split(None, 2)
    if len(p) < 3: continue
    try: t=int(p[0])
    except ValueError: continue
    s=p[2]
    if p[1]=='E' and 'host notifier' in s and 'fired' in s and ', MSI' in s: t_notes.append(t)
    elif (p[1]=='E' and s.startswith('msi re-arm')) or (p[1]=='W' and s.startswith('bar0 0x00088704')): rearm.append(t)
    elif p[1]=='W' and s.startswith('bar0 0x00696000'): kicks.append(t)
import bisect
g=[]
for t in t_notes:
    i=bisect.bisect_right(rearm, t)
    if i < len(rearm): g.append(rearm[i]-t)
def q(v):
    v=sorted(v); n=len(v)
    return f"n={n} p50={v[n//2]} p90={v[int(n*.9)]} p99={v[int(n*.99)]} max={v[-1]} us" if n else "n=0"
print("notifier->rearm", q(g))
ki=[b-a for a,b in zip(kicks, kicks[1:])]
print("kick interval", q(ki))
print(f"rearms={len(rearm)} notifier_msi_notes={len(t_notes)}")
