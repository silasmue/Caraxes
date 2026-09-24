#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Guest side, read-only (docs/caraxes/intr-page.md): the VF CPU interrupt tree as the guest
# sees it through BAR0 (resource0), several samples: leaves, leaf enables, TOP, TOP_EN_SET.
import glob
import mmap
import os
import struct
import time

dev = [d for d in glob.glob("/sys/bus/pci/devices/*")
       if open(d + "/vendor").read().strip() == "0x10de"][0]
fd = os.open(dev + "/resource0", os.O_RDONLY | os.O_SYNC)
m = mmap.mmap(fd, 0x1000, mmap.MAP_SHARED, mmap.PROT_READ, offset=0xB81000)


def rd(o):
    return struct.unpack_from("<I", m, o)[0]


for n in range(int(os.environ.get("N", "5"))):
    leaf = " ".join(f"{rd(4 * i):08x}" for i in range(8))
    en = " ".join(f"{rd(0x200 + 4 * i):08x}" for i in range(8))
    print(f"{time.time():.3f} leaf {leaf} | en_set {en} | top {rd(0x600):x} top_en_set {rd(0x608):08x} "
          f"trigger {rd(0x640):08x} en_clear0 {rd(0x400):x}")
    time.sleep(0.2)
