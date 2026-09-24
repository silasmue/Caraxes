// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
/*
 * kmsflip: a libdrm-free KMS pacing probe for the Caraxes display (docs/caraxes/display-cd.md).
 * Sets a mode on the first connected connector with two dumb buffers (legacy SETCRTC), then
 * issues page flips with events back to back and reports the spacing of the completions:
 * mean rate, min/max interval and the jitter against the mode's period. With a vblank-paced
 * device the completions come once per refresh; a device that completes flips at once
 * (the v1 displayless symptom) shows intervals far below the period.
 *
 *   kmsflip [--card /dev/dri/card0] [--mode WxH@HZ | --mode-index N] [--flips N]
 *           [--fill] [--cursor] [--keep]
 *
 * --fill writes a pattern into both buffers (slow through BAR1) and reads one row back.
 * Output: one "RESULT" line with key=value pairs, preceded by the per-step log.
 */
#include <errno.h>
#include <fcntl.h>
#include <math.h>
#include <poll.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>
#include <drm/drm.h>
#include <drm/drm_mode.h>

static double now_s(void)
{
	struct timespec t;
	clock_gettime(CLOCK_MONOTONIC, &t);
	return t.tv_sec + t.tv_nsec * 1e-9;
}

struct buf {
	uint32_t handle, fb, pitch;
	uint64_t size;
};

static int make_buf(int fd, unsigned w, unsigned h, struct buf *b)
{
	struct drm_mode_create_dumb cd = { .width = w, .height = h, .bpp = 32 };
	struct drm_mode_fb_cmd fc = { 0 };

	if (ioctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &cd))
		return -1;
	fc.width = w;
	fc.height = h;
	fc.pitch = cd.pitch;
	fc.bpp = 32;
	fc.depth = 24;
	fc.handle = cd.handle;
	if (ioctl(fd, DRM_IOCTL_MODE_ADDFB, &fc))
		return -1;
	b->handle = cd.handle;
	b->fb = fc.fb_id;
	b->pitch = cd.pitch;
	b->size = cd.size;
	return 0;
}

static void fill(int fd, struct buf *b, unsigned w, unsigned h, uint32_t seed)
{
	struct drm_mode_map_dumb md = { .handle = b->handle };
	uint32_t *p, row[4096], back[4096];
	unsigned y, x;
	double t0;

	if (ioctl(fd, DRM_IOCTL_MODE_MAP_DUMB, &md))
		return;
	p = mmap(0, b->size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, md.offset);
	if (p == MAP_FAILED)
		return;
	t0 = now_s();
	for (y = 0; y < h; y++) {
		for (x = 0; x < w && x < 4096; x++)
			row[x] = 0xff000000u | ((x * 255 / w) << 16) | ((y * 255 / h) << 8) | (seed & 0xff);
		memcpy((char *)p + (size_t)y * b->pitch, row, w * 4);
	}
	memcpy(back, (char *)p + (size_t)(h / 2) * b->pitch, w * 4);
	for (x = 0; x < w && x < 4096; x++)
		row[x] = 0xff000000u | ((x * 255 / w) << 16) | (((h / 2) * 255 / h) << 8) | (seed & 0xff);
	printf("fill fb %u: %ux%u in %.2f s, row %u read back %s\n", b->fb, w, h, now_s() - t0,
	       h / 2, memcmp(row, back, w * 4) ? "DIFFERENT" : "identical");
	munmap(p, b->size);
}

