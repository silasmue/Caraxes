#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Bounded Linux CPU/IRQ census using /proc. No GPU access or service changes."""

import argparse
import json
import os
import time
from pathlib import Path


CPU_FIELDS = ("user", "nice", "system", "idle", "iowait", "irq", "softirq", "steal")


def parse_cpus(text):
    rows = {}
    for line in text.splitlines():
        parts = line.split()
        if parts and (parts[0] == "cpu" or parts[0][3:].isdigit() and parts[0].startswith("cpu")):
            if len(parts) < 5:
                raise ValueError("short CPU accounting row")
            # guest/guest_nice are already included in user/nice; never sum them again.
            values = [int(x) for x in parts[1:9]]
            rows[parts[0]] = dict(zip(CPU_FIELDS, values + [0] * (8 - len(values))))
    if "cpu" not in rows:
        raise ValueError("missing aggregate CPU accounting")
    return rows


def parse_irqs(text):
    rows = {}
    for line in text.splitlines():
        if ":" not in line:
            continue
        key, rest = line.split(":", 1)
        parts = rest.split()
        count = 0
        while count < len(parts) and parts[count].isdigit():
            count += 1
        if count:
            # Also handles single-count ERR/MIS rows and named softirq rows.
            rows[key.strip()] = {"counts": [int(x) for x in parts[:count]],
                                 "label": " ".join(parts[count:])}
    return rows


def parse_process(text):
    left, right = text.index("("), text.rindex(")")
    fields = text[right + 1:].split()
    return {"pid": int(text[:left]), "name": text[left + 1:right], "state": fields[0],
            "user_ticks": int(fields[11]), "system_ticks": int(fields[12]),
            "start_ticks": int(fields[19])}


def processes(root, pids, threads):
    result, missed = {}, 0
    for pid in pids:
        base = root / str(pid)
        paths = [("process", base / "stat")]
        if threads:
            try:
                paths.extend(("thread", p / "stat") for p in (base / "task").iterdir())
            except OSError:
                missed += 1
        for kind, path in paths:
            try:
                row = parse_process(path.read_text())
            except (OSError, ValueError, IndexError):
                missed += 1
                continue
            row.update(kind=kind, owner_pid=pid)
            result[f"{kind}:{pid}:{row['pid']}"] = row
    return {"rows": result, "unreadable_or_vanished": missed}


def snapshot(root, selected, threads, include_processes):
    begin = time.monotonic_ns()
    out = {"unix_before_ns": time.time_ns(),
           "cpus": parse_cpus((root / "stat").read_text())}
    for name in ("interrupts", "softirqs"):
        try:
            text = (root / name).read_text()
            rows = parse_irqs(text)
            out[name] = {"rows": rows, "error": None if rows else "empty or unavailable"}
        except OSError as error:
            out[name] = {"rows": {}, "error": str(error)}
    if include_processes:
        pids = selected or sorted(int(p.name) for p in root.iterdir() if p.name.isdigit())
        out["processes"] = processes(root, pids, threads)
    out.update(mono_before_ns=begin, mono_after_ns=time.monotonic_ns(), unix_after_ns=time.time_ns())
    return out


def elapsed(first, last):
    a = (first["mono_before_ns"] + first["mono_after_ns"]) / 2
    b = (last["mono_before_ns"] + last["mono_after_ns"]) / 2
    return (b - a) / 1e9


def cpu_delta(first, last, seconds, hz):
    result = {}
    for key in first.keys() & last.keys():
        ticks = {f: last[key][f] - first[key][f] for f in CPU_FIELDS}
        negative = [f for f, value in ticks.items() if value < 0]
        result[key] = {"ticks": ticks, "regressed_fields": negative,
                       "percent_of_one_cpu": None if negative else
                       {f: value / hz / seconds * 100 for f, value in ticks.items()}}
    return result


