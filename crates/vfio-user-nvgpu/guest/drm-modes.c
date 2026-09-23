// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//
// A libdrm-free DRM readout for the guest (docs/caraxes/display-ab.md, gate D3): every
// connector of /dev/dri/card0 (or argv[1]) with its type, status and mode list, each mode
// with pixel clock, totals and refresh. Uses only the kernel uapi (DRM_IOCTL_MODE_*);
// opens the node read-only-ish and never sets a mode.
//   cc -O2 -I/usr/include/drm -o drm-modes drm-modes.c && ./drm-modes
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>
#include <drm.h>
#include <drm_mode.h>

static const char *type_name(uint32_t t)
{
    switch (t) {
    case DRM_MODE_CONNECTOR_DisplayPort: return "DP";
    case DRM_MODE_CONNECTOR_HDMIA: return "HDMI-A";
    case DRM_MODE_CONNECTOR_DVID: return "DVI-D";
    case DRM_MODE_CONNECTOR_VIRTUAL: return "Virtual";
    default: return "other";
    }
}

int main(int argc, char **argv)
{
    const char *node = argc > 1 ? argv[1] : "/dev/dri/card0";
    int fd = open(node, O_RDWR | O_CLOEXEC);
    if (fd < 0) { perror(node); return 1; }
    struct drm_mode_card_res res = {0};
    if (ioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, &res)) { perror("GETRESOURCES"); return 1; }
    uint32_t *conns = calloc(res.count_connectors, sizeof(uint32_t));
    uint32_t *crtcs = calloc(res.count_crtcs ? res.count_crtcs : 1, sizeof(uint32_t));
    uint32_t *encs = calloc(res.count_encoders ? res.count_encoders : 1, sizeof(uint32_t));
    uint32_t *fbs = calloc(res.count_fbs ? res.count_fbs : 1, sizeof(uint32_t));
    res.connector_id_ptr = (uintptr_t)conns;
    res.crtc_id_ptr = (uintptr_t)crtcs;
    res.encoder_id_ptr = (uintptr_t)encs;
    res.fb_id_ptr = (uintptr_t)fbs;
    if (ioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, &res)) { perror("GETRESOURCES"); return 1; }
    printf("%s: %u connector(s), %u crtc(s), %u encoder(s), max %ux%u\n", node,
           res.count_connectors, res.count_crtcs, res.count_encoders, res.max_width, res.max_height);
    for (uint32_t i = 0; i < res.count_connectors; i++) {
        struct drm_mode_get_connector c = { .connector_id = conns[i] };
        if (ioctl(fd, DRM_IOCTL_MODE_GETCONNECTOR, &c)) { perror("GETCONNECTOR"); continue; }
        struct drm_mode_modeinfo *modes = calloc(c.count_modes ? c.count_modes : 1, sizeof(*modes));
        uint32_t *props = calloc(c.count_props ? c.count_props : 1, sizeof(uint32_t));
        uint64_t *vals = calloc(c.count_props ? c.count_props : 1, sizeof(uint64_t));
        uint32_t *cencs = calloc(c.count_encoders ? c.count_encoders : 1, sizeof(uint32_t));
        c.modes_ptr = (uintptr_t)modes;
        c.props_ptr = (uintptr_t)props;
        c.prop_values_ptr = (uintptr_t)vals;
        c.encoders_ptr = (uintptr_t)cencs;
        if (ioctl(fd, DRM_IOCTL_MODE_GETCONNECTOR, &c)) { perror("GETCONNECTOR"); continue; }
        printf("connector %u: %s-%u %s, %ux%u mm, %u mode(s), encoder %u\n", c.connector_id,
               type_name(c.connector_type), c.connector_type_id,
               c.connection == 1 ? "connected" : c.connection == 2 ? "disconnected" : "unknown",
               c.mm_width, c.mm_height, c.count_modes, c.encoder_id);
        for (uint32_t m = 0; m < c.count_modes; m++) {
            struct drm_mode_modeinfo *x = &modes[m];
            double hz = (double)x->clock * 1000.0 / ((double)x->htotal * x->vtotal);
            printf("  %-12s %4ux%-4u %8.3f Hz  clock %7u kHz  htotal %4u vtotal %4u%s\n", x->name,
                   x->hdisplay, x->vdisplay, hz, x->clock, x->htotal, x->vtotal,
                   (x->type & DRM_MODE_TYPE_PREFERRED) ? "  preferred" : "");
        }
    }
    close(fd);
    return 0;
}
