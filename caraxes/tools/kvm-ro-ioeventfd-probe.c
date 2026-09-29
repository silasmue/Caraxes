// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
/*
 * kvm-ro-ioeventfd-probe: can a register page be read-only guest memory and still carry a
 * datamatch ioeventfd? (docs/research/trap-census-2026-09-30.md, the usermode page item.)
 *
 * A throwaway VM (/dev/kvm only, no irqchip, no devices): 64 KiB of RAM with 16-bit code at
 * 0x1000, and one 4 KiB KVM_MEM_READONLY slot at 0x20000 standing in for the usermode page
 * (TIME at +0x80, the doorbell at +0x90). A datamatch ioeventfd (0xCA700000, len 4) sits at
 * 0x20090. The guest loops M times over one access; the probe counts userspace exits and
 * eventfd signals and times the loop:
 *   A. reads of +0x80 (the clock word): memory, expect no exit;
 *   B. writes of the token to +0x90: expect the eventfd, no exit;
 *   C. writes of another value to +0x90: expect a KVM_EXIT_MMIO each (the trapped path);
 *   D. writes of the token to 0x30090, outside any slot: the ordinary MMIO ioeventfd, for
 *      comparison.
 * Build: gcc -O2 -Wall -o kvm-ro-ioeventfd-probe kvm-ro-ioeventfd-probe.c
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/kvm.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/eventfd.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

#define RAM_SIZE 0x10000
#define RO_GPA 0x20000ULL
#define OUT_GPA 0x30000ULL
#define TOKEN 0xCA700000U
#define M 200000

static double now_ns(void)
{
	struct timespec t;
	clock_gettime(CLOCK_MONOTONIC, &t);
	return t.tv_sec * 1e9 + t.tv_nsec;
}

static void die(const char *m)
{
	perror(m);
	exit(1);
}

struct vm {
	int kvm, vm, vcpu, efd;
	struct kvm_run *run;
	uint8_t *ram, *ro;
};

static void setup(struct vm *g, uint64_t ioev_gpa)
{
	g->vm = ioctl(g->kvm, KVM_CREATE_VM, 0);
	if (g->vm < 0)
		die("KVM_CREATE_VM");
	g->ram = mmap(NULL, RAM_SIZE, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS, -1, 0);
	g->ro = mmap(NULL, 0x1000, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS, -1, 0);
	if (g->ram == MAP_FAILED || g->ro == MAP_FAILED)
		die("mmap");
	memset(g->ro, 0, 0x1000);
	*(uint32_t *)(g->ro + 0x80) = 0x12345678; /* the clock word, as the device would publish it */
	struct kvm_userspace_memory_region r0 = { .slot = 0, .guest_phys_addr = 0,
		.memory_size = RAM_SIZE, .userspace_addr = (uintptr_t)g->ram };
	struct kvm_userspace_memory_region r1 = { .slot = 1, .flags = KVM_MEM_READONLY,
		.guest_phys_addr = RO_GPA, .memory_size = 0x1000, .userspace_addr = (uintptr_t)g->ro };
	if (ioctl(g->vm, KVM_SET_USER_MEMORY_REGION, &r0) < 0)
		die("SET_USER_MEMORY_REGION ram");
	if (ioctl(g->vm, KVM_SET_USER_MEMORY_REGION, &r1) < 0)
		die("SET_USER_MEMORY_REGION readonly");
	g->efd = eventfd(0, EFD_CLOEXEC | EFD_NONBLOCK);
	struct kvm_ioeventfd a = { .datamatch = TOKEN, .addr = ioev_gpa, .len = 4, .fd = g->efd,
		.flags = KVM_IOEVENTFD_FLAG_DATAMATCH };
	if (ioctl(g->vm, KVM_IOEVENTFD, &a) < 0)
		die("KVM_IOEVENTFD");
	g->vcpu = ioctl(g->vm, KVM_CREATE_VCPU, 0);
	if (g->vcpu < 0)
		die("KVM_CREATE_VCPU");
	int sz = ioctl(g->kvm, KVM_GET_VCPU_MMAP_SIZE, 0);
	g->run = mmap(NULL, sz, PROT_READ | PROT_WRITE, MAP_SHARED, g->vcpu, 0);
	if (g->run == MAP_FAILED)
		die("mmap run");
}

