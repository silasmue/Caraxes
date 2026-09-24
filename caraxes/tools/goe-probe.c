// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
/*
 * goe-probe: read the 615 guest kernel's operational events the way a user tool does, to
 * see the records the device produced reach the guest (docs/caraxes/gsp-plan/events.md,
 * round 2 item 5). Runs in a 615.71.09 guest as root (the 0x90D0 bind is CAP_SYS_ADMIN
 * gated, op_event_buffer_bind.c:323-331).
 *
 * Path: the device writes a GOE record into the kernel's import buffer and posts
 * POST_EVENT; the kernel drains it (gpu_op_event_import.c:1300-1458) into its
 * OpEventLog, which pushes every new group to the kernel-side binds
 * (op_event_log.c:773-990). This probe owns one such bind: an NV_EVENT_BUFFER (0x90CD) on
 * three deviceless system-memory objects of its own (NV01_MEMORY_DEVICELESS 0x90CE under
 * the client: a bind needs an event buffer without hSubDevice, op_event_buffer_bind.c:
 * 392-399), allocated user-read-only as a user client must (event_buffer.c:101-107 refuses
 * the kernel's internal allocation on Linux, :236-240 needs
 * NVOS32_ATTR2_PROTECTION_USER_READ_ONLY) and mapped read-only into this process
 * (NV_ESC_RM_MAP_MEMORY on a fresh control FD, then mmap offset 0), enabled KEEP_OLDEST,
 * and an NV_OPERATIONAL_EVENT_BUFFER_BIND (0x90D0) with the GPU's UUID,
 * GOE records, every level and severity, starting after the bind (the only cursor
 * the kernel takes, :352-356). It then prints every record that arrives within
 * `seconds` (moving the get pointer with UPDATE_GET) and exits 0 when at least one did,
 * 3 when none did.
 *
 * Usage: goe-probe [seconds]   (default 60)
 * Layouts (615.71.09 SDK headers): NVOS64/NVOS54 nvos.h; NV0080_ALLOC_PARAMETERS 56
 * bytes; NV_EVENT_BUFFER_ALLOC_PARAMETERS cl90cd.h:164-180 (72 bytes); the bind params
 * cl90d0.h:157-168 (104 bytes); NV_EVENT_BUFFER_HEADER cl90cd.h:65-71;
 * NV_GPU_OPERATIONAL_EVENT_RECORD cl90d0.h:250-291 (104 bytes); ENABLE_EVENTS
 * ctrl90cd.h:94-97; GPU_GET_GID_INFO ctrl2080gpu.h:1750-1773.
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

#define NV_IOCTL_MAGIC 'F'
#define NV_ESC_RM_ALLOC 0x2b
#define NV_ESC_RM_CONTROL 0x2a
#define NV_ESC_CHECK_VERSION_STR 210
#define NV_ESC_RM_MAP_MEMORY 0x4e
#define NV01_MEMORY_DEVICELESS 0x90ce
/* NV_MEMORY_ALLOCATION_PARAMS (nvos.h, 128 bytes): owner @0, type @4, flags @8, attr @24,
 * attr2 @28, size @64. attr: LOCATION_PCI, COHERENCY_CACHED; attr2:
 * PROTECTION_USER_READ_ONLY (22:22). */
#define MEM_ATTR_PCI_CACHED 0x32000000u
#define MEM_ATTR2_USER_READ_ONLY (1u << 22)
/* NVOS33 flags: ACCESS_READ_ONLY (1:0 = 1), RESERVE_ON_UNMAP_ENABLE (19:19). */
#define MAP_FLAGS_READ_ONLY (1u | (1u << 19))
#define H_MEM_HEADER 0x5e0e0010u
#define H_MEM_RECORDS 0x5e0e0011u
#define H_MEM_VARDATA 0x5e0e0012u
#define NV_RM_API_VERSION_CMD_QUERY '2'

#define NV01_ROOT_CLIENT 0x41
#define NV01_DEVICE_0 0x0080
#define NV20_SUBDEVICE_0 0x2080
#define NV_EVENT_BUFFER 0x90cd
#define NV_OPERATIONAL_EVENT_BUFFER_BIND 0x90d0

