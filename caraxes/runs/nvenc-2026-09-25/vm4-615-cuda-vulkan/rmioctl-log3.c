// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//
// LD_PRELOAD logger of a process's NVIDIA RM escapes, for the guest: every RM_CONTROL
// (cmd, params size, status, the first params bytes) and RM_ALLOC (class, parent, status),
// after the kernel answered, one line each on stderr. It shows the controls the guest kernel
// answers itself, which never reach the device's trace (video-engines.md §5, the 615 NVENC
// refusal).
//   gcc -O2 -shared -fPIC -o rmioctl-log.so rmioctl-log.c -ldl
//   LD_PRELOAD=./rmioctl-log.so ./nvenc-probe -n 1 2> rm.log
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>
#include <time.h>
static double nowf(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+t.tv_nsec/1e9;}

// NVOS54_PARAMETERS (nvos.h): the RM_CONTROL escape.
struct nvos54 {
    uint32_t hClient, hObject, cmd, flags;
    uint64_t params;
    uint32_t paramsSize, status;
};
// NVOS64_PARAMETERS (nvos.h): the RM_ALLOC escape.
struct nvos64 {
    uint32_t hRoot, hObjectParent, hObjectNew, hClass;
    uint64_t pAllocParms, pRightsRequested;
    uint32_t paramsSize, flags, status;
};

#define NV_IOCTL_MAGIC 'F'
#define ESC_RM_CONTROL 0x2a
#define ESC_RM_ALLOC 0x2b

