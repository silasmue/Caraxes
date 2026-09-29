// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
/*
 * Wayland port of Leandro-Test scripts/guest/vkprobe.c for the Caraxes display gate
 * (docs/caraxes/display-e.md): the same steps, verdict lines and options, with a
 * wl_surface + xdg_toplevel and VK_KHR_wayland_surface instead of an X window.
 * Report each Vulkan swapchain setup/acquire/present result on a Wayland compositor.
 * --present clears frames to cycling colors and records the first failure and FPS.
 * Under FIFO, achieved FPS also measures frame pacing.
 * Build (next to build-wl.sh's xdg-shell-protocol.c):
 *   gcc -O2 -Wall -I. -o vkprobe-wl vkprobe-wl.c xdg-shell-protocol.c -lvulkan -lwayland-client
 * Run: WAYLAND_DISPLAY=wayland-1 ./vkprobe-wl (swapchain creation only)
 *   WAYLAND_DISPLAY=wayland-1 ./vkprobe-wl --present 120
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <wayland-client.h>
#include "xdg-shell-client-protocol.h"
#define VK_USE_PLATFORM_WAYLAND_KHR
#include <vulkan/vulkan.h>

static struct wl_compositor *compositor;
static struct xdg_wm_base *wm_base;

static void wm_ping(void *d, struct xdg_wm_base *b, uint32_t serial)
{
	(void)d;
	xdg_wm_base_pong(b, serial);
}
static const struct xdg_wm_base_listener wm_l = { wm_ping };

static void reg_global(void *d, struct wl_registry *r, uint32_t name,
		       const char *iface, uint32_t ver)
{
	(void)d;
	(void)ver;
	if (!strcmp(iface, wl_compositor_interface.name))
		compositor = wl_registry_bind(r, name, &wl_compositor_interface, 4);
	else if (!strcmp(iface, xdg_wm_base_interface.name)) {
		wm_base = wl_registry_bind(r, name, &xdg_wm_base_interface, 1);
		xdg_wm_base_add_listener(wm_base, &wm_l, NULL);
	}
}
static void reg_remove(void *d, struct wl_registry *r, uint32_t name)
{
	(void)d;
	(void)r;
	(void)name;
}
static const struct wl_registry_listener reg_l = { reg_global, reg_remove };

static void xs_configure(void *d, struct xdg_surface *s, uint32_t serial)
{
	(void)d;
	xdg_surface_ack_configure(s, serial);
}
static const struct xdg_surface_listener xs_l = { xs_configure };

static const char *rstr(VkResult r)
{
	switch (r) {
	case VK_SUCCESS:
		return "VK_SUCCESS";
	case VK_NOT_READY:
		return "VK_NOT_READY";
	case VK_TIMEOUT:
		return "VK_TIMEOUT";
	case VK_INCOMPLETE:
		return "VK_INCOMPLETE";
	case VK_ERROR_OUT_OF_HOST_MEMORY:
		return "VK_ERROR_OUT_OF_HOST_MEMORY";
	case VK_ERROR_OUT_OF_DEVICE_MEMORY:
		return "VK_ERROR_OUT_OF_DEVICE_MEMORY";
	case VK_ERROR_INITIALIZATION_FAILED:
		return "VK_ERROR_INITIALIZATION_FAILED";
	case VK_ERROR_DEVICE_LOST:
		return "VK_ERROR_DEVICE_LOST";
	case VK_ERROR_EXTENSION_NOT_PRESENT:
		return "VK_ERROR_EXTENSION_NOT_PRESENT";
	case VK_ERROR_FEATURE_NOT_PRESENT:
		return "VK_ERROR_FEATURE_NOT_PRESENT";
	case VK_ERROR_INCOMPATIBLE_DRIVER:
		return "VK_ERROR_INCOMPATIBLE_DRIVER";
	case VK_ERROR_SURFACE_LOST_KHR:
		return "VK_ERROR_SURFACE_LOST_KHR";
	case VK_ERROR_NATIVE_WINDOW_IN_USE_KHR:
		return "VK_ERROR_NATIVE_WINDOW_IN_USE_KHR";
	case VK_ERROR_OUT_OF_DATE_KHR:
		return "VK_ERROR_OUT_OF_DATE_KHR";
	case VK_ERROR_INCOMPATIBLE_DISPLAY_KHR:
		return "VK_ERROR_INCOMPATIBLE_DISPLAY_KHR";
	case VK_ERROR_UNKNOWN:
		return "VK_ERROR_UNKNOWN";
	default:
		return "(other)";
	}
}
#define STEP(what, expr)                                     \
	do {                                                 \
		VkResult _r = (expr);                        \
		printf("  %-42s %s\n", what, rstr(_r));      \
		if (_r != VK_SUCCESS && _r != VK_INCOMPLETE) \
			return 1;                            \
	} while (0)

static double now_s(void)
{
	struct timespec ts;

	clock_gettime(CLOCK_MONOTONIC, &ts);
	return ts.tv_sec + ts.tv_nsec / 1e9;
}

int main(int argc, char **argv)
{
	int present_frames = 0, all_queues = 0, all_ext = 0, i2;
	int ext_limit = -1, no_features = 0, rt_only = 0;

	for (i2 = 1; i2 < argc; i2++) {
		if (!strcmp(argv[i2], "--present") && i2 + 1 < argc)
			present_frames = atoi(argv[++i2]);
		else if (!strcmp(argv[i2], "--queues"))
			/* One queue per FAMILY in a single vkCreateDevice --
			 * the shape CS2 uses and the plain probe does not.
			 * The driver then creates several channels at once,
			 * which is where the interleaved-mapping suspicion
			 * of OPEN-QUESTIONS nr 11 lives. */
			all_queues = 1;
		else if (!strcmp(argv[i2], "--all-ext"))
			/* Enable EVERY device extension the driver offers and
			 * every 1.1/1.2/1.3 feature it reports: the
			 * brute-force reproducer for a CreateDevice that dies
			 * only under a rich engine's request (nr 11). */
			all_ext = 1;
		else if (!strcmp(argv[i2], "--ext-limit") && i2 + 1 < argc)
			/* Bisection: enable only the first N of the offered
			 * extensions (sorted as enumerated). */
			ext_limit = atoi(argv[++i2]);
		else if (!strcmp(argv[i2], "--no-features"))
			/* Bisection: extensions without the feature chain. */
			no_features = 1;
		else if (!strcmp(argv[i2], "--rt"))
			/* Exactly swapchain + deferred_host_operations +
			 * acceleration_structure, dependencies satisfied --
			 * the minimal spec-clean raytracing CreateDevice. */
			rt_only = 1;
	}
	const char *iexts[] = { VK_KHR_SURFACE_EXTENSION_NAME,
				VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME };
	const char *dexts[] = { VK_KHR_SWAPCHAIN_EXTENSION_NAME };
	VkApplicationInfo app = { .sType = VK_STRUCTURE_TYPE_APPLICATION_INFO,
				  .apiVersion = VK_API_VERSION_1_1 };
	VkInstanceCreateInfo ici = {
		.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
		.pApplicationInfo = &app,
		.enabledExtensionCount = 2,
		.ppEnabledExtensionNames = iexts
	};
	VkInstance inst;
	VkPhysicalDevice phys[8];
	uint32_t n = 8, i, qfam = UINT32_MAX, nfmt = 0, nmode = 0;
	VkSurfaceKHR surf;
	VkSurfaceCapabilitiesKHR caps;
	VkPhysicalDeviceProperties props;
	struct wl_display *dpy;
	struct wl_surface *win;
	float prio = 1.0f;

	/* Unbuffered, or a later crash eats every line printed before it --
	 * measured tonight as a bisection poisoned by its own tooling. */
	setvbuf(stdout, NULL, _IONBF, 0);

	if (all_ext)
		app.apiVersion = VK_API_VERSION_1_3;
	STEP("vkCreateInstance", vkCreateInstance(&ici, NULL, &inst));
	STEP("vkEnumeratePhysicalDevices",
	     vkEnumeratePhysicalDevices(inst, &n, phys));
	if (!n) {
		printf("  no physical devices\n");
		return 1;
	}
	vkGetPhysicalDeviceProperties(phys[0], &props);
	printf("  device 0: %s\n", props.deviceName);

	dpy = wl_display_connect(NULL);
	if (!dpy) {
		printf("  cannot open display\n");
		return 1;
	}
	wl_registry_add_listener(wl_display_get_registry(dpy), &reg_l, NULL);
	wl_display_roundtrip(dpy);
	if (!compositor || !wm_base) {
		printf("  compositor lacks wl_compositor or xdg_wm_base\n");
		return 1;
	}
	win = wl_compositor_create_surface(compositor);
	{
		struct xdg_surface *xs = xdg_wm_base_get_xdg_surface(wm_base, win);
		struct xdg_toplevel *tl;

		xdg_surface_add_listener(xs, &xs_l, NULL);
		tl = xdg_surface_get_toplevel(xs);
		xdg_toplevel_set_title(tl, "vkprobe-wl");
		wl_surface_commit(win);
		wl_display_roundtrip(dpy);
	}

	{
		VkWaylandSurfaceCreateInfoKHR si = {
			.sType = VK_STRUCTURE_TYPE_WAYLAND_SURFACE_CREATE_INFO_KHR,
			.display = dpy,
			.surface = win
		};
		STEP("vkCreateWaylandSurfaceKHR",
		     vkCreateWaylandSurfaceKHR(inst, &si, NULL, &surf));
	}

	/* A queue family that can present. Without one the swapchain is not
	 * the thing that is broken. */
	uint32_t nfam = 0;
	{
		uint32_t nq = 0;
		VkQueueFamilyProperties qp[16];

		vkGetPhysicalDeviceQueueFamilyProperties(phys[0], &nq, NULL);
		if (nq > 16)
			nq = 16;
		vkGetPhysicalDeviceQueueFamilyProperties(phys[0], &nq, qp);
		nfam = nq;
		for (i = 0; i < nq; i++) {
			VkBool32 ok = VK_FALSE;
			vkGetPhysicalDeviceSurfaceSupportKHR(phys[0], i, surf,
							     &ok);
			printf("  queue family %u: graphics=%d present=%d\n", i,
			       !!(qp[i].queueFlags & VK_QUEUE_GRAPHICS_BIT),
			       ok);
			if (ok && (qp[i].queueFlags & VK_QUEUE_GRAPHICS_BIT) &&
			    qfam == UINT32_MAX)
				qfam = i;
		}
		if (qfam == UINT32_MAX) {
			printf("  no present-capable graphics queue\n");
			return 1;
		}
	}

	STEP("vkGetPhysicalDeviceSurfaceCapabilitiesKHR",
	     vkGetPhysicalDeviceSurfaceCapabilitiesKHR(phys[0], surf, &caps));
	printf("  minImageCount %u  maxImageCount %u  current %ux%u\n",
	       caps.minImageCount, caps.maxImageCount, caps.currentExtent.width,
	       caps.currentExtent.height);
	STEP("vkGetPhysicalDeviceSurfaceFormatsKHR",
	     vkGetPhysicalDeviceSurfaceFormatsKHR(phys[0], surf, &nfmt, NULL));
	printf("  surface formats: %u\n", nfmt);
	STEP("vkGetPhysicalDeviceSurfacePresentModesKHR",
	     vkGetPhysicalDeviceSurfacePresentModesKHR(phys[0], surf, &nmode,
						       NULL));
	printf("  present modes: %u\n", nmode);
	if (!nfmt || !nmode) {
		printf("  nothing to build a swapchain from\n");
		return 1;
	}

	{
		VkSurfaceFormatKHR fmts[32];
		VkDeviceQueueCreateInfo qcis[16];
		uint32_t nqci = 0, fi;

		if (all_queues) {
			for (fi = 0; fi < nfam && fi < 16; fi++) {
				qcis[nqci] = (VkDeviceQueueCreateInfo){
					.sType =
						VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
					.queueFamilyIndex = fi,
					.queueCount = 1,
					.pQueuePriorities = &prio
				};
				nqci++;
			}
			printf("  creating device with %u queue families\n",
			       nqci);
		} else {
			qcis[0] = (VkDeviceQueueCreateInfo){
				.sType =
					VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
				.queueFamilyIndex = qfam,
				.queueCount = 1,
				.pQueuePriorities = &prio
			};
			nqci = 1;
		}
		static VkExtensionProperties eprops[512];
		static const char *enames[512];
		VkPhysicalDeviceVulkan13Features f13 = {
			.sType =
				VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_3_FEATURES
		};
		VkPhysicalDeviceVulkan12Features f12 = {
			.sType =
				VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_2_FEATURES,
			.pNext = &f13
		};
		VkPhysicalDeviceVulkan11Features f11 = {
			.sType =
				VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_1_FEATURES,
			.pNext = &f12
		};
		VkPhysicalDeviceFeatures2 f2 = {
			.sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
			.pNext = &f11
		};
		VkDeviceCreateInfo dci = {
			.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
			.queueCreateInfoCount = nqci,
			.pQueueCreateInfos = qcis,
			.enabledExtensionCount = 1,
			.ppEnabledExtensionNames = dexts
		};

		if (rt_only) {
			static const char *rtx[] = {
				VK_KHR_SWAPCHAIN_EXTENSION_NAME,
				"VK_KHR_deferred_host_operations",
				"VK_KHR_acceleration_structure"
			};

			dci.enabledExtensionCount = 3;
			dci.ppEnabledExtensionNames = rtx;
			printf("  creating device with swapchain + deferred_host_ops + acceleration_structure\n");
		} else if (all_ext) {
			uint32_t ne = 512, k;

			vkEnumerateDeviceExtensionProperties(phys[0], NULL, &ne,
							     eprops);
			for (k = 0; k < ne; k++)
				enames[k] = eprops[k].extensionName;
			if (ext_limit >= 0 && (uint32_t)ext_limit < ne)
				ne = ext_limit;
			/* VK_KHR_swapchain stays enabled through every
			 * bisection step: the probe presents through it, and
			 * dropping it just crashes the tool, not the driver. */
			for (k = 0; k < ne; k++)
				if (!strcmp(enames[k],
					    VK_KHR_SWAPCHAIN_EXTENSION_NAME))
					break;
			if (k == ne)
				enames[ne++] = VK_KHR_SWAPCHAIN_EXTENSION_NAME;
			/* Ask which features exist, then request exactly those
			 * back: "everything you offer, on". */
			vkGetPhysicalDeviceFeatures2(phys[0], &f2);
			dci.enabledExtensionCount = ne;
			dci.ppEnabledExtensionNames = enames;
			if (!no_features)
				dci.pNext = &f2;
			printf("  creating device with %u extensions%s\n", ne,
			       no_features ? "" : ", all offered features on");
			if (ne <= 8)
				for (k = 0; k < ne; k++)
					printf("    [%u] %s\n", k, enames[k]);
			else
				printf("  last enabled: %s\n", enames[ne - 1]);
		}
		VkDevice dev;
		VkSwapchainKHR sc;
		VkSwapchainCreateInfoKHR sci = {
			.sType = VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR,
			.surface = surf,
			.minImageCount = caps.minImageCount,
			.imageArrayLayers = 1,
			.imageUsage = VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
			.imageSharingMode = VK_SHARING_MODE_EXCLUSIVE,
			.preTransform = caps.currentTransform,
			.compositeAlpha = VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
			.presentMode = VK_PRESENT_MODE_FIFO_KHR,
			.clipped = VK_TRUE
		};

		if (nfmt > 32)
			nfmt = 32;
		vkGetPhysicalDeviceSurfaceFormatsKHR(phys[0], surf, &nfmt,
						     fmts);
		sci.imageFormat = fmts[0].format;
		sci.imageColorSpace = fmts[0].colorSpace;
		sci.imageExtent = caps.currentExtent;
		/* A Wayland surface has no size of its own (0xFFFFFFFF): the
		 * client picks it, 640x480 as the X window of the original. */
		if (sci.imageExtent.width == 0xFFFFFFFFu) {
			sci.imageExtent.width = 640;
			sci.imageExtent.height = 480;
		}
		/* TRANSFER_DST so the loop below may clear the images. Every
		 * implementation that offers COLOR_ATTACHMENT offers this
		 * too, and the gate's creation-only run is unaffected. */
		sci.imageUsage |= VK_IMAGE_USAGE_TRANSFER_DST_BIT;
		STEP("vkCreateDevice",
		     vkCreateDevice(phys[0], &dci, NULL, &dev));
		STEP("vkCreateSwapchainKHR",
		     vkCreateSwapchainKHR(dev, &sci, NULL, &sc));
		printf("  swapchain created\n");

		if (present_frames > 0) {
			VkImage imgs[8];
			uint32_t nimg = 8, f;
			VkQueue q;
			VkCommandPool pool;
			VkCommandBuffer cb;
			VkSemaphore s_acq, s_ren;
			VkFence fence;
			VkCommandPoolCreateInfo pci = {
				.sType =
					VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
				.flags =
					VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT,
				.queueFamilyIndex = qfam
			};
			VkCommandBufferAllocateInfo cbi = {
				.sType =
					VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
				.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY,
				.commandBufferCount = 1
			};
			VkSemaphoreCreateInfo semci = {
				.sType = VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO
			};
			VkFenceCreateInfo fci = {
				.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO
			};
			double t0;

			STEP("vkGetSwapchainImagesKHR",
			     vkGetSwapchainImagesKHR(dev, sc, &nimg, imgs));
			vkGetDeviceQueue(dev, qfam, 0, &q);
			STEP("vkCreateCommandPool",
			     vkCreateCommandPool(dev, &pci, NULL, &pool));
			cbi.commandPool = pool;
			STEP("vkAllocateCommandBuffers",
			     vkAllocateCommandBuffers(dev, &cbi, &cb));
			STEP("vkCreateSemaphore(acquire)",
			     vkCreateSemaphore(dev, &semci, NULL, &s_acq));
			STEP("vkCreateSemaphore(render)",
			     vkCreateSemaphore(dev, &semci, NULL, &s_ren));
			STEP("vkCreateFence",
			     vkCreateFence(dev, &fci, NULL, &fence));

			t0 = now_s();
			for (f = 0; f < (uint32_t)present_frames; f++) {
				uint32_t idx;
				VkResult r;
				/* Cycling clear colour: a grab that reads the
				 * window can tell frame N from frame N+20. */
				VkClearColorValue col = {
					.float32 = { (f % 3 == 0), (f % 3 == 1),
						     (f % 3 == 2), 1.0f }
				};
				VkImageSubresourceRange range = {
					.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT,
					.levelCount = 1,
					.layerCount = 1
				};
				VkCommandBufferBeginInfo bi = {
					.sType =
						VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
					.flags =
						VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT
				};
				VkImageMemoryBarrier to_dst = {
					.sType =
						VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER,
					.srcAccessMask = 0,
					.dstAccessMask =
						VK_ACCESS_TRANSFER_WRITE_BIT,
					.oldLayout = VK_IMAGE_LAYOUT_UNDEFINED,
					.newLayout =
						VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
					.srcQueueFamilyIndex =
						VK_QUEUE_FAMILY_IGNORED,
					.dstQueueFamilyIndex =
						VK_QUEUE_FAMILY_IGNORED,
					.subresourceRange = range
				};
				VkImageMemoryBarrier to_present = to_dst;
				VkPipelineStageFlags wait_stage =
					VK_PIPELINE_STAGE_TRANSFER_BIT;
				VkSubmitInfo si = {
					.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO,
					.waitSemaphoreCount = 1,
					.pWaitSemaphores = &s_acq,
					.pWaitDstStageMask = &wait_stage,
					.commandBufferCount = 1,
					.pCommandBuffers = &cb,
					.signalSemaphoreCount = 1,
					.pSignalSemaphores = &s_ren
				};
				VkPresentInfoKHR pi = {
					.sType =
						VK_STRUCTURE_TYPE_PRESENT_INFO_KHR,
					.waitSemaphoreCount = 1,
					.pWaitSemaphores = &s_ren,
					.swapchainCount = 1,
					.pSwapchains = &sc,
					.pImageIndices = &idx
				};

				/* 2 s, not forever: an acquire that never
				 * returns IS the finding under a compositor
				 * that stopped serving frames. */
				r = vkAcquireNextImageKHR(dev, sc,
							  2000000000ull, s_acq,
							  VK_NULL_HANDLE, &idx);
				if (r != VK_SUCCESS && r != VK_SUBOPTIMAL_KHR) {
					printf("  frame %u: vkAcquireNextImageKHR          %s\n",
					       f, rstr(r));
					return 1;
				}

				vkBeginCommandBuffer(cb, &bi);
				to_dst.image = imgs[idx];
				vkCmdPipelineBarrier(
					cb, VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT,
					VK_PIPELINE_STAGE_TRANSFER_BIT, 0, 0,
					NULL, 0, NULL, 1, &to_dst);
				vkCmdClearColorImage(
					cb, imgs[idx],
					VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
					&col, 1, &range);
				to_present.srcAccessMask =
					VK_ACCESS_TRANSFER_WRITE_BIT;
				to_present.dstAccessMask = 0;
				to_present.oldLayout =
					VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
				to_present.newLayout =
					VK_IMAGE_LAYOUT_PRESENT_SRC_KHR;
				to_present.image = imgs[idx];
				vkCmdPipelineBarrier(
					cb, VK_PIPELINE_STAGE_TRANSFER_BIT,
					VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT, 0,
					0, NULL, 0, NULL, 1, &to_present);
				vkEndCommandBuffer(cb);

				r = vkQueueSubmit(q, 1, &si, fence);
				if (r != VK_SUCCESS) {
					printf("  frame %u: vkQueueSubmit                  %s\n",
					       f, rstr(r));
					return 1;
				}
				r = vkQueuePresentKHR(q, &pi);
				if (r != VK_SUCCESS && r != VK_SUBOPTIMAL_KHR) {
					printf("  frame %u: vkQueuePresentKHR              %s\n",
					       f, rstr(r));
					return 1;
				}
				/* Pace on the fence, and a fence that never
				 * signals is a finding too. */
				r = vkWaitForFences(dev, 1, &fence, VK_TRUE,
						    2000000000ull);
				if (r != VK_SUCCESS) {
					printf("  frame %u: vkWaitForFences                %s\n",
					       f, rstr(r));
					return 1;
				}
				vkResetFences(dev, 1, &fence);
			}
			{
				double dt = now_s() - t0;

				printf("  presented %d frames, all VK_SUCCESS, %.1f FPS\n",
				       present_frames, present_frames / dt);
			}
		}
	}
	return 0;
}
