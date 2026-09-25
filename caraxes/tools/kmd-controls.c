// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//
// kmd-controls: replay, on the physical card through the host RM, the informational controls
// the Windows KMD (617.14) sends in its adapter-start block (caraxes/runs/win11-2026-09-25/
// night-2026-09-25, Y4-clk64-fbtopo), and print the card's exact answers in the format of
// chips/tu106/recorded-controls.tsv (caraxes/tools/recorded-controls.py): cmd, status,
// request hex, answer hex, comment. The pure-emulation device serves them from that table.
//
// Getters only. Objects: a root client (0x41), the probed GPUs attached, NV01_DEVICE_0,
// NV20_SUBDEVICE_0; freed (root client) at the end. Skipped on purpose: every control that
// sets, initializes or allocates, and the BUS_GET_INFO_V2 *_CLEAR indices (0x20-0x23,
// 0x28-0x2b), which clear the card's PCIe error counters (kern_bus_ctrl.c:228-235).
// DIFR (0x20802801) only with ctrlParamVal SUPPORT_STATUS (3), never ENABLE/DISABLE.
//
// Layouts: 615.71.09 SDK headers (vendor/nvidia-rm-headers/615.71.09), GRMGR/CE/GSP/LPWR from
// the 610.57.04 tree (vendor/open-gpu-kernel-modules). Sizes equal the KMD's paramsSize.
//
// Build: cc -O2 -Wall -o kmd-controls kmd-controls.c
// Run:   nice -n 15 ./kmd-controls > kmd-controls.tsv   (unprivileged; log on stderr)
//        nice -n 15 ./kmd-controls engines2 >> kmd-controls.tsv   (engine ids 0x11-0x19 only)
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#define NV_IOCTL_MAGIC 'F'
#define NV_ESC_RM_FREE 0x29
#define NV_ESC_RM_CONTROL 0x2a
#define NV_ESC_RM_ALLOC 0x2b
#define NV_ESC_CHECK_VERSION_STR 210
#define NV_RM_API_VERSION_CMD_QUERY '2'
#define NV01_ROOT_CLIENT 0x41
#define NV01_DEVICE_0 0x0080
#define NV20_SUBDEVICE_0 0x2080
#define CMD_GPU_GET_PROBED_IDS 0x214
#define CMD_GPU_ATTACH_IDS 0x215
#define MAX_GPUS 32
#define INVALID_ID 0xffffffffu
#define H_DEVICE 0x5eb00001u
#define H_SUBDEVICE 0x5eb00002u

struct nvos00 { uint32_t hRoot, hObjectParent, hObjectOld, status; };
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
typedef struct { uint32_t cmd, reply; char versionString[64]; } nv_rm_api_version_t;
struct dev_params {
    uint32_t deviceId, hClientShare, hTargetClient, hTargetDevice, flags, pad;
    uint64_t vaSpaceSize, vaStartInternal, vaLimitInternal;
    uint32_t vaMode, pad2;
};

static int fd = -1;
static uint32_t hc, hobj[2];
static int issued, nvok;

static void hex(const uint8_t *p, uint32_t n) {
    for (uint32_t i = 0; i < n; i++) printf("%02x", p[i]);
}

static void put32(uint8_t *b, size_t o, uint32_t v) { memcpy(b + o, &v, 4); }
static uint32_t get32(const uint8_t *b, size_t o) { uint32_t v; memcpy(&v, b + o, 4); return v; }

// Issue one control on the device (dev=1) or the subdevice (dev=0) and print the TSV line.
static uint32_t issue(int dev, uint32_t cmd, uint8_t *p, uint32_t n, const char *fmt, ...) {
    uint8_t *req = malloc(n ? n : 1);
    memcpy(req, p, n);
    struct nvos54 c = {hc, hobj[dev], cmd, 0, (uint64_t)(uintptr_t)p, n, 0};
    uint32_t st = ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_CONTROL, struct nvos54), &c) ? 0xffffffffu : c.status;
    char note[256];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(note, sizeof note, fmt, ap);
    va_end(ap);
    printf("%#x\t0x%x\t", cmd, st);
    hex(req, n);
    putchar('\t');
    hex(p, n);
    printf("\t%s (host RM 615.71.09, RTX 2070)\n", note);
    fflush(stdout);
    fprintf(stderr, "kmd-controls: %#010x %-50s status %#x\n", cmd, note, st);
    issued++;
    nvok += st == 0;
    free(req);
    return st;
}