int ioctl(int fd, unsigned long req, ...)
{
    static int (*real)(int, unsigned long, ...);
    va_list ap;
    va_start(ap, req);
    void *arg = va_arg(ap, void *);
    va_end(ap);
    if (!real)
        real = (int (*)(int, unsigned long, ...))dlsym(RTLD_NEXT, "ioctl");
    double t0 = nowf();
    int r = real(fd, req, arg);
    double dt = nowf() - t0;
    if (_IOC_TYPE(req) == NV_IOCTL_MAGIC && dt > 0.2)
        fprintf(stderr, "RMSLOW tid=%ld nr=%#lx %.3fs at %.3f\n", (long)gettid(), _IOC_NR(req), dt, t0);
    if (_IOC_TYPE(req) == NV_IOCTL_MAGIC && arg && _IOC_NR(req) == 0x29) {
        uint32_t *f = arg;
        fprintf(stderr, "RMFREE tid=%ld root=%#x parent=%#x obj=%#x status=%#x at %.3f\n", (long)gettid(), f[0], f[1], f[2], f[3], t0);
    }
    if (_IOC_TYPE(req) == NV_IOCTL_MAGIC && arg) {
        uint32_t *w32 = arg; unsigned nr = _IOC_NR(req), sz = _IOC_SIZE(req);
        if (nr == 0x27 || nr == 0x4a || nr == 0x4e || nr == 0x57 || nr == 0x58 || nr == 0x4f) {
            fprintf(stderr, "%.6f RMESC nr=%#x tid=%ld dt=%.4f raw=", t0, nr, (long)gettid(), dt);
            for (unsigned i = 0; i < sz / 4 && i < 24; i++) fprintf(stderr, "%08x ", w32[i]);
            fputc('\n', stderr);
        }
    }
    if (_IOC_TYPE(req) != NV_IOCTL_MAGIC || !arg)
        return r;
    if (_IOC_NR(req) == ESC_RM_CONTROL && _IOC_SIZE(req) >= sizeof(struct nvos54)) {
        struct nvos54 *p = arg;
        uint8_t *w = (uint8_t *)(uintptr_t)p->params;
        // Experiments (off by default): RMLOG_VIRT=<n> rewrites the answer of
        // NV0080_CTRL_CMD_GPU_GET_VIRTUALIZATION_MODE (0x800289, virtualizationMode @0);
        // RMLOG_GRCAPS=<46 hex digits> rewrites NV0080_CTRL_CMD_GR_GET_CAPS_V2's capsTbl
        // (0x801109, 23 bytes @0).
        const char *e;
        if (w && p->status == 0 && p->cmd == 0x800289 && (e = getenv("RMLOG_VIRT")) && p->paramsSize >= 4) {
            uint32_t v = (uint32_t)strtoul(e, NULL, 0);
            memcpy(w, &v, 4);
            fprintf(stderr, "RMLOG rewrote 0x800289 virtualizationMode to %u\n", v);
        }
        // RMLOG_ENGINES=<comma-separated NV2080 engine types> rewrites NV2080_CTRL_CMD_GPU_GET_ENGINES_V2
        // (0x20800170: engineCount @0, engineList[] @4).
        if (w && p->status == 0 && p->cmd == 0x20800170 && (e = getenv("RMLOG_ENGINES")) && p->paramsSize >= 8) {
            uint32_t n = 0;
            for (char *s = (char *)e; *s && 4 + 4 * (n + 1) <= p->paramsSize;) {
                uint32_t v = (uint32_t)strtoul(s, &s, 0);
                memcpy(w + 4 + 4 * n++, &v, 4);
                if (*s == ',')
                    s++;
            }
            memcpy(w, &n, 4);
            fprintf(stderr, "RMLOG rewrote 0x20800170 to %u engines\n", n);
        }
        if (w && p->status == 0 && p->cmd == 0x801109 && (e = getenv("RMLOG_GRCAPS")) && strlen(e) >= 46 &&
            p->paramsSize >= 23) {
            for (int i = 0; i < 23; i++) {
                char h[3] = {e[2 * i], e[2 * i + 1], 0};
                w[i] = (uint8_t)strtoul(h, NULL, 16);
            }
            fprintf(stderr, "RMLOG rewrote 0x801109 capsTbl\n");
        }
        fprintf(stderr, "%.6f RMCTRL tid=%ld client=%#x cmd=%#010x obj=%#x size=%u status=%#x params=", t0, (long)gettid(), p->hClient, p->cmd, p->hObject,
                p->paramsSize, p->status);
        const uint8_t *b = (const uint8_t *)(uintptr_t)p->params;
        for (uint32_t i = 0; b && i < p->paramsSize && i < (p->cmd == 0x20800170 ? 64u : 32u); i++)
            fprintf(stderr, "%02x", b[i]);
        fputc('\n', stderr);
        // NV0080_CTRL_CMD_GPU_GET_CLASSLIST (0x800201): numClasses @0, classList (NvP64) @8.
        if (p->cmd == 0x800201 && p->status == 0 && b && p->paramsSize >= 16) {
            uint32_t n;
            uint64_t lp;
            memcpy(&n, b, 4);
            memcpy(&lp, b + 8, 8);
            const uint32_t *l = (const uint32_t *)(uintptr_t)lp;
            if (l) {
                fprintf(stderr, "RMCLASSES %u:", n);
                for (uint32_t i = 0; i < n && i < 512; i++)
                    fprintf(stderr, " %04x", l[i]);
                fputc('\n', stderr);
            }
        }
    } else if (_IOC_NR(req) == ESC_RM_ALLOC && _IOC_SIZE(req) >= sizeof(struct nvos64)) {
        struct nvos64 *p = arg;
        fprintf(stderr, "%.6f RMALLOC tid=%ld root=%#x class=%#06x parent=%#x new=%#x status=%#x size=%u", t0, (long)gettid(), p->hRoot, p->hClass,
                p->hObjectParent, p->hObjectNew, p->status, p->paramsSize);
        const uint8_t *a = (const uint8_t *)(uintptr_t)p->pAllocParms;
        uint32_t u[64];
        // NV_CHANNEL_ALLOC_PARAMS (615.71.09 layout, nvrm-sys v615.rs): the handles and flags.
        if (a && (p->hClass & 0xff) == 0x6f && p->hClass >= 0xa06f) {
            memcpy(u, a, 256);
            uint64_t gp;
            memcpy(&gp, a + 8, 8);
            fprintf(stderr,
                    " | hObjectError=%#x hObjectBuffer=%#x gpFifoOffset=%#lx entries=%u flags=%#x"
                    " hContextShare=%#x hVASpace=%#x hHandleVASpace=%#x engineType=%#x cid=%u"
                    " subDeviceId=%u hObjectEccError=%#x hPhysChannelGroup=%#x internalFlags=%#x",
                    u[0], u[1], (unsigned long)gp, u[4], u[5], u[6], u[7], u[8], u[34], u[35],
                    u[36], u[37], u[62], u[63]);
        } else if (a && p->paramsSize) {
            fprintf(stderr, " | params=");
            for (uint32_t i = 0; i < p->paramsSize && i < 32; i++)
                fprintf(stderr, "%02x", a[i]);
        }
        fputc('\n', stderr);
    }
    return r;
}
