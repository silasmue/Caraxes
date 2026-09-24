#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""lab-ingest.py: file a lab-kit result tarball at home and say what it means for the chip
tables (docs/caraxes/gsp-plan/lab-kit.md section 6).

  python3 caraxes/tools/lab-ingest.py TARBALL [--chip NAME] [--file] [--report FILE]

What it does, in order (nothing is written without --file, apart from --report):
  1. checks TARBALL against TARBALL.sha256 and every file against the SHA256SUMS inside;
  2. runs the identifier check again on the unpacked files: generic e-mail, MAC and UUID shapes
     in every text file (the kit's scrub replaced the machine's own values; a hit here is either a
     value the kit did not know, or a new file kind), and refuses to file on a hit;
  3. reads the sweep (sweep/sweep-user.txt, sweep-priv.txt) and compares it with
     crates/vfio-user-nvgpu/chips/<chip>/chip.toml `[features]`:
       - a family named absent whose getters the card answers NV_OK: the card has the feature,
         or answers the empty NV_OK that `empty_ok` records;
       - a family named present whose getters the card answers NV_ERR_NOT_SUPPORTED;
       - per category A getter the card answers NV_OK: a candidate row for chips/<chip>/controls.toml
         (the fields are named by hand from the struct; this lists id, name, size and bytes);
  4. with --file: copies the result to caraxes/reference/<chip>/lab-kit-<stamp>/ (the traces go
     to matrix/traces/<driver>/<chip>/ with the name of the probe, the layout of the native traces
     of the RTX 2070, which stay out of git as matrix/traces/610.57.04/ does).

Not done yet (named so the next step is clear): turning the sweep's bytes into named
controls.toml fields (needs the layouts of caraxes/tools/ctrl-layouts.py for each struct), and
feeding the traces to Leandro-Test's `ioctl-matrix.sh catalog`.

