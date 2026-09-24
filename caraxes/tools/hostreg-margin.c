// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
/*
 * hostreg-margin: cuMemHostRegister on guest pages that lie in the 2 MiB rounding of the
 * swiotlb alias (docs/caraxes/memory-model.md §2, "The alias in the device"). The guest's
 * DMA table gives those ordinary pages DMA addresses at 0xf0_xxxx_x000; the Xid 31 of
 * 2026-09-24 (cuda-hostreg on vm1) was a pinned buffer that happened to own some of them.
 * hostregprobe hits that by chance; this probe makes it deterministic.
 *
 * It allocates guest RAM in 2 MiB chunks (no THP) until MemAvailable runs low, reads each
 * page's frame from /proc/self/pagemap (needs root), keeps the chunks that own pages in
 * [alias_lo, pool_lo) or [pool_hi, alias_hi), frees the rest, then registers each kept
 * chunk (DEVICEMAP) and runs an HtoD/DtoH round trip over it, checked byte for byte.
 *
 * usage: sudo hostreg-margin <pool_lo hex> <pool_hi hex>   (the guest's "software IO TLB:
 *        mapped [mem A-B]" line; the alias is the pool rounded out to 2 MiB)
 * env:   HRM_RESERVE_MIB (default 384): MemAvailable left to the guest.
 * exit:  0 all round trips correct, 1 a CUDA error or a wrong byte, 2 no margin page found.
 * Build: gcc -O2 -Wall -o hostreg-margin hostreg-margin.c -I<cuda include> -L<libdir> -lcuda
 */

#define _GNU_SOURCE
#include <cuda.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#define CHUNK (2ul << 20)
#define PAGE 4096ul
#define MAXCHUNKS 4096

static const char *es(CUresult r)
{
	const char *s = NULL;
	cuGetErrorString(r, &s);
	return s ? s : "?";
}

static long mem_available_kib(void)
{
	FILE *f = fopen("/proc/meminfo", "r");
	char line[256];
	long v = -1;
	if (!f)
		return -1;
	while (fgets(line, sizeof line, f))
		if (sscanf(line, "MemAvailable: %ld kB", &v) == 1)
			break;
	fclose(f);
	return v;
}

/* Pages of the chunk at `p` whose guest physical address lies in a margin. */
static int margin_pages(int pm, void *p, uint64_t m[2][2], uint64_t *first)
{
	uint64_t ent[CHUNK / PAGE];
	off_t off = (off_t)((uintptr_t)p / PAGE) * 8;
	int n = 0;
	if (pread(pm, ent, sizeof ent, off) != (ssize_t)sizeof ent)
		return -1;
	for (size_t i = 0; i < CHUNK / PAGE; i++) {
		if (!(ent[i] >> 63))
			continue;
		uint64_t gpa = (ent[i] & ((1ull << 55) - 1)) * PAGE;
		for (int k = 0; k < 2; k++)
			if (gpa >= m[k][0] && gpa < m[k][1]) {
				if (!n)
					*first = gpa;
				n++;
			}
	}
	return n;
}

