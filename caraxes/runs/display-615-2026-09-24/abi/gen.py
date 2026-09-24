# SPDX-License-Identifier: MIT
# Generate, compile and run a C program printing sizeof and top-level field offsets of the
# display structs against the SDK headers of one open-gpu-kernel-modules tree. This is
# caraxes/runs/display-ab-2026-09-23/abi/gen.py with the tree, the name list and the output
# as parameters:
#   python3 gen.py <tree> <names.txt> <out.txt>
# The 615.71.09 tree is /home/silas/git/Leandro/vendor/open-gpu-kernel-modules, the 610.57.04
# tree is vendor/open-gpu-kernel-modules of the main checkout.
import re, glob, sys, subprocess, os
root = os.path.join(sys.argv[1], "src/common")
inc = root + "/sdk/nvidia/inc"
work = os.path.dirname(os.path.abspath(sys.argv[3]))
names = [w for l in open(sys.argv[2]) if not l.startswith("#") for w in l.split()]
src = {}
for p in glob.glob(inc + "/**/*.h", recursive=True):
    src[p] = open(p, errors="ignore").read()
out = ['#include <stdio.h>', '#include <stddef.h>', '#include "nvtypes.h"', '#include "nvmisc.h"',
       '#include "nvstatus.h"', '#include "nvlimits.h"', '#include "nvos.h"']
hdrs = set(); body = []
for n in names:
    found = None
    for p, s in src.items():
        m = re.search(r"typedef struct\s+(\w+)?\s*\{((?:[^{}]|\{[^{}]*\})*)\}\s*" + re.escape(n) + r"\s*;", s)
        if m:
            found = (p, m.group(2)); break
    if not found:
        print("missing", n, file=sys.stderr); continue
    p, b = found
    hdrs.add(p[len(inc)+1:])
    b = re.sub(r"/\*.*?\*/", "", b, flags=re.S); b = re.sub(r"//[^\n]*", "", b)
    b = re.sub(r"NV_DECLARE_ALIGNED\(([^,]*),\s*\d+\)", r"\1", b)
    b = re.sub(r"(struct|union)\s*\{[^{}]*\}", "NESTED", b)
    body.append(f'  printf("{n} sizeof %zu\\n", sizeof({n}));')
    for line in b.split(";"):
        line = line.strip()
        if not line or "{" in line or "}" in line: continue
        m = re.match(r"[\w\s\*]+?\s+\**(\w+)\s*(?:\[[^\]]*\])*$", line)
        if m:
            f = m.group(1)
            body.append(f'  printf("{n}.{f} %zu\\n", offsetof({n}, {f}));')
for h in sorted(hdrs): out.append(f'#include "{h}"')
out.append("int main(void){"); out += body; out.append("return 0;}")
csrc, exe = os.path.join(work, "m.c"), os.path.join(work, "m")
open(csrc, "w").write("\n".join(out) + "\n")
cc = ["gcc", "-o", exe, csrc, "-I" + inc, "-I" + inc + "/ctrl",
      "-I" + inc + "/class", "-I" + inc + "/alloc", "-I" + root + "/inc"]
r = subprocess.run(cc, capture_output=True, text=True)
if r.returncode:
    print(r.stderr[:4000]); sys.exit(1)
res = subprocess.run([exe], capture_output=True, text=True).stdout
os.remove(csrc); os.remove(exe)
hdr = ("# SPDX-License-Identifier: MIT\n# gcc-measured sizeof/offsetof of the display control structs, "
       + open(os.path.join(sys.argv[1], "version.mk")).readline().split("=")[1].strip()
       + " SDK headers, x86_64\n# (generator: caraxes/runs/display-615-2026-09-24/abi/gen.py).\n")
open(sys.argv[3], "w").write(hdr + res)
