// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
/*
 * hostreg-read: read BAR0 registers of a host GPU through the host RM as a client, READ-ONLY
 * (the Windows guest's first contact, caraxes/runs/win11-2026-09-25/README.md). The host's
 * BAR0 is not readable otherwise: sysfs resource0 is claimed exclusively by nvidia (EINVAL)
 * and /dev/mem is strict (EPERM).
 *
 * Path: the version handshake, /dev/nvidia0 held open, a root client (0x41), the probed GPU
 * ids attached, a device and a subdevice (as rc-journal-probe.c), then
 * NV2080_CTRL_CMD_GPU_EXEC_REG_OPS (0x20800122) with NV2080_CTRL_GPU_REG_OP_READ_32
 * operations of type GLOBAL only, 64 per call, bNonTransactional = 1 so that a refused
 * offset does not fail its batch. The op type is READ_32 in every entry; the tool has no
 * write path. Access control (610.57.04 open sources): gpuValidateRegOffset
 * (gpu_access.c:1212-1236) lets an administrator read any offset below the BAR0 size and
 * holds everyone else to the user register access map GSP-RM hands out
 * (gpu_register_access_map.c:137-172, :208-254); a refused offset comes back with
 * regStatus INVALID_OFFSET (gpuValidateRegOps, subdevice_ctrl_gpu_regops.c:59-104). On a GSP client the
 * validated list goes to GSP-RM as RPC 50 GPU_EXEC_REG_OPS.
 *
 * Usage: hostreg-read [-g index] [-f file] [offset|start-end ...]
 *   offsets in hex; `start-end` reads every dword in [start, end) (end exclusive); a file
 *   holds one offset or range per line, `#` starts a comment. -g picks the probed GPU
 *   (default 0).
 * Output (stdout): "0x%08x 0x%08x" (offset value) per successful read; a refused or failed
 * op prints "0x%08x ! status 0x%02x (<names>)". Diagnostics go to stderr.
 * Exit: 0 every op read, 2 some ops refused, 1 setup or ioctl failure.
 * Layouts (610.57.04 and 615.71.09 SDK headers, the same): NVOS64/NVOS54 nvos.h;
 * NV0080_ALLOC_PARAMETERS 56 bytes; NV2080_CTRL_GPU_REG_OP (32 bytes) and
 * NV2080_CTRL_GPU_EXEC_REG_OPS_PARAMS (48 bytes, regOps at 24, grRouteInfo at 32)
 * ctrl2080gpu.h:650-751.
 * Build: gcc -O2 -Wall -o hostreg-read hostreg-read.c
 */
#define _GNU_SOURCE
#include <ctype.h>
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#define NV_IOCTL_MAGIC 'F'
#define NV_ESC_RM_ALLOC 0x2b
#define NV_ESC_RM_CONTROL 0x2a
#define NV_ESC_CHECK_VERSION_STR 210
#define NV_RM_API_VERSION_CMD_QUERY '2'

#define NV01_ROOT_CLIENT 0x41
#define NV01_DEVICE_0 0x0080
#define NV20_SUBDEVICE_0 0x2080

#define CMD_GPU_GET_PROBED_IDS 0x214
#define CMD_GPU_ATTACH_IDS 0x215
#define CMD_GPU_EXEC_REG_OPS 0x20800122

#define REG_OP_READ_32 0x00
#define REG_OP_TYPE_GLOBAL 0x00

#define MAX_GPUS 32
#define H_DEVICE 0x5e0f0001u
#define H_SUBDEVICE 0x5e0f0002u
#define BATCH 64
#define MAX_OFFSETS (1u << 22)

typedef struct {
	uint32_t cmd, reply;
	char versionString[64];
} nv_rm_api_version_t;

struct nvos64 {
	uint32_t hRoot, hObjectParent, hObjectNew, hClass;
	uint64_t pAllocParms, pRightsRequested;
	uint32_t paramsSize, flags, status;
};

struct nvos54 {
	uint32_t hClient, hObject, cmd, flags;
	uint64_t params;
	uint32_t paramsSize, status;
};

struct dev_params { /* NV0080_ALLOC_PARAMETERS */
	uint32_t deviceId, hClientShare, hTargetClient, hTargetDevice, flags, pad;
	uint64_t vaSpaceSize, vaStartInternal, vaLimitInternal;
	uint32_t vaMode, pad2;
};

struct reg_op { /* NV2080_CTRL_GPU_REG_OP, 32 bytes */
	uint8_t regOp, regType, regStatus, regQuad;
	uint32_t regGroupMask, regSubGroupMask, regOffset;
	uint32_t regValueHi, regValueLo, regAndNMaskHi, regAndNMaskLo;
};

