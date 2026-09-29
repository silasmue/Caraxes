// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//
// covwork: CUDA driver-API workloads the gates do not cover or cover thinly, one per
// subcommand, for the coverage map (docs/caraxes/coverage-map.md). Built in the guest like
// census-cu (gcc against the venv's cuda.h and -lcuda; no nvcc: the kernels are PTX below,
// JIT-compiled by the driver). Every subcommand prints "COV <name> ok|fail <detail>" last.
//
//   covwork graphs N      stream capture into a graph (kernel, memset, D2H copy, host node),
//                         instantiate, launch N times; an explicit graph with a host node
//   covwork streams N     four streams at two priorities, cross-stream event waits,
//                         cuLaunchHostFunc callbacks, event queries and timing,
//                         stream-ordered allocations (cuMemAllocAsync) in a blocking-sync
//                         context
//   covwork managed       cuMemAllocManaged, advise, prefetch (expected to fail on the device)
//   covwork texsurf       a CUDA array read through a texture object, written through a
//                         surface object, 2D copies to and from arrays
//   covwork zerocopy      mapped pinned host memory written by a kernel (cuMemHostGetDevicePointer)
//   covwork query         every device attribute, UUID, PCI bus id, memory info, limits
//   covwork oom           allocations beyond the FB until CUDA_ERROR_OUT_OF_MEMORY, then free
//   covwork spin MS       one kernel spinning MS milliseconds (killed with kill -9 from outside)
//   covwork illegal       a kernel storing to an unmapped address (CUDA_ERROR_ILLEGAL_ADDRESS)
//   covwork nvrtc LIB     compile a CUDA C kernel with NVRTC (dlopen LIB), load its PTX, run it
#include <cuda.h>
#include <dlfcn.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static const char *NAME = "?";

#define CK(call)                                                                      \
    do {                                                                              \
        CUresult r_ = (call);                                                         \
        if (r_ != CUDA_SUCCESS) {                                                     \
            const char *s_ = "?";                                                     \
            cuGetErrorName(r_, &s_);                                                  \
            printf("ERROR %s: %d %s\nCOV %s fail %s=%d\n", #call, (int)r_, s_, NAME, s_, \
                   (int)r_);                                                          \
            fflush(stdout);                                                           \
            exit(1);                                                                  \
        }                                                                             \
    } while (0)

