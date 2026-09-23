// SPDX-License-Identifier: MIT
/*
 * bench-cu: CUDA driver-API microbenchmarks for the P13 benchmark
 * (docs/caraxes/benchmark.md). One binary, built once in the guest and copied
 * to the host, so native, v1 and v2 run the same code against the same
 * libcuda (610.57.04).
 *
 *   bench-cu init             time cuInit, cuDeviceGet, cuCtxCreate, cuCtxDestroy
 *   bench-cu copy [reps]      H2D, D2H, D2D at 1, 16, 256 MiB; pinned and pageable host
 *                             memory; synchronous cuMemcpy*, each copy timed alone
 *   bench-cu churn [cycles]   fixed VRAM churn: per cycle 64 x 1 MiB and 8 x 64 MiB
 *                             cuMemAlloc then cuMemFree in reverse order
 *
 * Output: one "KEY k=v ..." line per result on stdout (medians of the reps within
 * the run); errors on stderr, exit 1. Build (guest):
 *   gcc -O2 -o bench-cu bench-cu.c -I<dir of cuda.h> -L/opt/nvrm/lib -lcuda
 */
#include <cuda.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static void ck(CUresult r, const char *what)
{
	if (r == CUDA_SUCCESS)
		return;
	const char *s = NULL;
	cuGetErrorString(r, &s);
	fprintf(stderr, "ERROR %s: %d %s\n", what, r, s ? s : "?");
	exit(1);
}

static double now_us(void)
{
	struct timespec ts;
	clock_gettime(CLOCK_MONOTONIC, &ts);
	return ts.tv_sec * 1e6 + ts.tv_nsec / 1e3;
}

static int cmp_d(const void *a, const void *b)
{
	double x = *(const double *)a, y = *(const double *)b;
	return (x > y) - (x < y);
}

static double median(double *v, int n)
{
	qsort(v, n, sizeof *v, cmp_d);
	return n % 2 ? v[n / 2] : (v[n / 2 - 1] + v[n / 2]) / 2;
}

static CUcontext make_ctx(void)
{
	CUdevice dev;
	CUcontext ctx;
	ck(cuDeviceGet(&dev, 0), "cuDeviceGet");
#if CUDA_VERSION >= 12050
	CUctxCreateParams p;
	memset(&p, 0, sizeof p);
	ck(cuCtxCreate(&ctx, &p, 0, dev), "cuCtxCreate");
#else
	ck(cuCtxCreate(&ctx, 0, dev), "cuCtxCreate");
#endif
	return ctx;
}

static int do_init(void)
{
	double t0 = now_us();
	ck(cuInit(0), "cuInit");
	double t1 = now_us();
	CUdevice dev;
	ck(cuDeviceGet(&dev, 0), "cuDeviceGet");
	double t2 = now_us();
	CUcontext ctx;
#if CUDA_VERSION >= 12050
	CUctxCreateParams p;
	memset(&p, 0, sizeof p);
	ck(cuCtxCreate(&ctx, &p, 0, dev), "cuCtxCreate");
#else
	ck(cuCtxCreate(&ctx, 0, dev), "cuCtxCreate");
#endif
	double t3 = now_us();
	ck(cuCtxDestroy(ctx), "cuCtxDestroy");
	double t4 = now_us();
	printf("INIT cuinit_ms=%.3f devget_ms=%.3f ctxcreate_ms=%.3f ctxdestroy_ms=%.3f\n",
	       (t1 - t0) / 1e3, (t2 - t1) / 1e3, (t3 - t2) / 1e3, (t4 - t3) / 1e3);
	return 0;
}

enum { H2D, D2H, D2D };
static const char *dirname_[] = { "h2d", "d2h", "d2d" };

