# SPDX-License-Identifier: MIT
# Extract the display route (D2 evidence) from a device trace: every display control and
# display class allocation the guest sent, in order, with the device's answer and label,
# plus the display BAR0 accesses summarized per register/page.
# Usage: python3 route.py <device-trace.txt> [--all]
import re
import sys
from collections import Counter, OrderedDict

DISPLAY_INTERNAL = {
    0x20800A01, 0x20800A49, 0x20800A4B, 0x20800A4D, 0x20800A54, 0x20800A58, 0x20800A5D,
    0x20800A76, 0x20800A77, 0x20800AC6, 0x20800ADF, 0x20800AF0, 0x20800AF1, 0x20800AF2,
}
DISPLAY_CLASSES = ("NV04_DISPLAY_COMMON", "NVC570", "NVC57", "NVC372", "NVC371", "NVC573")


def is_display(cmd):
    return cmd in DISPLAY_INTERNAL or (cmd >> 16) in (0x0073, 0x5070, 0xC370, 0xC372)


def main():
    path = sys.argv[1]
    ctrl = re.compile(r"ctrl (0x[0-9a-f]+): (.*) -> status (0x[0-9a-f]+) \[(.*)\]$")
    alloc = re.compile(r"alloc: (.*) -> status (0x[0-9a-f]+) \[(.*)\]$")
    regs = Counter()
    n = 0
    for line in open(path, errors="replace"):
        m = ctrl.search(line)
        if m and is_display(int(m.group(1), 16)):
            n += 1
            print(f"{n:4} ctrl {m.group(1)} status {m.group(3)} [{m.group(4)}] {m.group(2)}")
            continue
        m = alloc.search(line)
        if m and any(c in m.group(1) for c in DISPLAY_CLASSES):
            n += 1
            print(f"{n:4} alloc status {m.group(2)} [{m.group(3)}] {m.group(1)}")
            continue
        f = line.split()
        if len(f) >= 6 and f[2] == "bar0" and f[1] in ("R", "W"):
            off = int(f[3], 16)
            if off == 0x21C04 or 0x610000 <= off < 0x6F1000:
                key = f"{f[1]} {off & ~0xFFF if 0x680000 <= off < 0x6E0000 else off:#x}"
                regs[key] += 1
        if "display channel" in line:
            n += 1
            print(f"{n:4} mmio {line.split('E boot: ')[-1].strip()}")
    print("\nDisplay BAR0 accesses (register, or 4 KiB USERD page):")
    for k, v in sorted(regs.items()):
        print(f"  {k}: {v}")


main()
