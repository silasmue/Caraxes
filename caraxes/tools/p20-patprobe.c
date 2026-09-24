// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
// P20 pattern probe (docs/caraxes/parallel.md, items 3d and 4b), CUDA driver API only.
//
//   p20-patprobe fill <MiB> <tag> <hold_s>
//       cuMemAlloc <MiB> of device memory, fill it with 16-byte records
//       {0xCA7A5E5A, 0x50A77E2D, tag, index} by host-to-device copies, read back the
//       first and last chunk to prove the pattern landed, then keep the allocation for
//       <hold_s> seconds (0: exit at once; the allocation is freed at exit).
//   p20-patprobe scan [maxMiB]
//       cuMemAlloc 64 MiB chunks until the allocator refuses (or maxMiB), copy each back
//       and count records whose two magic words match, per tag, plus the non-zero bytes.
//
// Output lines are `key=value` for the host-side checker (caraxes/tools/p20-check.py).
#include <cuda.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define MAGIC0 0xCA7A5E5Au
#define MAGIC1 0x50A77E2Du
#define CHUNK (64ull << 20)
#define MAXTAGS 16

static void die(const char *what, CUresult r)
{
    const char *s = NULL;
    cuGetErrorName(r, &s);
    printf("error=%s:%d:%s\n", what, (int)r, s ? s : "?");
    exit(1);
}

#define CK(x) do { CUresult r_ = (x); if (r_ != CUDA_SUCCESS) die(#x, r_); } while (0)

static double now(void)
{
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec + t.tv_nsec * 1e-9;
}

static volatile sig_atomic_t stop;
static void on_term(int s) { (void)s; stop = 1; }

static int fill(size_t mib, uint32_t tag, int hold)
{
    CUdeviceptr d;
    size_t bytes = mib << 20;
    uint32_t *h = malloc(CHUNK), *back = malloc(CHUNK);
    if (!h || !back)
        return 1;
    CK(cuMemAlloc(&d, bytes));
    double t0 = now();
    for (size_t off = 0; off < bytes; off += CHUNK) {
        size_t n = bytes - off < CHUNK ? bytes - off : CHUNK;
        for (size_t i = 0; i < n / 16; i++) {
            h[4 * i] = MAGIC0;
            h[4 * i + 1] = MAGIC1;
            h[4 * i + 2] = tag;
            h[4 * i + 3] = (uint32_t)((off / 16) + i);
        }
        CK(cuMemcpyHtoD(d + off, h, n));
    }
    CK(cuCtxSynchronize());
    double t1 = now();
    // Verify the first and the last chunk.
    size_t bad = 0;
    size_t offs[2] = {0, bytes > CHUNK ? bytes - CHUNK : 0};
    for (int k = 0; k < 2; k++) {
        size_t off = offs[k], n = bytes - off < CHUNK ? bytes - off : CHUNK;
        CK(cuMemcpyDtoH(back, d + off, n));
        for (size_t i = 0; i < n / 16; i++)
            if (back[4 * i] != MAGIC0 || back[4 * i + 1] != MAGIC1 || back[4 * i + 2] != tag ||
                back[4 * i + 3] != (uint32_t)((off / 16) + i))
                bad++;
    }
    printf("filled_mib=%zu tag=0x%x dptr=0x%llx fill_s=%.3f verify_bad=%zu\n", mib, tag,
           (unsigned long long)d, t1 - t0, bad);
    fflush(stdout);
    if (hold > 0) {
        signal(SIGTERM, on_term);
        signal(SIGINT, on_term);
        for (int i = 0; i < hold && !stop; i++)
            sleep(1);
        printf("held_s=%d\n", hold);
    }
    CK(cuMemFree(d));
    return bad ? 2 : 0;
}

static int scan(size_t max_mib)
{
    CUdeviceptr ds[256];
    int n = 0;
    uint32_t *h = malloc(CHUNK);
    if (!h)
        return 1;
    size_t free_b = 0, total_b = 0;
    CK(cuMemGetInfo(&free_b, &total_b));
    CUresult last = CUDA_SUCCESS;
    while (n < 256 && (max_mib == 0 || (size_t)n * 64 < max_mib)) {
        last = cuMemAlloc(&ds[n], CHUNK);
        if (last != CUDA_SUCCESS)
            break;
        n++;
    }
    const char *ls = NULL;
    cuGetErrorName(last, &ls);
    uint32_t tags[MAXTAGS];
    uint64_t counts[MAXTAGS];
    int ntags = 0;
    uint64_t hits = 0, nonzero = 0, words = 0;
    unsigned long long lo = ~0ull, hi = 0;
    double t0 = now();
    for (int c = 0; c < n; c++) {
        if (ds[c] < lo)
            lo = ds[c];
        if (ds[c] + CHUNK > hi)
            hi = ds[c] + CHUNK;
        CK(cuMemcpyDtoH(h, ds[c], CHUNK));
        for (size_t i = 0; i < CHUNK / 4; i++) {
            words++;
            if (h[i])
                nonzero++;
        }
        for (size_t i = 0; i + 3 < CHUNK / 4; i++) {
            if (h[i] != MAGIC0 || h[i + 1] != MAGIC1)
                continue;
            hits++;
            int t;
            for (t = 0; t < ntags && tags[t] != h[i + 2]; t++)
                ;
            if (t == ntags && ntags < MAXTAGS) {
                tags[ntags] = h[i + 2];
                counts[ntags++] = 0;
            }
            if (t < MAXTAGS)
                counts[t]++;
        }
    }
    double t1 = now();
    printf("scan_mib=%d mem_free_mib=%zu mem_total_mib=%zu stop=%s dptr_lo=0x%llx dptr_hi=0x%llx "
           "records=%llu nonzero_words=%llu words=%llu scan_s=%.3f\n",
           n * 64, free_b >> 20, total_b >> 20, ls ? ls : "?", lo, hi,
           (unsigned long long)hits, (unsigned long long)nonzero, (unsigned long long)words,
           t1 - t0);
    for (int t = 0; t < ntags; t++)
        printf("tag=0x%x records=%llu\n", tags[t], (unsigned long long)counts[t]);
    for (int c = 0; c < n; c++)
        cuMemFree(ds[c]);
    return 0;
}

int main(int argc, char **argv)
{
    if (argc < 2)
        goto usage;
    CUdevice dev;
    CUcontext ctx;
    CK(cuInit(0));
    CK(cuDeviceGet(&dev, 0));
#if CUDA_VERSION >= 12050
    CUctxCreateParams cp;
    memset(&cp, 0, sizeof cp);
    CK(cuCtxCreate(&ctx, &cp, 0, dev));
#else
    CK(cuCtxCreate(&ctx, 0, dev));
#endif
    if (!strcmp(argv[1], "fill") && argc == 5)
        return fill(strtoull(argv[2], NULL, 0), (uint32_t)strtoul(argv[3], NULL, 0),
                    atoi(argv[4]));
    if (!strcmp(argv[1], "scan"))
        return scan(argc > 2 ? strtoull(argv[2], NULL, 0) : 0);
usage:
    fprintf(stderr, "usage: p20-patprobe fill <MiB> <tag> <hold_s> | scan [maxMiB]\n");
    return 64;
}
