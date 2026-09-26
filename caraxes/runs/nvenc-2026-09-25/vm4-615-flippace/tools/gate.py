#!/usr/bin/env python3
# gate.py <trace-window.txt.xz>: MMU-invalidate trigger writes, busy trigger reads, the time from a
# trigger write to the first read that finds TRIGGER FALSE (A) or to the next traced event (B, the
# write itself waits), slow pool polls and the longest gap between display kicks.
import lzma, sys
w=[]; busy=0; done=[]; nxt=[]; slow=[]; kicks=[]; pend=None; after_w=None; t0=t1=None
for l in lzma.open(sys.argv[1], 'rt', errors='replace'):
    p=l.split()
    if len(p) < 4: continue
    try: t=int(p[0])
    except ValueError: continue
    t0 = t if t0 is None else t0; t1=t
    if after_w is not None: nxt.append(t-after_w); after_w=None
    if p[1]=='W' and p[2]=='bar0' and p[3]=='0x00b830b0' and int(p[5],16) & 0x80000000:
        w.append(t); pend=t; after_w=t
    elif p[1]=='R' and p[2]=='bar0' and p[3]=='0x00b830b0':
        v=int(p[5],16)
        if v & 0x80000000: busy+=1
        elif pend is not None: done.append(t-pend); pend=None
    elif p[1]=='W' and p[2]=='bar0' and p[3]=='0x00696000': kicks.append(t)
    if 'held the device' in l:
        try: slow.append(int(l.split('held the device ')[1].split()[0]))
        except Exception: pass
def q(v):
    v=sorted(v); n=len(v)
    return f"n={n} p50={v[n//2]/1e3:.1f} p99={v[int(n*.99)]/1e3:.1f} max={v[-1]/1e3:.1f} sum={sum(v)/1e6:.2f}s" if n else "n=0"
span=(t1-t0)/1e6 if t0 else 0
kg=[b-a for a,b in zip(kicks,kicks[1:])]
print(f"span={span:.1f}s triggers={len(w)} ({len(w)/span:.0f}/s) busy_reads={busy} | write->applied ms {q(done)} | write->next event ms {q(nxt)} | slow_polls={len(slow)} ({sum(slow)/1e3:.0f} ms) | max kick gap {max(kg)/1e3 if kg else 0:.0f} ms")