#define CMD_GPU_GET_PROBED_IDS 0x214
#define CMD_GPU_ATTACH_IDS 0x215
#define CMD_GPU_GET_GID_INFO 0x2080014a
#define CMD_EVENT_BUFFER_ENABLE_EVENTS 0x90cd0101
#define CMD_EVENT_BUFFER_UPDATE_GET 0x90cd0102

#define GID_FLAGS_SHA1_BINARY 0x2 /* TYPE_SHA1 0 (2:2), FORMAT_BINARY 2 (1:0) */
#define OVERFLOW_KEEP_OLDEST 1
#define RECORD_FORMAT_GOE 1
#define CURSOR_AFTER_BIND 0xffffffffffffffffull

#define MAX_GPUS 32
#define INVALID_ID 0xffffffffu
#define H_DEVICE 0x5e0e0001u
#define H_SUBDEVICE 0x5e0e0002u
#define H_EVBUF 0x5e0e0003u
#define H_BIND 0x5e0e0004u

#define RECORD_SIZE 104
#define RECORD_COUNT 64
#define VARDATA_SIZE 0x4000

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

struct evbuf_params { /* NV_EVENT_BUFFER_ALLOC_PARAMETERS, 72 bytes */
	uint64_t bufferHeader, recordBuffer;
	uint32_t recordSize, recordCount;
	uint64_t vardataBuffer;
	uint32_t vardataBufferSize, recordsFreeThreshold;
	uint64_t notificationHandle;
	uint32_t vardataFreeThreshold, hSubDevice, flags;
	uint32_t hBufferHeader, hRecordBuffer, hVardataBuffer;
};

struct bind_params { /* NV_OPERATIONAL_EVENT_BUFFER_BIND_ALLOC_PARAMETERS, 104 bytes */
	uint32_t hEventBuffer;
	uint8_t recordFormat, scope, minLogLevel, minSeverity, bindId, rsvd[3];
	uint8_t resourceUuid[16];
	uint64_t startingCursor;
	uint64_t capDescriptors[8];
};

struct mem_params { /* NV_MEMORY_ALLOCATION_PARAMS, 128 bytes */
	uint32_t owner, type, flags, width, height;
	int32_t pitch;
	uint32_t attr, attr2, format, comprCovg, zcullCovg, pad;
	uint64_t rangeLo, rangeHi, size, alignment, offset, limit, address;
	uint32_t ctagOffset, hVASpace, internalflags, tag;
	uint32_t numaNode, pad2;
};

struct nvos33_fd { /* nv_ioctl_nvos33_parameters_with_fd, 56 bytes */
	uint32_t hClient, hDevice, hMemory, pad;
	uint64_t offset, length, pLinearAddress;
	uint32_t status, flags;
	int32_t fd, pad2;
};

struct gid_params { /* NV2080_CTRL_GPU_GET_GID_INFO_PARAMS */
	uint32_t index, flags, length;
	uint8_t data[256];
};

struct evbuf_header { /* NV_EVENT_BUFFER_HEADER */
	volatile uint32_t recordGet, recordPut;
	volatile uint64_t recordCount, recordDropcount, vardataDropcount;
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
	printf("goe-probe: alloc %-24s class %#06x: ret %d status %#x\n", what, cls, r, a.status);
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
		printf("goe-probe: control %-22s %#010x: ret %d status %#x\n", what, cmd, r, c.status);
	return (r || c.status) ? -1 : 0;
}

/* One user-read-only deviceless memory object of `size` bytes under the client, mapped
 * read-only into this process. */
