# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Fill the free VRAM from a CUDA context and hold it. Usage: fill.py SECONDS
import ctypes, sys, time
cu = ctypes.CDLL("libcuda.so.1")
assert cu.cuInit(0) == 0
dev = ctypes.c_int(); assert cu.cuDeviceGet(ctypes.byref(dev), 0) == 0
ctx = ctypes.c_void_p(); assert cu.cuCtxCreate_v2(ctypes.byref(ctx), 0, dev) == 0
held = 0; ptrs = []
for step in (64 << 20, 8 << 20, 2 << 20):
    while True:
        p = ctypes.c_uint64()
        if cu.cuMemAlloc_v2(ctypes.byref(p), ctypes.c_size_t(step)) != 0: break
        ptrs.append(p); held += step
print("fill: holding %d MiB" % (held >> 20), flush=True)
time.sleep(float(sys.argv[1]))
