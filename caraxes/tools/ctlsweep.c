// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//
// ctlsweep: issue, inside the guest, through the stock RM ioctl path, every getter control of
// the generated table (ctlsweep-table.h, caraxes/tools/ctlsweep-gen.py from controls.tsv), once,
// with zeroed parameters of the control's size, on the object it belongs to. The guest kernel
// RM forwards the GSP-routed ones to the device as GSP_RM_CONTROL, so with the device's
// profile on (CARAXES_PROFILE=1) the sweep shows which handlers a user-space client reaches
// (docs/caraxes/coverage-map.md §3). The way crates/vfio-user-nvgpu/examples/qb1-sweep.rs
// sweeps the host's getters, but from the guest's side. Getters only: nothing is set.
//
// The setup is Leandro-Test's rmdirect.c: the version handshake, /dev/nvidia0 held open, a
// root client (0x41), the probed GPU ids attached, a device and a subdevice, then the objects
// some controls live on: NV04_DISPLAY_COMMON, NV20_SUBDEVICE_DIAG, NV40_I2C, GF100_ZBC_CLEAR
// (each optional: a refused allocation skips its controls). Run as root for the priv rows.
//
// Output: one line per control, "id name size object status", then "COV ctlsweep ok N sent,
// M NV_OK, K skipped (no object)". Build: gcc -O2 -Wall -o ctlsweep ctlsweep.c
#define _GNU_SOURCE
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
#define NV04_DISPLAY_COMMON 0x0073
#define NV20_SUBDEVICE_DIAG 0x208f
#define NV40_I2C 0x402c
#define GF100_ZBC_CLEAR 0x9096
#define CMD_GPU_GET_PROBED_IDS 0x214
#define CMD_GPU_ATTACH_IDS 0x215
#define MAX_GPUS 32
#define INVALID_ID 0xffffffffu

enum obj { OBJ_CLIENT, OBJ_DEVICE, OBJ_SUBDEVICE, OBJ_DISPCOMMON, OBJ_DIAG, OBJ_I2C, OBJ_ZBC, OBJ_N };
static const char *OBJ_NAME[OBJ_N] = {"client", "device", "subdevice", "dispcommon", "diag", "i2c", "zbc"};

struct sweep_entry {
    uint32_t id, size;
    enum obj obj;
    const char *name, *priv, *prov;
};
#include "ctlsweep-table.h"

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
typedef struct {
    uint32_t cmd, reply;
    char versionString[64];
} nv_rm_api_version_t;
struct dev_params {
    uint32_t deviceId, hClientShare, hTargetClient, hTargetDevice, flags, pad;
    uint64_t vaSpaceSize, vaStartInternal, vaLimitInternal;
    uint32_t vaMode, pad2;
};

static int fd = -1;

static uint32_t ctrl(uint32_t hc, uint32_t ho, uint32_t cmd, void *p, uint32_t len) {
    struct nvos54 c = {hc, ho, cmd, 0, (uint64_t)(uintptr_t)p, len, 0};
    if (ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_CONTROL, struct nvos54), &c) != 0)
        return 0xffffffffu;
    return c.status;
}

static uint32_t alloc(uint32_t hc, uint32_t parent, uint32_t h, uint32_t cls, void *p, uint32_t len) {
    struct nvos64 a;
    memset(&a, 0, sizeof a);
    a.hRoot = hc;
    a.hObjectParent = parent;
    a.hObjectNew = h;
    a.hClass = cls;
    a.pAllocParms = (uint64_t)(uintptr_t)p;
    a.paramsSize = len;
    if (ioctl(fd, _IOWR(NV_IOCTL_MAGIC, NV_ESC_RM_ALLOC, struct nvos64), &a) != 0) return 0xffffffffu;
    return a.status;
}