static const void *ro_buffer(uint32_t hc, uint32_t h, uint64_t size, const char *what)
{
	struct mem_params mp;
	memset(&mp, 0, sizeof mp);
	mp.owner = 0x5e0e0000u;
	mp.attr = MEM_ATTR_PCI_CACHED;
	mp.attr2 = MEM_ATTR2_USER_READ_ONLY;
	mp.size = size;
	if (rm_alloc(hc, hc, h, NV01_MEMORY_DEVICELESS, &mp, sizeof mp, what))
		return NULL;
	int mfd = open("/dev/nvidiactl", O_RDWR);
	if (mfd < 0)
		return NULL;
	struct nvos33_fd m;
	memset(&m, 0, sizeof m);
	m.hClient = hc;
	m.hDevice = hc;
	m.hMemory = h;
	m.length = size;
	m.flags = MAP_FLAGS_READ_ONLY;
	m.fd = mfd;
	int r = ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_MAP_MEMORY, struct nvos33_fd), &m);
	printf("goe-probe: map %-26s ret %d status %#x\n", what, r, m.status);
	if (r || m.status)
		return NULL;
	void *p = mmap(NULL, size, PROT_READ, MAP_SHARED, mfd, 0);
	return p == MAP_FAILED ? NULL : p;
}

static uint32_t rd32(const uint8_t *b, int o) { uint32_t v; memcpy(&v, b + o, 4); return v; }
static uint64_t rd64(const uint8_t *b, int o) { uint64_t v; memcpy(&v, b + o, 8); return v; }
static uint16_t rd16(const uint8_t *b, int o) { uint16_t v; memcpy(&v, b + o, 2); return v; }

/* One GOE record and its vardata contexts, as gpu_op_event_import.c decodes them. */
static void print_record(const uint8_t *r, const uint8_t *vardata, uint32_t vstart)
{
	char sig[17];
	memcpy(sig, r + 32, 16);
	sig[16] = 0;
	uint32_t vd = rd32(r, 4), total = rd32(r, 68);
	printf("goe-probe: record type %u subtype %u varData %#x trace %#llx instance %llu ts %llu us\n",
	       rd16(r, 0), rd16(r, 2), vd, (unsigned long long)rd64(r, 8),
	       (unsigned long long)rd64(r, 16), (unsigned long long)rd64(r, 24));
	printf("goe-probe:   module '%s' category %#x code %u severity %u attributes %#x group-attributes %#x bindId %u cursor %llu\n",
	       sig, rd16(r, 48), rd16(r, 50), r[52], r[53], r[54], r[55],
	       (unsigned long long)rd64(r, 56));
	printf("goe-probe:   contexts %u size %u group %u/%u scope %u originator %u logLevel %u source %u uuid %02x%02x%02x%02x...\n",
	       rd32(r, 64), total, r[77], r[76], r[96], r[97], r[101], r[102], r[80], r[81], r[82],
	       r[83]);
	if (vd & 1)
		vstart = 0;
	for (uint32_t off = 0; off + 8 <= total && vstart + off + 8 <= VARDATA_SIZE;) {
		const uint8_t *c = vardata + vstart + off;
		uint32_t size = rd32(c, 4);
		printf("goe-probe:   context type %#x version %#x size %u", rd16(c, 0), rd16(c, 2), size);
		if (rd16(c, 0) == 0x8001 && size >= 5) /* legacy Xid: xidCode, message */
			printf(": Xid %u '%.*s'", rd32(c, 8), (int)(size - 4), (const char *)c + 12);
		printf("\n");
		off += (8 + size + 31) & ~31u;
	}
}

