// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
/*
 * kmsgrab: readback checks for the Caraxes display gates (docs/caraxes/display-cd.md,
 * testing.md). Everything read here is memory the guest can reach; nothing scans out.
 *
 *   kmsgrab pixel [WxH]      set WxH (default the first mode) on a dumb buffer holding a
 *                            deterministic pattern, then read the scanout back through the
 *                            export path (GETCRTC -> GETFB2 -> MAP_DUMB) and hash it (FNV-1a
 *                            64). Prints "PIXEL ... written=H readback=H" and exits 0 when
 *                            they match, 1 when not, 2 when the chain refused.
 *   kmsgrab sample X Y ...   read pixels of the framebuffer the first active CRTC scans out
 *                            now (what a KMS grabber such as Sunshine capture=kms reads).
 *                            A GPU-rendered scanout (NVIDIA EGL/GBM) is block-linear
 *                            (DRM_FORMAT_MOD_NVIDIA_BLOCK_LINEAR_2D); its pixel address is
 *                            computed through the GOB layout (display-e.md).
 *   kmsgrab info             print the active CRTC's framebuffer (GETFB2).
 */
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
#include <drm/drm.h>
#include <drm/drm_fourcc.h>
#include <drm/drm_mode.h>

static double now_s(void)
{
	struct timespec t;
	clock_gettime(CLOCK_MONOTONIC, &t);
	return t.tv_sec + t.tv_nsec * 1e-9;
}

static uint32_t pattern(unsigned x, unsigned y)
{
	uint32_t v = x * 2654435761u ^ (y * 40503u + 0x9e37u);
	v ^= v >> 13;
	return 0xff000000u | (v & 0xffffff);
}

static uint64_t fnv(uint64_t h, const void *p, size_t n)
{
	const unsigned char *b = p;
	while (n--)
		h = (h ^ *b++) * 0x100000001b3ull;
	return h;
}

static int active_fb(int fd, uint32_t *crtc_out, struct drm_mode_fb_cmd2 *fb2, struct drm_mode_crtc *cr)
{
	uint32_t crtcs[16];
	struct drm_mode_card_res res = { .crtc_id_ptr = (uintptr_t)crtcs, .count_crtcs = 16 };
	unsigned i;

	if (ioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, &res))
		return -1;
	for (i = 0; i < res.count_crtcs && i < 16; i++) {
		memset(cr, 0, sizeof(*cr));
		cr->crtc_id = crtcs[i];
		if (ioctl(fd, DRM_IOCTL_MODE_GETCRTC, cr) == 0 && cr->mode_valid && cr->fb_id) {
			memset(fb2, 0, sizeof(*fb2));
			fb2->fb_id = cr->fb_id;
			if (ioctl(fd, DRM_IOCTL_MODE_GETFB2, fb2))
				return -1;
			*crtc_out = crtcs[i];
			return 0;
		}
	}
	errno = ENOENT;
	return -1;
}

/* The byte offset of pixel (x, y) in a buffer with modifier `mod`: pitch-linear, or NVIDIA
 * block-linear (drm_fourcc.h: vendor 0x03, bit 4 set, log2 block height in GOBs in bits
 * 3:0). A GOB is 64 bytes x 8 rows (512 bytes) with the sector swizzle of nouveau's
 * nv50 tiling; a block is one GOB wide and 2^h GOBs tall; blocks run left to right, then
 * down. */
static size_t pixel_offset(uint64_t mod, uint32_t pitch, unsigned x, unsigned y)
{
	size_t xb = (size_t)x * 4, gob, blk_rows, h;

	if ((mod >> 56) != 0x03 || !(mod & 0x10))
		return (size_t)y * pitch + xb;
	h = mod & 0xf;
	blk_rows = (size_t)8 << h;
	gob = ((xb & 63) >> 5) << 8 | ((y & 7) >> 1) << 6 | ((xb & 31) >> 4) << 5 | (y & 1) << 4 | (xb & 15);
	return (y / blk_rows) * ((size_t)pitch / 64) * (512 << h) + (xb / 64) * (512 << h) +
	       ((y % blk_rows) / 8) * 512 + gob;
}

