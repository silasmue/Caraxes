// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
/*
 * wlframes: the phase C frame-callback probe (docs/caraxes/display-cd.md). A small
 * xdg_toplevel with a wl_shm buffer commits once per wl_surface.frame callback, each
 * commit damaging a small square (so a CPU renderer copies little), and asks for
 * wp_presentation feedback. It reports the frame-callback rate, the presentation
 * interval and the refresh the compositor announces.
 *
 *   wlframes [seconds] [size] [damage]      defaults: 10 s, 64x64 surface, 8x8 damage
 *
 * Build (guest): wayland-scanner generates the xdg-shell and presentation-time glue;
 * see weston-pace.sh.
 */
#define _GNU_SOURCE
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>
#include <wayland-client.h>
#include "xdg-shell-client-protocol.h"
#include "presentation-time-client-protocol.h"

static struct wl_compositor *comp;
static struct wl_shm *shm;
static struct xdg_wm_base *wm;
static struct wp_presentation *pres;
static struct wl_surface *surf;
static struct wl_buffer *buf;
static uint32_t *pix;
static int size = 64, dmg = 8, configured, frames, presented, discarded;
static double t_first, t_last, *cb_t, *pr_t;
static int cb_n, pr_n, cap;
static uint32_t refresh_ns;
static unsigned last_flags;

static double mono(void)
{
	struct timespec t;
	clock_gettime(CLOCK_MONOTONIC, &t);
	return t.tv_sec + t.tv_nsec * 1e-9;
}

static void reg_global(void *d, struct wl_registry *r, uint32_t name, const char *iface, uint32_t v)
{
	(void)d;
	if (!strcmp(iface, "wl_compositor"))
		comp = wl_registry_bind(r, name, &wl_compositor_interface, 4);
	else if (!strcmp(iface, "wl_shm"))
		shm = wl_registry_bind(r, name, &wl_shm_interface, 1);
	else if (!strcmp(iface, "xdg_wm_base"))
		wm = wl_registry_bind(r, name, &xdg_wm_base_interface, 1);
	else if (!strcmp(iface, "wp_presentation"))
		pres = wl_registry_bind(r, name, &wp_presentation_interface, 1);
	(void)v;
}
static void reg_remove(void *d, struct wl_registry *r, uint32_t n) { (void)d; (void)r; (void)n; }
static const struct wl_registry_listener reg_l = { reg_global, reg_remove };

static void wm_ping(void *d, struct xdg_wm_base *w, uint32_t s) { (void)d; xdg_wm_base_pong(w, s); }
static const struct xdg_wm_base_listener wm_l = { wm_ping };

static void xs_configure(void *d, struct xdg_surface *x, uint32_t s)
{
	(void)d;
	xdg_surface_ack_configure(x, s);
	configured = 1;
}
static const struct xdg_surface_listener xs_l = { xs_configure };
static void top_configure(void *d, struct xdg_toplevel *t, int32_t w, int32_t h, struct wl_array *s)
{ (void)d; (void)t; (void)w; (void)h; (void)s; }
static void top_close(void *d, struct xdg_toplevel *t) { (void)d; (void)t; }
static const struct xdg_toplevel_listener top_l = { top_configure, top_close };

static void pf_sync(void *d, struct wp_presentation_feedback *f, struct wl_output *o) { (void)d; (void)f; (void)o; }
static void pf_presented(void *d, struct wp_presentation_feedback *f, uint32_t hi, uint32_t lo, uint32_t ns,
			 uint32_t refresh, uint32_t shi, uint32_t slo, uint32_t flags)
{
	(void)d; (void)shi; (void)slo;
	if (pr_n < cap)
		pr_t[pr_n++] = ((double)(((uint64_t)hi << 32) | lo)) + ns * 1e-9;
	refresh_ns = refresh;
	last_flags = flags;
	presented++;
	wp_presentation_feedback_destroy(f);
}
static void pf_discarded(void *d, struct wp_presentation_feedback *f)
{
	(void)d;
	discarded++;
	wp_presentation_feedback_destroy(f);
}
static const struct wp_presentation_feedback_listener pf_l = { pf_sync, pf_presented, pf_discarded };

static const struct wl_callback_listener frame_l;
static void draw(void)
{
	struct wl_callback *cb;
	int x0 = size > dmg ? (frames * 3) % (size - dmg) : 0, y, x;

	for (y = 0; y < dmg; y++)
		for (x = 0; x < dmg; x++)
			pix[y * size + x0 + x] = 0xff000000u | (frames * 40503u);
	wl_surface_attach(surf, buf, 0, 0);
	wl_surface_damage(surf, x0, 0, dmg, dmg);
	cb = wl_surface_frame(surf);
	wl_callback_add_listener(cb, &frame_l, NULL);
	if (pres)
		wp_presentation_feedback_add_listener(wp_presentation_feedback(pres, surf), &pf_l, NULL);
	wl_surface_commit(surf);
}
static void frame_done(void *d, struct wl_callback *cb, uint32_t ms)
{
	(void)d; (void)ms;
	wl_callback_destroy(cb);
	t_last = mono();
	if (!frames)
		t_first = t_last;
	if (cb_n < cap)
		cb_t[cb_n++] = t_last;
	frames++;
	draw();
}
static const struct wl_callback_listener frame_l = { frame_done };