Python 3.11 or later (tomllib), standard library only. No network, no GPU.
"""

import argparse
import hashlib
import os
import re
import shutil
import sys
import tarfile
import tempfile
import tomllib

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
CHIPS = os.path.join(ROOT, "crates", "vfio-user-nvgpu", "chips")
NV_OK, NOT_SUPPORTED = 0x0, 0x56
GENERIC = [
    ("e-mail", re.compile(rb"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")),
    ("mac", re.compile(rb"\b(?:[0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}\b")),
    ("uuid", re.compile(rb"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b")),
]
TEXT = (".txt", ".csv", ".xml", ".md", ".log", ".jsonl", ".tsv", ".out")


def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def check_sums(tb, d):
    side = tb + ".sha256"
    if os.path.exists(side):
        want = open(side).read().split()[0]
        if sha256(tb) != want:
            sys.exit("lab-ingest: %s does not match %s" % (tb, side))
    bad = []
    for line in open(os.path.join(d, "SHA256SUMS")):
        want, name = line.rstrip("\n").split("  ", 1)
        if sha256(os.path.join(d, name)) != want:
            bad.append(name)
    if bad:
        sys.exit("lab-ingest: sums differ: %s" % ", ".join(bad))


def identifier_check(d):
    hits = []
    for dp, _, files in os.walk(d):
        for fn in files:
            if not fn.endswith(TEXT):
                continue
            data = open(os.path.join(dp, fn), "rb").read()
            for kind, rx in GENERIC:
                if rx.search(data):
                    hits.append((os.path.relpath(os.path.join(dp, fn), d), kind))
    return hits


def read_sweep(d):
    """name -> (id, size, status, answer) of the plain rows; index and display rows kept apart."""
    rows = {}
    for fn in ("sweep-user.txt", "sweep-priv.txt"):
        p = os.path.join(d, "sweep", fn)
        if not os.path.exists(p):
            continue
        for line in open(p):
            if line.startswith("#") or line.startswith("error"):
                continue
            f = line.rstrip("\n").split("\t")
            if len(f) < 5 or f[3] == "-":
                continue
            st = int(f[3], 16)
            # the privileged pass overrides the user pass's INSUFFICIENT_PERMISSIONS
            rows[f[1]] = (int(f[0], 16), int(f[2]), st, f[4])
    return rows


def read_list(d):
    """name -> (category, family) from the sweep list the kit carried."""
    out = {}
    for fn in os.listdir(os.path.join(d, "sweep")):
        if fn.startswith("sweep-") and fn.endswith(".tsv"):
            for line in open(os.path.join(d, "sweep", fn)):
                if line.startswith("#"):
                    continue
                f = line.rstrip("\n").split("\t")
                out[f[1]] = (f[4], f[5])
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("tarball")
    ap.add_argument("--chip", help="chip directory under chips/ (default: from the tarball name)")
    ap.add_argument("--file", action="store_true", help="copy into caraxes/reference and matrix/traces")
    ap.add_argument("--report", help="write the report here as well")
    a = ap.parse_args()
    m = re.match(r"lab-kit-([a-z0-9-]+?)-(\d{8}-\d{6})\.tar\.gz$", os.path.basename(a.tarball))
    if not m:
        sys.exit("lab-ingest: %s is no lab-kit result name" % a.tarball)
    chip, stamp = a.chip or m.group(1), m.group(2)
    rep = []
    with tempfile.TemporaryDirectory() as t:
        with tarfile.open(a.tarball) as tf:
            tf.extractall(t, filter="data")
        d = os.path.join(t, os.listdir(t)[0])
        check_sums(a.tarball, d)
        rep.append("# lab-ingest %s, chip %s" % (os.path.basename(a.tarball), chip))
        rep.append("sums: ok")
        hits = identifier_check(d)
        for p, k in hits:
            rep.append("IDENTIFIER SHAPE %s in %s" % (k, p))
        sweep, lst = read_sweep(d), read_list(d)
        chipfile = os.path.join(CHIPS, chip, "chip.toml")
        feats = {}
        if os.path.exists(chipfile):
            feats = tomllib.load(open(chipfile, "rb")).get("features", {})
        else:
            rep.append("no %s: the family comparison is skipped" % os.path.relpath(chipfile, ROOT))
        absent, present = set(feats.get("absent", [])), set(feats.get("present", []))
        byfam = {}
        for name, (cid, size, st, ans) in sweep.items():
            cat, fam = lst.get(name.split("[")[0], ("?", "-"))
            if fam != "-":
                byfam.setdefault(fam, []).append((name, st))
        rep.append("")
        rep.append("## feature families: the card's statuses against chip.toml [features]")
        for fam in sorted(byfam):
            sts = byfam[fam]
            ok = [n for n, s in sts if s == NV_OK]
            ns = [n for n, s in sts if s == NOT_SUPPORTED]
            mark = "absent" if fam in absent else "present" if fam in present else "unlisted"
            verdict = ""
            if mark == "absent" and ok:
                verdict = "CHECK: absent here, the card answers NV_OK (feature present, or empty_ok)"
            if mark == "present" and ns and not ok:
                verdict = "CHECK: present here, the card answers NOT_SUPPORTED"
            rep.append("%-18s %-8s getters %d, NV_OK %d, NOT_SUPPORTED %d %s" % (fam, mark, len(sts), len(ok), len(ns), verdict))
        rep.append("")
        rep.append("## category A getters the card answers NV_OK (candidates for chips/%s/controls.toml)" % chip)
        for name, (cid, size, st, ans) in sorted(sweep.items(), key=lambda kv: kv[1][0]):
            if lst.get(name.split("[")[0], ("?",))[0] == "A" and st == NV_OK:
                rep.append("%#010x %s size %d: %s" % (cid, name, size, ans[:160]))
        if a.file:
            if hits:
                sys.exit("lab-ingest: identifier shapes found, not filed (see the report)")
            dest = os.path.join(ROOT, "caraxes", "reference", chip, "lab-kit-%s" % stamp)
            if os.path.exists(dest):
                sys.exit("lab-ingest: %s exists" % dest)
            shutil.copytree(d, dest)
            drv = "unknown"
            pf = os.path.join(d, "preflight.txt")
            if os.path.exists(pf):
                mm = re.search(r"driver running: (\S+);", open(pf).read())
                drv = mm.group(1) if mm else drv
            tdir = os.path.join(ROOT, "matrix", "traces", drv, chip)
            os.makedirs(tdir, exist_ok=True)
            for fn in os.listdir(os.path.join(d, "traces")) if os.path.isdir(os.path.join(d, "traces")) else []:
                if fn.endswith((".jsonl", ".tsv", ".out")):
                    shutil.copy2(os.path.join(d, "traces", fn), tdir)
            rep.append("")
            rep.append("filed: %s and %s" % (os.path.relpath(dest, ROOT), os.path.relpath(tdir, ROOT)))
    text = "\n".join(rep) + "\n"
    sys.stdout.write(text)
    if a.report:
        open(a.report, "w").write(text)


if __name__ == "__main__":
    main()
