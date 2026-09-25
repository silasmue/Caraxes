#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""The forward cache off against on (run.sh off, run.sh on): per nvidia-smi workload window,
the device time of the guest's controls, the host RM control calls, and for the cached
getters (src/fwdcache.rs STATIC) the guest controls against the host calls; the cache's
window rows at exit. Reads the SIGUSR2 dumps through profile-rank.py --tsv.

Usage: compare.py [OFF_DIR ON_DIR] (default off/ and on/ beside this script)
"""
import csv, io, lzma, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
RANK = os.path.join(ROOT, "caraxes", "tools", "profile-rank.py")
SRC = os.path.join(ROOT, "crates", "vfio-user-nvgpu", "src", "fwdcache.rs")
STATIC = {int(m.group(1).replace("_", ""), 16): m.group(2)
          for m in re.finditer(r'\((0x[0-9A-Fa-f_]+), "(\w+)", "', open(SRC).read())}
WORK = ["smi-q-timed", "smi-query-timed", "smi-dmon"]


def rows(run, label):
    marks = [l.split("\t") for l in open(os.path.join(run, "marks.tsv")).read().split("\n") if l]
    n = {m[1]: int(m[0]) for m in marks}[label]
    p = os.path.join(run, "profile", "dev.%d.tsv.xz" % n)
    b = os.path.join(run, "profile", "dev.%d.tsv.xz" % (n - 1))
    env = dict(os.environ, CARAXES_PROFILE_ABI="gsp-610.57.04")
    out = subprocess.run([sys.executable, RANK, p, "--base", b, "--top", "100000", "--tsv",
                          "--cost", "device"], capture_output=True, text=True, env=env).stdout
    return list(csv.DictReader(io.StringIO(out), delimiter="\t"))


def summary(run, label):
    r = rows(run, label)
    ctl = [x for x in r if x["kind"] == "control"]
    host = [x for x in r if x["kind"] == "host" and x["key"].startswith("0x2a:")]
    ms = lambda xs: sum(int(x["total_ns"]) for x in xs) / 1e6
    cnt = lambda xs: sum(int(x["count"]) for x in xs)
    sc = [x for x in ctl if int(x["key"], 16) in STATIC]
    sh = [x for x in host if int(x["key"].split(":")[1], 16) in STATIC]
    return dict(ctl_n=cnt(ctl), ctl_ms=ms(ctl), host_n=cnt(host), host_ms=ms(host),
                st_ctl_n=cnt(sc), st_ctl_ms=ms(sc), st_host_n=cnt(sh), st_host_ms=ms(sh))


def main():
    off, on = (sys.argv[1:3] if len(sys.argv) > 2 else
               (os.path.join(HERE, "off"), os.path.join(HERE, "on")))
    print("| Workload | Run | Guest controls | Device ms | Host RM controls | Host ms | "
          "Cached getters: guest controls | their host calls | their device ms |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|")
    for w in WORK:
        for name, run in (("off", off), ("on", on)):
            s = summary(run, w)
            print("| %s | %s | %d | %.1f | %d | %.1f | %d | %d | %.2f |" % (
                w, name, s["ctl_n"], s["ctl_ms"], s["host_n"], s["host_ms"],
                s["st_ctl_n"], s["st_host_n"], s["st_ctl_ms"]))
    for name, run in (("off", off), ("on", on)):
        p = os.path.join(run, "profile", "dev.exit.tsv.xz")
        if os.path.exists(p):
            for l in lzma.open(p, "rt"):
                if "forward cache" in l:
                    f = l.rstrip("\n").split("\t")
                    if f[0] == "device":
                        print("%s, device lifetime: %s = %s" % (name, f[4], f[6]))


if __name__ == "__main__":
    main()
