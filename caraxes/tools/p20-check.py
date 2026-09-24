#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""P20 isolation checker (docs/caraxes/parallel.md item 3a and 3c).

Usage: p20-check.py <A-dir> <B-dir> [host-xid.txt]

Each directory holds one device's evidence: `gpu.log` (the device's stderr with the
reservation, the registered guest RAM chunks and the exit summary) and `shadow.txt` (the
`shadow:` lines of its trace: every sync line carries the extent of all physical
addresses mapped so far, every refused leaf has its own line). For each device the
checker verifies:
  1. every mapped vidmem PA lies in its own reservation and none in the other's;
  2. every sysmem chunk it mapped from is one of its own registered chunks, none of the
     other's;
  3. every refused leaf (the D16/D17 filter) is outside its own ranges, and says whether
     it would have hit the other VM's ranges;
  4. the DMA addresses the device refused (exit summary) are outside the other VM's
     ranges or, if not, reports them;
  5. with host-xid.txt: every Xid names one of the two device pids (contained).
Exit 0 when every check holds, 1 otherwise.
"""
import re
import sys
from pathlib import Path

HEX = r"0x[0-9a-f]+"


def ranges_of(d: Path):
    log = (d / "gpu.log").read_text(errors="replace")
    m = re.search(rf"reserved host VRAM \[({HEX}), ({HEX})\)", log)
    if not m:
        sys.exit(f"{d}: no reservation line in gpu.log")
    res = (int(m.group(1), 16), int(m.group(2), 16))
    chunks = []
    for m in re.finditer(rf"registered guest RAM chunk GPA ({HEX})\.\.({HEX}) -> IOVA ({HEX})\.\.({HEX})", log):
        g, ge, i, _ = (int(x, 16) for x in m.groups())
        chunks.append([g, i, ge - g])
    if not chunks:
        # dma-ranges fallback: gpa:iova:size, hex without 0x
        t = (d / "dma-ranges").read_text().strip() if (d / "dma-ranges").exists() else ""
        for e in filter(None, t.split(",")):
            g, i, s = (int(x, 16) for x in e.split(":"))
            chunks.append([g, i, s])
    pid = None
    m = re.search(r"^pid (\d+)", (d / "pid.txt").read_text(), re.M) if (d / "pid.txt").exists() else None
    if m:
        pid = int(m.group(1))
    return res, chunks, pid, log


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    dirs = [Path(sys.argv[1]), Path(sys.argv[2])]
    info = [ranges_of(d) for d in dirs]
    ok = True
    out = []

    def say(s):
        out.append(s)

    for i, d in enumerate(dirs):
        (rlo, rhi), chunks, _, log = info[i]
        (olo, ohi), ochunks, _, _ = info[1 - i]
        own_iova = {c[1]: c[2] for c in chunks}
        oth_iova = {c[1]: c[2] for c in ochunks}
        name = d.name
        say(f"== {name}: reservation [{rlo:#x}, {rhi:#x}); guest RAM IOVA chunks "
            + " ".join(f"{b:#x}+{s:#x}" for b, s in sorted(own_iova.items())))
        if rlo < ohi and olo < rhi:
            ok = False
            say(f"FAIL reservations overlap: [{rlo:#x},{rhi:#x}) and [{olo:#x},{ohi:#x})")
        for b, s in own_iova.items():
            for ob, os_ in oth_iova.items():
                if b < ob + os_ and ob < b + s:
                    ok = False
                    say(f"FAIL guest RAM IOVA ranges overlap: {b:#x}+{s:#x} / {ob:#x}+{os_:#x}")
        shadow = (d / "shadow.txt").read_text(errors="replace") if (d / "shadow.txt").exists() else ""
        syncs = re.findall(r"mapped PAs so far: vidmem (none|\[(0x[0-9a-f]+), (0x[0-9a-f]+)\)); sysmem chunks ([^\n]*)", shadow)
        lo = hi = None
        sys_bases = {}
        for whole, a, b, sc in syncs:
            if a:
                a, b = int(a, 16), int(b, 16)
                lo = a if lo is None else min(lo, a)
                hi = b if hi is None else max(hi, b)
            for tok in sc.split():
                if ":" in tok and tok != "none":
                    base, n = tok.split(":")
                    sys_bases[int(base, 16)] = max(sys_bases.get(int(base, 16), 0), int(n))
        say(f"{len(syncs)} shadow sync lines; union of mapped vidmem PAs "
            + (f"[{lo:#x}, {hi:#x})" if lo is not None else "none")
            + "; sysmem chunks mapped from: "
            + (" ".join(f"{b:#x} ({n} maps)" for b, n in sorted(sys_bases.items())) or "none"))
        if lo is not None:
            if not (rlo <= lo and hi <= rhi):
                ok = False
                say("FAIL mapped vidmem outside the own reservation")
            elif lo < ohi and olo < hi:
                ok = False
                say("FAIL mapped vidmem intersects the other VM's reservation")
            else:
                say("ok: every mapped vidmem PA inside the own reservation, none in the other's")
        for b in sys_bases:
            if b not in own_iova:
                ok = False
                say(f"FAIL sysmem chunk {b:#x} is not one of the own registered chunks"
                    + (" (it is the OTHER VM's)" if b in oth_iova else ""))
        if sys_bases and all(b in own_iova for b in sys_bases):
            say("ok: every mapped sysmem chunk is one of the own registered chunks, none of the other's")
        refused = re.findall(r"REFUSED leaf va (0x[0-9a-f]+) -> (Fb|Sys)\(([0-9a-f]+)\) size (0x[0-9a-f]+) kind (0x[0-9a-f]+): ([^(\[]*)", shadow)
        kinds = {}
        for va, ap, pa, size, kind, why in refused:
            pa, size = int(pa, 16), int(size, 16)
            if ap == "Fb":
                own = rlo <= pa and pa + size <= rhi
                oth = pa < ohi and olo < pa + size
            else:
                own = any(b <= pa < b + s for b, s in own_iova.items())
                oth = any(b <= pa < b + s for b, s in oth_iova.items())
            key = (ap, pa, size, kind, why.strip(), own, oth)
            kinds[key] = kinds.get(key, 0) + 1
            if own and "compressible" not in why:
                ok = False
        say(f"{len(refused)} refused-leaf lines, {len(kinds)} distinct:")
        for (ap, pa, size, kind, why, own, oth), n in sorted(kinds.items()):
            where = "OWN range (unexpected)" if own else ("the OTHER VM's range" if oth else "neither VM's range")
            say(f"  {ap} {pa:#x} size {size:#x} kind {kind} x{n}: {why}; lies in {where}")
        m = re.search(r"dma table: (\d+) DMA address\(es\) outside the table refused(?: \(first: ([^)]*)\))?", log)
        if m:
            addrs = [int(x, 16) for x in (m.group(2) or "").split()]
            hits = [a for a in addrs if any(b <= a < b + s for b, s in oth_iova.items())]
            say(f"device DMA refusals: {m.group(1)} (first {len(addrs)} checked: "
                + (f"{len(hits)} in the other VM's IOVA ranges" if hits else "none in the other VM's IOVA ranges") + ")")
        m = re.search(r"fb: reservation \[[^)]*\); accesses outside it: (\d+) reads, (\d+) writes", log)
        if m:
            say(f"FB store accesses outside the reservation: {m.group(1)} reads, {m.group(2)} writes")
        m = re.search(r"vidmem PTE kinds walked[^\n]*", log)
        if m:
            say("comptag audit: " + m.group(0))
            if "COMPRESSIBLE KIND OR NON-ZERO" in m.group(0):
                ok = False
    if len(sys.argv) > 3:
        pids = {info[0][2], info[1][2]} - {None}
        xids = [l for l in Path(sys.argv[3]).read_text().splitlines() if "Xid" in l and not l.startswith("#")]
        bad = [l for l in xids if not any(f"pid={p}," in l for p in pids)]
        say(f"== host Xid lines: {len(xids)}; device pids {sorted(pids)}; not contained: {len(bad)}")
        for l in bad:
            say("  NOT CONTAINED: " + l)
        ok = ok and not bad
    say("RESULT: " + ("PASS" if ok else "FAIL"))
    print("\n".join(out))
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
