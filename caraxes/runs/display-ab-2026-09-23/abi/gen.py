# SPDX-License-Identifier: MIT
# Generate, compile and run a C program printing sizeof and top-level field offsets
# of the display structs against the 610.57.04 SDK headers.
import re, glob, sys, subprocess, os
root = "/home/silas/git/Caraxes/vendor/open-gpu-kernel-modules/src/common"
inc = root + "/sdk/nvidia/inc"
here = os.path.dirname(os.path.abspath(__file__))
names = [w for l in open(os.path.join(here, "names.txt")) if not l.startswith("#") for w in l.split()]
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
open(os.path.join(here, "m.c"), "w").write("\n".join(out) + "\n")
cc = ["gcc", "-o", os.path.join(here, "m"), os.path.join(here, "m.c"), "-I" + inc, "-I" + inc + "/ctrl",
      "-I" + inc + "/class", "-I" + inc + "/alloc", "-I" + root + "/inc"]
r = subprocess.run(cc, capture_output=True, text=True)
if r.returncode:
    print(r.stderr[:4000]); sys.exit(1)
res = subprocess.run([os.path.join(here, "m")], capture_output=True, text=True).stdout
open(os.path.join(here, "layouts.txt"), "w").write(res)
print(res)
