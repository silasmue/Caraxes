// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// Ask nvidia-drm for a 1920x1080 XRGB8888 dumb buffer (an NVKMS scanout allocation), hold it,
// free it. Usage: dumb [SECONDS]
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
    int fd = open("/dev/dri/card0", O_RDWR);
    struct drm_mode_create_dumb c = { .width = 1920, .height = 1080, .bpp = 32 };
    struct drm_mode_destroy_dumb d;
    int hold = argc > 1 ? atoi(argv[1]) : 3;
    if (fd < 0) { perror("open"); return 2; }
    if (ioctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &c)) { printf("dumb 1920x1080: FAILED errno %d (%s)\n", errno, strerror(errno)); return 1; }
    printf("dumb 1920x1080: ok handle %u pitch %u size 0x%llx\n", c.handle, c.pitch, (unsigned long long)c.size);
    sleep(hold);
    d.handle = c.handle;
    ioctl(fd, DRM_IOCTL_MODE_DESTROY_DUMB, &d);
    printf("dumb destroyed\n");
    return 0;
}