int main(int argc, char **argv)
{
	const char *card = "/dev/dri/card0";
	unsigned want_w = 0, want_h = 0, want_hz = 0, flips = 120;
	int mode_index = -1, do_fill = 0, keep = 0, cursor = 0, ai;
	struct drm_mode_card_res res = { 0 };
	uint32_t conns[16], crtcs[16], encs[16];
	struct drm_mode_modeinfo modes[64], *mode = NULL;
	struct drm_mode_get_connector c = { 0 };
	unsigned i, conn_id = 0, crtc_id;
	int fd;

	for (ai = 1; ai < argc; ai++) {
		if (!strcmp(argv[ai], "--card") && ai + 1 < argc)
			card = argv[++ai];
		else if (!strcmp(argv[ai], "--mode") && ai + 1 < argc)
			sscanf(argv[++ai], "%ux%u@%u", &want_w, &want_h, &want_hz);
		else if (!strcmp(argv[ai], "--mode-index") && ai + 1 < argc)
			mode_index = atoi(argv[++ai]);
		else if (!strcmp(argv[ai], "--flips") && ai + 1 < argc)
			flips = (unsigned)atoi(argv[++ai]);
		else if (!strcmp(argv[ai], "--fill"))
			do_fill = 1;
		else if (!strcmp(argv[ai], "--cursor"))
			cursor = 1;
		else if (!strcmp(argv[ai], "--keep"))
			keep = 1;
		else {
			fprintf(stderr, "usage: %s [--card P] [--mode WxH@HZ|--mode-index N] [--flips N] [--fill] [--keep]\n", argv[0]);
			return 2;
		}
	}
	fd = open(card, O_RDWR | O_CLOEXEC);
	if (fd < 0) {
		printf("open %s: %s\n", card, strerror(errno));
		return 2;
	}
	if (ioctl(fd, DRM_IOCTL_SET_MASTER, 0))
		printf("SET_MASTER: %s (continuing)\n", strerror(errno));
	res.connector_id_ptr = (uintptr_t)conns;
	res.count_connectors = 16;
	res.crtc_id_ptr = (uintptr_t)crtcs;
	res.count_crtcs = 16;
	res.encoder_id_ptr = (uintptr_t)encs;
	res.count_encoders = 16;
	if (ioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, &res)) {
		printf("GETRESOURCES: %s\n", strerror(errno));
		return 2;
	}
	for (i = 0; i < res.count_connectors && i < 16 && !conn_id; i++) {
		memset(&c, 0, sizeof(c));
		c.connector_id = conns[i];
		c.modes_ptr = (uintptr_t)modes;
		c.count_modes = 64;
		if (ioctl(fd, DRM_IOCTL_MODE_GETCONNECTOR, &c) == 0 && c.connection == 1 && c.count_modes)
			conn_id = conns[i];
	}
	if (!conn_id) {
		printf("no connected connector\n");
		return 2;
	}
	for (i = 0; i < c.count_modes && i < 64; i++) {
		struct drm_mode_modeinfo *m = &modes[i];
		printf("  mode %u: %ux%u@%u clock %u htotal %u vtotal %u\n", i, m->hdisplay, m->vdisplay,
		       m->vrefresh, m->clock, m->htotal, m->vtotal);
		if (!mode && (int)i == mode_index)
			mode = m;
		if (!mode && mode_index < 0 && want_w && m->hdisplay == want_w && m->vdisplay == want_h &&
		    (!want_hz || m->vrefresh == want_hz))
			mode = m;
	}
	if (!mode)
		mode = &modes[0];
	crtc_id = crtcs[0];
	{
		double period = (double)mode->htotal * mode->vtotal / (mode->clock * 1000.0);
		struct buf b[2];
		struct drm_mode_crtc set = { 0 };
		double *t = calloc(flips + 1, sizeof(double)), *kt = calloc(flips + 1, sizeof(double));
		unsigned done = 0, timeouts = 0, f;
		double sum = 0, mn = 1e9, mx = 0, dev = 0, t_set;

		printf("connector %u crtc %u mode %ux%u@%u (clock %u kHz, period %.3f ms)\n", conn_id, crtc_id,
		       mode->hdisplay, mode->vdisplay, mode->vrefresh, mode->clock, period * 1e3);
		if (make_buf(fd, mode->hdisplay, mode->vdisplay, &b[0]) ||
		    make_buf(fd, mode->hdisplay, mode->vdisplay, &b[1])) {
			printf("dumb buffers: %s\n", strerror(errno));
			return 2;
		}
		if (do_fill) {
			fill(fd, &b[0], mode->hdisplay, mode->vdisplay, 0x40);
			fill(fd, &b[1], mode->hdisplay, mode->vdisplay, 0xc0);
		}
		set.crtc_id = crtc_id;
		set.fb_id = b[0].fb;
		set.set_connectors_ptr = (uintptr_t)&conn_id;
		set.count_connectors = 1;
		set.mode = *mode;
		set.mode_valid = 1;
		t_set = now_s();
		if (ioctl(fd, DRM_IOCTL_MODE_SETCRTC, &set)) {
			printf("SETCRTC: %s\n", strerror(errno));
			return 1;
		}
		printf("SETCRTC ok in %.3f s\n", now_s() - t_set);
		for (f = 0; f < flips; f++) {
			struct drm_mode_crtc_page_flip pf = { .crtc_id = crtc_id, .fb_id = b[(f + 1) & 1].fb,
							      .flags = DRM_MODE_PAGE_FLIP_EVENT,
							      .user_data = f };
			struct pollfd p = { .fd = fd, .events = POLLIN };
			char ev[1024];

			if (ioctl(fd, DRM_IOCTL_MODE_PAGE_FLIP, &pf)) {
				printf("flip %u: PAGE_FLIP: %s\n", f, strerror(errno));
				break;
			}
			if (poll(&p, 1, 1000) <= 0) {
				printf("flip %u: no completion within 1000 ms\n", f);
				timeouts++;
				break;
			}
			if (read(fd, ev, sizeof(ev)) > 0) {
				struct drm_event_vblank *v = (struct drm_event_vblank *)ev;
				t[done] = now_s();
				kt[done] = v->tv_sec + v->tv_usec * 1e-6;
				done++;
			}
		}
		for (f = 1; f < done; f++) {
			double d = kt[f] - kt[f - 1];
			sum += d;
			if (d < mn)
				mn = d;
			if (d > mx)
				mx = d;
			dev += (d - period) * (d - period);
		}
		if (done > 1) {
			double mean = sum / (done - 1);
			printf("RESULT flips=%u completed=%u timeouts=%u mode=%ux%u@%u period_ms=%.3f mean_ms=%.3f "
			       "rate_hz=%.2f min_ms=%.3f max_ms=%.3f rms_dev_ms=%.3f total_s=%.3f\n",
			       flips, done, timeouts, mode->hdisplay, mode->vdisplay, mode->vrefresh, period * 1e3,
			       mean * 1e3, 1.0 / mean, mn * 1e3, mx * 1e3, sqrt(dev / (done - 1)) * 1e3,
			       t[done - 1] - t[0]);
		} else {
			printf("RESULT flips=%u completed=%u timeouts=%u\n", flips, done, timeouts);
		}
		{
			struct drm_mode_crtc got = { .crtc_id = crtc_id };
			ioctl(fd, DRM_IOCTL_MODE_GETCRTC, &got);
			printf("GETCRTC: fb %u (expected %u), mode_valid %u, %ux%u\n", got.fb_id,
			       b[done & 1].fb, got.mode_valid, got.mode.hdisplay, got.mode.vdisplay);
		}
		if (cursor) {
			/* A 64x64 ARGB cursor through the legacy cursor ioctls (NVKMS's cursor
			 * channel), then 60 moves one frame apart. */
			struct drm_mode_create_dumb cd = { .width = 64, .height = 64, .bpp = 32 };
			struct drm_mode_cursor cu = { 0 };
			int moved = 0, i;

			if (ioctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &cd) == 0) {
				struct drm_mode_map_dumb md = { .handle = cd.handle };
				uint32_t *p;
				if (!ioctl(fd, DRM_IOCTL_MODE_MAP_DUMB, &md) &&
				    (p = mmap(0, cd.size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, md.offset)) != MAP_FAILED) {
					for (i = 0; i < 64 * 64; i++)
						p[i] = 0xffff00ffu;
					munmap(p, cd.size);
				}
				cu.flags = DRM_MODE_CURSOR_BO;
				cu.crtc_id = crtc_id;
				cu.handle = cd.handle;
				cu.width = 64;
				cu.height = 64;
				printf("CURSOR set: %s\n", ioctl(fd, DRM_IOCTL_MODE_CURSOR, &cu) ? strerror(errno) : "ok");
				for (i = 0; i < 60; i++) {
					cu.flags = DRM_MODE_CURSOR_MOVE;
					cu.x = 10 * i;
					cu.y = 5 * i;
					if (ioctl(fd, DRM_IOCTL_MODE_CURSOR, &cu) == 0)
						moved++;
					usleep(16667);
				}
				printf("CURSOR moves=%d of 60\n", moved);
				cu.flags = DRM_MODE_CURSOR_BO;
				cu.handle = 0;
				printf("CURSOR off: %s\n", ioctl(fd, DRM_IOCTL_MODE_CURSOR, &cu) ? strerror(errno) : "ok");
			} else {
				printf("CURSOR dumb buffer: %s\n", strerror(errno));
			}
		}
		if (keep) {
			printf("keeping the mode; Ctrl-C to end\n");
			pause();
		}
		return (done == flips && !timeouts) ? 0 : 1;
	}
}