static const char PTX[] =
    ".version 7.5\n.target sm_75\n.address_size 64\n"
    ".visible .entry fill(.param .u64 p, .param .u32 v, .param .u32 n)\n{\n"
    " .reg .pred %p<2>; .reg .b32 %r<8>; .reg .b64 %rd<4>;\n"
    " ld.param.u64 %rd1, [p]; ld.param.u32 %r1, [v]; ld.param.u32 %r2, [n];\n"
    " mov.u32 %r3, %ctaid.x; mov.u32 %r4, %ntid.x; mov.u32 %r5, %tid.x;\n"
    " mad.lo.s32 %r6, %r3, %r4, %r5; setp.ge.u32 %p1, %r6, %r2; @%p1 bra DONE;\n"
    " cvta.to.global.u64 %rd1, %rd1; mul.wide.u32 %rd2, %r6, 4; add.s64 %rd3, %rd1, %rd2;\n"
    " add.s32 %r7, %r1, %r6; st.global.u32 [%rd3], %r7;\nDONE:\n ret;\n}\n"
    ".visible .entry spin(.param .u64 cycles)\n{\n"
    " .reg .pred %p<2>; .reg .b64 %rd<5>;\n"
    " ld.param.u64 %rd1, [cycles]; mov.u64 %rd2, %clock64;\n"
    "LOOP:\n mov.u64 %rd3, %clock64; sub.s64 %rd4, %rd3, %rd2; setp.lt.u64 %p1, %rd4, %rd1;\n"
    " @%p1 bra LOOP;\n ret;\n}\n"
    ".visible .entry bad(.param .u64 addr)\n{\n"
    " .reg .b64 %rd<2>; .reg .b32 %r<2>;\n"
    " ld.param.u64 %rd1, [addr]; mov.u32 %r1, 1; st.global.u32 [%rd1], %r1;\n ret;\n}\n"
    ".visible .entry texsurf(.param .u64 tex, .param .u64 surf, .param .u32 w, .param .u32 h)\n{\n"
    " .reg .pred %p<3>; .reg .b32 %r<12>; .reg .f32 %f<8>; .reg .b64 %rd<3>;\n"
    " ld.param.u64 %rd1, [tex]; ld.param.u64 %rd2, [surf]; ld.param.u32 %r1, [w]; ld.param.u32 %r2, [h];\n"
    " mov.u32 %r3, %ctaid.x; mov.u32 %r4, %ntid.x; mov.u32 %r5, %tid.x; mad.lo.s32 %r6, %r3, %r4, %r5;\n"
    " mov.u32 %r3, %ctaid.y; mov.u32 %r4, %ntid.y; mov.u32 %r5, %tid.y; mad.lo.s32 %r7, %r3, %r4, %r5;\n"
    " setp.ge.u32 %p1, %r6, %r1; setp.ge.u32 %p2, %r7, %r2; or.pred %p1, %p1, %p2; @%p1 bra DONE;\n"
    " cvt.rn.f32.u32 %f1, %r6; add.f32 %f1, %f1, 0f3F000000;\n"
    " cvt.rn.f32.u32 %f2, %r7; add.f32 %f2, %f2, 0f3F000000;\n"
    " tex.2d.v4.f32.f32 {%f3, %f4, %f5, %f6}, [%rd1, {%f1, %f2}];\n"
    " add.f32 %f3, %f3, 0f3F800000; mov.b32 %r9, %f3; shl.b32 %r8, %r6, 2;\n"
    " sust.b.2d.b32.trap [%rd2, {%r8, %r7}], {%r9};\nDONE:\n ret;\n}\n";

static CUdevice dev;
static CUcontext ctx;
static CUmodule mod;
static CUfunction f_fill, f_spin, f_bad, f_texsurf;

static void setup(unsigned flags) {
    CK(cuInit(0));
    CK(cuDeviceGet(&dev, 0));
#if CUDA_VERSION >= 13000
    CUctxCreateParams cp;
    memset(&cp, 0, sizeof cp);
    CK(cuCtxCreate(&ctx, &cp, flags, dev));
#else
    CK(cuCtxCreate(&ctx, flags, dev));
#endif
    CK(cuModuleLoadData(&mod, PTX));
    CK(cuModuleGetFunction(&f_fill, mod, "fill"));
    CK(cuModuleGetFunction(&f_spin, mod, "spin"));
    CK(cuModuleGetFunction(&f_bad, mod, "bad"));
    CK(cuModuleGetFunction(&f_texsurf, mod, "texsurf"));
}

static void launch_fill(CUstream s, CUdeviceptr p, unsigned v, unsigned n) {
    void *args[] = {&p, &v, &n};
    CK(cuLaunchKernel(f_fill, (n + 255) / 256, 1, 1, 256, 1, 1, 0, s, args, NULL));
}

static int check_fill(const unsigned *h, unsigned v, unsigned n) {
    for (unsigned i = 0; i < n; i++)
        if (h[i] != v + i) return 0;
    return 1;
}

static volatile int host_calls;
static void CUDA_CB host_fn(void *arg) {
    (void)arg;
    __sync_fetch_and_add(&host_calls, 1);
}

