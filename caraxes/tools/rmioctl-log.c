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
#include <sys/ioctl.h>

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
    int r = real(fd, req, arg);
    if (_IOC_TYPE(req) != NV_IOCTL_MAGIC || !arg)
        return r;
    if (_IOC_NR(req) == ESC_RM_CONTROL && _IOC_SIZE(req) >= sizeof(struct nvos54)) {
        struct nvos54 *p = arg;
        fprintf(stderr, "RMCTRL cmd=%#010x obj=%#x size=%u status=%#x params=", p->cmd, p->hObject,
                p->paramsSize, p->status);
        const uint8_t *b = (const uint8_t *)(uintptr_t)p->params;
        for (uint32_t i = 0; b && i < p->paramsSize && i < 32; i++)
            fprintf(stderr, "%02x", b[i]);
        fputc('\n', stderr);
    } else if (_IOC_NR(req) == ESC_RM_ALLOC && _IOC_SIZE(req) >= sizeof(struct nvos64)) {
        struct nvos64 *p = arg;
        fprintf(stderr, "RMALLOC class=%#06x parent=%#x new=%#x status=%#x\n", p->hClass,
                p->hObjectParent, p->hObjectNew, p->status);
    }
    return r;
}
