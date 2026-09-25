#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# docs/caraxes/intr-page.md §11.7 (tab.py, run from caraxes/runs/) and §11.8 (the others,
# with device-trace.txt.xz files as arguments).
import re, os
series = [("B", "display-page-2026-09-24/B-intr-page"), ("C", "display-page-2026-09-24/C-display-page")] + [
    (n, "display-page-2026-09-25/" + d) for n, d in [
        ("D", "D-recommended"), ("E", "E-flush-defer"), ("F", "F-invalidate-page"),
        ("D2", "D2-recommended-rerun"), ("G", "G-vblank-spin"), ("H", "H-no-spin"),
        ("I", "I-windows"), ("I2", "I2-windows-rerun"), ("P1", "P1-flush-page-memory"), ("T1", "T1-tokens-reference"), ("U1", "U1-usermode-page"), ("U2", "U2-usermode-page-spin"), ("W1", "W1-all-windows"), ("W2", "W2-kick-page-tokens"), ("J1", "J1-no-poll-after-trigger"), ("K1", "K1-poll-after-trigger")]]


def pf(t, pat):
    s = 0.0
    for l in t.splitlines():
        if re.match(pat, l):
            s += float(l.split()[-1])
    return s


for w in ["mutter", "weston", "vkcube", "glmark2"]:
    print("##", w)
    print("series all bar2W bar2R time pcfg inv kicks bar1 other | rate")
    for n, d in series:
        p = f"{d}/traps-{w}/trap-table.txt"
        if not os.path.exists(p):
            continue
        t = open(p).read()
        tot = pf(t, r"all trapped accesses")
        b2w = pf(t, r"BAR2 writes, all")
        b2r = pf(t, r"BAR2 reads\s")
        tm = pf(t, r"  BAR0 R 0x00bb008[04]")
        pc = pf(t, r"  BAR0 [RW] 0x00088704")
        inv = pf(t, r"  BAR0 [RW] 0x00b830")
        k = pf(t, r"  BAR0 [RW] 0x0069600[04]")
        b1 = pf(t, r"BAR1 writes, all") + pf(t, r"BAR1 reads")
        oth = tot - b2w - b2r - tm - pc - inv - k - b1
        g = open(f"{d}/traps-{w}/guest.txt").read()
        m = re.search(r"FRAMES .*rate_hz=([\d.]+).*sd_ms=([\d.]+)", g)
        a = re.search(r"fps=([\d.]+)", g) or re.search(r"FPS: (\d+)", g)
        fr = f"{m.group(1)} Hz sd {m.group(2)}" if m else (a.group(1) + " fps" if a else "-")
        lat = re.search(r"per-frame base: window 6 = \d+ \(([\d.]+) Hz\)", t)
        sd = re.search(r"latch intervals.*sd ([\d.]+) ms", t)
        print(n, f"{tot:.2f} {b2w:.2f} {b2r:.2f} {tm:.2f} {pc:.2f} {inv:.2f} {k:.2f} {b1:.2f} {oth:.2f} |", fr,
              "| latch", lat.group(1) if lat else "-", "sd", sd.group(1) if sd else "-")