/* Bytes to map: the height rounded up to a whole block for block-linear. */
static size_t map_size(uint64_t mod, uint32_t pitch, uint32_t height)
{
	size_t rows = height;

	if ((mod >> 56) == 0x03 && (mod & 0x10)) {
		size_t blk = (size_t)8 << (mod & 0xf);
		rows = (rows + blk - 1) / blk * blk;
	}
	return (size_t)pitch * rows;
}

static void *map_handle(int fd, uint32_t handle, size_t size)
{
	struct drm_mode_map_dumb md = { .handle = handle };
	void *p;

	if (ioctl(fd, DRM_IOCTL_MODE_MAP_DUMB, &md))
		return NULL;
	p = mmap(0, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, md.offset);
	return p == MAP_FAILED ? NULL : p;
}

static int cmd_pixel(int fd, unsigned w, unsigned h)
{
	uint32_t conns[16], crtcs[16], conn = 0, x, y, crtc;
	struct drm_mode_card_res res = { .connector_id_ptr = (uintptr_t)conns, .count_connectors = 16,
					 .crtc_id_ptr = (uintptr_t)crtcs, .count_crtcs = 16 };
	struct drm_mode_modeinfo modes[64], *mode = NULL;
	struct drm_mode_get_connector c = { 0 };
	struct drm_mode_create_dumb cd = { 0 };
	struct drm_mode_fb_cmd2 fb = { 0 }, fb2;
	struct drm_mode_crtc set = { 0 }, got;
	uint32_t *p, *row;
	uint64_t hw = 0xcbf29ce484222325ull, hr = 0xcbf29ce484222325ull;
	double t0, tw, tr;
	unsigned i;

	if (ioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, &res))
		return 2;
	for (i = 0; i < res.count_connectors && !conn; i++) {
		memset(&c, 0, sizeof(c));
		c.connector_id = conns[i];
		c.modes_ptr = (uintptr_t)modes;
		c.count_modes = 64;
		if (!ioctl(fd, DRM_IOCTL_MODE_GETCONNECTOR, &c) && c.connection == 1 && c.count_modes)
			conn = conns[i];
	}
	if (!conn) {
		printf("PIXEL refused: no connected connector\n");
		return 2;
	}
	for (i = 0; i < c.count_modes && i < 64 && !mode; i++)
		if (!w || (modes[i].hdisplay == w && modes[i].vdisplay == h))
			mode = &modes[i];
	if (!mode) {
		printf("PIXEL refused: no %ux%u mode\n", w, h);
		return 2;
	}
	w = mode->hdisplay;
	h = mode->vdisplay;
	cd.width = w;
	cd.height = h;
	cd.bpp = 32;
	if (ioctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &cd)) {
		printf("PIXEL refused: CREATE_DUMB %s\n", strerror(errno));
		return 2;
	}
	fb.width = w;
	fb.height = h;
	fb.pixel_format = DRM_FORMAT_XRGB8888;
	fb.handles[0] = cd.handle;
	fb.pitches[0] = cd.pitch;
	if (ioctl(fd, DRM_IOCTL_MODE_ADDFB2, &fb)) {
		printf("PIXEL refused: ADDFB2 %s\n", strerror(errno));
		return 2;
	}
	p = map_handle(fd, cd.handle, cd.size);
	row = malloc(w * 4);
	if (!p || !row) {
		printf("PIXEL refused: map\n");
		return 2;
	}
	t0 = now_s();
	for (y = 0; y < h; y++) {
		for (x = 0; x < w; x++)
			row[x] = pattern(x, y);
		memcpy((char *)p + (size_t)y * cd.pitch, row, w * 4);
		hw = fnv(hw, row, w * 4);
	}
	tw = now_s() - t0;
	munmap(p, cd.size);
	crtc = crtcs[0];
	set.crtc_id = crtc;
	set.fb_id = fb.fb_id;
	set.set_connectors_ptr = (uintptr_t)&conn;
	set.count_connectors = 1;
	set.mode = *mode;
	set.mode_valid = 1;
	if (ioctl(fd, DRM_IOCTL_MODE_SETCRTC, &set)) {
		printf("PIXEL refused: SETCRTC %s\n", strerror(errno));
		return 2;
	}
	/* Read back through the export path a KMS grabber uses. */
	memset(&got, 0, sizeof(got));
	got.crtc_id = crtc;
	if (ioctl(fd, DRM_IOCTL_MODE_GETCRTC, &got) || got.fb_id != fb.fb_id) {
		printf("PIXEL refused: GETCRTC fb %u, expected %u\n", got.fb_id, fb.fb_id);
		return 2;
	}
	memset(&fb2, 0, sizeof(fb2));
	fb2.fb_id = got.fb_id;
	if (ioctl(fd, DRM_IOCTL_MODE_GETFB2, &fb2) || !fb2.handles[0]) {
		printf("PIXEL refused: GETFB2 %s (handle %u)\n", strerror(errno), fb2.handles[0]);
		return 2;
	}
	p = map_handle(fd, fb2.handles[0], (size_t)fb2.pitches[0] * fb2.height);
	if (!p) {
		printf("PIXEL refused: map of the GETFB2 handle\n");
		return 2;
	}
	t0 = now_s();
	for (y = 0; y < fb2.height; y++) {
		memcpy(row, (char *)p + (size_t)y * fb2.pitches[0], fb2.width * 4);
		hr = fnv(hr, row, fb2.width * 4);
	}
	tr = now_s() - t0;
	printf("PIXEL mode=%ux%u@%u fb=%u pitch=%u format=%.4s written=0x%016llx readback=0x%016llx readback_via=getfb2 "
	       "write_s=%.2f read_s=%.2f write_MBps=%.2f read_MBps=%.2f\n",
	       w, h, mode->vrefresh, got.fb_id, fb2.pitches[0], (char *)&fb2.pixel_format,
	       (unsigned long long)hw, (unsigned long long)hr, tw, tr, w * h * 4 / tw / 1e6,
	       w * h * 4 / tr / 1e6);
	return hw == hr ? 0 : 1;
}