struct exec_reg_ops { /* NV2080_CTRL_GPU_EXEC_REG_OPS_PARAMS, 48 bytes */
	uint32_t hClientTarget, hChannelTarget, bNonTransactional, reserved00[2], regOpCount;
	uint64_t regOps;
	uint32_t grFlags, grPad;
	uint64_t grRoute;
};

_Static_assert(sizeof(struct reg_op) == 32, "NV2080_CTRL_GPU_REG_OP");
_Static_assert(sizeof(struct exec_reg_ops) == 48, "NV2080_CTRL_GPU_EXEC_REG_OPS_PARAMS");

static int fd = -1;
static uint32_t *offs;
static size_t noffs;

static int rm_alloc(uint32_t root, uint32_t parent, uint32_t h, uint32_t cls, void *p, uint32_t size,
		    const char *what)
{
	struct nvos64 a;
	memset(&a, 0, sizeof a);
	a.hRoot = root;
	a.hObjectParent = parent;
	a.hObjectNew = h;
	a.hClass = cls;
	a.pAllocParms = (uint64_t)(uintptr_t)p;
	a.paramsSize = size;
	int r = ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_ALLOC, struct nvos64), &a);
	fprintf(stderr, "hostreg-read: alloc %-10s class %#06x: ret %d status %#x\n", what, cls, r,
		a.status);
	return (r || a.status) ? -1 : 0;
}

static int rm_ctrl(uint32_t client, uint32_t obj, uint32_t cmd, void *p, uint32_t size,
		   uint32_t *status)
{
	struct nvos54 c;
	memset(&c, 0, sizeof c);
	c.hClient = client;
	c.hObject = obj;
	c.cmd = cmd;
	c.params = (uint64_t)(uintptr_t)p;
	c.paramsSize = size;
	int r = ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_CONTROL, struct nvos54), &c);
	if (status)
		*status = r ? 0xffffffffu : c.status;
	return (r || c.status) ? -1 : 0;
}

static void add(uint32_t off)
{
	if (noffs >= MAX_OFFSETS) {
		fprintf(stderr, "hostreg-read: more than %u offsets\n", MAX_OFFSETS);
		exit(1);
	}
	offs[noffs++] = off & ~3u;
}

/* One token: "off" or "start-end" (end exclusive). */
static int parse(const char *s)
{
	char *e;
	errno = 0;
	unsigned long a = strtoul(s, &e, 16);
	if (errno || e == s)
		return -1;
	if (*e == '-') {
		const char *t = e + 1;
		unsigned long b = strtoul(t, &e, 16);
		if (errno || e == t || b < a || b > 0x1000000ul)
			return -1;
		for (unsigned long o = a & ~3ul; o < b; o += 4)
			add((uint32_t)o);
	} else {
		add((uint32_t)a);
	}
	while (*e && isspace((unsigned char)*e))
		e++;
	return *e ? -1 : 0;
}

static int parse_file(const char *path)
{
	FILE *f = fopen(path, "r");
	char line[256];
	if (!f) {
		perror(path);
		return -1;
	}
	while (fgets(line, sizeof line, f)) {
		char *h = strchr(line, '#');
		if (h)
			*h = 0;
		char *s = line;
		while (*s && isspace((unsigned char)*s))
			s++;
		if (!*s)
			continue;
		if (parse(s)) {
			fprintf(stderr, "hostreg-read: bad line in %s: %s", path, line);
			fclose(f);
			return -1;
		}
	}
	fclose(f);
	return 0;
}

static void status_names(uint8_t s, char *buf, size_t n)
{
	static const char *names[] = { "INVALID_OP",		"INVALID_TYPE", "INVALID_OFFSET",
				       "UNSUPPORTED_OP", "INVALID_MASK", "NOACCESS" };
	buf[0] = 0;
	for (int i = 0; i < 6; i++)
		if (s & (1u << i))
			snprintf(buf + strlen(buf), n - strlen(buf), "%s%s", buf[0] ? "|" : "", names[i]);
	if (!buf[0])
		snprintf(buf, n, "?");
}

