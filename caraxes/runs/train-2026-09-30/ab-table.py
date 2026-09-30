#!/usr/bin/env python3
"""Train A/B table: per session, cuCtxCreate/Destroy of the user bench-cu init runs (mean,
median, the medians of the fast and the slow half), churn alloc/free medians, the root copy
and init, Xid, and the gather cache line of the exit summary."""
import re, statistics as st, sys, pathlib
base = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else '/mnt/vmstore/caraxes/runs-2026-09-30/train/ab')
def kv(line):
    return {k: float(v) for k, v in re.findall(r'(\w+)=([0-9.]+)', line)}
rows = []
for d in sorted(p for p in base.iterdir() if p.is_dir() and (p / 'guest.txt').exists()):
    g = (d / 'guest.txt').read_text(errors='replace').splitlines()
    init = [kv(l) for l in g if l.startswith('INIT ')]
    rinit = [kv(l) for l in g if l.startswith('ROOT INIT ')]
    churn = [kv(l) for l in g if l.startswith('CHURN ')]
    c = [i['ctxcreate_ms'] for i in init]
    ds = [i['ctxdestroy_ms'] for i in init]
    if not c:
        continue
    s = sorted(c); h = len(s) // 2
    ch1 = [x for x in churn if x.get('block_mib') == 1]
    ch64 = [x for x in churn if x.get('block_mib') == 64]
    rbw = [kv(l) for l in g if l.startswith('ROOT BW ') and 'size_mib=16 ' in l and 'mem=pinned' in l]
    ubw = [kv(l) for l in g if l.startswith('BW ') and 'size_mib=16 ' in l and 'mem=pinned' in l]
    xid = [l for l in g if l.startswith('--- xid:')]
    log = (d / 'gpu.log').read_text(errors='replace') if (d / 'gpu.log').exists() else ''
    m = re.findall(r'gather descriptors: (\d+) made, (\d+) taken from the cache of freed ones, (\d+) evicted', log)
    gc = f'{m[-1][0]} made / {m[-1][1]} cached / {m[-1][2]} evicted' if m else '-'
    refused = len(re.findall(r'BAR1 window refused', log))
    slot = (d / 'slot.txt').read_text().split()[1] if (d / 'slot.txt').exists() else '?'
    rows.append((d.name, slot, len(c), st.mean(c), st.median(c), st.median(s[:h]), st.median(s[h:]), st.median(ds),
                 st.median([x['alloc_med_us'] for x in ch1]) if ch1 else 0, st.median([x['free_med_us'] for x in ch1]) if ch1 else 0,
                 st.median([x['alloc_med_us'] for x in ch64]) if ch64 else 0, st.median([x['free_med_us'] for x in ch64]) if ch64 else 0,
                 ' / '.join(f"{b['med_us']/1000:.2f}" for b in ubw[:2]), ' / '.join(f"{b['med_us']/1000:.2f}" for b in rbw[:2]),
                 st.median([i['ctxcreate_ms'] for i in rinit]) if rinit else 0, xid[-1].split(':')[1].strip() if xid else '?', gc, refused))
print('| session | slot | n | create mean | median | fast half | slow half | destroy med | churn 1 MiB alloc/free us | 64 MiB alloc/free us | user h2d/d2h 16 MiB ms | root h2d/d2h 16 MiB ms | root create med | guest Xid | gathers | BAR1 windows refused |')
print('|' + '---|' * 18)
for r in rows:
    print(f'| {r[0]} | {r[1]} | {r[2]} | {r[3]:.0f} | {r[4]:.0f} | {r[5]:.0f} | {r[6]:.0f} | {r[7]:.0f} | {r[8]:.0f} / {r[9]:.0f} | {r[10]:.0f} / {r[11]:.0f} | {r[12]} | {r[13]} | {r[14]:.0f} | {r[15]} | {r[16]} | {r[17]} |')