static int graphs(int n) {
    const unsigned N = 1 << 16;
    CUstream s;
    CUdeviceptr d;
    unsigned *h;
    CK(cuStreamCreate(&s, CU_STREAM_NON_BLOCKING));
    CK(cuMemAlloc(&d, N * 4));
    CK(cuMemAllocHost((void **)&h, N * 4));
    CUgraph g;
    CUgraphExec ge;
    CK(cuStreamBeginCapture(s, CU_STREAM_CAPTURE_MODE_GLOBAL));
    CK(cuMemsetD32Async(d, 0, N, s));
    launch_fill(s, d, 7, N);
    CK(cuMemcpyDtoHAsync(h, d, N * 4, s));
    CK(cuLaunchHostFunc(s, host_fn, NULL));
    CK(cuStreamEndCapture(s, &g));
    size_t nodes = 0;
    CK(cuGraphGetNodes(g, NULL, &nodes));
    CK(cuGraphInstantiate(&ge, g, 0));
    for (int i = 0; i < n; i++) CK(cuGraphLaunch(ge, s));
    CK(cuStreamSynchronize(s));
    int ok = check_fill(h, 7, N) && host_calls == n;
    // An explicit graph: kernel node, then a host node depending on it.
    CUgraph g2;
    CUgraphExec ge2;
    CUgraphNode kn, hn;
    CK(cuGraphCreate(&g2, 0));
    unsigned v = 11, cnt = N;
    void *args[] = {&d, &v, &cnt};
    CUDA_KERNEL_NODE_PARAMS kp;
    memset(&kp, 0, sizeof kp);
    kp.func = f_fill;
    kp.gridDimX = (N + 255) / 256;
    kp.gridDimY = kp.gridDimZ = 1;
    kp.blockDimX = 256;
    kp.blockDimY = kp.blockDimZ = 1;
    kp.kernelParams = args;
    CK(cuGraphAddKernelNode(&kn, g2, NULL, 0, &kp));
    CUDA_HOST_NODE_PARAMS hp = {host_fn, NULL};
    CK(cuGraphAddHostNode(&hn, g2, &kn, 1, &hp));
    CK(cuGraphInstantiate(&ge2, g2, 0));
    for (int i = 0; i < n; i++) CK(cuGraphLaunch(ge2, s));
    CK(cuStreamSynchronize(s));
    CK(cuMemcpyDtoH(h, d, N * 4));
    ok = ok && check_fill(h, 11, N) && host_calls == 2 * n;
    CK(cuGraphExecDestroy(ge2));
    CK(cuGraphDestroy(g2));
    CK(cuGraphExecDestroy(ge));
    CK(cuGraphDestroy(g));
    CK(cuMemFreeHost(h));
    CK(cuMemFree(d));
    CK(cuStreamDestroy(s));
    printf("COV graphs %s %zu captured nodes, %d launches of each graph, %d host calls\n",
           ok ? "ok" : "fail", nodes, n, host_calls);
    return !ok;
}

static int streams(int n) {
    const unsigned N = 1 << 18;
    int lo, hi;
    CK(cuCtxGetStreamPriorityRange(&lo, &hi));
    CUstream s[4];
    CUevent ev[4], t0, t1;
    CUdeviceptr d[4];
    unsigned *h;
    for (int i = 0; i < 4; i++) {
        CK(cuStreamCreateWithPriority(&s[i], CU_STREAM_NON_BLOCKING, i & 1 ? hi : lo));
        CK(cuEventCreate(&ev[i], CU_EVENT_DISABLE_TIMING | (i == 3 ? CU_EVENT_BLOCKING_SYNC : 0)));
        CK(cuMemAlloc(&d[i], N * 4));
    }
    CK(cuEventCreate(&t0, CU_EVENT_DEFAULT));
    CK(cuEventCreate(&t1, CU_EVENT_DEFAULT));
    CK(cuMemAllocHost((void **)&h, N * 4));
    CK(cuEventRecord(t0, s[0]));
    long polls = 0;
    int pools = 1;
    CUresult pool_err = CUDA_SUCCESS;
    for (int it = 0; it < n; it++) {
        for (int i = 0; i < 4; i++) {
            if (i > 0) CK(cuStreamWaitEvent(s[i], ev[i - 1], 0));
            launch_fill(s[i], d[i], it + i, N);
            // Stream-ordered allocation from the default pool, used and freed in order;
            // the device refuses it today (801, coverage-map.md), so it is tried and noted.
            CUdeviceptr a;
            if (pools) {
                CUresult r = cuMemAllocAsync(&a, 1 << 20, s[i]);
                if (r == CUDA_SUCCESS) {
                    CK(cuMemsetD32Async(a, it, (1 << 20) / 4, s[i]));
                    CK(cuMemFreeAsync(a, s[i]));
                } else {
                    pools = 0;
                    pool_err = r;
                }
            }
            CK(cuLaunchHostFunc(s[i], host_fn, NULL));
            CK(cuEventRecord(ev[i], s[i]));
        }
        while (cuEventQuery(ev[3]) == CUDA_ERROR_NOT_READY) polls++;
        CK(cuStreamQuery(s[0]));
    }
    CK(cuEventRecord(t1, s[3]));
    CK(cuEventSynchronize(t1));
    float ms = 0;
    CK(cuEventElapsedTime(&ms, t0, t1));
    CK(cuMemcpyDtoH(h, d[3], N * 4));
    int ok = check_fill(h, n - 1 + 3, N) && host_calls == 4 * n;
    for (int i = 0; i < 4; i++) {
        CK(cuStreamDestroy(s[i]));
        CK(cuEventDestroy(ev[i]));
        CK(cuMemFree(d[i]));
    }
    CK(cuEventDestroy(t0));
    CK(cuEventDestroy(t1));
    CK(cuMemFreeHost(h));
    const char *pe = "none";
    if (pool_err != CUDA_SUCCESS) cuGetErrorName(pool_err, &pe);
    printf("COV streams %s %d rounds of 4 streams (priorities %d..%d), %d host calls, %ld event polls, %.1f ms; cuMemAllocAsync %s (error %s)\n",
           ok ? "ok" : "fail", n, lo, hi, host_calls, polls, ms, pools ? "used" : "refused", pe);
    return !ok;
}

