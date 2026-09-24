// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
/*
 * rc-journal-probe: the device's RC journal read back through the guest kernel, the way a
 * user tool reads the RC error list (docs/caraxes/gsp-plan/c-guest-state.md §8, the
 * reconciliation of 2026-09-25). Runs in the guest as root.
 *
 * The guest kernel forwards the three controls to the device on a GSP client
 * (kernel_rc_ctrl.c:103-165: RC_GET_ERROR_COUNT and RC_GET_ERROR_V2 go to the physical RM
 * API; RC_SET_CLEAN_ERROR_HISTORY is routed to GSP). The device answers them from its one
 * RC journal (crates/vfio-user-nvgpu/src/rc.rs). A user process cannot log an Xid itself
 * (INTERNAL_LOG_OOB_XID is INTERNAL, sent by the guest kernel for its own Xids only), so
 * the records come from the device: start it with CARAXES_RC_SELFTEST=<seconds> for a
 * SIMULATED record every so often, or let a real RC happen.
 *
 * Steps: wait up to `seconds` for RC_GET_ERROR_COUNT >= 1; read every record with
 * RC_GET_ERROR_V2 (outputRecordSize and the RmProtoBuf_RECORD header, rmcd.h:108-111);
 * RC_SET_CLEAN_ERROR_HISTORY; RC_GET_ERROR_COUNT again. Exit 0 when the count reached 1,
 * record 0 came back with a header, the record past the last came back empty, and the
 * count after the clean was lower than before it (0 unless a new record arrived in
 * between); 3 when no record arrived; 1 on an ioctl failure.
 *
 * Usage: rc-journal-probe [seconds]   (default 60)
 * Layouts (610.57.04 and 615.71.09 SDK headers, the same): NVOS64/NVOS54 nvos.h;
 * NV0080_ALLOC_PARAMETERS 56 bytes; NV2080_CTRL_RC_GET_ERROR_COUNT_PARAMS and
 * NV2080_CTRL_RC_GET_ERROR_V2_PARAMS ctrl2080rc.h:90-129.
 */
#define _GNU_SOURCE
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <time.h>
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
#define CMD_RC_GET_ERROR_COUNT 0x20802205
#define CMD_RC_SET_CLEAN_ERROR_HISTORY 0x20802207
#define CMD_RC_GET_ERROR_V2 0x20802213

#define MAX_GPUS 32
#define H_DEVICE 0x5e0f0001u
#define H_SUBDEVICE 0x5e0f0002u
#define RC_ERROR_PARAMS_BUFFER_SIZE 0x2000

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

struct error_v2 { /* NV2080_CTRL_RC_GET_ERROR_V2_PARAMS, 8200 bytes */
	uint32_t whichBuffer, outputRecordSize;
	uint8_t recordBuffer[RC_ERROR_PARAMS_BUFFER_SIZE];
};

static int fd = -1;

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
	printf("rc-journal-probe: alloc %-10s class %#06x: ret %d status %#x\n", what, cls, r, a.status);
	return (r || a.status) ? -1 : 0;
}

static int rm_ctrl(uint32_t client, uint32_t obj, uint32_t cmd, void *p, uint32_t size,
		   const char *what)
{
	struct nvos54 c;
	memset(&c, 0, sizeof c);
	c.hClient = client;
	c.hObject = obj;
	c.cmd = cmd;
	c.params = (uint64_t)(uintptr_t)p;
	c.paramsSize = size;
	int r = ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_CONTROL, struct nvos54), &c);
	if (what)
		printf("rc-journal-probe: control %-26s %#010x: ret %d status %#x\n", what, cmd, r, c.status);
	return (r || c.status) ? -1 : 0;
}

static int error_count(uint32_t hc, uint32_t *count)
{
	*count = 0xffffffffu;
	return rm_ctrl(hc, H_SUBDEVICE, CMD_RC_GET_ERROR_COUNT, count, sizeof *count, NULL);
}