static int do_copy(int reps_base)
{
	ck(cuInit(0), "cuInit");
	CUcontext ctx = make_ctx();
	const size_t sizes_mib[] = { 1, 16, 256 };
	for (unsigned si = 0; si < 3; si++) {
		size_t bytes = sizes_mib[si] << 20;
		/* fewer reps for the big size, at least 5 */
		int reps = sizes_mib[si] == 1 ? reps_base * 5 : sizes_mib[si] == 16 ? reps_base * 2 : reps_base;
		if (reps < 5)
			reps = 5;
		CUdeviceptr d0, d1;
		ck(cuMemAlloc(&d0, bytes), "cuMemAlloc d0");
		ck(cuMemAlloc(&d1, bytes), "cuMemAlloc d1");
		void *pinned;
		ck(cuMemHostAlloc(&pinned, bytes, 0), "cuMemHostAlloc");
		void *pageable = malloc(bytes);
		if (!pageable) {
			fprintf(stderr, "ERROR malloc\n");
			return 1;
		}
		/* pattern and a roundtrip check per size */
		for (size_t i = 0; i < bytes / 4; i++) {
			((uint32_t *)pageable)[i] = (uint32_t)(i * 2654435761u);
			((uint32_t *)pinned)[i] = (uint32_t)(i * 2654435761u);
		}
		double *t = malloc(sizeof(double) * reps);
		for (int mem = 0; mem < 2; mem++) {
			void *h = mem ? pageable : pinned;
			const char *mname = mem ? "pageable" : "pinned";
			for (int dir = 0; dir < 3; dir++) {
				if (dir == D2D && mem == 1)
					continue; /* D2D has no host side: report once */
				for (int r = -1; r < reps; r++) { /* r = -1: warmup */
					double a = now_us();
					if (dir == H2D)
						ck(cuMemcpyHtoD(d0, h, bytes), "HtoD");
					else if (dir == D2H)
						ck(cuMemcpyDtoH(h, d0, bytes), "DtoH");
					else
						ck(cuMemcpyDtoD(d1, d0, bytes), "DtoD");
					ck(cuCtxSynchronize(), "sync");
					double b = now_us();
					if (r >= 0)
						t[r] = b - a;
				}
				double med = median(t, reps);
				printf("BW dir=%s mem=%s size_mib=%zu reps=%d med_us=%.1f min_us=%.1f gbps=%.3f\n",
				       dirname_[dir], dir == D2D ? "device" : mname, sizes_mib[si], reps,
				       med, t[0], bytes / med / 1e3);
			}
		}
		/* correctness: D2D copy of d0 (last H2D/D2H roundtrip) back to host */
		memset(pageable, 0, bytes);
		ck(cuMemcpyDtoH(pageable, d1, bytes), "check DtoH");
		size_t bad = 0;
		for (size_t i = 0; i < bytes / 4; i++)
			bad += ((uint32_t *)pageable)[i] != (uint32_t)(i * 2654435761u);
		printf("CHECK size_mib=%zu bad_words=%zu\n", sizes_mib[si], bad);
		free(t);
		free(pageable);
		cuMemFreeHost(pinned);
		cuMemFree(d0);
		cuMemFree(d1);
		if (bad)
			return 1;
	}
	cuCtxDestroy(ctx);
	return 0;
}

static int do_churn(int cycles)
{
	ck(cuInit(0), "cuInit");
	CUcontext ctx = make_ctx();
	struct {
		int n;
		size_t mib;
	} pat[] = { { 64, 1 }, { 8, 64 } };
	for (unsigned p = 0; p < 2; p++) {
		int n = pat[p].n;
		CUdeviceptr *d = calloc(n, sizeof *d);
		double *ta = malloc(sizeof(double) * n * cycles);
		double *tf = malloc(sizeof(double) * n * cycles);
		double *tc = malloc(sizeof(double) * cycles);
		int k = 0;
		for (int c = 0; c < cycles; c++) {
			double c0 = now_us();
			for (int i = 0; i < n; i++) {
				double a = now_us();
				ck(cuMemAlloc(&d[i], pat[p].mib << 20), "churn alloc");
				ta[k + i] = now_us() - a;
			}
			for (int i = n - 1; i >= 0; i--) {
				double a = now_us();
				ck(cuMemFree(d[i]), "churn free");
				tf[k + i] = now_us() - a;
			}
			tc[c] = now_us() - c0;
			k += n;
		}
		double sum = 0;
		for (int c = 0; c < cycles; c++)
			sum += tc[c];
		printf("CHURN blocks=%d block_mib=%zu cycles=%d alloc_med_us=%.1f free_med_us=%.1f cycle_med_ms=%.3f total_ms=%.1f\n",
		       n, pat[p].mib, cycles, median(ta, k), median(tf, k), median(tc, cycles) / 1e3,
		       sum / 1e3);
		free(d);
		free(ta);
		free(tf);
		free(tc);
	}
	cuCtxDestroy(ctx);
	return 0;
}

int main(int argc, char **argv)
{
	const char *m = argc > 1 ? argv[1] : "";
	if (!strcmp(m, "init"))
		return do_init();
	if (!strcmp(m, "copy"))
		return do_copy(argc > 2 ? atoi(argv[2]) : 10);
	if (!strcmp(m, "churn"))
		return do_churn(argc > 2 ? atoi(argv[2]) : 20);
	fprintf(stderr, "usage: bench-cu init|copy [reps]|churn [cycles]\n");
	return 2;
}