static int managed(void) {
    const size_t sz = 1 << 22;
    CUdeviceptr m;
    CUresult r = cuMemAllocManaged(&m, sz, CU_MEM_ATTACH_GLOBAL);
    if (r != CUDA_SUCCESS) {
        const char *s = "?";
        cuGetErrorName(r, &s);
        printf("COV managed ok cuMemAllocManaged refused: %s=%d (the device's answer)\n", s, (int)r);
        return 0;
    }
    memset((void *)(uintptr_t)m, 1, sz);
    launch_fill(NULL, m, 3, sz / 4);
    CK(cuCtxSynchronize());
    int ok = check_fill((unsigned *)(uintptr_t)m, 3, sz / 4);
    CK(cuMemFree(m));
    printf("COV managed %s cuMemAllocManaged 4 MiB written by host and kernel\n", ok ? "ok" : "fail");
    return !ok;
}

static int texsurf(void) {
    const unsigned W = 256, H = 128;
    CUarray in, out;
    CUDA_ARRAY3D_DESCRIPTOR ad;
    memset(&ad, 0, sizeof ad);
    ad.Width = W;
    ad.Height = H;
    ad.Format = CU_AD_FORMAT_FLOAT;
    ad.NumChannels = 1;
    ad.Flags = CUDA_ARRAY3D_SURFACE_LDST;
    CK(cuArray3DCreate(&in, &ad));
    CK(cuArray3DCreate(&out, &ad));
    float *h = malloc(W * H * 4), *r = malloc(W * H * 4);
    for (unsigned i = 0; i < W * H; i++) h[i] = (float)i;
    CUDA_MEMCPY2D c;
    memset(&c, 0, sizeof c);
    c.srcMemoryType = CU_MEMORYTYPE_HOST;
    c.srcHost = h;
    c.srcPitch = W * 4;
    c.dstMemoryType = CU_MEMORYTYPE_ARRAY;
    c.dstArray = in;
    c.WidthInBytes = W * 4;
    c.Height = H;
    CK(cuMemcpy2D(&c));
    CUDA_RESOURCE_DESC rd;
    memset(&rd, 0, sizeof rd);
    rd.resType = CU_RESOURCE_TYPE_ARRAY;
    rd.res.array.hArray = in;
    CUDA_TEXTURE_DESC td;
    memset(&td, 0, sizeof td);
    td.addressMode[0] = td.addressMode[1] = CU_TR_ADDRESS_MODE_CLAMP;
    td.filterMode = CU_TR_FILTER_MODE_POINT;
    CUtexObject tex;
    CK(cuTexObjectCreate(&tex, &rd, &td, NULL));
    rd.res.array.hArray = out;
    CUsurfObject surf;
    CK(cuSurfObjectCreate(&surf, &rd));
    unsigned w = W, hh = H;
    void *args[] = {&tex, &surf, &w, &hh};
    CK(cuLaunchKernel(f_texsurf, W / 16, H / 16, 1, 16, 16, 1, 0, NULL, args, NULL));
    CK(cuCtxSynchronize());
    memset(&c, 0, sizeof c);
    c.srcMemoryType = CU_MEMORYTYPE_ARRAY;
    c.srcArray = out;
    c.dstMemoryType = CU_MEMORYTYPE_HOST;
    c.dstHost = r;
    c.dstPitch = W * 4;
    c.WidthInBytes = W * 4;
    c.Height = H;
    CK(cuMemcpy2D(&c));
    int ok = 1;
    for (unsigned i = 0; i < W * H; i++)
        if (r[i] != h[i] + 1.0f) ok = 0;
    CK(cuTexObjectDestroy(tex));
    CK(cuSurfObjectDestroy(surf));
    CK(cuArrayDestroy(in));
    CK(cuArrayDestroy(out));
    printf("COV texsurf %s %ux%u float array through a texture object into a surface object\n",
           ok ? "ok" : "fail", W, H);
    return !ok;
}

