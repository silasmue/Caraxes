// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// The CUDA step of the limits runs (README.md next to this file): the driver API through
// dlopen, no CUDA headers in the guest. cuInit, the device count, per device its name, SM
// count and memory, then a context and a 1 MiB allocation on device 0; every call's result
// is printed with its error name, and the probe stops at the first failure.
// Build: cc -O2 -o cuda-probe cuda-probe.c -ldl
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>

typedef int CUresult;
typedef int CUdevice;
typedef void *CUcontext;
typedef unsigned long long CUdeviceptr;

static const char *(*err_name)(CUresult, const char **);

static int check(const char *what, CUresult r)
{
    const char *n = "?";
    if (err_name)
        ((CUresult (*)(CUresult, const char **))err_name)(r, &n);
    printf("cuda-probe: %s -> %d %s\n", what, r, n);
    return r != 0;
}

int main(void)
{
    void *h = dlopen("libcuda.so.1", RTLD_NOW);
    if (!h) {
        printf("cuda-probe: dlopen: %s\n", dlerror());
        return 2;
    }
    err_name = (const char *(*)(CUresult, const char **))dlsym(h, "cuGetErrorName");
    CUresult (*init)(unsigned) = (CUresult (*)(unsigned))dlsym(h, "cuInit");
    CUresult (*count)(int *) = (CUresult (*)(int *))dlsym(h, "cuDeviceGetCount");
    CUresult (*get)(CUdevice *, int) = (CUresult (*)(CUdevice *, int))dlsym(h, "cuDeviceGet");
    CUresult (*name)(char *, int, CUdevice) = (CUresult (*)(char *, int, CUdevice))dlsym(h, "cuDeviceGetName");
    CUresult (*attr)(int *, int, CUdevice) = (CUresult (*)(int *, int, CUdevice))dlsym(h, "cuDeviceGetAttribute");
    CUresult (*mem)(size_t *, CUdevice) = (CUresult (*)(size_t *, CUdevice))dlsym(h, "cuDeviceTotalMem_v2");
    CUresult (*ctx)(CUcontext *, unsigned, CUdevice) = (CUresult (*)(CUcontext *, unsigned, CUdevice))dlsym(h, "cuCtxCreate_v2");
    CUresult (*alloc)(CUdeviceptr *, size_t) = (CUresult (*)(CUdeviceptr *, size_t))dlsym(h, "cuMemAlloc_v2");
    if (check("cuInit", init(0)))
        return 1;
    int n = 0;
    if (check("cuDeviceGetCount", count(&n)))
        return 1;
    printf("cuda-probe: %d device(s)\n", n);
    for (int i = 0; i < n; i++) {
        CUdevice d;
        char s[256] = "";
        int sms = 0, major = 0, minor = 0;
        size_t bytes = 0;
        check("cuDeviceGet", get(&d, i));
        check("cuDeviceGetName", name(s, sizeof s, d));
        check("cuDeviceGetAttribute(MULTIPROCESSOR_COUNT)", attr(&sms, 16, d));
        check("cuDeviceGetAttribute(COMPUTE_CAPABILITY_MAJOR)", attr(&major, 75, d));
        check("cuDeviceGetAttribute(COMPUTE_CAPABILITY_MINOR)", attr(&minor, 76, d));
        check("cuDeviceTotalMem", mem(&bytes, d));
        printf("cuda-probe: device %d: %s, %d SMs, %d.%d, %zu MiB\n", i, s, sms, major, minor, bytes >> 20);
    }
    if (n == 0)
        return 1;
    CUdevice d0;
    CUcontext c;
    CUdeviceptr p;
    check("cuDeviceGet(0)", get(&d0, 0));
    if (check("cuCtxCreate", ctx(&c, 0, d0)))
        return 1;
    if (check("cuMemAlloc(1 MiB)", alloc(&p, 1 << 20)))
        return 1;
    printf("cuda-probe: done\n");
    return 0;
}
