// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//
// scanout-oom: a scanout allocation refused because processes filled the video memory.
//
// As DRM master (no display manager or compositor running) it lights the first connected
// connector of an nvidia-drm card with its preferred mode, fills the free video memory of that
// GPU from a CUDA context, and then allocates scanout buffers the way Xwayland's GBM backend
// does: DRM_IOCTL_NVIDIA_GEM_ALLOC_NVKMS_MEMORY, block linear, the size of an XRGB8888 surface
// of the mode. Each round allocates BUFFERS buffers, holds them for a second, frees them, and
// then takes whatever became free from CUDA for a second, as a game that sizes its pools to the
// free video memory does. A stock driver refuses the first buffer; with the display reserve
// every buffer is granted.
//
// Usage: scanout-oom [-d CARD] [-r ROUNDS] [-b BUFFERS]   the test (default card0, 3 rounds, 2)
//        scanout-oom [-d CARD] -M                         walk the modes: preferred, smallest, off
//        scanout-oom [-d CARD] -H SECONDS                 light the preferred mode and hold it
// Exit status: 0 every buffer granted, 1 a buffer refused, 2 setup failed.
// Build: cc -O2 -Wall -o scanout-oom scanout-oom.c -ldl   (kernel UAPI headers; CUDA is loaded
// at run time from the driver's libcuda.so.1, no toolkit needed)
#include <ctype.h>
#include <dlfcn.h>
#include <drm/drm.h>
#include <drm/drm_mode.h>
#include <errno.h>
#include <fcntl.h>
#include <libgen.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/ioctl.h>
#include <time.h>
#include <unistd.h>

// nvidia-drm's GEM allocation (kernel-open/nvidia-drm/nv_drm_common_ioctl.h, MIT).
struct nv_gem_alloc_nvkms_memory {
    uint32_t handle;
    uint8_t  block_linear;
    uint8_t  compressible;
    uint16_t pad0;
    uint64_t memory_size;
    uint32_t flags;
    uint32_t pad1;
};
#define NV_IOCTL_GEM_ALLOC_NVKMS_MEMORY \
    DRM_IOWR(DRM_COMMAND_BASE + 0x0b, struct nv_gem_alloc_nvkms_memory)

// The CUDA driver API calls used, from libcuda.so.1.
typedef int (*cuInit_t)(unsigned);
typedef int (*cuDeviceGetCount_t)(int *);
typedef int (*cuDeviceGet_t)(int *, int);
typedef int (*cuDeviceGetPCIBusId_t)(char *, int, int);
typedef int (*cuCtxCreate_t)(void **, unsigned, int);
typedef int (*cuMemAlloc_t)(unsigned long long *, size_t);
typedef int (*cuMemFree_t)(unsigned long long);
typedef int (*cuMemGetInfo_t)(size_t *, size_t *);

static struct {
    cuMemAlloc_t memAlloc;
    cuMemFree_t memFree;
    cuMemGetInfo_t memGetInfo;
    unsigned long long *ptrs;
    size_t count, cap, bytes;
} cu;

static int fd;
static uint32_t connId, crtcId;
static struct drm_mode_modeinfo modes[128];
static unsigned numModes;

static void die(const char *what)
{
    fprintf(stderr, "scanout-oom: %s: %s\n", what, errno ? strerror(errno) : "failed");
    exit(2);
}

// Take CUDA allocations of step bytes until one fails or until the deadline.
static void cuTake(size_t step, const struct timespec *until)
{
    for (;;) {
        unsigned long long p;
        struct timespec now;

        if (until) {
            clock_gettime(CLOCK_MONOTONIC, &now);
            if (now.tv_sec > until->tv_sec ||
                (now.tv_sec == until->tv_sec && now.tv_nsec >= until->tv_nsec))
                return;
        }
        if (cu.memAlloc(&p, step) != 0) {
            if (!until)
                return;
            usleep(10000);
            continue;
        }
        if (cu.count == cu.cap) {
            cu.cap = cu.cap ? 2 * cu.cap : 1024;
            cu.ptrs = realloc(cu.ptrs, cu.cap * sizeof(*cu.ptrs));
            if (!cu.ptrs)
                die("realloc");
        }
        cu.ptrs[cu.count++] = p;
        cu.bytes += step;
    }
}

