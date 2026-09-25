import re,sys,collections
parts=['sync','walk','decide','userd','refs','barrier','moves','resync']
reg=collections.Counter(); regms=collections.Counter(); held=[]; dom=collections.Counter()
for l in sys.stdin:
    m=re.search(r'at (0x[0-9a-f]+) held the device (\d+) us',l)
    if not m: continue
    r,h=m.group(1),int(m.group(2)); reg[r]+=1; regms[r]+=h/1000; held.append(h)
    v={p:int(re.search(r'\b%s (\d+) us'%p,l).group(1)) for p in parts if re.search(r'\b%s (\d+) us'%p,l)}
    if v: dom[max(v,key=v.get)]+=1
held.sort(); n=len(held)
if n: print(f"n={n} p50={held[n//2]} p90={held[int(n*.9)]} p99={held[int(n*.99)]} max={held[-1]} us; sum {sum(held)/1e6:.2f} s")
print("dominant part:",dict(dom.most_common()))
for r,c in reg.most_common(8): print(f"  {r}: {c} polls, {regms[r]:.0f} ms")