int main(int argc, char **argv)
{
	int gpu = 0, opt;
	offs = calloc(MAX_OFFSETS, sizeof *offs);
	if (!offs)
		return 1;
	while ((opt = getopt(argc, argv, "g:f:")) != -1) {
		if (opt == 'g')
			gpu = atoi(optarg);
		else if (opt == 'f') {
			if (parse_file(optarg))
				return 1;
		} else {
			fprintf(stderr, "usage: hostreg-read [-g index] [-f file] [offset|start-end ...]\n");
			return 1;
		}
	}
	for (int i = optind; i < argc; i++)
		if (parse(argv[i])) {
			fprintf(stderr, "hostreg-read: bad offset or range: %s\n", argv[i]);
			return 1;
		}
	if (!noffs) {
		fprintf(stderr, "hostreg-read: no offsets\n");
		return 1;
	}

	fd = open("/dev/nvidiactl", O_RDWR);
	if (fd < 0) {
		perror("hostreg-read: /dev/nvidiactl");
		return 1;
	}
	nv_rm_api_version_t ver;
	memset(&ver, 0, sizeof ver);
	ver.cmd = NV_RM_API_VERSION_CMD_QUERY;
	if (ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_CHECK_VERSION_STR, nv_rm_api_version_t), &ver)) {
		perror("hostreg-read: version");
		return 1;
	}
	fprintf(stderr, "hostreg-read: driver %.64s, uid %d\n", ver.versionString, (int)geteuid());
	int gfd = open("/dev/nvidia0", O_RDWR);
	if (gfd < 0)
		perror("hostreg-read: /dev/nvidia0");

	struct nvos64 root;
	memset(&root, 0, sizeof root);
	root.hClass = NV01_ROOT_CLIENT;
	if (ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_ALLOC, struct nvos64), &root) || root.status) {
		fprintf(stderr, "hostreg-read: root client status %#x\n", root.status);
		return 1;
	}
	uint32_t hc = root.hObjectNew, st = 0;
	uint32_t probed[3 * MAX_GPUS];
	memset(probed, 0xff, sizeof probed);
	if (rm_ctrl(hc, hc, CMD_GPU_GET_PROBED_IDS, probed, sizeof probed, &st)) {
		fprintf(stderr, "hostreg-read: GPU_GET_PROBED_IDS status %#x\n", st);
		return 1;
	}
	if (gpu < 0 || gpu >= MAX_GPUS || probed[gpu] == 0xffffffffu) {
		fprintf(stderr, "hostreg-read: no probed GPU %d\n", gpu);
		return 1;
	}
	uint32_t attach[MAX_GPUS + 1];
	memset(attach, 0xff, sizeof attach);
	attach[0] = probed[gpu];
	attach[MAX_GPUS] = 0;
	rm_ctrl(hc, hc, CMD_GPU_ATTACH_IDS, attach, sizeof attach, NULL);
	struct dev_params dp;
	memset(&dp, 0, sizeof dp);
	dp.deviceId = (uint32_t)gpu;
	if (rm_alloc(hc, hc, H_DEVICE, NV01_DEVICE_0, &dp, sizeof dp, "device"))
		return 1;
	uint32_t sub = 0;
	if (rm_alloc(hc, H_DEVICE, H_SUBDEVICE, NV20_SUBDEVICE_0, &sub, sizeof sub, "subdevice"))
		return 1;

	size_t ok = 0, refused = 0;
	struct reg_op ops[BATCH];
	for (size_t base = 0; base < noffs; base += BATCH) {
		uint32_t n = (uint32_t)(noffs - base < BATCH ? noffs - base : BATCH);
		memset(ops, 0, sizeof ops);
		for (uint32_t i = 0; i < n; i++) {
			ops[i].regOp = REG_OP_READ_32;
			ops[i].regType = REG_OP_TYPE_GLOBAL;
			ops[i].regOffset = offs[base + i];
			ops[i].regValueLo = 0xdeadbeefu;
		}
		struct exec_reg_ops p;
		memset(&p, 0, sizeof p);
		p.bNonTransactional = 1;
		p.regOpCount = n;
		p.regOps = (uint64_t)(uintptr_t)ops;
		if (rm_ctrl(hc, H_SUBDEVICE, CMD_GPU_EXEC_REG_OPS, &p, sizeof p, &st)) {
			/* The whole batch failed: every op reports the call's status. */
			fprintf(stderr, "hostreg-read: EXEC_REG_OPS at %#x (%u ops): status %#x\n",
				offs[base], n, st);
			for (uint32_t i = 0; i < n; i++)
				printf("0x%08x ! call status 0x%x\n", offs[base + i], st);
			refused += n;
			continue;
		}
		for (uint32_t i = 0; i < n; i++) {
			if (ops[i].regStatus) {
				char names[96];
				status_names(ops[i].regStatus, names, sizeof names);
				printf("0x%08x ! status 0x%02x (%s)\n", ops[i].regOffset, ops[i].regStatus,
				       names);
				refused++;
			} else {
				printf("0x%08x 0x%08x\n", ops[i].regOffset, ops[i].regValueLo);
				ok++;
			}
		}
	}
	fprintf(stderr, "hostreg-read: %zu read, %zu refused\n", ok, refused);
	if (gfd >= 0)
		close(gfd);
	close(fd);
	return refused ? 2 : 0;
}