// A CUDA context on the GPU of the DRM card, whose PCI address is pciAddr.
static void cuOpen(const char *pciAddr)
{
    void *lib = dlopen("libcuda.so.1", RTLD_NOW);
    cuInit_t init;
    cuDeviceGetCount_t getCount;
    cuDeviceGet_t get;
    cuDeviceGetPCIBusId_t getBusId;
    cuCtxCreate_t ctxCreate;
    void *ctx;
    int n, i, dev;

    if (!lib) {
        fprintf(stderr, "scanout-oom: %s\n", dlerror());
        exit(2);
    }
    init = (cuInit_t)dlsym(lib, "cuInit");
    getCount = (cuDeviceGetCount_t)dlsym(lib, "cuDeviceGetCount");
    get = (cuDeviceGet_t)dlsym(lib, "cuDeviceGet");
    getBusId = (cuDeviceGetPCIBusId_t)dlsym(lib, "cuDeviceGetPCIBusId");
    ctxCreate = (cuCtxCreate_t)dlsym(lib, "cuCtxCreate_v2");
    cu.memAlloc = (cuMemAlloc_t)dlsym(lib, "cuMemAlloc_v2");
    cu.memFree = (cuMemFree_t)dlsym(lib, "cuMemFree_v2");
    cu.memGetInfo = (cuMemGetInfo_t)dlsym(lib, "cuMemGetInfo_v2");
    if (!init || !getCount || !get || !getBusId || !ctxCreate ||
        !cu.memAlloc || !cu.memFree || !cu.memGetInfo || init(0) || getCount(&n))
        die("CUDA");

    for (i = 0; i < n; i++) {
        char busId[32];

        if (get(&dev, i) == 0 && getBusId(busId, sizeof(busId), dev) == 0 &&
            strcasecmp(busId, pciAddr) == 0)
            break;
    }
    if (i == n) {
        fprintf(stderr, "scanout-oom: no CUDA device at %s\n", pciAddr);
        exit(2);
    }
    if (ctxCreate(&ctx, 0, dev))
        die("cuCtxCreate");
}

static uint64_t align(uint64_t v, uint64_t a)
{
    return (v + a - 1) / a * a;
}

// The size NVIDIA's GBM backend gives an XRGB8888 block-linear surface: 0x870000 at 1920x1080.
static uint64_t scanoutSize(unsigned w, unsigned h)
{
    return align(align((uint64_t)w * 4, 64) * align(h, 128), 0x10000);
}

static void findConnector(void)
{
    struct drm_mode_card_res res = { 0 };
    uint32_t crtcs[32], conns[32], encs[32];
    unsigned c;

    res.crtc_id_ptr = (uintptr_t)crtcs;
    res.count_crtcs = 32;
    res.connector_id_ptr = (uintptr_t)conns;
    res.count_connectors = 32;
    res.encoder_id_ptr = (uintptr_t)encs;
    res.count_encoders = 32;
    if (ioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, &res))
        die("GETRESOURCES");

    for (c = 0; c < res.count_connectors && c < 32; c++) {
        struct drm_mode_get_connector conn = { .connector_id = conns[c] };
        uint32_t connEncs[16];
        unsigned e, k;

        conn.modes_ptr = (uintptr_t)modes;
        conn.count_modes = 128;
        conn.encoders_ptr = (uintptr_t)connEncs;
        conn.count_encoders = 16;
        if (ioctl(fd, DRM_IOCTL_MODE_GETCONNECTOR, &conn) || conn.connection != 1 ||
            conn.count_modes == 0)
            continue;

        for (e = 0; e < conn.count_encoders && e < 16; e++) {
            struct drm_mode_get_encoder enc = { .encoder_id = connEncs[e] };

            if (ioctl(fd, DRM_IOCTL_MODE_GETENCODER, &enc))
                continue;
            for (k = 0; k < res.count_crtcs && k < 32; k++) {
                if (enc.possible_crtcs & (1u << k)) {
                    connId = conns[c];
                    crtcId = crtcs[k];
                    numModes = conn.count_modes < 128 ? conn.count_modes : 128;
                    return;
                }
            }
        }
    }
    fprintf(stderr, "scanout-oom: no connected connector with a usable CRTC\n");
    exit(2);
}

static const struct drm_mode_modeinfo *preferredMode(void)
{
    unsigned k;

    for (k = 0; k < numModes; k++)
        if (modes[k].type & DRM_MODE_TYPE_PREFERRED)
            return &modes[k];
    return &modes[0];
}

// Light the connector with mode m on a dumb framebuffer, or turn it off (m NULL).
static void setMode(const struct drm_mode_modeinfo *m)
{
    struct drm_mode_create_dumb cd = { .bpp = 32 };
    struct drm_mode_fb_cmd fb = { 0 };
    struct drm_mode_crtc sc = { .crtc_id = crtcId };

    if (!m) {
        if (ioctl(fd, DRM_IOCTL_MODE_SETCRTC, &sc))
            die("SETCRTC off");
        printf("display off\n");
        fflush(stdout);
        return;
    }
    cd.width = m->hdisplay;
    cd.height = m->vdisplay;
    if (ioctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &cd))
        die("CREATE_DUMB for the mode");
    fb.width = m->hdisplay;
    fb.height = m->vdisplay;
    fb.bpp = 32;
    fb.depth = 24;
    fb.pitch = cd.pitch;
    fb.handle = cd.handle;
    if (ioctl(fd, DRM_IOCTL_MODE_ADDFB, &fb))
        die("ADDFB");
    sc.fb_id = fb.fb_id;
    sc.set_connectors_ptr = (uintptr_t)&connId;
    sc.count_connectors = 1;
    sc.mode = *m;
    sc.mode_valid = 1;
    if (ioctl(fd, DRM_IOCTL_MODE_SETCRTC, &sc))
        die("SETCRTC");
    printf("mode %ux%u lit\n", m->hdisplay, m->vdisplay);
    fflush(stdout);
}