static int zerocopy(void) {
    const unsigned N = 1 << 18;
    unsigned *h;
    CK(cuMemHostAlloc((void **)&h, N * 4, CU_MEMHOSTALLOC_DEVICEMAP | CU_MEMHOSTALLOC_PORTABLE));
    CUdeviceptr d;
    CK(cuMemHostGetDevicePointer(&d, h, 0));
    launch_fill(NULL, d, 5, N);
    CK(cuCtxSynchronize());
    int ok = check_fill(h, 5, N);
    unsigned *g = malloc(N * 4);
    CK(cuMemHostRegister(g, N * 4, CU_MEMHOSTREGISTER_DEVICEMAP));
    CUdeviceptr dg;
    CK(cuMemHostGetDevicePointer(&dg, g, 0));
    launch_fill(NULL, dg, 9, N);
    CK(cuCtxSynchronize());
    ok = ok && check_fill(g, 9, N);
    CK(cuMemHostUnregister(g));
    CK(cuMemFreeHost(h));
    printf("COV zerocopy %s kernel stores into mapped pinned and registered host memory\n", ok ? "ok" : "fail");
    return !ok;
}

static int query(void) {
    int n = 0, v;
    for (int a = 1; a < 160; a++)
        if (cuDeviceGetAttribute(&v, (CUdevice_attribute)a, dev) == CUDA_SUCCESS) n++;
    char name[128], bus[64];
    CUuuid u;
    size_t fr, tot;
    CK(cuDeviceGetName(name, sizeof name, dev));
    CK(cuDeviceGetPCIBusId(bus, sizeof bus, dev));
    CK(cuDeviceGetUuid(&u, dev));
    CK(cuDeviceTotalMem(&tot, dev));
    for (int i = 0; i < 20; i++) CK(cuMemGetInfo(&fr, &tot));
    size_t lim;
    CK(cuCtxGetLimit(&lim, CU_LIMIT_STACK_SIZE));
    CK(cuCtxSetLimit(CU_LIMIT_STACK_SIZE, lim * 2));
    CK(cuCtxSetLimit(CU_LIMIT_PRINTF_FIFO_SIZE, 1 << 21));
    CK(cuCtxSetLimit(CU_LIMIT_MALLOC_HEAP_SIZE, 16 << 20));
    int nb = 0;
    CK(cuOccupancyMaxActiveBlocksPerMultiprocessor(&nb, f_fill, 256, 0));
    printf("COV query ok %d attributes, %s at %s, %zu of %zu MiB free, %d blocks/SM\n", n, name, bus,
           fr >> 20, tot >> 20, nb);
    return 0;
}

