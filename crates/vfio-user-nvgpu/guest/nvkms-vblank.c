// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
/*
 * nvkms-vblank: exercise the vblank (LAST_DATA) half of the Caraxes display clock
 * (docs/caraxes/display-cd.md). It opens /dev/nvidia-modeset as an NVKMS client, allocates
 * the device and asks NVKMS_IOCTL_NOTIFY_VBLANK for one api head N times; each request
 * registers an RM vblank callback (NV9010-style, nvApiHeadRegisterVBlankCallback), which
 * makes the guest kernel enable FE_RM_INTR_EN_HEAD_TIMING.LAST_DATA, and the unicast fd
 * becomes readable at the next vblank interrupt the guest RM services. The spacing of
 * those wakeups is the vblank rate. The same interface serves the NVIDIA X driver's
 * swap-interval pacing.
 *
 * Needs a mode set on the head (e.g. kmsflip --keep in the background).
 * Build (host, static, against the 610.57.04 headers):
 *   cc -O2 -static -I$OGK/src/nvidia-modeset/interface -I$OGK/src/common/sdk/nvidia/inc \
 *      -I$OGK/src/common/inc -I$OGK/src/common/unix/common/inc -o nvkms-vblank nvkms-vblank.c -lm
 *
 *   nvkms-vblank [count] [apiHead] [rmDeviceId]     defaults: 120, 0, 0
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
#include <time.h>
#include <unistd.h>
#include "nvkms-api.h"
#include "nvkms-ioctl.h"

#ifndef NV_VERSION
#define NV_VERSION "610.57.04"
#endif

static int nvkms(int fd, NvU32 cmd, void *p, NvU32 size)
{
	struct NvKmsIoctlParams io = { .cmd = cmd, .size = size, .address = (NvU64)(uintptr_t)p };
	return ioctl(fd, NVKMS_IOCTL_IOWR, &io);
}

static double mono(void)
{
	struct timespec t;
	clock_gettime(CLOCK_MONOTONIC, &t);
	return t.tv_sec + t.tv_nsec * 1e-9;
}

int main(int argc, char **argv)
{
	int count = argc > 1 ? atoi(argv[1]) : 120;
	NvU32 head = argc > 2 ? (NvU32)atoi(argv[2]) : 0;
	NvU32 devid = argc > 3 ? (NvU32)strtoul(argv[3], 0, 0) : 0;
	struct NvKmsAllocDeviceParams *ad = calloc(1, sizeof(*ad));
	double *t = calloc(count + 1, sizeof(double)), s = 0, ss = 0, mn = 1e9, mx = 0, m;
	int fd, i, got = 0, timeouts = 0, k = 0;

	fd = open("/dev/nvidia-modeset", O_RDWR | O_CLOEXEC);
	if (fd < 0) {
		printf("open /dev/nvidia-modeset: %s\n", strerror(errno));
		return 2;
	}
	strncpy(ad->request.versionString, NV_VERSION, sizeof(ad->request.versionString) - 1);
	ad->request.deviceId.rmDeviceId = devid;
	ad->request.deviceId.migDevice = 0;
	ad->request.no3d = NV_TRUE;
	ad->request.enableConsoleHotplugHandling = NV_FALSE;
	if (nvkms(fd, NVKMS_IOCTL_ALLOC_DEVICE, ad, sizeof(*ad)) ||
	    ad->reply.status != NVKMS_ALLOC_DEVICE_STATUS_SUCCESS) {
		printf("ALLOC_DEVICE: %s, status %d\n", strerror(errno), ad->reply.status);
		return 2;
	}
	printf("device %u, %u disp(s), %u api head(s)\n", ad->reply.deviceHandle, ad->reply.numDisps,
	       ad->reply.numHeads);
	for (i = 0; i < count; i++) {
		struct NvKmsNotifyVblankParams nv = { 0 };
		struct pollfd p;
		int efd = open("/dev/nvidia-modeset", O_RDWR | O_CLOEXEC);

		if (efd < 0)
			break;
		nv.request.deviceHandle = ad->reply.deviceHandle;
		nv.request.dispHandle = ad->reply.dispHandles[0];
		nv.request.head = head;
		nv.request.unicastEvent.fd = efd;
		if (nvkms(fd, NVKMS_IOCTL_NOTIFY_VBLANK, &nv, sizeof(nv))) {
			printf("NOTIFY_VBLANK %d: %s\n", i, strerror(errno));
			close(efd);
			break;
		}
		p.fd = efd;
		p.events = POLLIN | POLLPRI;
		p.revents = 0;
		if (poll(&p, 1, 1000) <= 0) {
			timeouts++;
			printf("vblank %d: no notification within 1000 ms\n", i);
			close(efd);
			if (timeouts > 2)
				break;
			continue;
		}
		t[got++] = mono();
		close(efd);
	}
	for (i = 2; i < got; i++) {
		double d = t[i] - t[i - 1];
		s += d; ss += d * d; k++;
		if (d < mn) mn = d;
		if (d > mx) mx = d;
	}
	m = k ? s / k : 0;
	printf("VBLANK requested=%d notified=%d timeouts=%d intervals=%d mean_ms=%.3f rate_hz=%.2f min_ms=%.3f max_ms=%.3f sd_ms=%.3f\n",
	       count, got, timeouts, k, m * 1e3, m ? 1 / m : 0, k ? mn * 1e3 : 0, mx * 1e3,
	       k ? sqrt(fmax(ss / k - m * m, 0)) * 1e3 : 0);
	return got == count ? 0 : 1;
}
