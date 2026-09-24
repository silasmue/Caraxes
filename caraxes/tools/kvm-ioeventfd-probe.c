// SPDX-License-Identifier: MIT
/*
 * kvm-ioeventfd-probe: measures Linux KVM ioeventfd behaviour with
 * KVM_IOEVENTFD_FLAG_DATAMATCH, as input for docs/caraxes/doorbell-token-pool.md
 * (the datamatch token pool of the Caraxes doorbell).
 *
 * It uses only /dev/kvm with a throwaway VM (no irqchip, no devices) and
 * measures:
 *   1. how many datamatch ioeventfds fit at one MMIO address, and the cost
 *      of each registration (KVM copies the io bus array every time);
 *   2. the collision rules at one address (datamatch/wildcard/length);
 *   3. the per-write cost of a guest MMIO write as a function of the number
 *      of registered entries and the position of the matching one, versus a
 *      write that exits to userspace.
 *
 * Build: gcc -O2 -Wall -o kvm-ioeventfd-probe kvm-ioeventfd-probe.c
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/kvm.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/eventfd.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/utsname.h>
#include <time.h>
#include <unistd.h>

#define CAP_ADDR   0xe6bb0090ULL
#define TOKEN_BASE 0xCA700000U
#define CAP_MAX    4096
#define RUN_ADDR   0x20090ULL
#define RAM_SIZE   0x10000
#define CODE_AT    0x1000
#define M_WRITES   200000
#define REPS       3

static int kvm_fd;
static volatile sig_atomic_t timed_out;

static void on_alarm(int sig) { (void)sig; timed_out = 1; }

static double now_ns(void)
{
	struct timespec ts;
	clock_gettime(CLOCK_MONOTONIC, &ts);
	return ts.tv_sec * 1e9 + ts.tv_nsec;
}

static const char *ename(int e)
{
	const char *n = strerrorname_np(e);
	return n ? n : "?";
}

static int new_vm(void)
{
	int vm = ioctl(kvm_fd, KVM_CREATE_VM, 0);
	if (vm < 0) {
		perror("KVM_CREATE_VM");
		exit(1);
	}
	return vm;
}

static int new_efd(void)
{
	int fd = eventfd(0, EFD_CLOEXEC | EFD_NONBLOCK);
	if (fd < 0) {
		perror("eventfd");
		exit(1);
	}
	return fd;
}

/* Returns the ioctl result with errno preserved. dm < 0 means no DATAMATCH. */
static int ioev(int vm, uint64_t addr, uint32_t len, int64_t dm, int fd, int deassign)
{
	struct kvm_ioeventfd a;
	memset(&a, 0, sizeof(a));
	a.addr = addr;
	a.len = len;
	a.fd = fd;
	if (dm >= 0) {
		a.flags |= KVM_IOEVENTFD_FLAG_DATAMATCH;
		a.datamatch = (uint64_t)dm;
	}
	if (deassign)
		a.flags |= KVM_IOEVENTFD_FLAG_DEASSIGN;
	return ioctl(vm, KVM_IOEVENTFD, &a);
}

static int report(const char *what, int r)
{
	int e = errno;
	if (r < 0)
		printf("    %-50s ret=%d errno=%s\n", what, r, ename(e));
	else
		printf("    %-50s ret=%d\n", what, r);
	return r;
}

/* ---------------------------------------------------------------- 1 */

static void part_capacity(void)
{
	static int fds[CAP_MAX];
	static double t[CAP_MAX];
	int vm = new_vm();
	int n = 0, first_err = 0;

	printf("== 1. Capacity: datamatch ioeventfds at one MMIO address ==\n");
	printf("    addr=0x%llx len=4 datamatch=0x%08x+i, up to %d\n",
	       (unsigned long long)CAP_ADDR, TOKEN_BASE, CAP_MAX);
	for (int i = 0; i < CAP_MAX; i++) {
		fds[i] = new_efd();
		double t0 = now_ns();
		int r = ioev(vm, CAP_ADDR, 4, TOKEN_BASE + i, fds[i], 0);
		double t1 = now_ns();
		if (r < 0) {
			first_err = errno;
			close(fds[i]);
			break;
		}
		t[i] = t1 - t0;
		n++;
	}
	printf("    registered: %d\n", n);
	if (first_err)
		printf("    first failure at registration #%d (0-based): errno=%s (%s)\n", n,
		       ename(first_err), strerror(first_err));
	else
		printf("    no failure up to %d\n", CAP_MAX);
	if (n > 0) {
		double sum = 0, f = 0, l = 0;
		int k = n < 16 ? n : 16;
		for (int i = 0; i < n; i++)
			sum += t[i];
		for (int i = 0; i < k; i++) {
			f += t[i];
			l += t[n - k + i];
		}
		printf("    registration time: mean %.1f us, first %d mean %.1f us, last %d mean %.1f us\n",
		       sum / n / 1e3, k, f / k / 1e3, k, l / k / 1e3);
	}
	int dfail = 0, dfirst = 0;
	double d0 = now_ns();
	for (int i = 0; i < n; i++) {
		if (ioev(vm, CAP_ADDR, 4, TOKEN_BASE + i, fds[i], 1) < 0) {
			if (!dfail)
				dfirst = errno;
			dfail++;
		}
		close(fds[i]);
	}
	double d1 = now_ns();
	printf("    deassigned %d, failures %d%s%s, mean %.1f us per deassign\n", n, dfail,
	       dfail ? ", first errno=" : "", dfail ? ename(dfirst) : "",
	       n ? (d1 - d0) / n / 1e3 : 0.0);
	close(vm);
	printf("\n");
}

