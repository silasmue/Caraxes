#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""The launch chain of one or more series (docs/caraxes/launch-path.md).

Reads the device logs a series of caraxes/runs/opt-launch-2026-09-25/bench.sh kept
(<run>/<metric>/v2-device-rep<r>.txt, with the exit summary's `launch timing:` lines of
CARAXES_LAUNCH_TIMING=1) and the guest's results (<run>/<metric>/v2/<metric>-<r>.txt), and
prints per metric the median over the reps of each step's p50 and mean, the reactor counters,
and the guest numbers (nvprobe's ms/launch, bench-cu launch's percentiles, rlprobe, copy, PPO).

    launch-chain.py RUN [RUN ...]
"""
import re
import statistics
import sys
from pathlib import Path

STEP = re.compile(r"launch timing: (?P<step>[^:]+): n (?P<n>\d+), mean (?P<mean>[\d.]+) us, "
                  r"p10 (?P<p10>[\d.]+) us, p50 (?P<p50>[\d.]+) us, p90 (?P<p90>[\d.]+) us, "
                  r"p99 (?P<p99>[\d.]+) us")
HEAD = re.compile(r"launch timing: (\d+) ring\(s\) by the reactor \((\d+) after a blocking wait\), "
                  r"(\d+) host doorbell\(s\); host events by the age of the last host doorbell: "
                  r"(\d+) none yet, (\d+) < 1 ms, (\d+) < 100 ms, (\d+) older")
MSI = re.compile(r"MSIs: (\d+) for raises")
FLUSH = re.compile(r"launch fast: BAR1 flushes (\d+) done, (\d+) skipped")
LEAN = re.compile(r"ran-ahead checks skipped with no plan pending: (\d+)")
DIRECT = re.compile(r"intr page direct: (\d+) raise")
GATE = re.compile(r"intr gate: .*?; (\d+) event\(s\) delivered at once, (\d+) held, (\d+) delivery")
WINDOW = re.compile(r"doorbell poll \(tokens\): hybrid \(([^)]*)\)")


def med(v):
    return statistics.median(v) if v else float("nan")


def guest(metric, text):
    """The guest's numbers of one result file."""
    out = {}
    if m := re.search(r"MSPERLAUNCH=([\d.]+)", text):
        out[f"{metric} ms/launch"] = float(m.group(1))
    if m := re.search(r"^# STEPCHAIN (.*)$", text, re.M):
        for k, v in re.findall(r"(\w+)_p50_us=([\d.]+)", m.group(1)):
            out[f"step {k} p50 us"] = float(v)
    if m := re.search(r"^LAUNCH (.*)$", text, re.M):
        for k, v in re.findall(r"(\w+)=([\d.]+)", m.group(1)):
            if k.endswith("_us") and k.split("_")[1] in ("p50", "p90", "mean"):
                out[k] = float(v)
    if m := re.search(r"stage4: \d+ episodes in ([\d.]+)s", text):
        out["rl stage4 s"] = float(m.group(1))
    if m := re.search(r"transfers: .*\(~(\d+)/s\)", text):
        out["rl xfer/s"] = float(m.group(1))
    if m := re.search(r"^steps/s:\s+([\d.]+)", text, re.M):
        out["ppo steps/s"] = float(m.group(1))
    for d, mem, size, g in re.findall(r"^BW dir=(\w+) mem=(\w+) size_mib=(\d+) .*gbps=([\d.]+)", text, re.M):
        if (d, mem, size) in (("h2d", "pinned", "1"), ("h2d", "pageable", "1"), ("h2d", "pinned", "16"),
                              ("d2d", "device", "16")):
            out[f"{d} {mem} {size}M GB/s"] = float(g)
    if m := re.search(r"# MEAS .*?dev_cpu_ms=(\d+)", text):
        out["dev_cpu_ms"] = float(m.group(1))
    return out


def run(root):
    root = Path(root)
    print(f"## {root.name}\n")
    # A series of bench.sh has <run>/<metric>/; one of ab.sh has <run>/<A|B>/<life>/.
    dirs = [p for p in root.iterdir() if p.is_dir()]
    dirs = [q for p in dirs for q in ([p] if list(p.glob("v2-device-rep*.txt"))
                                      else [x for x in p.iterdir() if x.is_dir()])]
    for mdir in sorted(dirs):
        logs = sorted(mdir.glob("v2-device-rep*.txt"))
        if not logs:
            continue
        steps, heads, extra = {}, [], []
        for log in logs:
            t = log.read_text(errors="replace")
            for m in STEP.finditer(t):
                steps.setdefault(m["step"], []).append((float(m["p50"]), float(m["mean"]), int(m["n"])))
            if h := HEAD.search(t):
                heads.append([int(x) for x in h.groups()])
            bits = []
            if m := MSI.search(t):
                bits.append(f"MSIs for raises {m.group(1)}")
            if m := FLUSH.search(t):
                bits.append(f"flushes {m.group(1)} done / {m.group(2)} skipped")
            if m := LEAN.search(t):
                bits.append(f"ran-ahead reads skipped {m.group(1)}")
            if m := DIRECT.search(t):
                bits.append(f"direct completions {m.group(1)}")
            if m := GATE.search(t):
                bits.append(f"gate {m.group(1)} at once / {m.group(2)} held / {m.group(3)} releases")
            if m := WINDOW.search(t):
                bits.append(f"hybrid {m.group(1)}")
            extra.append("; ".join(bits))
        print(f"### {mdir.relative_to(root)} ({len(logs)} device logs)\n")
        print("| step | p50 us (median over reps) | mean us | n per rep |")
        print("|---|---|---|---|")
        for k, v in steps.items():
            print(f"| {k} | {med([x[0] for x in v]):.1f} | {med([x[1] for x in v]):.2f} | "
                  f"{int(med([x[2] for x in v]))} |")
        if heads:
            h = [int(med([x[i] for x in heads])) for i in range(7)]
            print(f"\nrings {h[0]} ({h[1]} after a blocking wait), host doorbells {h[2]}; "
                  f"host events: {h[3]} before any doorbell, {h[4]} < 1 ms, {h[5]} < 100 ms, "
                  f"{h[6]} older (medians over reps)")
        for i, e in enumerate(extra, 1):
            if e:
                print(f"- rep {i}: {e}")
        res = {}
        for f in sorted(mdir.glob("**/v2/*.txt")):
            metric = re.sub(r"-\d+$", "", f.stem)
            for k, v in guest(metric, f.read_text(errors="replace")).items():
                res.setdefault(k, []).append(v)
        if res:
            print("\nguest: " + ", ".join(
                f"{k} {med(v):g} [{min(v):g}-{max(v):g}]" for k, v in res.items()))
        print()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    for r in sys.argv[1:]:
        run(r)
