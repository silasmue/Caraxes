// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// Guest smoke test of the display reserve: as DRM master (no compositor running), light the first
// connected connector with each mode given as WxH, one after the other, then turn it off. After
// each step it waits and runs the command given with -c (nvidia-smi, the kernel log), so the
// reserve NVKMS declares for the committed mode can be read. Raw DRM ioctls, no libdrm.
// Usage: modeset [-c COMMAND] [-w SECONDS] WxH...
#include <drm/drm.h>
#include <drm/drm_mode.h>
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

static int fd;

static void run(const char *cmd, const char *what)
{
    printf("== %s\n", what);
    fflush(stdout);
    if (cmd)
        system(cmd);
}

int main(int argc, char **argv)
{
    struct drm_mode_card_res res = { 0 };
    uint32_t crtcs[16], conns[16], encs[16];
    struct drm_mode_modeinfo modes[64];
    struct drm_mode_get_connector conn = { 0 };
    const char *cmd = NULL;
    int wait = 5, opt, i, c;
    uint32_t connId = 0, crtcId;

    while ((opt = getopt(argc, argv, "c:w:")) != -1) {
        if (opt == 'c') cmd = optarg;
        else if (opt == 'w') wait = atoi(optarg);
    }
    fd = open("/dev/dri/card0", O_RDWR | O_CLOEXEC);
    if (fd < 0 || ioctl(fd, DRM_IOCTL_SET_MASTER, 0)) { perror("card0 master"); return 2; }

    res.crtc_id_ptr = (uintptr_t)crtcs; res.count_crtcs = 16;
    res.connector_id_ptr = (uintptr_t)conns; res.count_connectors = 16;
    res.encoder_id_ptr = (uintptr_t)encs; res.count_encoders = 16;
    if (ioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, &res)) { perror("GETRESOURCES"); return 2; }
    crtcId = crtcs[0];
    for (c = 0; c < (int)res.count_connectors && !connId; c++) {
        memset(&conn, 0, sizeof(conn));
        conn.connector_id = conns[c];
        conn.modes_ptr = (uintptr_t)modes; conn.count_modes = 64;
        if (ioctl(fd, DRM_IOCTL_MODE_GETCONNECTOR, &conn) == 0 &&
            conn.connection == 1 && conn.count_modes > 0)
            connId = conns[c];
    }
    if (!connId) { fprintf(stderr, "no connected connector\n"); return 2; }

    for (i = optind; i < argc; i++) {
        unsigned w, h;
        struct drm_mode_modeinfo *m = NULL;
        struct drm_mode_create_dumb cd = { 0 };
        struct drm_mode_fb_cmd fb = { 0 };
        struct drm_mode_crtc sc = { 0 };
        char what[64];
        unsigned k;

        if (sscanf(argv[i], "%ux%u", &w, &h) != 2) continue;
        for (k = 0; k < conn.count_modes && !m; k++)
            if (modes[k].hdisplay == w && modes[k].vdisplay == h) m = &modes[k];
        if (!m) { fprintf(stderr, "mode %s not offered\n", argv[i]); continue; }

        cd.width = w; cd.height = h; cd.bpp = 32;
        if (ioctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &cd)) { perror("CREATE_DUMB"); return 1; }
        fb.width = w; fb.height = h; fb.bpp = 32; fb.depth = 24; fb.pitch = cd.pitch; fb.handle = cd.handle;
        if (ioctl(fd, DRM_IOCTL_MODE_ADDFB, &fb)) { perror("ADDFB"); return 1; }
        sc.crtc_id = crtcId; sc.fb_id = fb.fb_id; sc.set_connectors_ptr = (uintptr_t)&connId;
        sc.count_connectors = 1; sc.mode = *m; sc.mode_valid = 1;
        if (ioctl(fd, DRM_IOCTL_MODE_SETCRTC, &sc)) { perror("SETCRTC"); return 1; }
        sleep(wait);
        snprintf(what, sizeof(what), "mode %ux%u", w, h);
        run(cmd, what);
    }

    {
        struct drm_mode_crtc off = { .crtc_id = crtcId };
        if (ioctl(fd, DRM_IOCTL_MODE_SETCRTC, &off)) perror("SETCRTC off");
    }
    sleep(wait);
    run(cmd, "off");
    return 0;
}