/* ---------------------------------------------------------------- 2 */

static void part_collisions(void)
{
	int vm, a, b, c;
	const uint64_t A = CAP_ADDR;
	const uint32_t X = TOKEN_BASE;

	printf("== 2. Collision rules at one address (fresh VM per case) ==\n");
	printf("    addr=0x%llx, X=0x%08x\n", (unsigned long long)A, X);

	printf("  (a) same (addr, len 4, datamatch) twice, two eventfds\n");
	vm = new_vm(); a = new_efd(); b = new_efd();
	report("assign len4 dm=X fd1", ioev(vm, A, 4, X, a, 0));
	report("assign len4 dm=X fd2", ioev(vm, A, 4, X, b, 0));
	close(a); close(b); close(vm);

	printf("  (b) wildcard len 4 after datamatch entries\n");
	vm = new_vm(); a = new_efd(); b = new_efd(); c = new_efd();
	report("assign len4 dm=X", ioev(vm, A, 4, X, a, 0));
	report("assign len4 dm=X+1", ioev(vm, A, 4, X + 1, b, 0));
	report("assign len4 wildcard", ioev(vm, A, 4, -1, c, 0));
	close(a); close(b); close(c); close(vm);

	printf("  (c) datamatch after a wildcard\n");
	vm = new_vm(); a = new_efd(); b = new_efd();
	report("assign len4 wildcard", ioev(vm, A, 4, -1, a, 0));
	report("assign len4 dm=X", ioev(vm, A, 4, X, b, 0));
	close(a); close(b); close(vm);

	printf("  (d) len 8 datamatch where len 4 datamatch entries exist\n");
	vm = new_vm(); a = new_efd(); b = new_efd(); c = new_efd();
	report("assign len4 dm=X", ioev(vm, A, 4, X, a, 0));
	report("assign len4 dm=X+1", ioev(vm, A, 4, X + 1, b, 0));
	report("assign len8 dm=X", ioev(vm, A, 8, X, c, 0));
	close(a); close(b); close(c); close(vm);

	printf("  (e) len 0 (any size, no datamatch) where len 4 datamatch exists\n");
	vm = new_vm(); a = new_efd(); b = new_efd(); c = new_efd();
	printf("    KVM_CAP_IOEVENTFD_ANY_LENGTH = %d\n",
	       ioctl(kvm_fd, KVM_CHECK_EXTENSION, KVM_CAP_IOEVENTFD_ANY_LENGTH));
	report("assign len4 dm=X", ioev(vm, A, 4, X, a, 0));
	report("assign len0 (no datamatch)", ioev(vm, A, 0, -1, b, 0));
	report("control: assign len0 at addr+0x100 alone", ioev(vm, A + 0x100, 0, -1, c, 0));
	close(a); close(b); close(c); close(vm);

	printf("  (f) deassign of a datamatch that was never assigned\n");
	vm = new_vm(); a = new_efd(); b = new_efd();
	report("assign len4 dm=X fd1", ioev(vm, A, 4, X, a, 0));
	report("deassign len4 dm=X+5 fd1 (never assigned)", ioev(vm, A, 4, X + 5, a, 1));
	report("deassign len4 dm=X fd2 (right dm, wrong fd)", ioev(vm, A, 4, X, b, 1));
	report("deassign len4 dm=X fd1 (correct)", ioev(vm, A, 4, X, a, 1));
	report("deassign len4 dm=X fd1 again", ioev(vm, A, 4, X, a, 1));
	close(a); close(b); close(vm);
	printf("\n");
}

