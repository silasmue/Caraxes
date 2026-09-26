#!/usr/bin/env python3
# stall.py <trace.xz> <rmlog>...: late RPC answers (rx -> tx reply > 100 ms, or none) in the trace
# window, doorbells per second, and ioctls over 100 ms from the rmioctl logs.
import sys,lzma,re,collections
rx={}; late=[]; db=collections.Counter(); t0=t1=None
for l in lzma.open(sys.argv[1],'rt',errors='replace'):
    p=l.split(None,2)
    if len(p)<2 or not p[0].isdigit(): continue
    t=int(p[0]); t0=t if t0 is None else t0; t1=t
    m=re.search(r'rx seq \d+ (\S+) \(\d+\) sequence (\d+).*?(hObject=\S+ cmd=\S+|hClass=\S+|)',l)
    if m: rx[m.group(2)]=(t,m.group(1),m.group(3)); continue
    m=re.search(r'tx seq \d+ reply (\S+) sequence (\d+) result (\S+)',l)
    if m and m.group(2) in rx:
        ta,n,x=rx.pop(m.group(2)); d=(t-ta)/1e3
        if d>100: late.append((d,ta,n,x,m.group(3)))
    if 'doorbell' in l: db[t//1000000]+=1
print(f"window {t0}..{t1} us")
print("late RPC answers (>100 ms):", len(late))
for d,ta,n,x,r in sorted(late,reverse=True)[:10]: print(f"  {d:9.1f} ms at {ta} {n} {x} result {r}")
print("RPCs without reply in window:", len(rx))
for s,(ta,n,x) in sorted(rx.items(),key=lambda k:k[1][0])[:10]: print(f"  seq {s} at {ta} {n} {x}")
print("doorbell lines per second:", " ".join(f"{k}:{v}" for k,v in sorted(db.items())))
for f in sys.argv[2:]:
    slow=[l.strip()[:200] for l in open(f,errors='replace') if l.startswith('SLOW') or re.search(r'us=(\d{6,})',l)]
    print(f"{f.split('/')[-1]}: {len(slow)} ioctl(s) > 100 ms"); [print("  ",s) for s in slow[:8]]
