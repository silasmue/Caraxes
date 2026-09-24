#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""bench-summary: tables for docs/caraxes/benchmark.md from bench-run.sh output.

Usage: bench-summary.py TARGETDIR... [--busy 0.2]
Each TARGETDIR is SERIES/<target> written by caraxes/tools/bench-run.sh (series or startup);
the target (native, v1, v2) is its basename, one column per argument in the given order.
Per value: median over the reps, spread = min-max, n; ratio = median / median of the native
column of the same series (the run interleaved with it), else of the series whose name is a
prefix of this one followed by '-', else of the first native column. A run counts as "loaded" when another VM's device process (vfio-user-nvgpu or
vhost-user-nvrm of a VM that is not the target) used more than --busy (fraction of one CPU,
default 0.2) during it: that VM was active at the same time. The host load average at
the start of each run is summarised too. Prints Markdown.
"""
import re, sys, statistics
from pathlib import Path
from collections import defaultdict

args = sys.argv[1:]
busy_frac = 0.2
if "--busy" in args:
    i = args.index("--busy"); busy_frac = float(args[i + 1]); del args[i:i + 2]

# data[target][(metric, key)] = [values]; meta per target
data = defaultdict(lambda: defaultdict(list))
loaded = defaultdict(lambda: [0, 0])  # target -> [loaded runs, runs]
loadavg = defaultdict(list)
src = {}

def num(s):
    return float(s)

def parse(target, f):
    t = f.read_text(errors="replace")
    m = f.stem.rsplit("-", 1)[0]
    d = data[target]
    def put(k, v, unit, better):
        d[(m, k, unit, better)].append(num(v))
    if m.startswith(("launch-", "short-")):
        x = re.search(r"MSPERLAUNCH=([0-9.]+)", t)
        if x: put("ms/launch", x.group(1), "ms", "low")
    elif m == "copy":
        for x in re.finditer(r"BW dir=(\w+) mem=(\w+) size_mib=(\d+) .* gbps=([0-9.]+)", t):
            put(f"{x.group(1)} {x.group(2)} {x.group(3)} MiB", x.group(4), "GB/s", "high")
    elif m == "mm":
        for x in re.finditer(r"N=(\d+)\s+([0-9.]+)ms/mm\s+([0-9.]+) GFLOP/s", t):
            put(f"N={x.group(1)}", x.group(3), "GFLOP/s", "high")
    elif m == "rl":
        x = re.search(r"stage4:.*mean10=([0-9.]+)", t)
        if x: put("mean10 (reward, not a time)", x.group(1), "", "info")
        x = re.search(r"# WALL wall_ms=(\d+) rc=0", t)
        if x: put("process wall (incl. torch import)", int(x.group(1)) / 1000, "s", "low")
        x = re.search(r"stage4: \d+ episodes in ([0-9.]+)s", t)
        if x: put("stage4 (50 training episodes)", x.group(1), "s", "low")
        x = re.search(r"~(\d+)/s", t)
        if x: put("H2D+D2H transfers per second", x.group(1), "1/s", "high")
    elif m == "ppo":
        x = re.search(r"# PPO steps_per_s=(\d+) solve_s=([-0-9.]+) episodes=(\d+) reward100=([0-9.]+)", t)
        if x:
            put("environment steps per second", x.group(1), "1/s", "high")
            if float(x.group(2)) >= 0:
                put("time to solved (mean100 >= 475)", x.group(2), "s", "low")
            put("reward100 at the end (not a time)", x.group(4), "", "info")
    elif m == "init":
        x = re.search(r"INIT cuinit_ms=([0-9.]+) devget_ms=[0-9.]+ ctxcreate_ms=([0-9.]+) ctxdestroy_ms=([0-9.]+)", t)
        if x:
            put("cuInit", x.group(1), "ms", "low"); put("cuCtxCreate", x.group(2), "ms", "low")
            put("cuCtxDestroy", x.group(3), "ms", "low")
    elif m in ("time0", "time1"):
        x = re.search(r"TIME wall_s=([0-9.]+) user_s=([0-9.]+) sys_s=([0-9.]+)", t)
        if x:
            what = "nvprobe 0 (cuInit)" if m == "time0" else "nvprobe 1 (cuInit+ctx)"
            put(f"{what} wall", num(x.group(1)) * 1000, "ms", "low")
            put(f"{what} user+sys", (num(x.group(2)) + num(x.group(3))) * 1000, "ms", "low")
    elif m == "ioctl":
        for k in ("p50_us", "p99_us", "mean_us"):
            x = re.search(rf"ioctlping .*{k}=([0-9.]+)", t)
            if x: put(f"CHECK_VERSION_STR {k[:-3]}", x.group(1), "us", "low")
    elif m == "ctrl":
        for x in re.finditer(r"ctrlping bytes=(\d+) n=\d+ fails=0 p50_us=([0-9.]+) p90_us=[0-9.]+ p99_us=([0-9.]+)", t):
            put(f"RM_CONTROL {x.group(1)} B p50", x.group(2), "us", "low")
            put(f"RM_CONTROL {x.group(1)} B p99", x.group(3), "us", "low")
    elif m == "churn":
        for x in re.finditer(r"CHURN blocks=(\d+) block_mib=(\d+) cycles=\d+ alloc_med_us=([0-9.]+) free_med_us=([0-9.]+) cycle_med_ms=([0-9.]+)", t):
            p = f"{x.group(1)}x{x.group(2)} MiB"
            put(f"{p} cuMemAlloc", x.group(3), "us", "low"); put(f"{p} cuMemFree", x.group(4), "us", "low")
            put(f"{p} cycle", x.group(5), "ms", "low")
    elif m == "startup":
        x = re.search(r"STARTUP load_ms=(\d+) load_to_cuinit_ms=(\d+)", t)
        if x:
            put("module load", x.group(1), "ms", "low"); put("load to first cuInit", x.group(2), "ms", "low")
    # device process CPU and RSS over the run
    x = re.search(r"# MEAS dev_pid=(\S+) dev_cpu_ms=(\d+) dev_rss_hwm_kb=(\S+)/(\S+) vmm_cpu_ms=(\d+) others: (.*)", t)
    if x:
        if x.group(1) != "-":
            d[("cpu", f"{m} device CPU", "ms", "low")].append(int(x.group(2)))
            d[("cpu", f"{m} VMM CPU", "ms", "low")].append(int(x.group(5)))
            if x.group(4) not in ("-", ""):
                d[("rss", "device HWM", "MiB", "low")].append(int(x.group(4)) / 1024)
                d[("rss", "device RSS after run", "MiB", "low")].append(int(x.group(3)) / 1024)
        wall = re.search(r"# WALL wall_ms=(\d+)", t)
        wall = max(int(wall.group(1)), 1) if wall else None
        busy = [int(v) for n, v in re.findall(r"(\S+)-dev=(\d+)ms", x.group(6))]
        la = re.search(r"# loadavg ([0-9.]+)", t)
        if la: loadavg[target].append(float(la.group(1)))
        loaded[target][1] += 1
        if wall and any(v / wall > busy_frac for v in busy):
            loaded[target][0] += 1

# One column per argument. A column's label is its target, plus its series directory when
# the same target appears twice (two native references); its ratio uses the native column
# of the same series directory, else the first native column.
cols = []
for a in args:
    p = Path(a)
    cols.append((p.name, p.parent.name, str(p)))
dup = {t for t, _, _ in cols if [c[0] for c in cols].count(t) > 1}
targets = []
for t, ser, path in cols:
    lab = f"{t} ({ser})" if t in dup else t
    targets.append(lab)
    src[lab] = path
    for f in sorted(Path(path).glob("*-*.txt")):
        parse(lab, f)
natref = {}
for (t, ser, _), lab in zip(cols, targets):
    if t == "native":
        continue
    same = [l for (t2, s2, _), l in zip(cols, targets) if t2 == "native" and s2 == ser]
    pref = [l for (t2, s2, _), l in zip(cols, targets) if t2 == "native" and ser.startswith(s2 + "-")]
    anyn = [l for (t2, _, _), l in zip(cols, targets) if t2 == "native"]
    natref[lab] = (same or pref or anyn or [None])[0]

def fmt(v):
    if v >= 100: return f"{v:.0f}"
    if v >= 10: return f"{v:.1f}"
    if v >= 1: return f"{v:.2f}"
    return f"{v:.3f}"

def cell(vals):
    if not vals: return "n/a"
    md = statistics.median(vals)
    return f"{fmt(md)} [{fmt(min(vals))}-{fmt(max(vals))}] n={len(vals)}"

keys = []
for t in targets:
    for k in data[t]:
        if k not in keys: keys.append(k)
groups = defaultdict(list)
for k in keys: groups[k[0]].append(k)

print("Sources: " + ", ".join(f"{t}={src[t]}" for t in targets))
print("Runs during which another VM's device process used more than %d%% of one CPU (that VM was active): " % (busy_frac * 100)
      + ", ".join(f"{t} {loaded[t][0]}/{loaded[t][1]}" for t in targets))
print("Host 1-min load average at run start, median [min-max]: "
      + ", ".join(f"{t} {cell(loadavg[t])}" for t in targets))
print()
TITLES = [
    ("launch-auto", "(1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)"),
    ("launch-blocking", "(1) Kernel launch, 30M-cycle spin kernel, sched=blocking"),
    ("short-auto", "(1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto"),
    ("short-blocking", "(1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking"),
    ("copy", "(2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)"),
    ("mm", "(3) Matmul throughput, fp32 torch.mm (mmsweep)"),
    ("rl", "(4) rlprobe, 50 REINFORCE episodes"),
    ("ppo", "(4) PPO CartPole, 128 vector environments, one H2D and one synchronous D2H per step (pporl, 60 s)"),
    ("startup", "(5) Module load to first successful cuInit (in the guest)"),
    ("init", "(5) cuInit / cuCtxCreate inside one process (bench-cu init)"),
    ("time0", "(5) nvprobe 0 process, bash time"),
    ("time1", "(5) nvprobe 1 process, bash time"),
    ("ioctl", "(6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR"),
    ("ctrl", "(6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size"),
    ("churn", "(7) VRAM churn, 20 cycles (bench-cu churn)"),
    ("cpu", "(8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run"),
    ("rss", "(8) Device process memory (over all runs)"),
]
order = [g for g, _ in TITLES if g in groups] + [g for g in groups if g not in dict(TITLES) and g != "load"]
for g in order:
    ks = groups[g]
    print(f"### {dict(TITLES).get(g, g)}\n")
    hdr = "| value | unit | " + " | ".join(targets) + " | " + " | ".join(f"{t} / {natref[t]}" for t in targets if t in natref) + " |"
    print(hdr); print("|" + "---|" * (hdr.count("|") - 1))
    for k in ks:
        _, name, unit, better = k
        row = [cell(data[t].get(k, [])) for t in targets]
        rat = []
        for t in targets:
            if t not in natref: continue
            nat = data[natref[t]].get(k) if natref[t] else None
            v = data[t].get(k)
            rat.append(f"{statistics.median(v) / statistics.median(nat):.2f}" if v and nat and statistics.median(nat) else "-")
        print(f"| {name} | {unit}{' (' + better + ' is better)' if better in ('low','high') else ''} | " + " | ".join(row) + " | " + " | ".join(rat) + " |")
    print()