/* 16-bit real-mode loop at 0x1000 over one access of DS:off, DS based at `base`. */
static void code(struct vm *g, int write, uint32_t value, uint16_t off, uint64_t base)
{
	uint8_t *p = g->ram + 0x1000;
	uint32_t m = M;
	int i = 0;
	p[i++] = 0x66; p[i++] = 0xb9; memcpy(p + i, &m, 4); i += 4;       /* mov ecx, M     */
	p[i++] = 0x66; p[i++] = 0xb8; memcpy(p + i, &value, 4); i += 4;   /* mov eax, value */
	int loop = i;
	p[i++] = 0x66; p[i++] = write ? 0xa3 : 0xa1;                      /* mov [off], eax / mov eax, [off] */
	memcpy(p + i, &off, 2); i += 2;
	p[i++] = 0x66; p[i++] = 0x49;                                     /* dec ecx        */
	p[i++] = 0x75; p[i] = (uint8_t)(loop - (i + 1)); i++;             /* jnz loop       */
	p[i++] = 0xf4;                                                    /* hlt            */
	struct kvm_sregs s;
	ioctl(g->vcpu, KVM_GET_SREGS, &s);
	s.cs.base = 0; s.cs.selector = 0;
	s.ds.base = base; s.ds.selector = base >> 4;
	ioctl(g->vcpu, KVM_SET_SREGS, &s);
	struct kvm_regs r = { .rip = 0x1000, .rflags = 2 };
	ioctl(g->vcpu, KVM_SET_REGS, &r);
}

static void run_case(const char *name, int write, uint32_t value, uint16_t off, uint64_t base,
		     uint64_t ioev_gpa)
{
	struct vm g = { .kvm = open("/dev/kvm", O_RDWR | O_CLOEXEC) };
	if (g.kvm < 0)
		die("/dev/kvm");
	setup(&g, ioev_gpa);
	code(&g, write, value, off, base);
	long exits = 0, mmio_w = 0, mmio_r = 0;
	double t0 = now_ns();
	for (;;) {
		if (ioctl(g.vcpu, KVM_RUN, 0) < 0)
			die("KVM_RUN");
		if (g.run->exit_reason == KVM_EXIT_HLT)
			break;
		if (g.run->exit_reason == KVM_EXIT_MMIO) {
			exits++;
			if (g.run->mmio.is_write)
				mmio_w++;
			else
				mmio_r++;
			continue;
		}
		printf("  %s: unexpected exit %u\n", name, g.run->exit_reason);
		return;
	}
	double t1 = now_ns();
	uint64_t ev = 0;
	if (read(g.efd, &ev, 8) != 8)
		ev = 0;
	struct kvm_regs r;
	ioctl(g.vcpu, KVM_GET_REGS, &r);
	printf("  %-58s %7.3f us/access, userspace exits %ld (W %ld, R %ld), eventfd %llu, eax 0x%08llx\n",
	       name, (t1 - t0) / M / 1e3, exits, mmio_w, mmio_r, (unsigned long long)ev,
	       (unsigned long long)(r.rax & 0xffffffff));
	close(g.vcpu);
	close(g.vm);
	close(g.kvm);
}

int main(void)
{
	int kvm = open("/dev/kvm", O_RDWR | O_CLOEXEC);
	if (kvm < 0)
		die("/dev/kvm");
	printf("KVM_CAP_READONLY_MEM %d, KVM_CAP_IOEVENTFD %d; %d accesses per case\n",
	       ioctl(kvm, KVM_CHECK_EXTENSION, KVM_CAP_READONLY_MEM),
	       ioctl(kvm, KVM_CHECK_EXTENSION, KVM_CAP_IOEVENTFD), M);
	close(kvm);
	run_case("A. read of the clock word in the read-only slot", 0, 0, 0x80, RO_GPA, RO_GPA + 0x90);
	run_case("B. token write to +0x90 of the read-only slot (ioeventfd)", 1, TOKEN, 0x90, RO_GPA, RO_GPA + 0x90);
	run_case("C. other value to +0x90 of the read-only slot", 1, 0x00010005, 0x90, RO_GPA, RO_GPA + 0x90);
	run_case("D. token write to 0x30090, no slot (MMIO ioeventfd)", 1, TOKEN, 0x90, OUT_GPA, OUT_GPA + 0x90);
	return 0;
}