static int oom(void) {
    CUdeviceptr p[64];
    int k = 0;
    size_t total = 0;
    CUresult r = cuMemAlloc(&p[0], (size_t)4 << 30);
    const char *s0 = "?";
    cuGetErrorName(r, &s0);
    if (r == CUDA_SUCCESS) cuMemFree(p[0]);
    for (; k < 64; k++) {
        r = cuMemAlloc(&p[k], 64 << 20);
        if (r != CUDA_SUCCESS) break;
        total += 64 << 20;
    }
    const char *s = "?";
    cuGetErrorName(r, &s);
    for (int i = 0; i < k; i++) CK(cuMemFree(p[i]));
    // The context must still work.
    CUdeviceptr d;
    unsigned h[256];
    CK(cuMemAlloc(&d, sizeof h));
    launch_fill(NULL, d, 1, 256);
    CK(cuMemcpyDtoH(h, d, sizeof h));
    CK(cuMemFree(d));
    int ok = r == CUDA_ERROR_OUT_OF_MEMORY && check_fill(h, 1, 256);
    printf("COV oom %s 4 GiB alloc: %s; then %d x 64 MiB (%zu MiB) before %s; context usable after\n",
           ok ? "ok" : "fail", s0, k, total >> 20, s);
    return !ok;
}

static int spin(int ms) {
    int khz = 0;
    CK(cuDeviceGetAttribute(&khz, CU_DEVICE_ATTRIBUTE_CLOCK_RATE, dev));
    unsigned long long cycles = (unsigned long long)khz * ms;
    void *args[] = {&cycles};
    CK(cuLaunchKernel(f_spin, 1, 1, 1, 32, 1, 1, 0, NULL, args, NULL));
    printf("SPINNING pid %d for %d ms\n", getpid(), ms);
    fflush(stdout);
    CK(cuCtxSynchronize());
    printf("COV spin ok %d ms kernel finished\n", ms);
    return 0;
}

static int illegal(void) {
    unsigned long long addr = 0x10;
    void *args[] = {&addr};
    CK(cuLaunchKernel(f_bad, 1, 1, 1, 1, 1, 1, 0, NULL, args, NULL));
    CUresult r = cuCtxSynchronize();
    const char *s = "?";
    cuGetErrorName(r, &s);
    CUresult r2 = cuMemAlloc(&(CUdeviceptr){0}, 4096);
    const char *s2 = "?";
    cuGetErrorName(r2, &s2);
    int ok = r == CUDA_ERROR_ILLEGAL_ADDRESS;
    printf("COV illegal %s store to 0x10: synchronize %s=%d; next allocation %s\n", ok ? "ok" : "fail", s,
           (int)r, s2);
    return !ok;
}

typedef int (*nvrtcCreateProgram_t)(void **, const char *, const char *, int, const char *const *,
                                   const char *const *);
typedef int (*nvrtcCompileProgram_t)(void *, int, const char *const *);
typedef int (*nvrtcGetPTXSize_t)(void *, size_t *);
typedef int (*nvrtcGetPTX_t)(void *, char *);
typedef int (*nvrtcGetProgramLog_t)(void *, char *);
typedef int (*nvrtcGetProgramLogSize_t)(void *, size_t *);