/* ---------------------------------------------------------------- 3 */

struct rig {
	int vm, vcpu;
	struct kvm_run *run;
	uint8_t *mem;
};

static void rig_init(struct rig *g)
{
	g->vm = new_vm();
	g->mem = mmap(NULL, RAM_SIZE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	if (g->mem == MAP_FAILED) { perror("mmap"); exit(1); }
	struct kvm_userspace_memory_region reg = {
		.slot = 0, .guest_phys_addr = 0, .memory_size = RAM_SIZE,
		.userspace_addr = (uintptr_t)g->mem,
	};
	if (ioctl(g->vm, KVM_SET_USER_MEMORY_REGION, &reg) < 0) { perror("SET_USER_MEMORY_REGION"); exit(1); }
	g->vcpu = ioctl(g->vm, KVM_CREATE_VCPU, 0);
	if (g->vcpu < 0) { perror("KVM_CREATE_VCPU"); exit(1); }
	int sz = ioctl(kvm_fd, KVM_GET_VCPU_MMAP_SIZE, 0);
	g->run = mmap(NULL, sz, PROT_READ | PROT_WRITE, MAP_SHARED, g->vcpu, 0);
	if (g->run == MAP_FAILED) { perror("mmap run"); exit(1); }

	/* Guest code at 0x1000, 16-bit real mode. */
	uint8_t *p = g->mem + CODE_AT;
	uint32_t m = M_WRITES;
	p[0] = 0x66; p[1] = 0xb9; memcpy(p + 2, &m, 4);         /* 0x1000 mov ecx, M          */
	p[6] = 0x66; p[7] = 0xb8;                              /* 0x1006 mov eax, imm32      */
	/* imm32 at 0x1008, patched per run */
	p[12] = 0x66; p[13] = 0xa3; p[14] = 0x90; p[15] = 0x00; /* 0x100c mov [0x0090], eax   */
	p[16] = 0x66; p[17] = 0x49;                            /* 0x1010 dec ecx             */
	p[18] = 0x75; p[19] = (uint8_t)(0x100c - 0x1014);      /* 0x1012 jnz 0x100c (0xf8)   */
	p[20] = 0xf4;                                          /* 0x1014 hlt                 */

	struct kvm_sregs s;
	if (ioctl(g->vcpu, KVM_GET_SREGS, &s) < 0) { perror("GET_SREGS"); exit(1); }
	s.cs.base = 0; s.cs.selector = 0;
	s.ds.base = RUN_ADDR & ~0xffffULL;
	s.ds.selector = (RUN_ADDR & ~0xffffULL) >> 4;
	if (ioctl(g->vcpu, KVM_SET_SREGS, &s) < 0) { perror("SET_SREGS"); exit(1); }
}

/* Returns ns per guest write, or < 0 on error. */
static double rig_run(struct rig *g, uint32_t value, long *mmio_exits)
{
	memcpy(g->mem + CODE_AT + 8, &value, 4);
	struct kvm_regs r;
	memset(&r, 0, sizeof(r));
	r.rip = CODE_AT;
	r.rflags = 2;
	if (ioctl(g->vcpu, KVM_SET_REGS, &r) < 0) { perror("SET_REGS"); exit(1); }

	long exits = 0;
	timed_out = 0;
	alarm(20);
	double t0 = now_ns();
	for (;;) {
		if (ioctl(g->vcpu, KVM_RUN, 0) < 0) {
			int e = errno;
			if (e == EINTR && !timed_out)
				continue;
			alarm(0);
			printf("    KVM_RUN failed: %s%s\n", ename(e), timed_out ? " (timeout)" : "");
			return -1;
		}
		if (g->run->exit_reason == KVM_EXIT_MMIO) {
			exits++;
			continue;
		}
		if (g->run->exit_reason == KVM_EXIT_HLT)
			break;
		alarm(0);
		printf("    unexpected exit reason %u\n", g->run->exit_reason);
		return -1;
	}
	double t1 = now_ns();
	alarm(0);
	*mmio_exits = exits;
	return (t1 - t0) / M_WRITES;
}

static int cmpd(const void *a, const void *b)
{
	double x = *(const double *)a, y = *(const double *)b;
	return x < y ? -1 : x > y;
}

/* efd: the eventfd expected to fire, or -1. */
static void rig_case(struct rig *g, const char *label, uint32_t value, int efd)
{
	double v[REPS];
	long exits = 0;
	uint64_t cnt[REPS];
	for (int i = 0; i < REPS; i++) {
		v[i] = rig_run(g, value, &exits);
		if (v[i] < 0)
			return;
		cnt[i] = 0;
		if (efd >= 0 && read(efd, &cnt[i], 8) != 8)
			cnt[i] = 0;
	}
	qsort(v, REPS, sizeof(v[0]), cmpd);
	printf("    %-28s min %7.1f ns  median %7.1f ns  userspace exits/run %ld", label,
	       v[0], v[REPS / 2], exits);
	if (efd >= 0) {
		printf("  eventfd counter per run:");
		for (int i = 0; i < REPS; i++)
			printf(" %llu", (unsigned long long)cnt[i]);
		printf(" (expect %d)", M_WRITES);
	}
	printf("\n");
}

static void part_exit_cost(void)
{
	static const int Ns[] = { 16, 64, 253, 1000 };
	static int fds[1000];
	struct rig g;
	char lab[64];

	printf("== 3. Exit-path cost per guest MMIO write (M=%d writes, %d reps) ==\n",
	       M_WRITES, REPS);
	printf("    addr=0x%llx (outside the 64 KiB RAM slot), len 4, 32-bit writes\n",
	       (unsigned long long)RUN_ADDR);
	rig_init(&g);

	rig_case(&g, "N=0 (all exit to userspace)", TOKEN_BASE, -1);

	fds[0] = new_efd();
	if (report("assign N=1 dm=0xCA700000", ioev(g.vm, RUN_ADDR, 4, TOKEN_BASE, fds[0], 0)) == 0) {
		rig_case(&g, "N=1 matched", TOKEN_BASE, fds[0]);
		rig_case(&g, "N=1 unmatched", 0xdeadbeef, -1);
		ioev(g.vm, RUN_ADDR, 4, TOKEN_BASE, fds[0], 1);
	}
	close(fds[0]);

	for (unsigned k = 0; k < sizeof(Ns) / sizeof(Ns[0]); k++) {
		int want = Ns[k], n = 0;
		for (int i = 0; i < want; i++) {
			fds[i] = new_efd();
			if (ioev(g.vm, RUN_ADDR, 4, TOKEN_BASE + i, fds[i], 0) < 0) {
				printf("    N=%d: registration #%d failed: %s\n", want, i, ename(errno));
				close(fds[i]);
				break;
			}
			n++;
		}
		if (n == 0)
			continue;
		snprintf(lab, sizeof(lab), "N=%d match first", n);
		rig_case(&g, lab, TOKEN_BASE, fds[0]);
		snprintf(lab, sizeof(lab), "N=%d match last", n);
		rig_case(&g, lab, TOKEN_BASE + n - 1, fds[n - 1]);
		snprintf(lab, sizeof(lab), "N=%d unmatched", n);
		rig_case(&g, lab, 0xdeadbeef, -1);
		for (int i = 0; i < n; i++) {
			if (ioev(g.vm, RUN_ADDR, 4, TOKEN_BASE + i, fds[i], 1) < 0)
				printf("    deassign #%d failed: %s\n", i, ename(errno));
			close(fds[i]);
		}
	}
	close(g.vcpu);
	close(g.vm);
	printf("\n");
}

/* ---------------------------------------------------------------- 4 */

int main(void)
{
	struct utsname u;
	struct sigaction sa;

	setvbuf(stdout, NULL, _IOLBF, 0);
	memset(&sa, 0, sizeof(sa));
	sa.sa_handler = on_alarm; /* no SA_RESTART, so KVM_RUN returns EINTR */
	sigaction(SIGALRM, &sa, NULL);

	kvm_fd = open("/dev/kvm", O_RDWR | O_CLOEXEC);
	if (kvm_fd < 0) { perror("/dev/kvm"); return 1; }

	uname(&u);
	printf("== 4. Host ==\n");
	printf("    kernel release: %s (%s)\n", u.release, u.machine);
	printf("    online CPUs: %ld\n", sysconf(_SC_NPROCESSORS_ONLN));
	printf("    KVM_GET_API_VERSION=%d KVM_CAP_IOEVENTFD=%d\n",
	       ioctl(kvm_fd, KVM_GET_API_VERSION, 0),
	       ioctl(kvm_fd, KVM_CHECK_EXTENSION, KVM_CAP_IOEVENTFD));
	printf("    note: the host may be under load from other VMs; timings are wall clock\n\n");

	part_capacity();
	part_collisions();
	part_exit_cost();
	close(kvm_fd);
	return 0;
}
