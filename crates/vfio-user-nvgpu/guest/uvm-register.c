// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//
// Experiment E5a guest helper: open /dev/nvidia-uvm, issue UVM_INITIALIZE and
// UVM_REGISTER_GPU for the GPU whose UUID is given (or read from
// /proc/driver/nvidia/gpus/*/information), then UVM_UNREGISTER_GPU.
// Layouts: kernel-open/nvidia-uvm/uvm_linux_ioctl.h:31-37 (UVM_INITIALIZE) and
// uvm_ioctl.h:405-427 (UVM_REGISTER_GPU = 37, UVM_UNREGISTER_GPU = 38).
// The caller must hold /dev/nvidia0 open: nvGpuOpsGetGpuInfo returns
// NV_ERR_INSUFFICIENT_PERMISSIONS otherwise (src/nvidia/src/kernel/rmapi/
// nv_gpu_ops.c:7671-7674, kernel-open/nvidia/nv.c:5917-5923).
// Usage: uvm-register [--flags N] [GPU-uuid]. N is the UVM_INITIALIZE flags word;
// 1 is UVM_INIT_FLAGS_DISABLE_PAGEABLE_MIGRATIONS (uvm_types.h:63-69), which also
// skips the HMM device-private memory registration (uvm_va_space.c:805,
// uvm_gpu.c:2926-2934).
// Build in the guest: cc -O2 -o uvm-register uvm-register.c

#include <fcntl.h>
#include <glob.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#define UVM_INITIALIZE 0x30000001
#define UVM_REGISTER_GPU 37
#define UVM_UNREGISTER_GPU 38

struct uvm_initialize_params {
    uint64_t flags;
    uint32_t rm_status;
};

struct uvm_register_gpu_params {
    uint8_t gpu_uuid[16];
    uint8_t numa_enabled;
    int32_t numa_node_id;
    int32_t rm_ctrl_fd;
    uint32_t h_client;
    uint32_t h_smc_part_ref;
    uint32_t rm_status;
};

struct uvm_unregister_gpu_params {
    uint8_t gpu_uuid[16];
    uint32_t rm_status;
};

// "GPU-9bd7b93d-3d23-c074-6704-984ef33ab7a7" -> 16 bytes.
static int parse_uuid(const char *s, uint8_t out[16])
{
    int n = 0;
    if (strncmp(s, "GPU-", 4) == 0)
        s += 4;
    while (*s && n < 32) {
        if (*s == '-') {
            s++;
            continue;
        }
        char h[3] = {s[0], s[1], 0};
        if (!s[1])
            return -1;
        out[n / 2] = (uint8_t)strtoul(h, NULL, 16);
        n += 2;
        s += 2;
    }
    return n == 32 ? 0 : -1;
}

static int uuid_from_proc(char *buf, size_t len)
{
    glob_t g;
    if (glob("/proc/driver/nvidia/gpus/*/information", 0, NULL, &g) != 0 || g.gl_pathc == 0)
        return -1;
    FILE *f = fopen(g.gl_pathv[0], "r");
    globfree(&g);
    if (!f)
        return -1;
    char line[256];
    int rc = -1;
    while (fgets(line, sizeof(line), f)) {
        char *p = strstr(line, "GPU-");
        if (strncmp(line, "GPU UUID", 8) == 0 && p) {
            p[strcspn(p, " \t\n")] = 0;
            snprintf(buf, len, "%s", p);
            rc = 0;
            break;
        }
    }
    fclose(f);
    return rc;
}

int main(int argc, char **argv)
{
    char uuid_str[64];
    uint8_t uuid[16];
    uint64_t flags = 0;
    int a = 1;
    if (argc > 2 && strcmp(argv[1], "--flags") == 0) {
        flags = strtoull(argv[2], NULL, 0);
        a = 3;
    }
    if (argc > a)
        snprintf(uuid_str, sizeof(uuid_str), "%s", argv[a]);
    else if (uuid_from_proc(uuid_str, sizeof(uuid_str)) != 0) {
        fprintf(stderr, "no GPU UUID in /proc/driver/nvidia/gpus\n");
        return 2;
    }
    if (parse_uuid(uuid_str, uuid) != 0) {
        fprintf(stderr, "bad UUID %s\n", uuid_str);
        return 2;
    }
    int ctl = open("/dev/nvidiactl", O_RDWR | O_CLOEXEC);
    int gpu = open("/dev/nvidia0", O_RDWR | O_CLOEXEC);
    if (ctl < 0 || gpu < 0) {
        perror("open /dev/nvidiactl or /dev/nvidia0");
        return 1;
    }
    int fd = open("/dev/nvidia-uvm", O_RDWR | O_CLOEXEC);
    if (fd < 0) {
        perror("open /dev/nvidia-uvm");
        return 1;
    }
    struct uvm_initialize_params init = {0};
    init.flags = flags;
    int r = ioctl(fd, UVM_INITIALIZE, &init);
    printf("UVM_INITIALIZE flags 0x%llx: ioctl %d rmStatus 0x%x\n", (unsigned long long)flags, r,
           init.rm_status);
    if (r != 0 || init.rm_status != 0)
        return 1;
    struct uvm_register_gpu_params reg = {0};
    memcpy(reg.gpu_uuid, uuid, 16);
    reg.rm_ctrl_fd = -1;
    printf("UVM_REGISTER_GPU %s ...\n", uuid_str);
    fflush(stdout);
    r = ioctl(fd, UVM_REGISTER_GPU, &reg);
    printf("UVM_REGISTER_GPU: ioctl %d rmStatus 0x%x numaEnabled %u numaNodeId %d\n", r,
           reg.rm_status, reg.numa_enabled, reg.numa_node_id);
    int ok = r == 0 && reg.rm_status == 0;
    if (ok) {
        struct uvm_unregister_gpu_params unreg = {0};
        memcpy(unreg.gpu_uuid, uuid, 16);
        r = ioctl(fd, UVM_UNREGISTER_GPU, &unreg);
        printf("UVM_UNREGISTER_GPU: ioctl %d rmStatus 0x%x\n", r, unreg.rm_status);
    }
    close(fd);
    close(gpu);
    close(ctl);
    return ok ? 0 : 1;
}