static int nvrtc(const char *lib) {
    void *h = dlopen(lib, RTLD_NOW);
    if (!h) {
        printf("COV nvrtc fail dlopen %s: %s\n", lib, dlerror());
        return 1;
    }
    nvrtcCreateProgram_t create = (nvrtcCreateProgram_t)dlsym(h, "nvrtcCreateProgram");
    nvrtcCompileProgram_t compile = (nvrtcCompileProgram_t)dlsym(h, "nvrtcCompileProgram");
    nvrtcGetPTXSize_t ptxsize = (nvrtcGetPTXSize_t)dlsym(h, "nvrtcGetPTXSize");
    nvrtcGetPTX_t getptx = (nvrtcGetPTX_t)dlsym(h, "nvrtcGetPTX");
    nvrtcGetProgramLogSize_t logsize = (nvrtcGetProgramLogSize_t)dlsym(h, "nvrtcGetProgramLogSize");
    nvrtcGetProgramLog_t getlog = (nvrtcGetProgramLog_t)dlsym(h, "nvrtcGetProgramLog");
    const char *src =
        "extern \"C\" __global__ void saxpy(float a, const float *x, float *y, unsigned n) {\n"
        "  unsigned i = blockIdx.x * blockDim.x + threadIdx.x;\n"
        "  if (i < n) y[i] = a * x[i] + y[i];\n}\n";
    void *prog = NULL;
    const char *opts[] = {"--gpu-architecture=compute_75"};
    if (create(&prog, src, "saxpy.cu", 0, NULL, NULL) != 0 || compile(prog, 1, opts) != 0) {
        size_t n = 0;
        char buf[4096] = "";
        if (prog && logsize(prog, &n) == 0 && n < sizeof buf) getlog(prog, buf);
        printf("COV nvrtc fail compile: %s\n", buf);
        return 1;
    }
    size_t n = 0;
    ptxsize(prog, &n);
    char *ptx = malloc(n);
    getptx(prog, ptx);
    CUmodule m;
    CUfunction f;
    CK(cuModuleLoadData(&m, ptx));
    CK(cuModuleGetFunction(&f, m, "saxpy"));
    const unsigned N = 1 << 16;
    float *x = malloc(N * 4), *y = malloc(N * 4);
    for (unsigned i = 0; i < N; i++) {
        x[i] = (float)i;
        y[i] = 1.0f;
    }
    CUdeviceptr dx, dy;
    CK(cuMemAlloc(&dx, N * 4));
    CK(cuMemAlloc(&dy, N * 4));
    CK(cuMemcpyHtoD(dx, x, N * 4));
    CK(cuMemcpyHtoD(dy, y, N * 4));
    float a = 2.0f;
    unsigned nn = N;
    void *args[] = {&a, &dx, &dy, &nn};
    CK(cuLaunchKernel(f, N / 256, 1, 1, 256, 1, 1, 0, NULL, args, NULL));
    CK(cuMemcpyDtoH(y, dy, N * 4));
    int ok = 1;
    for (unsigned i = 0; i < N; i++)
        if (y[i] != 2.0f * i + 1.0f) ok = 0;
    CK(cuModuleUnload(m));
    printf("COV nvrtc %s saxpy compiled by NVRTC (%zu bytes of PTX), loaded and verified\n", ok ? "ok" : "fail", n);
    return !ok;
}

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: covwork graphs|streams|managed|texsurf|zerocopy|query|oom|spin|illegal|nvrtc [arg]\n");
        return 2;
    }
    NAME = argv[1];
    const char *arg = argc > 2 ? argv[2] : NULL;
    setup(!strcmp(NAME, "streams") ? CU_CTX_SCHED_BLOCKING_SYNC : 0);
    int rc;
    if (!strcmp(NAME, "graphs")) rc = graphs(arg ? atoi(arg) : 100);
    else if (!strcmp(NAME, "streams")) rc = streams(arg ? atoi(arg) : 200);
    else if (!strcmp(NAME, "managed")) rc = managed();
    else if (!strcmp(NAME, "texsurf")) rc = texsurf();
    else if (!strcmp(NAME, "zerocopy")) rc = zerocopy();
    else if (!strcmp(NAME, "query")) rc = query();
    else if (!strcmp(NAME, "oom")) rc = oom();
    else if (!strcmp(NAME, "spin")) rc = spin(arg ? atoi(arg) : 2000);
    else if (!strcmp(NAME, "illegal")) rc = illegal();
    else if (!strcmp(NAME, "nvrtc")) rc = nvrtc(arg ? arg : "libnvrtc.so");
    else {
        fprintf(stderr, "unknown %s\n", NAME);
        return 2;
    }
    fflush(stdout);
    cuCtxDestroy(ctx);
    return rc;
}