int main(int argc, char **argv)
{
	int seconds = argc > 1 ? atoi(argv[1]) : 60;
	fd = open("/dev/nvidiactl", O_RDWR);
	if (fd < 0) {
		perror("goe-probe: /dev/nvidiactl");
		return 1;
	}
	nv_rm_api_version_t ver;
	memset(&ver, 0, sizeof ver);
	ver.cmd = NV_RM_API_VERSION_CMD_QUERY;
	if (ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_CHECK_VERSION_STR, nv_rm_api_version_t), &ver)) {
		perror("goe-probe: version");
		return 1;
	}
	printf("goe-probe: driver %.64s\n", ver.versionString);
	int gfd = open("/dev/nvidia0", O_RDWR);
	if (gfd < 0)
		perror("goe-probe: /dev/nvidia0");

	struct nvos64 root;
	memset(&root, 0, sizeof root);
	root.hClass = NV01_ROOT_CLIENT;
	if (ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_ALLOC, struct nvos64), &root) || root.status) {
		fprintf(stderr, "goe-probe: root client status %#x\n", root.status);
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

	struct gid_params gid;
	memset(&gid, 0, sizeof gid);
	gid.flags = GID_FLAGS_SHA1_BINARY;
	if (rm_ctrl(hc, H_SUBDEVICE, CMD_GPU_GET_GID_INFO, &gid, sizeof gid, "GPU_GET_GID_INFO") ||
	    gid.length != 16) {
		fprintf(stderr, "goe-probe: no binary GPU UUID (length %u)\n", gid.length);
		return 1;
	}

	struct evbuf_header *hdr = (struct evbuf_header *)ro_buffer(hc, H_MEM_HEADER, 4096, "header memory");
	const uint8_t *records = ro_buffer(hc, H_MEM_RECORDS, (RECORD_SIZE * RECORD_COUNT + 4095) & ~4095, "record memory");
	const uint8_t *vardata = ro_buffer(hc, H_MEM_VARDATA, VARDATA_SIZE, "vardata memory");
	printf("goe-probe: header %p records %p vardata %p\n", (void *)hdr, (void *)records,
	       (void *)vardata);
	if (!hdr || !records || !vardata)
		return 1;

	struct evbuf_params eb;
	memset(&eb, 0, sizeof eb);
	eb.recordSize = RECORD_SIZE;
	eb.recordCount = RECORD_COUNT;
	eb.vardataBufferSize = VARDATA_SIZE;
	eb.recordsFreeThreshold = RECORD_COUNT - 1;
	eb.vardataFreeThreshold = 1024;
	eb.hSubDevice = 0; /* deviceless backing; the bind names the GPU by UUID */
	eb.hBufferHeader = H_MEM_HEADER;
	eb.hRecordBuffer = H_MEM_RECORDS;
	eb.hVardataBuffer = H_MEM_VARDATA;
	/* Parent: the client, as the kernel's own import allocates it (the subdevice is a
	 * parameter; a subdevice parent is NV_ERR_INVALID_OBJECT_PARENT). */
	if (rm_alloc(hc, hc, H_EVBUF, NV_EVENT_BUFFER, &eb, sizeof eb, "event buffer"))
		return 1;

	uint32_t enable[2] = { 1, OVERFLOW_KEEP_OLDEST };
	if (rm_ctrl(hc, H_EVBUF, CMD_EVENT_BUFFER_ENABLE_EVENTS, enable, sizeof enable,
		    "ENABLE_EVENTS"))
		return 1;

	struct bind_params bp;
	memset(&bp, 0, sizeof bp);
	bp.hEventBuffer = H_EVBUF;
	bp.recordFormat = RECORD_FORMAT_GOE;
	memcpy(bp.resourceUuid, gid.data, 16);
	bp.startingCursor = CURSOR_AFTER_BIND;
	if (rm_alloc(hc, hc, H_BIND, NV_OPERATIONAL_EVENT_BUFFER_BIND, &bp, sizeof bp, "GOE bind"))
		return 1;
	printf("goe-probe: bound (bindId %u); waiting %d s for operational events\n", bp.bindId,
	       seconds);
	fflush(stdout);

	int seen = 0;
	uint32_t vget = 0;
	time_t end = time(NULL) + seconds;
	while (time(NULL) < end) {
		uint32_t get = hdr->recordGet, put = hdr->recordPut;
		while (get != put) {
			const uint8_t *r = records + (size_t)(get % RECORD_COUNT) * RECORD_SIZE;
			print_record(r, vardata, vget);
			vget = rd32(r, 4) & ~31u;
			get = (get + 1) % RECORD_COUNT;
			seen++;
		}
		if (seen) {
			/* The header is read-only here: UPDATE_GET moves the get pointer. */
			uint32_t upd[2] = { get, vget };
			rm_ctrl(hc, H_EVBUF, CMD_EVENT_BUFFER_UPDATE_GET, upd, sizeof upd, NULL);
		}
		fflush(stdout);
		usleep(200000);
	}
	printf("goe-probe: %d record(s); header recordCount %llu drops %llu/%llu\n", seen,
	       (unsigned long long)hdr->recordCount, (unsigned long long)hdr->recordDropcount,
	       (unsigned long long)hdr->vardataDropcount);
	return seen ? 0 : 3;
}
