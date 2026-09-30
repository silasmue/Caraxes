#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Per-thread CPU times of a process, for the trap census (2026-09-30).

  taskstat.py snap PID OUT.json      record utime, stime and guest_time (clock ticks) per thread
  taskstat.py diff A.json B.json     per thread group (name without digits): % of a core over
                                     the interval; for vCPU threads also host % (utime + stime
                                     - guest_time: KVM exit handling and Cloud Hypervisor's own
                                     work on the vCPU thread) and guest %.
Reads only /proc/PID/task/*/stat and comm.
"""
import json
import os
import re
import sys
import time


def snap(pid):
    out = {"t": time.time(), "hz": os.sysconf("SC_CLK_TCK"), "threads": {}}
    base = f"/proc/{pid}/task"
    for tid in os.listdir(base):
        try:
            with open(f"{base}/{tid}/stat") as f:
                s = f.read()
            with open(f"{base}/{tid}/comm") as f:
                comm = f.read().strip()
        except OSError:
            continue
        rest = s[s.rindex(")") + 2:].split()
        # fields after the comm: rest[0] is field 3 (state); utime 14, stime 15, guest_time 43
        ut, st, gt = int(rest[11]), int(rest[12]), int(rest[40])
        out["threads"][tid] = {"comm": comm, "utime": ut, "stime": st, "guest": gt}
    return out


def diff(a, b):
    dt = b["t"] - a["t"]
    hz = b["hz"]
    groups = {}
    for tid, t1 in b["threads"].items():
        t0 = a["threads"].get(tid, {"utime": 0, "stime": 0, "guest": 0})
        g = re.sub(r"\d+$", "", t1["comm"]) or t1["comm"]
        d = groups.setdefault(g, {"n": 0, "cpu": 0.0, "host": 0.0, "guest": 0.0})
        d["n"] += 1
        cpu = (t1["utime"] - t0["utime"] + t1["stime"] - t0["stime"]) / hz
        gu = (t1["guest"] - t0["guest"]) / hz
        d["cpu"] += cpu
        d["guest"] += gu
        d["host"] += cpu - gu
    rows = []
    for g, d in sorted(groups.items(), key=lambda kv: -kv[1]["cpu"]):
        if d["cpu"] <= 0:
            continue
        rows.append(f"{g:24s} x{d['n']:<3d} cpu {100*d['cpu']/dt:7.1f} %  host {100*d['host']/dt:7.1f} %  guest {100*d['guest']/dt:7.1f} %")
    return f"interval {dt:.1f} s (percent of one core)\n" + "\n".join(rows)


def main():
    if sys.argv[1] == "snap":
        with open(sys.argv[3], "w") as f:
            json.dump(snap(sys.argv[2]), f)
    elif sys.argv[1] == "diff":
        with open(sys.argv[2]) as f:
            a = json.load(f)
        with open(sys.argv[3]) as f:
            b = json.load(f)
        print(diff(a, b))
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