int main(int argc, char **argv) {
    const char *only = argc > 1 ? argv[1] : NULL;  // a name substring, for a rerun of one
    fd = open("/dev/nvidiactl", O_RDWR);
    if (fd < 0) {
        perror("open /dev/nvidiactl");
        return 1;
    }
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
    uint32_t hc = root.hObjectNew;
    uint32_t h[OBJ_N] = {hc, 0x5ea00001u, 0x5ea00002u, 0x5ea00003u, 0x5ea00004u, 0x5ea00005u, 0x5ea00006u};
    int have[OBJ_N] = {1};
    uint32_t probed[3 * MAX_GPUS];
    memset(probed, 0, sizeof probed);
    if (ctrl(hc, hc, CMD_GPU_GET_PROBED_IDS, probed, sizeof probed) == 0) {
        uint32_t at[MAX_GPUS + 1];
        memset(at, 0xff, sizeof at);
        for (int i = 0; i < MAX_GPUS && probed[i] != INVALID_ID; i++) at[i] = probed[i];
        at[MAX_GPUS] = 0;
        printf("# attach: status %#x\n", ctrl(hc, hc, CMD_GPU_ATTACH_IDS, at, sizeof at));
    }
    struct dev_params dp;
    memset(&dp, 0, sizeof dp);
    have[OBJ_DEVICE] = alloc(hc, hc, h[OBJ_DEVICE], NV01_DEVICE_0, &dp, sizeof dp) == 0;
    uint32_t sub = 0;
    have[OBJ_SUBDEVICE] = have[OBJ_DEVICE] && alloc(hc, h[OBJ_DEVICE], h[OBJ_SUBDEVICE], NV20_SUBDEVICE_0, &sub, sizeof sub) == 0;
    have[OBJ_DISPCOMMON] = have[OBJ_DEVICE] && alloc(hc, h[OBJ_DEVICE], h[OBJ_DISPCOMMON], NV04_DISPLAY_COMMON, NULL, 0) == 0;
    have[OBJ_DIAG] = have[OBJ_SUBDEVICE] && alloc(hc, h[OBJ_SUBDEVICE], h[OBJ_DIAG], NV20_SUBDEVICE_DIAG, NULL, 0) == 0;
    have[OBJ_I2C] = have[OBJ_SUBDEVICE] && alloc(hc, h[OBJ_SUBDEVICE], h[OBJ_I2C], NV40_I2C, NULL, 0) == 0;
    have[OBJ_ZBC] = have[OBJ_SUBDEVICE] && alloc(hc, h[OBJ_SUBDEVICE], h[OBJ_ZBC], GF100_ZBC_CLEAR, NULL, 0) == 0;
    printf("# objects:");
    for (int o = 0; o < OBJ_N; o++) printf(" %s=%s", OBJ_NAME[o], have[o] ? "yes" : "no");
    printf("\n# id\tname\tsize\tobject\tprivilege\tprovenance\tstatus\n");
    int sent = 0, ok = 0, skipped = 0;
    for (size_t i = 0; i < sizeof SWEEP / sizeof SWEEP[0]; i++) {
        const struct sweep_entry *e = &SWEEP[i];
        if (only && !strstr(e->name, only)) continue;
        if (!have[e->obj]) {
            skipped++;
            printf("%#010x\t%s\t%u\t%s\t%s\t%s\tno object\n", e->id, e->name, e->size, OBJ_NAME[e->obj], e->priv, e->prov);
            continue;
        }
        void *p = calloc(1, e->size);
        uint32_t st = ctrl(hc, h[e->obj], e->id, p, e->size);
        free(p);
        sent++;
        ok += st == 0;
        printf("%#010x\t%s\t%u\t%s\t%s\t%s\t%#x\n", e->id, e->name, e->size, OBJ_NAME[e->obj], e->priv, e->prov, st);
        fflush(stdout);
    }
    printf("COV ctlsweep ok %d sent, %d NV_OK, %d skipped (no object)\n", sent, ok, skipped);
    close(gfd);
    close(fd);
    return 0;
}
