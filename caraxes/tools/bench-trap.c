// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
// Round trip of one trapped MMIO access of the Caraxes device, measured in the guest
// (docs/caraxes/mapping.md §6). Maps BAR0 through sysfs (root) and times N 4-byte
// writes to a latched scratch register and N reads of BOOT_0. Nothing reaches the GPU:
// the device model answers both. Usage: sudo ./bench-trap <pci resource0> [N] [write offset] [read offset]
// The write offset (default 0x1400) selects another BAR0 register for the writes, e.g.
// 0xbb0090, the usermode doorbell, which patch 0005 turns into an ioeventfd.
// P21 (docs/caraxes/bar1-passthrough.md): on resource1 (BAR1) use write offset "none" and
// a read offset, reads only: inside the passthrough area a read reaches VRAM directly,
// outside it traps. Writes to BAR1 would land in the guest's own VRAM, so they are skipped.
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

static double now_us(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec * 1e6 + t.tv_nsec / 1e3;
}

static int cmp(const void *a, const void *b) {
    double x = *(const double *)a, y = *(const double *)b;
    return x < y ? -1 : x > y;
}

int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "usage: %s <resource> [N] [write offset|none] [read offset]\n", argv[0]); return 2; }
    int n = argc > 2 ? atoi(argv[2]) : 10000;
    int writes = !(argc > 3 && strcmp(argv[3], "none") == 0);
    unsigned long woff = (argc > 3 && writes) ? strtoul(argv[3], NULL, 0) : 0x1400;
    unsigned long roff = argc > 4 ? strtoul(argv[4], NULL, 0) : 0;
    if (woff % 4 || woff >= (16 << 20)) { fprintf(stderr, "bad write offset %#lx\n", woff); return 2; }
    if (roff % 4 || roff >= (16 << 20)) { fprintf(stderr, "bad read offset %#lx\n", roff); return 2; }
    int fd = open(argv[1], O_RDWR | O_SYNC);
    if (fd < 0) { perror("open"); return 1; }
    volatile uint32_t *bar = mmap(NULL, 16 << 20, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (bar == MAP_FAILED) { perror("mmap"); return 1; }
    double *w = malloc(sizeof(double) * n), *r = malloc(sizeof(double) * n);
    uint32_t boot0 = 0;
    for (int i = 0; i < n; i++) {
        double t0 = now_us();
        if (writes)
            bar[woff / 4] = i;          /* default NV_PBUS_SW_SCRATCH(0): latched by the model */
        double t1 = now_us();
        boot0 = bar[roff / 4];           /* default NV_PMC_BOOT_0 */
        double t2 = now_us();
        w[i] = t1 - t0; r[i] = t2 - t1;
    }
    qsort(w, n, sizeof(double), cmp);
    qsort(r, n, sizeof(double), cmp);
    printf("TRAP n=%d woff=%s%#lx roff=%#lx value=%#x write_p50_us=%.1f write_p99_us=%.1f read_p50_us=%.1f read_p99_us=%.1f\n",
           n, writes ? "" : "none:", woff, roff, boot0, w[n / 2], w[n * 99 / 100], r[n / 2], r[n * 99 / 100]);
    return 0;
}
