// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
/*
 * census-cu: one CUDA driver-API operation at a time, for the trap census
 * (docs/caraxes/optimization-backlog.md). Every repetition of an operation is
 * bracketed by two marker writes to a latched BAR0 scratch register of the Caraxes
 * device (NV_PBUS_SW_SCRATCH(30), BAR0 0x1478), so the device trace shows exactly
 * which trapped accesses and which device work fall inside the operation
 * (caraxes/tools/census-table.py). Natively (no marker path) the same code only
 * times the operations.
 *
 *   census-cu init            cuInit alone (the process ends after it)
 *   census-cu ctx [reps]      cuCtxCreate and cuCtxDestroy, each marked
 *   census-cu ops [reps]      everything else in one context: see the table below
 *   census-cu ptx2cubin PTX OUT   link PTX to a cubin (cuLink*), for the module-load op
 *
 * Environment: CENSUS_MARK=<BAR0 resource file> (the guest's resource0; root) turns the
 * markers on. CENSUS_GAP_MS (default 30) is the idle time between two repetitions,
 * outside every marked window. CENSUS_PTX (default ./kernels.ptx) holds the spin
 * kernel, CENSUS_CUBIN (default ./census.cubin) the cubin of the module-load op.
 *
 * Marker value: 0xce000000 | op << 12 | rep << 1 | end (end 0 at the start, 1 at the
 * end). Output: one "OP name=... id=... reps=... med_us=... min_us=... max_us=..." line
 * per operation (per repetition time, the marker writes are outside it), errors on
 * stderr with exit 1. Build (guest, as bench-cu):
 *   gcc -O2 -Wall -o census-cu census-cu.c -I<dir of cuda.h> -L/opt/nvrm/lib -lcuda
 */
#include <cuda.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

#define MARK_OFF 0x1478u

static volatile uint32_t *bar0;

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

static void sleep_ms(long ms)
{
	struct timespec ts = { ms / 1000, (ms % 1000) * 1000000L };
	nanosleep(&ts, NULL);
}

static long gap_ms(void)
{
	const char *e = getenv("CENSUS_GAP_MS");
	return e ? atol(e) : 30;
}

