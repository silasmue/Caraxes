# In the guest, with the display reserve build loaded and GNOME up: fill the free VRAM from a
# CUDA process, then ask nvidia-drm for a 1920x1080 XRGB8888 dumb buffer (an NVKMS SCANOUT
# allocation of 0x870000 bytes), free it, and show the reserve's log and the VRAM numbers.
set -u
t() { echo "$(date +%T) $*"; }
used() { nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader; }
mkdir -p /tmp/drsv && cd /tmp/drsv
cat > dumb.c <<'C'
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <sys/ioctl.h>
#include <drm/drm.h>
#include <drm/drm_mode.h>
int main(int argc, char **argv)
{
    int fd = open("/dev/dri/card0", O_RDWR);
    struct drm_mode_create_dumb c = { .width = 1920, .height = 1080, .bpp = 32 };
    struct drm_mode_destroy_dumb d;
    int hold = argc > 1 ? atoi(argv[1]) : 3;
    if (fd < 0) { perror("open"); return 2; }
    if (ioctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &c)) { printf("dumb 1920x1080: FAILED errno %d (%s)\n", errno, strerror(errno)); return 1; }
    printf("dumb 1920x1080: ok handle %u pitch %u size 0x%llx\n", c.handle, c.pitch, (unsigned long long)c.size);
    sleep(hold);
    d.handle = c.handle;
    ioctl(fd, DRM_IOCTL_MODE_DESTROY_DUMB, &d);
    printf("dumb destroyed\n");
    return 0;
}
C
sed -i 's|#include <errno.h>|#include <errno.h>\n#include <stdlib.h>|' dumb.c
gcc -O2 -o dumb dumb.c || exit 1
cat > fill.py <<'P'
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
P
t "start: $(used)"
python3 fill.py 60 > fill.log 2>&1 &
FP=$!
sleep 15; cat fill.log; t "filled: $(used)"
sudo dmesg -C
t "dumb 1: $(sudo ./dumb 3)"
sleep 2; t "after dumb 1 freed: $(used)"
sleep 5; t "dumb 2: $(sudo ./dumb 3)"
sleep 2; t "after dumb 2 freed: $(used)"
sudo dmesg | grep -E "display reserve|NVKMS|Xid" | tail -20
kill $FP 2>/dev/null; wait $FP 2>/dev/null
sleep 3; t "filler gone: $(used)"
sudo dmesg | grep -E "display reserve" | tail -4