static void stats(const char *what, double *t, int n, int skip)
{
	double s = 0, ss = 0, mn = 1e9, mx = 0, m;
	int i, k = 0;

	for (i = skip + 1; i < n; i++) {
		double d = t[i] - t[i - 1];
		s += d; ss += d * d; k++;
		if (d < mn) mn = d;
		if (d > mx) mx = d;
	}
	if (!k) {
		printf("%s intervals=0\n", what);
		return;
	}
	m = s / k;
	printf("%s intervals=%d mean_ms=%.3f rate_hz=%.2f min_ms=%.3f max_ms=%.3f sd_ms=%.3f\n", what, k,
	       m * 1e3, 1 / m, mn * 1e3, mx * 1e3, sqrt(fmax(ss / k - m * m, 0)) * 1e3);
}

int main(int argc, char **argv)
{
	double secs = argc > 1 ? atof(argv[1]) : 10, t_end;
	struct wl_display *dpy;
	struct wl_registry *reg;
	struct xdg_surface *xs;
	struct xdg_toplevel *top;
	struct wl_shm_pool *pool;
	int fd, stride;

	if (argc > 2)
		size = atoi(argv[2]);
	if (argc > 3)
		dmg = atoi(argv[3]);
	if (dmg > size)
		dmg = size;
	cap = (int)(secs * 400) + 16;
	cb_t = calloc(cap, sizeof(double));
	pr_t = calloc(cap, sizeof(double));
	dpy = wl_display_connect(NULL);
	if (!dpy) {
		printf("no Wayland display\n");
		return 2;
	}
	reg = wl_display_get_registry(dpy);
	wl_registry_add_listener(reg, &reg_l, NULL);
	wl_display_roundtrip(dpy);
	if (!comp || !shm || !wm) {
		printf("missing globals (compositor %p shm %p xdg_wm_base %p)\n", (void *)comp, (void *)shm, (void *)wm);
		return 2;
	}
	printf("globals: wp_presentation %s\n", pres ? "yes" : "no");
	xdg_wm_base_add_listener(wm, &wm_l, NULL);
	surf = wl_compositor_create_surface(comp);
	xs = xdg_wm_base_get_xdg_surface(wm, surf);
	xdg_surface_add_listener(xs, &xs_l, NULL);
	top = xdg_surface_get_toplevel(xs);
	xdg_toplevel_add_listener(top, &top_l, NULL);
	xdg_toplevel_set_title(top, "wlframes");
	wl_surface_commit(surf);
	while (!configured && wl_display_dispatch(dpy) >= 0)
		;
	stride = size * 4;
	fd = memfd_create("wlframes", 0);
	if (ftruncate(fd, stride * size))
		return 2;
	pix = mmap(0, stride * size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
	memset(pix, 0x80, stride * size);
	pool = wl_shm_create_pool(shm, fd, stride * size);
	buf = wl_shm_pool_create_buffer(pool, 0, size, size, stride, WL_SHM_FORMAT_XRGB8888);
	draw();
	/* The measurement starts at the first frame callback: under a CPU renderer the
	 * first full-screen repaint (the toplevel being mapped) takes as long as BAR1
	 * needs for the whole scanout buffer. WLFRAMES_WAIT caps that wait (s). */
	{
		double wait = getenv("WLFRAMES_WAIT") ? atof(getenv("WLFRAMES_WAIT")) : 300;
		double t0 = mono();
		while (!frames && mono() < t0 + wait && wl_display_dispatch(dpy) >= 0)
			;
		printf("first frame callback after %.1f s\n", mono() - t0);
	}
	t_end = mono() + secs;
	while (mono() < t_end && wl_display_dispatch(dpy) >= 0)
		;
	printf("surface %dx%d, damage %dx%d per frame, %.1f s\n", size, size, dmg, dmg, secs);
	stats("FRAMES callbacks", cb_t, cb_n, 5);
	stats("PRESENTED feedback", pr_t, pr_n, 5);
	printf("presented=%d discarded=%d refresh_ns=%u (%.3f Hz) flags=0x%x (vsync %d hw_clock %d hw_completion %d)\n",
	       presented, discarded, refresh_ns, refresh_ns ? 1e9 / refresh_ns : 0.0, last_flags, !!(last_flags & 1),
	       !!(last_flags & 2), !!(last_flags & 4));
	return frames > 1 ? 0 : 1;
}