static void mark_open(void)
{
	const char *p = getenv("CENSUS_MARK");
	if (!p || !*p)
		return;
	int fd = open(p, O_RDWR | O_SYNC);
	if (fd < 0) {
		perror("CENSUS_MARK open");
		exit(1);
	}
	void *m = mmap(NULL, 1 << 16, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
	if (m == MAP_FAILED) {
		perror("CENSUS_MARK mmap");
		exit(1);
	}
	bar0 = m;
}

static void mark(unsigned op, unsigned rep, unsigned end)
{
	if (bar0) {
		__sync_synchronize();
		bar0[MARK_OFF / 4] = 0xce000000u | (op & 0xfff) << 12 | (rep & 0x7ff) << 1 | (end & 1);
		__sync_synchronize();
	}
}

static int cmp_d(const void *a, const void *b)
{
	double x = *(const double *)a, y = *(const double *)b;
	return (x > y) - (x < y);
}

static void report(const char *name, unsigned op, double *t, int n, int per)
{
	qsort(t, n, sizeof *t, cmp_d);
	double med = n % 2 ? t[n / 2] : (t[n / 2 - 1] + t[n / 2]) / 2;
	printf("OP name=%s id=%u reps=%d per_rep=%d med_us=%.1f min_us=%.1f max_us=%.1f\n", name, op, n, per,
	       med, t[0], t[n - 1]);
	fflush(stdout);
}

/* Operation ids (the census table's keys). */
enum {
	OP_IDLE = 1,
	OP_INIT = 2,
	OP_CTX_CREATE = 3,
	OP_CTX_DESTROY = 4,
	OP_ALLOC_1M = 10,
	OP_FREE_1M = 11,
	OP_ALLOC_64M = 12,
	OP_FREE_64M = 13,
	OP_HOSTALLOC_1M = 14,
	OP_FREEHOST_1M = 15,
	OP_HOSTREG_1M = 16,
	OP_HOSTUNREG_1M = 17,
	OP_LAUNCH_1 = 20,
	OP_LAUNCH_1000 = 21,
	OP_H2D_1M = 30,
	OP_D2H_1M = 31,
	OP_D2D_1M = 32,
	OP_H2D_16M = 33,
	OP_D2H_16M = 34,
	OP_D2D_16M = 35,
	OP_H2D_1M_PAGEABLE = 36,
	OP_D2H_1M_PAGEABLE = 37,
	OP_STREAM_SYNC = 40,
	OP_EVENT = 41,
	OP_MODULE_LOAD = 50,
	OP_MODULE_UNLOAD = 51,
};

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

/* An idle window of 100 ms: the background rate of trapped accesses (interrupt
 * servicing of other clients' work, the guest's timers) that every window carries. */
static void idle_windows(int reps)
{
	double *t = malloc(sizeof(double) * reps);
	for (int r = 0; r < reps; r++) {
		mark(OP_IDLE, r, 0);
		double a = now_us();
		sleep_ms(100);
		t[r] = now_us() - a;
		mark(OP_IDLE, r, 1);
		sleep_ms(gap_ms());
	}
	report("idle_100ms", OP_IDLE, t, reps, 1);
	free(t);
}

static int do_init(void)
{
	mark_open();
	double t[1];
	mark(OP_INIT, 0, 0);
	double a = now_us();
	ck(cuInit(0), "cuInit");
	t[0] = now_us() - a;
	mark(OP_INIT, 0, 1);
	report("cuInit", OP_INIT, t, 1, 1);
	return 0;
}

static int do_ctx(int reps)
{
	mark_open();
	ck(cuInit(0), "cuInit");
	CUdevice dev;
	ck(cuDeviceGet(&dev, 0), "cuDeviceGet");
	double *tc = malloc(sizeof(double) * reps), *td = malloc(sizeof(double) * reps);
	for (int r = 0; r < reps; r++) {
		CUcontext ctx;
		sleep_ms(gap_ms());
		mark(OP_CTX_CREATE, r, 0);
		double a = now_us();
#if CUDA_VERSION >= 12050
		CUctxCreateParams p;
		memset(&p, 0, sizeof p);
		ck(cuCtxCreate(&ctx, &p, 0, dev), "cuCtxCreate");
#else
		ck(cuCtxCreate(&ctx, 0, dev), "cuCtxCreate");
#endif
		tc[r] = now_us() - a;
		mark(OP_CTX_CREATE, r, 1);
		sleep_ms(gap_ms());
		mark(OP_CTX_DESTROY, r, 0);
		a = now_us();
		ck(cuCtxDestroy(ctx), "cuCtxDestroy");
		td[r] = now_us() - a;
		mark(OP_CTX_DESTROY, r, 1);
	}
	report("cuCtxCreate", OP_CTX_CREATE, tc, reps, 1);
	report("cuCtxDestroy", OP_CTX_DESTROY, td, reps, 1);
	return 0;
}

/* n allocations of mib each, every one marked, then the frees in reverse order. */
static void alloc_free(unsigned op_a, unsigned op_f, const char *na, const char *nf, size_t mib, int n)
{
	CUdeviceptr *d = calloc(n, sizeof *d);
	double *ta = malloc(sizeof(double) * n), *tf = malloc(sizeof(double) * n);
	for (int i = 0; i < n; i++) {
		sleep_ms(gap_ms());
		mark(op_a, i, 0);
		double a = now_us();
		ck(cuMemAlloc(&d[i], mib << 20), na);
		ta[i] = now_us() - a;
		mark(op_a, i, 1);
	}
	for (int i = n - 1; i >= 0; i--) {
		sleep_ms(gap_ms());
		mark(op_f, i, 0);
		double a = now_us();
		ck(cuMemFree(d[i]), nf);
		tf[i] = now_us() - a;
		mark(op_f, i, 1);
	}
	report(na, op_a, ta, n, 1);
	report(nf, op_f, tf, n, 1);
	free(d);
	free(ta);
	free(tf);
}

static void host_alloc(int n)
{
	void **h = calloc(n, sizeof *h);
	double *ta = malloc(sizeof(double) * n), *tf = malloc(sizeof(double) * n);
	for (int i = 0; i < n; i++) {
		sleep_ms(gap_ms());
		mark(OP_HOSTALLOC_1M, i, 0);
		double a = now_us();
		ck(cuMemHostAlloc(&h[i], 1 << 20, 0), "cuMemHostAlloc");
		ta[i] = now_us() - a;
		mark(OP_HOSTALLOC_1M, i, 1);
	}
	for (int i = n - 1; i >= 0; i--) {
		sleep_ms(gap_ms());
		mark(OP_FREEHOST_1M, i, 0);
		double a = now_us();
		ck(cuMemFreeHost(h[i]), "cuMemFreeHost");
		tf[i] = now_us() - a;
		mark(OP_FREEHOST_1M, i, 1);
	}
	report("cuMemHostAlloc_1M", OP_HOSTALLOC_1M, ta, n, 1);
	report("cuMemFreeHost_1M", OP_FREEHOST_1M, tf, n, 1);
	free(h);
	free(ta);
	free(tf);
}

static void host_register(int n)
{
	void **h = calloc(n, sizeof *h);
	double *ta = malloc(sizeof(double) * n), *tf = malloc(sizeof(double) * n);
	for (int i = 0; i < n; i++) {
		if (posix_memalign(&h[i], 4096, 1 << 20)) {
			fprintf(stderr, "ERROR posix_memalign\n");
			exit(1);
		}
		memset(h[i], 0, 1 << 20);
	}
	for (int i = 0; i < n; i++) {
		sleep_ms(gap_ms());
		mark(OP_HOSTREG_1M, i, 0);
		double a = now_us();
		ck(cuMemHostRegister(h[i], 1 << 20, 0), "cuMemHostRegister");
		ta[i] = now_us() - a;
		mark(OP_HOSTREG_1M, i, 1);
	}
	for (int i = n - 1; i >= 0; i--) {
		sleep_ms(gap_ms());
		mark(OP_HOSTUNREG_1M, i, 0);
		double a = now_us();
		ck(cuMemHostUnregister(h[i]), "cuMemHostUnregister");
		tf[i] = now_us() - a;
		mark(OP_HOSTUNREG_1M, i, 1);
	}
	report("cuMemHostRegister_1M", OP_HOSTREG_1M, ta, n, 1);
	report("cuMemHostUnregister_1M", OP_HOSTUNREG_1M, tf, n, 1);
	for (int i = 0; i < n; i++)
		free(h[i]);
	free(h);
	free(ta);
	free(tf);
}

static void copies(int n)
{
	const size_t sizes[] = { 1, 16 };
	const unsigned ops[2][3] = { { OP_H2D_1M, OP_D2H_1M, OP_D2D_1M }, { OP_H2D_16M, OP_D2H_16M, OP_D2D_16M } };
	const char *names[2][3] = { { "h2d_pinned_1M", "d2h_pinned_1M", "d2d_1M" },
				    { "h2d_pinned_16M", "d2h_pinned_16M", "d2d_16M" } };
	double *t = malloc(sizeof(double) * n);
	for (int s = 0; s < 2; s++) {
		size_t bytes = sizes[s] << 20;
		CUdeviceptr d0, d1;
		void *pinned, *pageable = malloc(bytes);
		ck(cuMemAlloc(&d0, bytes), "copy alloc d0");
		ck(cuMemAlloc(&d1, bytes), "copy alloc d1");
		ck(cuMemHostAlloc(&pinned, bytes, 0), "copy hostalloc");
		memset(pinned, 0x5a, bytes);
		memset(pageable, 0xa5, bytes);
		for (int dir = 0; dir < 3; dir++) {
			/* one warm-up copy outside the markers */
			for (int r = -1; r < n; r++) {
				if (r >= 0) {
					sleep_ms(gap_ms());
					mark(ops[s][dir], r, 0);
				}
				double a = now_us();
				if (dir == 0)
					ck(cuMemcpyHtoD(d0, pinned, bytes), "HtoD");
				else if (dir == 1)
					ck(cuMemcpyDtoH(pinned, d0, bytes), "DtoH");
				else
					ck(cuMemcpyDtoD(d1, d0, bytes), "DtoD");
				ck(cuCtxSynchronize(), "copy sync");
				double b = now_us();
				if (r >= 0) {
					mark(ops[s][dir], r, 1);
					t[r] = b - a;
				}
			}
			report(names[s][dir], ops[s][dir], t, n, 1);
		}
		if (s == 0) {
			for (int dir = 0; dir < 2; dir++) {
				unsigned op = dir ? OP_D2H_1M_PAGEABLE : OP_H2D_1M_PAGEABLE;
				for (int r = -1; r < n; r++) {
					if (r >= 0) {
						sleep_ms(gap_ms());
						mark(op, r, 0);
					}
					double a = now_us();
					if (dir == 0)
						ck(cuMemcpyHtoD(d0, pageable, bytes), "HtoD pageable");
					else
						ck(cuMemcpyDtoH(pageable, d0, bytes), "DtoH pageable");
					ck(cuCtxSynchronize(), "copy sync");
					double b = now_us();
					if (r >= 0) {
						mark(op, r, 1);
						t[r] = b - a;
					}
				}
				report(dir ? "d2h_pageable_1M" : "h2d_pageable_1M", op, t, n, 1);
			}
		}
		cuMemFreeHost(pinned);
		free(pageable);
		cuMemFree(d0);
		cuMemFree(d1);
	}
	free(t);
}

static int do_ops(int reps)
{
	mark_open();
	ck(cuInit(0), "cuInit");
	CUcontext ctx = make_ctx();
	(void)ctx;

	/* the spin kernel of nvprobe's kernels.ptx, 1000 cycles: nvprobe 4's short kernel */
	const char *ptxp = getenv("CENSUS_PTX") ? getenv("CENSUS_PTX") : "./kernels.ptx";
	FILE *f = fopen(ptxp, "rb");
	if (!f) {
		perror(ptxp);
		return 1;
	}
	static char ptx[1 << 16];
	size_t np = fread(ptx, 1, sizeof ptx - 1, f);
	fclose(f);
	ptx[np] = 0;
	CUmodule mod;
	CUfunction fspin;
	ck(cuModuleLoadData(&mod, ptx), "cuModuleLoadData");
	ck(cuModuleGetFunction(&fspin, mod, "spin"), "cuModuleGetFunction spin");
	CUdeviceptr out;
	ck(cuMemAlloc(&out, 4096), "cuMemAlloc out");
	long long cycles = 1000;
	void *sargs[] = { &cycles, &out };
	ck(cuLaunchKernel(fspin, 1, 1, 1, 32, 1, 1, 0, 0, sargs, NULL), "spin warm-up");
	ck(cuCtxSynchronize(), "spin warm-up sync");

	idle_windows(reps);

	alloc_free(OP_ALLOC_1M, OP_FREE_1M, "cuMemAlloc_1M", "cuMemFree_1M", 1, reps * 2);
	alloc_free(OP_ALLOC_64M, OP_FREE_64M, "cuMemAlloc_64M", "cuMemFree_64M", 64, reps);
	host_alloc(reps);
	host_register(reps);

	/* single launch + cuCtxSynchronize after an idle gap: the reactor may be asleep */
	double *t = malloc(sizeof(double) * (reps * 2 > 1000 ? reps * 2 : 1000));
	for (int r = 0; r < reps * 2; r++) {
		sleep_ms(gap_ms());
		mark(OP_LAUNCH_1, r, 0);
		double a = now_us();
		ck(cuLaunchKernel(fspin, 1, 1, 1, 32, 1, 1, 0, 0, sargs, NULL), "cuLaunchKernel");
		ck(cuCtxSynchronize(), "cuCtxSynchronize");
		t[r] = now_us() - a;
		mark(OP_LAUNCH_1, r, 1);
	}
	report("launch_1_sync", OP_LAUNCH_1, t, reps * 2, 1);

	/* 1000 launches back to back, each followed by cuCtxSynchronize (short-auto) */
	sleep_ms(gap_ms());
	mark(OP_LAUNCH_1000, 0, 0);
	for (int r = 0; r < 1000; r++) {
		double a = now_us();
		ck(cuLaunchKernel(fspin, 1, 1, 1, 32, 1, 1, 0, 0, sargs, NULL), "cuLaunchKernel");
		ck(cuCtxSynchronize(), "cuCtxSynchronize");
		t[r] = now_us() - a;
	}
	mark(OP_LAUNCH_1000, 0, 1);
	report("launch_1000_sync", OP_LAUNCH_1000, t, 1000, 1000);

	copies(reps);

	/* stream synchronise of an idle stream; event record + synchronise */
	CUstream st;
	CUevent ev;
	ck(cuStreamCreate(&st, CU_STREAM_NON_BLOCKING), "cuStreamCreate");
	ck(cuEventCreate(&ev, CU_EVENT_DEFAULT), "cuEventCreate");
	ck(cuStreamSynchronize(st), "warm-up");
	for (int r = 0; r < reps * 2; r++) {
		sleep_ms(gap_ms());
		mark(OP_STREAM_SYNC, r, 0);
		double a = now_us();
		ck(cuStreamSynchronize(st), "cuStreamSynchronize");
		t[r] = now_us() - a;
		mark(OP_STREAM_SYNC, r, 1);
	}
	report("cuStreamSynchronize_idle", OP_STREAM_SYNC, t, reps * 2, 1);
	ck(cuEventRecord(ev, st), "warm-up");
	ck(cuEventSynchronize(ev), "warm-up");
	for (int r = 0; r < reps * 2; r++) {
		sleep_ms(gap_ms());
		mark(OP_EVENT, r, 0);
		double a = now_us();
		ck(cuEventRecord(ev, st), "cuEventRecord");
		ck(cuEventSynchronize(ev), "cuEventSynchronize");
		t[r] = now_us() - a;
		mark(OP_EVENT, r, 1);
	}
	report("cuEventRecord_Synchronize", OP_EVENT, t, reps * 2, 1);
	cuEventDestroy(ev);
	cuStreamDestroy(st);

	/* cuModuleLoad of a small cubin file, then cuModuleUnload */
	const char *cub = getenv("CENSUS_CUBIN") ? getenv("CENSUS_CUBIN") : "./census.cubin";
	double *tu = malloc(sizeof(double) * reps);
	for (int r = 0; r < reps; r++) {
		CUmodule m;
		sleep_ms(gap_ms());
		mark(OP_MODULE_LOAD, r, 0);
		double a = now_us();
		ck(cuModuleLoad(&m, cub), "cuModuleLoad");
		t[r] = now_us() - a;
		mark(OP_MODULE_LOAD, r, 1);
		sleep_ms(gap_ms());
		mark(OP_MODULE_UNLOAD, r, 0);
		a = now_us();
		ck(cuModuleUnload(m), "cuModuleUnload");
		tu[r] = now_us() - a;
		mark(OP_MODULE_UNLOAD, r, 1);
	}
	report("cuModuleLoad_cubin", OP_MODULE_LOAD, t, reps, 1);
	report("cuModuleUnload", OP_MODULE_UNLOAD, tu, reps, 1);
	free(tu);
	free(t);
	cuMemFree(out);
	cuModuleUnload(mod);
	return 0;
}

static int do_ptx2cubin(const char *in, const char *outp)
{
	ck(cuInit(0), "cuInit");
	CUcontext ctx = make_ctx();
	FILE *f = fopen(in, "rb");
	if (!f) {
		perror(in);
		return 1;
	}
	static char ptx[1 << 16];
	size_t np = fread(ptx, 1, sizeof ptx - 1, f);
	fclose(f);
	ptx[np] = 0;
	CUlinkState ls;
	void *cubin;
	size_t sz;
	ck(cuLinkCreate(0, NULL, NULL, &ls), "cuLinkCreate");
	ck(cuLinkAddData(ls, CU_JIT_INPUT_PTX, ptx, np + 1, "kernels.ptx", 0, NULL, NULL), "cuLinkAddData");
	ck(cuLinkComplete(ls, &cubin, &sz), "cuLinkComplete");
	FILE *o = fopen(outp, "wb");
	if (!o || fwrite(cubin, 1, sz, o) != sz) {
		perror(outp);
		return 1;
	}
	fclose(o);
	cuLinkDestroy(ls);
	cuCtxDestroy(ctx);
	printf("CUBIN bytes=%zu out=%s\n", sz, outp);
	return 0;
}

int main(int argc, char **argv)
{
	const char *m = argc > 1 ? argv[1] : "";
	int reps = argc > 2 ? atoi(argv[2]) : 10;
	if (reps < 1)
		reps = 1;
	if (!strcmp(m, "init"))
		return do_init();
	if (!strcmp(m, "ctx"))
		return do_ctx(reps);
	if (!strcmp(m, "ops"))
		return do_ops(reps);
	if (!strcmp(m, "ptx2cubin") && argc > 3)
		return do_ptx2cubin(argv[2], argv[3]);
	fprintf(stderr, "usage: census-cu init | ctx [reps] | ops [reps] | ptx2cubin PTX OUT\n");
	return 2;
}
