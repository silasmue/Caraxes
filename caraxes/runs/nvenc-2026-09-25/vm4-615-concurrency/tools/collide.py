#!/usr/bin/env python3
# collide.py <trace.xz>: trapped doorbell writes (W bar0 0xbb0090 <token>) whose token the device
# routes to a *different* guest chid as a host token (the doorbell token collision of
# vm4-615-cuda-vulkan/NOTES.md), plus GR channels that get a pool token or shadow build.
import sys,lzma,re,collections
last=None; hits=collections.OrderedDict(); pool=[]
for l in lzma.open(sys.argv[1],'rt',errors='replace'):
    m=re.match(r'\s*(\d+) W bar0 0x00bb0090 4 (0x[0-9a-f]+)',l)
    if not m: m=re.match(r'\s*(\d+) E .*doorbell ioeventfd: \d+ guest write\(s\) of token (0x[0-9a-f]+)',l)
    if m: last=(int(m.group(1)),int(m.group(2),16)); continue
    m=re.match(r'\s*(\d+) E .*doorbell: guest chid (\d+) -> host token (0x[0-9a-f]+)',l)
    if m and last and int(m.group(1))-last[0]<2000:
        t,v=last; g=int(m.group(2)); ht=int(m.group(3),16)
        if ht==v and (v & 0xfff)!=g:
            k=(v,g); hits.setdefault(k,[t,0]); hits[k][1]+=1
        last=None
    if 'gets pool token' in l: pool.append(l.split(None,2)[2].strip()[:120])
for (v,g),(t,n) in hits.items():
    print(f"COLLISION raw token {v:#x} (guest chid {v & 0xfff}, runlist {(v>>16)&0x7f}) routed to guest chid {g} as its host token; first at {t} us, {n} ring(s)")
if not hits: print("no collision in window")