static void pciAddress(const char *card, char *out, size_t len)
{
    char link[PATH_MAX], target[PATH_MAX];
    char *copy = strdup(card);
    ssize_t n;

    snprintf(link, sizeof(link), "/sys/class/drm/%s/device", basename(copy));
    free(copy);
    n = readlink(link, target, sizeof(target) - 1);
    if (n < 0)
        die(link);
    target[n] = 0;
    snprintf(out, len, "%s", basename(target));
}

int main(int argc, char **argv)
{
    const char *card = "/dev/dri/card0";
    struct drm_version ver = { 0 };
    char name[32] = { 0 };
    char pciAddr[64];
    const struct drm_mode_modeinfo *m;
    int rounds = 3, buffers = 2, walk = 0, hold = 0, refused = 0, opt, r, b;
    size_t freeB, totalB;
    uint64_t size;

    while ((opt = getopt(argc, argv, "d:r:b:MH:")) != -1) {
        switch (opt) {
        case 'd': card = optarg; break;
        case 'r': rounds = atoi(optarg); break;
        case 'b': buffers = atoi(optarg); break;
        case 'M': walk = 1; break;
        case 'H': hold = atoi(optarg); break;
        default:
            fprintf(stderr, "usage: scanout-oom [-d CARD] [-r ROUNDS] [-b BUFFERS] | -M | -H SECONDS\n");
            return 2;
        }
    }
    if (buffers < 1 || buffers > 64 || rounds < 1)
        die("bad -b or -r");

    fd = open(card, O_RDWR | O_CLOEXEC);
    if (fd < 0)
        die(card);
    ver.name = name;
    ver.name_len = sizeof(name) - 1;
    if (ioctl(fd, DRM_IOCTL_VERSION, &ver) || strcmp(name, "nvidia-drm") != 0) {
        fprintf(stderr, "scanout-oom: %s is not an nvidia-drm card (%s)\n", card, name);
        return 2;
    }
    if (ioctl(fd, DRM_IOCTL_SET_MASTER, 0))
        die("DRM master (is a display manager or compositor running?)");
    pciAddress(card, pciAddr, sizeof(pciAddr));
    findConnector();
    m = preferredMode();

    if (walk) {
        const struct drm_mode_modeinfo *small = m;
        unsigned k;

        for (k = 0; k < numModes; k++)
            if ((uint64_t)modes[k].hdisplay * modes[k].vdisplay <
                (uint64_t)small->hdisplay * small->vdisplay)
                small = &modes[k];
        setMode(m);
        sleep(3);
        setMode(small);
        sleep(3);
        setMode(NULL);
        sleep(3);
        return 0;
    }

    setMode(m);
    if (hold) {
        sleep(hold);
        return 0;
    }
    sleep(2);   // the reserve fills in the background

    size = scanoutSize(m->hdisplay, m->vdisplay);
    cuOpen(pciAddr);
    cuTake(64 << 20, NULL);
    cuTake(8 << 20, NULL);
    cuTake(2 << 20, NULL);
    cu.memGetInfo(&freeB, &totalB);
    printf("%s (%s): filled %zu MiB of %zu MiB from CUDA, %zu MiB free; scanout buffer %llu KiB\n",
           card, pciAddr, cu.bytes >> 20, totalB >> 20, freeB >> 20,
           (unsigned long long)(size >> 10));

    for (r = 1; r <= rounds; r++) {
        uint32_t handles[64];
        size_t before = cu.bytes;
        struct timespec until;

        printf("round %d:", r);
        for (b = 0; b < buffers; b++) {
            struct nv_gem_alloc_nvkms_memory p = { .block_linear = 1, .memory_size = size };

            handles[b] = 0;
            if (ioctl(fd, NV_IOCTL_GEM_ALLOC_NVKMS_MEMORY, &p)) {
                printf(" buffer %d refused (%s);", b + 1, strerror(errno));
                refused++;
            } else {
                handles[b] = p.handle;
                printf(" buffer %d granted;", b + 1);
            }
            fflush(stdout);
        }
        sleep(1);
        for (b = 0; b < buffers; b++) {
            struct drm_gem_close gc = { .handle = handles[b] };

            if (handles[b])
                ioctl(fd, DRM_IOCTL_GEM_CLOSE, &gc);
        }
        clock_gettime(CLOCK_MONOTONIC, &until);
        until.tv_sec += 1;
        cuTake(2 << 20, &until);
        printf(" freed; CUDA took %zu MiB more\n", (cu.bytes - before) >> 20);
        fflush(stdout);
    }

    printf("%s\n", refused ? "REPRODUCED: a scanout buffer was refused"
                           : "every scanout buffer was granted");
    return refused ? 1 : 0;
}
