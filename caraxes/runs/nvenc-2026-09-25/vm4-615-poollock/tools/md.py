#!/usr/bin/env python3
# md.py <rundir>: markdown table from table.txt plus per-step files.
import re,sys,os
d=sys.argv[1]
rows=[]
print("| Step (host time) | Result | Kicks/s | Slow polls (window; /min since last; dominant) | Sunshine latency min/max/avg | Xid | Guest us/sy/st %, irq/s | D state | Host: device threads %, vCPU sum %, load |")
print("|---|---|---|---|---|---|---|---|---|")
for l in open(os.path.join(d,'table.txt')):
    f=l.split(' | ')
    head=f[0].split()
    t,step=head[0],head[1]
    res=''
    m=re.search(r'rc=(\d+)',f[0]); el=re.search(r'el=([0-9.]+)',f[0])
    if m:
        rc=int(m.group(1))
        res = f"vkcube PASS {120/float(el.group(1)):.1f} fps ({float(el.group(1)):.1f} s)" if rc==0 else (f"vkcube FAIL rc {rc}" + (" (abort in demo_create_device)" if rc==134 else " (timeout)" if rc==124 else ""))
    vm=re.search(r'vkmark=(\d*)',f[0]); gm=re.search(r'glmark2=(\d*)',f[0])
    if vm: res=f"vkmark {vm.group(1) or 'fail'}, glmark2 {gm.group(1) if gm and gm.group(1) not in ('','0') else 'fail/0'}"
    mv=re.search(r'moves (\d+) in ([0-9.]+) s',f[0])
    if mv: res=f"cursor {int(mv.group(1))/float(mv.group(2)):.0f} moves/s"
    if not res: res='idle 20 s'
    k=re.search(r'kicks_per_s=([0-9.]+)',f[1]).group(1)
    ws=re.search(r'win_slow=(\d+)',f[2]); pm=re.search(r'per_min=([0-9.]+)',f[2]); dom=re.search(r'dominant=(\S+)',f[2])
    sp=f"{ws.group(1)}; {pm.group(1)}; {dom.group(1)}"
    lat=f[3].replace('lat:','').replace('(min/max/avg):','').strip() or '-'
    x=f[4].replace('xid=','').strip()
    cpu=re.search(r'us=(\d+) sy=(\d+) st=(\d+) in=(\d+)',f[5])
    c=f"{cpu.group(1)}/{cpu.group(2)}/{cpu.group(3)}, {cpu.group(4)}" if cpu else '-'
    D=f[6].replace('D:','').strip() or '-'
    h=f[7] if len(f)>7 else ''
    dev=re.findall(r'(\S+):([0-9.]+)',h.split('ch:')[0].replace('host dev:',''))
    chs=sum(float(v) for _,v in re.findall(r'(vcpu\d+):([0-9.]+)',h))
    la=re.search(r'load=([0-9.]+)',h)
    hs=f"{' '.join(f'{n.rstrip(chr(43))}:{float(v):.0f}' for n,v in dev)}; {chs:.0f}; {la.group(1) if la else '-'}"
    print(f"| {step} ({t.split('-')[0]}) | {res} | {k} | {sp} | {lat} | {x} | {c} | {D} | {hs} |")
