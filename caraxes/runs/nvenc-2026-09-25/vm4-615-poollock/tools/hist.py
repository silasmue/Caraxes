#!/usr/bin/env python3
# hist.py <stepdir> [top]: trapped accesses of the step's trace window, per second (E lines excluded).
import sys,lzma,re,collections,os
d=sys.argv[1]; top=int(sys.argv[2]) if len(sys.argv)>2 else 15
f=os.path.join(d,'trace-window.txt.xz')
if not os.path.exists(f): f=os.path.join(d,'trace-first30s.txt.xz')
ev=collections.Counter(); EV=[('doorbell ioeventfd','doorbell ioeventfd'),('msi re-arm (eventfd)','msi re-arm ioeventfd'),('host notifier','host notifier events'),('mmu invalidate:','mmu invalidates handled'),('shadow: sync','shadow syncs'),('GSP_RM_CONTROL (','GSP RM controls'),('display channel','display kicks')]
cnt=collections.Counter(); name={}; tot=collections.Counter(); t0=t1=None
for l in lzma.open(f,'rt',errors='replace'):
    p=l.split()
    if len(p)>1 and p[1]=='E':
        for k,n in EV:
            if k in l: ev[n]+=1
    if len(p)<4 or p[1] not in ('R','W'): continue
    t=int(p[0]); t0=t if t0 is None else t0; t1=t
    bar,off=p[2],p[3]
    tot[(p[1],bar)]+=1
    if bar=='cfg': continue
    cnt[(bar,off,p[1])]+=1
    if (bar,off) not in name:
        rest=l.split(None,6)[6].strip() if len(l.split(None,6))>6 else ''
        name[(bar,off)]=re.sub(r'\s+',' ',rest)[:48]
dur=max((t1-t0)/1e6,1e-3) if t0 else 1
offs=collections.Counter()
for (b,o,rw),c in cnt.items(): offs[(b,o)]+=c
print(f"window {dur:.1f} s; trapped per second: " + ", ".join(f"{rw} {b} {c/dur:.0f}" for (rw,b),c in sorted(tot.items())))
print("device-side events per second (not traps): " + ", ".join(f"{n} {ev[n]/dur:.0f}" for _,n in EV))
print("| BAR | Offset | Name | Reads/s | Writes/s |"); print("|---|---|---|---|---|")
for (b,o),c in offs.most_common(top):
    print(f"| {b} | {o} | {name[(b,o)]} | {cnt[(b,o,'R')]/dur:.0f} | {cnt[(b,o,'W')]/dur:.0f} |")
