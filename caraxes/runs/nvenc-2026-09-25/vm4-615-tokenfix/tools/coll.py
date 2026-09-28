#!/usr/bin/env python3
# coll.py <trace-doorbell.txt.xz>...: per "[collision avoided]" ring, does the named guest channel run
# within 50 ms ("shadow: chid N scheduled" or "doorbell: guest chid N -> host token")?
import lzma, re, sys
for f in sys.argv[1:]:
    ev=[]
    for l in lzma.open(f,'rt',errors='replace'):
        p=l.split(None,1)
        try: ev.append((int(p[0]), l))
        except (ValueError, IndexError): pass
    n=ok=0; worst=0; bad=[]
    for i,(t,l) in enumerate(ev):
        m=re.search(r'guest chid (\d+) rung first.*collision avoided', l)
        if not m: continue
        c=m.group(1); n+=1
        pat=re.compile(rf'(shadow: chid {c} scheduled|doorbell: guest chid {c} -> host token)')
        hit=next((t2-t for t2,l2 in ev[i+1:] if t2-t<=50000 and pat.search(l2)), None)
        if hit is None: bad.append(c)
        else: ok+=1; worst=max(worst,hit)
    print(f"{f}: collisions {n}, guest channel ran within 50 ms {ok}, worst {worst} us, missed chids {bad}")