int main(int argc, char **argv)
{
	int fd = open("/dev/dri/card0", O_RDWR | O_CLOEXEC);
	struct drm_mode_fb_cmd2 fb2;
	struct drm_mode_crtc cr;
	uint32_t crtc;

	if (fd < 0 || argc < 2) {
		printf("usage: kmsgrab pixel [WxH] | sample X Y ... | info (%s)\n", strerror(errno));
		return 2;
	}
	if (!strcmp(argv[1], "pixel")) {
		unsigned w = 0, h = 0;
		if (argc > 2)
			sscanf(argv[2], "%ux%u", &w, &h);
		ioctl(fd, DRM_IOCTL_SET_MASTER, 0);
		return cmd_pixel(fd, w, h);
	}
	if (active_fb(fd, &crtc, &fb2, &cr)) {
		printf("GRAB no active CRTC framebuffer: %s\n", strerror(errno));
		return 2;
	}
	printf("GRAB crtc %u fb %u %ux%u format %.4s modifier 0x%llx pitch %u handle %u mode %ux%u@%u\n", crtc,
	       fb2.fb_id, fb2.width, fb2.height, (char *)&fb2.pixel_format, (unsigned long long)fb2.modifier[0],
	       fb2.pitches[0], fb2.handles[0], cr.mode.hdisplay, cr.mode.vdisplay, cr.mode.vrefresh);
	if (!strcmp(argv[1], "sample")) {
		uint32_t *p;
		int i;
		if (!fb2.handles[0]) {
			printf("GRAB no handle (not master and no CAP_SYS_ADMIN?)\n");
			return 2;
		}
		p = map_handle(fd, fb2.handles[0], map_size(fb2.modifier[0], fb2.pitches[0], fb2.height));
		if (!p) {
			printf("GRAB map failed (not a dumb buffer?)\n");
			return 2;
		}
		for (i = 2; i + 1 < argc; i += 2) {
			unsigned x = atoi(argv[i]), y = atoi(argv[i + 1]);
			if (x < fb2.width && y < fb2.height)
				printf("SAMPLE %u %u 0x%08x\n", x, y,
				       p[pixel_offset(fb2.modifier[0], fb2.pitches[0], x, y) / 4]);
		}
	}
	return 0;
}