int main(int argc, char **argv)
{
	if (argc != 3) {
		fprintf(stderr, "usage: %s <pool_lo hex> <pool_hi hex>\n", argv[0]);
		return 1;
	}
	uint64_t lo = strtoull(argv[1], NULL, 16), hi = strtoull(argv[2], NULL, 16);
	uint64_t alo = lo & ~(CHUNK - 1), ahi = (hi + CHUNK - 1) & ~(CHUNK - 1);
	uint64_t m[2][2] = { { alo, lo }, { hi, ahi } };
	long reserve = (getenv("HRM_RESERVE_MIB") ? atol(getenv("HRM_RESERVE_MIB")) : 384) * 1024;
	printf("margins [%#lx, %#lx) and [%#lx, %#lx): %lu pages\n", (unsigned long)alo,
	       (unsigned long)lo, (unsigned long)hi, (unsigned long)ahi,
	       (unsigned long)((lo - alo + ahi - hi) / PAGE));

	int pm = open("/proc/self/pagemap", O_RDONLY);
	if (pm < 0) {
		perror("/proc/self/pagemap");
		return 1;
	}
	static void *chunk[MAXCHUNKS];
	static int hits[MAXCHUNKS];
	static uint64_t firstgpa[MAXCHUNKS];
	int n = 0, kept = 0, total = 0;
	while (n < MAXCHUNKS && mem_available_kib() > reserve) {
		void *p = mmap(NULL, CHUNK, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
		if (p == MAP_FAILED)
			break;
		madvise(p, CHUNK, MADV_NOHUGEPAGE);
		memset(p, 0x5a, CHUNK);
		chunk[n] = p;
		hits[n] = margin_pages(pm, p, m, &firstgpa[n]);
		if (hits[n] < 0) {
			perror("pagemap (run as root)");
			return 1;
		}
		total += hits[n];
		n++;
	}
	for (int i = 0; i < n; i++) {
		if (hits[i] > 0) {
			kept++;
		} else {
			munmap(chunk[i], CHUNK);
			chunk[i] = NULL;
		}
	}
	printf("scanned %d chunks (%d MiB): %d margin pages in %d chunks\n", n, n * 2, total, kept);
	if (!kept)
		return 2;

	CUresult r = cuInit(0);
	CUdevice dev;
	CUcontext ctx;
	if (r || (r = cuDeviceGet(&dev, 0))) {
		printf("cuInit/cuDeviceGet: %d %s\n", r, es(r));
		return 1;
	}
#if CUDA_VERSION >= 12050
	CUctxCreateParams cparams;
	memset(&cparams, 0, sizeof cparams);
	r = cuCtxCreate(&ctx, &cparams, 0, dev);
#else
	r = cuCtxCreate(&ctx, 0, dev);
#endif
	if (r) {
		printf("cuCtxCreate: %d %s\n", r, es(r));
		return 1;
	}
	CUdeviceptr d = 0;
	if ((r = cuMemAlloc(&d, CHUNK))) {
		printf("cuMemAlloc: %d %s\n", r, es(r));
		return 1;
	}
	int ok = 0, failed = 0;
	for (int i = 0; i < n && !failed; i++) {
		if (!chunk[i])
			continue;
		unsigned char *p = chunk[i];
		CUdeviceptr dreg = 0;
		r = cuMemHostRegister(p, CHUNK, CU_MEMHOSTREGISTER_DEVICEMAP);
		if (!r)
			r = cuMemHostGetDevicePointer(&dreg, p, 0);
		if (r) {
			printf("chunk %d: HostRegister: %d %s\n", i, r, es(r));
			failed = 1;
			break;
		}
		for (size_t j = 0; j < CHUNK; j++)
			p[j] = (unsigned char)(j * 131 + 7 + i);
		CUresult r1 = cuMemcpyHtoD(d, p, CHUNK);
		memset(p, 0, CHUNK);
		CUresult r2 = r1 ? r1 : cuMemcpyDtoH(p, d, CHUNK);
		size_t bad = 0;
		for (size_t j = 0; j < CHUNK; j++)
			if (p[j] != (unsigned char)(j * 131 + 7 + i))
				bad++;
		printf("chunk %d: %d margin pages (first GPA %#lx), dptr %#llx: HtoD %d %s, DtoH %d %s, bad=%zu %s\n",
		       i, hits[i], (unsigned long)firstgpa[i], (unsigned long long)dreg, r1,
		       r1 ? es(r1) : "ok", r2, r2 ? es(r2) : "ok", bad, bad ? "WRONG" : "correct");
		if (r1 || r2 || bad)
			failed = 1;
		else
			ok++;
		cuMemHostUnregister(p);
	}
	printf("%s: %d of %d margin chunks round-tripped correctly\n",
	       failed ? "FAIL" : "PASS", ok, kept);
	return failed ? 1 : 0;
}
