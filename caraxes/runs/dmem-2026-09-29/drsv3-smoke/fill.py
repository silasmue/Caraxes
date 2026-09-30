# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Fill the free VRAM from a CUDA context and hold it. With "grab", keep taking every 2 MiB that
# becomes free, every 10 ms, as a game that sizes its pools to the free memory does.
# Usage: fill.py SECONDS [grab]
import ctypes, sys, time
cu = ctypes.CDLL("libcuda.so.1")
assert cu.cuInit(0) == 0
dev = ctypes.c_int(); assert cu.cuDeviceGet(ctypes.byref(dev), 0) == 0
ctx = ctypes.c_void_p(); assert cu.cuCtxCreate_v2(ctypes.byref(ctx), 0, dev) == 0
held = 0; ptrs = []
def take(step):
    global held
    p = ctypes.c_uint64()
    if cu.cuMemAlloc_v2(ctypes.byref(p), ctypes.c_size_t(step)) != 0:
        return False
    ptrs.append(p); held += step
    return True
for step in (64 << 20, 8 << 20, 2 << 20):
    while take(step):
        pass
print("fill: holding %d MiB" % (held >> 20), flush=True)
end = time.time() + float(sys.argv[1])
grab = len(sys.argv) > 2 and sys.argv[2] == "grab"
start = held
while time.time() < end:
    if grab:
        take(2 << 20)
        time.sleep(0.01)
    else:
        time.sleep(end - time.time())
if grab:
    print("fill: grabbed %d MiB more, holding %d MiB" % ((held - start) >> 20, held >> 20), flush=True)
