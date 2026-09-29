// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// Ask nvidia-drm for an XRGB8888 dumb buffer (an NVKMS scanout allocation), hold it, free it.
// 1920x1152 has the shape of the display reserve's 1080p runs (8640 KiB in 2 MiB pages);
// 1920x1080 is smaller (8100 KiB). Usage: dumb [SECONDS] [WIDTHxHEIGHT]
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <stdlib.h>
#include <sys/ioctl.h>
#include <drm/drm.h>
#include <drm/drm_mode.h>
int main(int argc, char **argv)
{
    struct drm_mode_create_dumb c = { .width = 1920, .height = 1080, .bpp = 32 };
    struct drm_mode_destroy_dumb d;
    int hold = argc > 1 ? atoi(argv[1]) : 3;
    int fd;
    if (argc > 2 && sscanf(argv[2], "%ux%u", &c.width, &c.height) != 2) { fprintf(stderr, "bad size\n"); return 2; }
    fd = open("/dev/dri/card0", O_RDWR);
    if (fd < 0) { perror("open"); return 2; }
    if (ioctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &c)) {
        printf("dumb %ux%u: FAILED errno %d (%s)\n", c.width, c.height, errno, strerror(errno));
        return 1;
    }
    printf("dumb %ux%u: ok pitch %u size 0x%llx;", c.width, c.height, c.pitch, (unsigned long long)c.size);
    fflush(stdout);
    sleep(hold);
    d.handle = c.handle;
    ioctl(fd, DRM_IOCTL_MODE_DESTROY_DUMB, &d);
    printf(" destroyed\n");
    return 0;
}