int main(int argc, char **argv)
{
	int seconds = argc > 1 ? atoi(argv[1]) : 60;
	fd = open("/dev/nvidiactl", O_RDWR);
	if (fd < 0) {
		perror("rc-journal-probe: /dev/nvidiactl");
		return 1;
	}
	nv_rm_api_version_t ver;
	memset(&ver, 0, sizeof ver);
	ver.cmd = NV_RM_API_VERSION_CMD_QUERY;
	if (ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_CHECK_VERSION_STR, nv_rm_api_version_t), &ver)) {
		perror("rc-journal-probe: version");
		return 1;
	}
	printf("rc-journal-probe: driver %.64s\n", ver.versionString);
	int gfd = open("/dev/nvidia0", O_RDWR);
	if (gfd < 0)
		perror("rc-journal-probe: /dev/nvidia0");

	struct nvos64 root;
	memset(&root, 0, sizeof root);
	root.hClass = NV01_ROOT_CLIENT;
	if (ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_ALLOC, struct nvos64), &root) || root.status) {
		fprintf(stderr, "rc-journal-probe: root client status %#x\n", root.status);
		return 1;
	}
	uint32_t hc = root.hObjectNew;
	uint32_t probed[3 * MAX_GPUS];
	memset(probed, 0, sizeof probed);
	rm_ctrl(hc, hc, CMD_GPU_GET_PROBED_IDS, probed, sizeof probed, "GPU_GET_PROBED_IDS");
	uint32_t attach[MAX_GPUS + 1];
	memset(attach, 0xff, sizeof attach);
	attach[0] = probed[0];
	attach[MAX_GPUS] = 0;
	rm_ctrl(hc, hc, CMD_GPU_ATTACH_IDS, attach, sizeof attach, "GPU_ATTACH_IDS");
	struct dev_params dp;
	memset(&dp, 0, sizeof dp);
	if (rm_alloc(hc, hc, H_DEVICE, NV01_DEVICE_0, &dp, sizeof dp, "device"))
		return 1;
	uint32_t sub = 0;
	if (rm_alloc(hc, H_DEVICE, H_SUBDEVICE, NV20_SUBDEVICE_0, &sub, sizeof sub, "subdevice"))
		return 1;

	uint32_t count = 0, first = 0;
	if (error_count(hc, &first)) {
		fprintf(stderr, "rc-journal-probe: RC_GET_ERROR_COUNT failed\n");
		return 1;
	}
	printf("rc-journal-probe: error count at start %u; waiting up to %d s for a record\n", first,
	       seconds);
	fflush(stdout);
	count = first;
	time_t end = time(NULL) + seconds;
	while (count < 1 && time(NULL) < end) {
		sleep(1);
		if (error_count(hc, &count))
			return 1;
	}
	printf("rc-journal-probe: error count %u\n", count);
	if (count < 1)
		return 3;

	/* Every record up to the first empty one (a record may arrive meanwhile: the walk
	 * then goes on to it). */
	static struct error_v2 v2;
	uint32_t headers = 0;
	for (uint32_t i = 0; i < 512; i++) {
		memset(&v2, 0xee, sizeof v2);
		v2.whichBuffer = i;
		if (rm_ctrl(hc, H_SUBDEVICE, CMD_RC_GET_ERROR_V2, &v2, sizeof v2, NULL))
			return 1;
		const uint8_t *r = v2.recordBuffer;
		uint32_t dw;
		memcpy(&dw, r + 4, 4);
		printf("rc-journal-probe: record %u: outputRecordSize %u", i, v2.outputRecordSize);
		if (v2.outputRecordSize >= 8)
			printf(" group %u type %u wRecordSize %u dwSize %u", r[0], r[1], r[2] | r[3] << 8, dw);
		printf("\n");
		if (v2.outputRecordSize == 0)
			break;
		headers++;
	}
	if (error_count(hc, &count))
		return 1;

	uint32_t before = count, after = 0;
	if (rm_ctrl(hc, H_SUBDEVICE, CMD_RC_SET_CLEAN_ERROR_HISTORY, NULL, 0,
		    "RC_SET_CLEAN_ERROR_HISTORY"))
		return 1;
	if (error_count(hc, &after))
		return 1;
	printf("rc-journal-probe: error count %u before the clean, %u after; %u record header(s)\n",
	       before, after, headers);
	return (headers >= 1 && after < before) ? 0 : 1;
}