def irq_delta(first, last):
    if first["error"] or last["error"]:
        return {"rows": None, "error": [first["error"], last["error"]]}
    a, b = first["rows"], last["rows"]
    rows, changed = {}, []
    for key in a.keys() & b.keys():
        x, y = a[key], b[key]
        if x["label"] != y["label"] or len(x["counts"]) != len(y["counts"]):
            changed.append(key)
            continue
        diff = [v - u for u, v in zip(x["counts"], y["counts"])]
        if any(v < 0 for v in diff):
            changed.append(key)
        elif sum(diff):
            rows[key] = {"count": sum(diff), "per_cpu_counts": diff, "label": y["label"]}
    return {"rows": rows, "changed_or_regressed": changed,
            "added": sorted(b.keys() - a.keys()), "removed": sorted(a.keys() - b.keys())}


def process_delta(first, last, seconds, hz, top):
    a, b = first["rows"], last["rows"]
    rows, replaced, regressed = [], [], []
    for key in a.keys() & b.keys():
        x, y = a[key], b[key]
        if x["start_ticks"] != y["start_ticks"]:
            replaced.append(key)
            continue
        user, system = y["user_ticks"] - x["user_ticks"], y["system_ticks"] - x["system_ticks"]
        if user < 0 or system < 0:
            regressed.append(key)
            continue
        rows.append(dict(y, user_seconds=user / hz, system_seconds=system / hz,
                         percent_of_one_cpu=(user + system) / hz / seconds * 100))
    process_rows = sorted((r for r in rows if r["kind"] == "process"),
                          key=lambda r: r["percent_of_one_cpu"], reverse=True)
    thread_rows = sorted((r for r in rows if r["kind"] == "thread"),
                         key=lambda r: r["percent_of_one_cpu"], reverse=True)
    return {"top_processes": process_rows[:top], "top_threads": thread_rows[:top],
            "matched_processes": len(process_rows), "matched_threads": len(thread_rows),
            "added": len(b.keys() - a.keys()), "removed": len(a.keys() - b.keys()),
            "replaced": replaced, "regressed": regressed}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=int, choices=range(1, 121), default=30, metavar="1..120")
    parser.add_argument("--pid", type=int, action="append", help="restrict process/thread rows; repeatable")
    parser.add_argument("--threads", action="store_true", help="also read thread counters at endpoints")
    parser.add_argument("--top", type=int, choices=range(1, 101), default=20, metavar="1..100")
    args = parser.parse_args()
    if args.pid and any(p <= 0 for p in args.pid):
        parser.error("--pid must be positive")
    root, hz = Path("/proc"), os.sysconf("SC_CLK_TCK")
    first = snapshot(root, args.pid, args.threads, True)
    previous, intervals = first, []
    deadline = time.monotonic() + args.seconds
    while True:
        time.sleep(min(1.0, max(0.0, deadline - time.monotonic())))
        final = time.monotonic() >= deadline
        current = snapshot(root, args.pid, args.threads, final)
        seconds = elapsed(previous, current)
        intervals.append({"seconds": seconds,
                          "capture_ns": current["mono_after_ns"] - current["mono_before_ns"],
                          "cpu": cpu_delta(previous["cpus"], current["cpus"], seconds, hz)})
        if final:
            break
        previous = current
    seconds = elapsed(first, current)
    result = {"schema": 1, "collector_pid": os.getpid(), "tick_hz": hz,
              "requested_seconds": args.seconds, "observed_seconds": seconds,
              "selected_pids": args.pid, "first": first, "last": current, "intervals": intervals,
              "cpu": cpu_delta(first["cpus"], current["cpus"], seconds, hz),
              "processes": process_delta(first["processes"], current["processes"], seconds, hz, args.top),
              "interrupts": irq_delta(first["interrupts"], current["interrupts"]),
              "softirqs": irq_delta(first["softirqs"], current["softirqs"]),
              "limits": ["Sequential reads; endpoint capture spans bound timestamp skew.",
                         "Process and thread rows overlap; never add their CPU times.",
                         "Processes that start, exit or reuse a PID are excluded from process deltas.",
                         "All process endpoints retained; --top limits only the ranked summary.",
                         "100% means one CPU. Aggregate CPU percentages can exceed 100%.",
                         "Kernel/IRQ counters do not identify a driver stack or prove causality."]}
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