// Index-list controls ({listSize u32, {index u32, data u32}[max]}): one call per index, then
// one call with every index that answered NV_OK alone.
static void index_list(uint32_t cmd, uint32_t size, uint32_t max, const char *name,
                       const uint32_t *idx, int n) {
    uint32_t ok[128];
    int nok = 0;
    for (int i = 0; i < n; i++) {
        uint8_t *b = calloc(1, size);
        put32(b, 0, 1);
        put32(b, 4, idx[i]);
        if (issue(0, cmd, b, size, "%s index %#x", name, idx[i]) == 0 && nok < (int)max) ok[nok++] = idx[i];
        free(b);
    }
    uint8_t *b = calloc(1, size);
    put32(b, 0, nok);
    for (int i = 0; i < nok; i++) put32(b, 4 + 8 * i, ok[i]);
    issue(0, cmd, b, size, "%s all %d indices that answered alone", name, nok);
    free(b);
}

static void simple(int dev, uint32_t cmd, uint32_t size, const char *name) {
    uint8_t *b = calloc(1, size);
    issue(dev, cmd, b, size, "%s", name);
    free(b);
}

int main(int argc, char **argv) {
    fd = open("/dev/nvidiactl", O_RDWR);
    if (fd < 0) { perror("open /dev/nvidiactl"); return 1; }
    nv_rm_api_version_t ver;
    memset(&ver, 0, sizeof ver);
    ver.cmd = NV_RM_API_VERSION_CMD_QUERY;
    if (ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_CHECK_VERSION_STR, nv_rm_api_version_t), &ver) != 0) {
        perror("NV_ESC_CHECK_VERSION_STR");
        return 1;
    }
    int gfd = open("/dev/nvidia0", O_RDWR);
    if (gfd < 0) perror("open /dev/nvidia0");
    struct nvos64 root;
    memset(&root, 0, sizeof root);
    root.hClass = NV01_ROOT_CLIENT;
    if (ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_ALLOC, struct nvos64), &root) != 0 || root.status) {
        fprintf(stderr, "root client: status %#x errno %d\n", root.status, errno);
        return 1;
    }
    hc = root.hObjectNew;
    uint32_t probed[3 * MAX_GPUS];
    memset(probed, 0, sizeof probed);
    struct nvos54 c = {hc, hc, CMD_GPU_GET_PROBED_IDS, 0, (uint64_t)(uintptr_t)probed, sizeof probed, 0};
    if (ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_CONTROL, struct nvos54), &c) == 0 && c.status == 0) {
        uint32_t at[MAX_GPUS + 1];
        memset(at, 0xff, sizeof at);
        for (int i = 0; i < MAX_GPUS && probed[i] != INVALID_ID; i++) at[i] = probed[i];
        at[MAX_GPUS] = 0;
        struct nvos54 a = {hc, hc, CMD_GPU_ATTACH_IDS, 0, (uint64_t)(uintptr_t)at, sizeof at, 0};
        ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_CONTROL, struct nvos54), &a);
        fprintf(stderr, "kmd-controls: gpu id %#x attach status %#x\n", probed[0], a.status);
    }
    struct dev_params dp;
    memset(&dp, 0, sizeof dp);
    struct nvos64 d = {hc, hc, H_DEVICE, NV01_DEVICE_0, (uint64_t)(uintptr_t)&dp, 0, sizeof dp, 0, 0};
    uint32_t sub = 0;
    struct nvos64 s = {hc, H_DEVICE, H_SUBDEVICE, NV20_SUBDEVICE_0, (uint64_t)(uintptr_t)&sub, 0, sizeof sub, 0, 0};
    if (ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_ALLOC, struct nvos64), &d) || d.status ||
        ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_ALLOC, struct nvos64), &s) || s.status) {
        fprintf(stderr, "device/subdevice: status %#x/%#x\n", d.status, s.status);
        goto out;
    }
    hobj[0] = H_SUBDEVICE;
    hobj[1] = H_DEVICE;

    // "engines2" (follow-up, 2026-09-26): only the graphics context buffers 0x11-0x19
    // (BUNDLE_CB .. SETUP), lines appended to the same TSV.
    if (argc > 1 && !strcmp(argv[1], "engines2")) {
        for (uint32_t e = 0x11; e <= 0x19; e++) {
            uint8_t b[12] = {0};
            put32(b, 0, e);
            issue(1, 0x801707, b, 12, "NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES engine %#x", e);
        }
        goto out;
    }
    printf("# SPDX-License-Identifier: MIT\n");
    printf("# Generated by caraxes/tools/kmd-controls.c on the host RTX 2070 (TU106, host RM 615.71.09); do not edit.\n");
    printf("# cmd\tstatus\trequest\tanswer\tcontrol (the Windows 617.14 KMD adapter-start block, replayed as a user client)\n");

    // Windows-only controls ahead of the KMD block.
    simple(0, 0x20800aaf, 3, "NV2080_CTRL_CMD_INTERNAL_GET_ENABLED_SEC2_CLASSES");
    simple(1, 0x800292, 804, "NV0080_CTRL_CMD_GPU_GET_CLASSLIST_V2");

    // FB_GET_INFO_V2: every index of ctrl2080fb.h (0x00-0x44; 0x2c-0x2e are not defined).
    uint32_t fbi[0x45];
    int nfb = 0;
    for (uint32_t i = 0; i <= 0x44; i++)
        if (i < 0x2c || i > 0x2e) fbi[nfb++] = i;
    index_list(0x20801303, 1028, 0x80, "NV2080_CTRL_CMD_FB_GET_INFO_V2", fbi, nfb);

    simple(1, 0x801306, 88, "NV0080_CTRL_CMD_FB_GET_COMPBIT_STORE_INFO");

    // BUS_GET_INFO_V2: every index of ctrl2080bus.h (0x00-0x33) except the *_CLEAR ones.
    uint32_t bi[0x34];
    int nb = 0;
    for (uint32_t i = 0; i <= 0x33; i++)
        if (!((i >= 0x20 && i <= 0x23) || (i >= 0x28 && i <= 0x2b))) bi[nb++] = i;
    index_list(0x20801823, 420, 0x34, "NV2080_CTRL_CMD_BUS_GET_INFO_V2", bi, nb);

    simple(0, 0x20800ab8, 4, "NV2080_CTRL_CMD_INTERNAL_GET_PCIE_P2P_CAPS");
    simple(0, 0x20800173, 12, "NV2080_CTRL_CMD_GPU_QUERY_FUNCTION_STATUS");
    simple(0, 0x20801315, 16, "NV2080_CTRL_CMD_FB_GET_GPU_CACHE_INFO");
    for (uint32_t l = 0; l < 5; l++) {
        uint8_t b[8] = {0};
        put32(b, 0, l);
        issue(0, 0x20802a12, b, 8, "NV2080_CTRL_CMD_CE_IS_DECOMP_LCE_ENABLED lce %u", l);
    }
    {
        uint8_t b[580] = {0};
        put32(b, 0, 1);
        put32(b, 4, 0x11);
        issue(0, 0x20800102, b, sizeof b, "NV2080_CTRL_CMD_GPU_GET_INFO_V2 index 0x11");
    }
    simple(1, 0x801c02, 12, "NV0080_CTRL_CMD_NVDEC_GET_CAPS_V2");
    simple(1, 0x801b02, 12, "NV0080_CTRL_CMD_NVENC_GET_CAPS_V2");
    simple(0, 0x20803601, 72, "NV2080_CTRL_CMD_GSP_GET_FEATURES");
    simple(0, 0x20800160, 16, "NV2080_CTRL_CMD_GPU_GET_VPR_CAPS");
    {
        uint8_t b[4] = {0};
        put32(b, 0, 3);
        issue(0, 0x20802801, b, 4, "NV2080_CTRL_CMD_LPWR_DIFR_CTRL SUPPORT_STATUS");
    }
    for (uint32_t e = 0; e <= 0x10; e++) {
        uint8_t b[12] = {0};
        put32(b, 0, e);
        issue(1, 0x801707, b, 12, "NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES engine %#x", e);
    }
    simple(0, 0x2080012f, 1664, "NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS");
    {
        uint8_t b[40] = {0};
        put32(b, 0, 4);
        issue(0, 0x2080121f, b, 40, "NV2080_CTRL_CMD_GR_GFX_POOL_QUERY_SIZE maxSlots 4");
    }

    // GRMGR_GET_GR_FS_INFO ({numQueries u16, rsvd[6], {queryType u16, rsvd[2], status u32,
    // data[12]}[96]}, 1928 bytes): the KMD's (0 queries), each query alone, then all together.
    {
        struct { uint16_t ty; uint32_t a, b; const char *what; } q[64];
        int nq = 0;
        q[nq++] = (typeof(q[0])){1, 0, 0, "GPC_COUNT"};
        for (uint32_t g = 0; g < 4; g++) {
            q[nq++] = (typeof(q[0])){2, g, 0, "CHIPLET_GPC_MAP"};
            q[nq++] = (typeof(q[0])){3, g, 0, "TPC_MASK"};
            q[nq++] = (typeof(q[0])){4, g, 0, "PPC_MASK"};
            q[nq++] = (typeof(q[0])){10, g, 0, "ROP_MASK"};
            q[nq++] = (typeof(q[0])){5, 0, g, "PARTITION_CHIPLET_GPC_MAP"};
        }
        q[nq++] = (typeof(q[0])){6, 0, 0, "CHIPLET_SYSPIPE_MASK"};
        q[nq++] = (typeof(q[0])){7, 0, 0, "PARTITION_CHIPLET_SYSPIPE_IDS"};
        q[nq++] = (typeof(q[0])){8, 0, 0, "PROFILER_MON_GPC_MASK"};
        q[nq++] = (typeof(q[0])){9, 0, 0, "PARTITION_SYSPIPE_ID"};
        q[nq++] = (typeof(q[0])){11, 0, 0, "CHIPLET_GRAPHICS_SYSPIPE_MASK"};
        q[nq++] = (typeof(q[0])){12, 0, 0, "GFX_CAPABLE_GPC_MASK"};
        uint8_t *b = calloc(1, 1928);
        issue(0, 0x20803801, b, 1928, "NV2080_CTRL_CMD_GRMGR_GET_GR_FS_INFO 0 queries (the KMD's request)");
        for (int i = 0; i < nq; i++) {
            memset(b, 0, 1928);
            b[0] = 1;
            memcpy(b + 8, &q[i].ty, 2);
            put32(b, 16, q[i].a);
            put32(b, 20, q[i].b);
            issue(0, 0x20803801, b, 1928, "NV2080_CTRL_CMD_GRMGR_GET_GR_FS_INFO %s %u %u", q[i].what, q[i].a, q[i].b);
        }
        memset(b, 0, 1928);
        b[0] = nq;
        for (int i = 0; i < nq; i++) {
            memcpy(b + 8 + 20 * i, &q[i].ty, 2);
            put32(b, 8 + 20 * i + 8, q[i].a);
            put32(b, 8 + 20 * i + 12, q[i].b);
        }
        issue(0, 0x20803801, b, 1928, "NV2080_CTRL_CMD_GRMGR_GET_GR_FS_INFO all %d queries", nq);
        free(b);
    }

    // Internal static KGR getters the same run answered NOT_SUPPORTED (before the KMD block).
    simple(0, 0x20800a30, 2048, "NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_PPC_MASKS");
    simple(0, 0x20800a2c, 320, "NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_ZCULL_INFO");
    simple(0, 0x20800a2e, 96, "NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_ROP_INFO");
    simple(0, 0x20800a34, 72, "NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER");
    simple(0, 0x20800b03, 16352, "NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER_V2");
    simple(0, 0x20800b05, 16352, "NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_THROTTLE_CTRL");
    simple(0, 0x20800a3f, 256, "NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_FECS_TRACE_DEFINES");

    (void)get32;
out:;
    struct nvos00 f = {hc, hc, hc, 0};
    int r = ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_FREE, struct nvos00), &f);
    fprintf(stderr, "kmd-controls: %d controls issued, %d NV_OK; root client freed: ret %d status %#x\n",
            issued, nvok, r, f.status);
    if (gfd >= 0) close(gfd);
    close(fd);
    return 0;
}
