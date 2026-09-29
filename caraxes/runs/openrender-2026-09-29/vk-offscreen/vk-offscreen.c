// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//
// vk-offscreen: render on a Vulkan device without any window system and read the result
// back (docs/research/open-render-2026-09-29.md). Two steps, each checked byte by byte:
//   A  vkCmdClearColorImage on a 256x256 RGBA8 image, copied to a host-visible buffer
//      (NVK: a 3D-class surface clear and a copy-engine copy);
//   B  a render pass that clears and draws one constant-colour triangle (vertex and
//      fragment shader, compiled by the driver), copied back the same way.
// Step B's image is written as a binary PPM when a path is given.
//
// Usage: vk-offscreen [--driver nvk|lvp|any] [--skip-draw] [out.ppm]
//   --driver nvk (default) picks the first device whose driverID is MESA_NVK and fails when
//   there is none; lvp picks Mesa's CPU implementation (lavapipe, a self-test of this
//   program); any picks the first device that is not a CPU implementation.
// Exit: 0 all steps passed, 1 a check failed, 2 setup failed, 3 the GPU did not finish.
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <vulkan/vulkan.h>

#define W 256
#define H 256
#define TIMEOUT_NS 10000000000ull

static const uint32_t vert_spv[] =
#include "tri.vert.inc"
    ;
static const uint32_t frag_spv[] =
#include "tri.frag.inc"
    ;

#define CHECK(call)                                                                        \
    do {                                                                                   \
        VkResult r_ = (call);                                                              \
        if (r_ != VK_SUCCESS) {                                                            \
            fprintf(stderr, "vk-offscreen: %s failed: %d (line %d)\n", #call, r_, __LINE__); \
            exit(r_ == VK_TIMEOUT || r_ == VK_ERROR_DEVICE_LOST ? 3 : 2);                  \
        }                                                                                  \
    } while (0)

static double now_ms(void)
{
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec * 1e3 + t.tv_nsec / 1e6;
}

struct ctx {
    VkPhysicalDevice pd;
    VkDevice dev;
    VkQueue queue;
    uint32_t qfam;
    VkCommandPool pool;
    VkPhysicalDeviceMemoryProperties mem;
};

static uint32_t memory_type(const struct ctx *c, uint32_t bits, VkMemoryPropertyFlags want)
{
    for (uint32_t i = 0; i < c->mem.memoryTypeCount; i++)
        if ((bits & (1u << i)) && (c->mem.memoryTypes[i].propertyFlags & want) == want)
            return i;
    fprintf(stderr, "vk-offscreen: no memory type for bits %#x flags %#x\n", bits, want);
    exit(2);
}

static VkDeviceMemory alloc_for(const struct ctx *c, VkMemoryRequirements req,
                                VkMemoryPropertyFlags want)
{
    VkMemoryAllocateInfo ai = {
        .sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
        .allocationSize = req.size,
        .memoryTypeIndex = memory_type(c, req.memoryTypeBits, want),
    };
    VkDeviceMemory m;
    CHECK(vkAllocateMemory(c->dev, &ai, NULL, &m));
    return m;
}

static void barrier(VkCommandBuffer cb, VkImage img, VkImageLayout from, VkImageLayout to,
                    VkAccessFlags src_access, VkAccessFlags dst_access,
                    VkPipelineStageFlags src_stage, VkPipelineStageFlags dst_stage)
{
    VkImageMemoryBarrier b = {
        .sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER,
        .srcAccessMask = src_access,
        .dstAccessMask = dst_access,
        .oldLayout = from,
        .newLayout = to,
        .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .image = img,
        .subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1},
    };
    vkCmdPipelineBarrier(cb, src_stage, dst_stage, 0, 0, NULL, 0, NULL, 1, &b);
}

static void copy_to_buffer(VkCommandBuffer cb, VkImage img, VkBuffer buf)
{
    VkBufferImageCopy r = {
        .imageSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1},
        .imageExtent = {W, H, 1},
    };
    vkCmdCopyImageToBuffer(cb, img, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, buf, 1, &r);
    VkBufferMemoryBarrier hb = {
        .sType = VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER,
        .srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT,
        .dstAccessMask = VK_ACCESS_HOST_READ_BIT,
        .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .buffer = buf,
        .size = VK_WHOLE_SIZE,
    };
    vkCmdPipelineBarrier(cb, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_HOST_BIT, 0, 0,
                         NULL, 1, &hb, 0, NULL);
}

// Records with `rec`, submits once, waits at most TIMEOUT_NS; returns the wall time in ms.
static double submit(const struct ctx *c, void (*rec)(VkCommandBuffer, void *), void *arg)
{
    VkCommandBufferAllocateInfo ai = {
        .sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
        .commandPool = c->pool,
        .level = VK_COMMAND_BUFFER_LEVEL_PRIMARY,
        .commandBufferCount = 1,
    };
    VkCommandBuffer cb;
    CHECK(vkAllocateCommandBuffers(c->dev, &ai, &cb));
    VkCommandBufferBeginInfo bi = {
        .sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
        .flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
    };
    CHECK(vkBeginCommandBuffer(cb, &bi));
    rec(cb, arg);
    CHECK(vkEndCommandBuffer(cb));
    VkFenceCreateInfo fi = {.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
    VkFence f;
    CHECK(vkCreateFence(c->dev, &fi, NULL, &f));
    VkSubmitInfo si = {
        .sType = VK_STRUCTURE_TYPE_SUBMIT_INFO,
        .commandBufferCount = 1,
        .pCommandBuffers = &cb,
    };
    double t0 = now_ms();
    CHECK(vkQueueSubmit(c->queue, 1, &si, f));
    CHECK(vkWaitForFences(c->dev, 1, &f, VK_TRUE, TIMEOUT_NS));
    double t = now_ms() - t0;
    vkDestroyFence(c->dev, f, NULL);
    vkFreeCommandBuffers(c->dev, c->pool, 1, &cb);
    return t;
}

struct clear_arg {
    VkImage img;
    VkBuffer buf;
};

static void rec_clear(VkCommandBuffer cb, void *p)
{
    struct clear_arg *a = p;
    barrier(cb, a->img, VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 0,
            VK_ACCESS_TRANSFER_WRITE_BIT, VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT,
            VK_PIPELINE_STAGE_TRANSFER_BIT);
    VkClearColorValue v = {.float32 = {0.0f, 0.25f, 0.5f, 1.0f}};
    VkImageSubresourceRange r = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1};
    vkCmdClearColorImage(cb, a->img, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, &v, 1, &r);
    barrier(cb, a->img, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
            VK_ACCESS_TRANSFER_WRITE_BIT, VK_ACCESS_TRANSFER_READ_BIT,
            VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT);
    copy_to_buffer(cb, a->img, a->buf);
}

struct draw_arg {
    VkImage img;
    VkBuffer buf;
    VkRenderPass rp;
    VkFramebuffer fb;
    VkPipeline pipe;
};

static void rec_draw(VkCommandBuffer cb, void *p)
{
    struct draw_arg *a = p;
    VkClearValue cv = {.color = {.float32 = {0.1f, 0.1f, 0.1f, 1.0f}}};
    VkRenderPassBeginInfo rb = {
        .sType = VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO,
        .renderPass = a->rp,
        .framebuffer = a->fb,
        .renderArea = {{0, 0}, {W, H}},
        .clearValueCount = 1,
        .pClearValues = &cv,
    };
    vkCmdBeginRenderPass(cb, &rb, VK_SUBPASS_CONTENTS_INLINE);
    vkCmdBindPipeline(cb, VK_PIPELINE_BIND_POINT_GRAPHICS, a->pipe);
    vkCmdDraw(cb, 3, 1, 0, 0);
    vkCmdEndRenderPass(cb);
    // The render pass leaves the image in TRANSFER_SRC_OPTIMAL (finalLayout).
    copy_to_buffer(cb, a->img, a->buf);
}

static int near(int got, int want) { return got >= want - 1 && got <= want + 1; }

static int pixel_is(const uint8_t *px, int x, int y, const int want[4])
{
    const uint8_t *q = px + 4 * (y * W + x);
    return near(q[0], want[0]) && near(q[1], want[1]) && near(q[2], want[2]) &&
           near(q[3], want[3]);
}

static void print_pixel(const char *what, const uint8_t *px, int x, int y)
{
    const uint8_t *q = px + 4 * (y * W + x);
    printf("  %s (%d,%d) = %d %d %d %d\n", what, x, y, q[0], q[1], q[2], q[3]);
}

static VkShaderModule shader(const struct ctx *c, const uint32_t *code, size_t size)
{
    VkShaderModuleCreateInfo si = {
        .sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
        .codeSize = size,
        .pCode = code,
    };
    VkShaderModule m;
    CHECK(vkCreateShaderModule(c->dev, &si, NULL, &m));
    return m;
}

int main(int argc, char **argv)
{
    const char *driver = "nvk";
    int skip_draw = 0;
    const char *ppm = NULL;
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--driver") && i + 1 < argc) {
            driver = argv[++i];
        } else if (!strcmp(argv[i], "--skip-draw")) {
            skip_draw = 1;
        } else if (argv[i][0] != '-') {
            ppm = argv[i];
        } else {
            fprintf(stderr, "usage: vk-offscreen [--driver nvk|lvp|any] [--skip-draw] [out.ppm]\n");
            return 2;
        }
    }

    VkApplicationInfo app = {
        .sType = VK_STRUCTURE_TYPE_APPLICATION_INFO,
        .pApplicationName = "vk-offscreen",
        .apiVersion = VK_API_VERSION_1_2,
    };
    VkInstanceCreateInfo ii = {.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
                               .pApplicationInfo = &app};
    VkInstance inst;
    CHECK(vkCreateInstance(&ii, NULL, &inst));

    uint32_t n = 0;
    CHECK(vkEnumeratePhysicalDevices(inst, &n, NULL));
    VkPhysicalDevice pds[16];
    if (n > 16)
        n = 16;
    CHECK(vkEnumeratePhysicalDevices(inst, &n, pds));
    struct ctx c = {0};
    for (uint32_t i = 0; i < n; i++) {
        VkPhysicalDeviceDriverProperties drv = {
            .sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DRIVER_PROPERTIES};
        VkPhysicalDeviceProperties2 p2 = {.sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
                                          .pNext = &drv};
        vkGetPhysicalDeviceProperties2(pds[i], &p2);
        printf("device %u: %s, %04x:%04x, driver %s (%s), id %d\n", i, p2.properties.deviceName,
               p2.properties.vendorID, p2.properties.deviceID, drv.driverName, drv.driverInfo,
               drv.driverID);
        int pick = !strcmp(driver, "any") ? p2.properties.deviceType != VK_PHYSICAL_DEVICE_TYPE_CPU
                   : !strcmp(driver, "lvp") ? drv.driverID == VK_DRIVER_ID_MESA_LLVMPIPE
                                            : drv.driverID == VK_DRIVER_ID_MESA_NVK;
        if (pick && !c.pd)
            c.pd = pds[i];
    }
    if (!c.pd) {
        fprintf(stderr, "vk-offscreen: no device for --driver %s\n", driver);
        return 2;
    }
    vkGetPhysicalDeviceMemoryProperties(c.pd, &c.mem);

    uint32_t nq = 0;
    vkGetPhysicalDeviceQueueFamilyProperties(c.pd, &nq, NULL);
    VkQueueFamilyProperties qf[16];
    if (nq > 16)
        nq = 16;
    vkGetPhysicalDeviceQueueFamilyProperties(c.pd, &nq, qf);
    c.qfam = UINT32_MAX;
    for (uint32_t i = 0; i < nq && c.qfam == UINT32_MAX; i++)
        if (qf[i].queueFlags & VK_QUEUE_GRAPHICS_BIT)
            c.qfam = i;
    if (c.qfam == UINT32_MAX) {
        fprintf(stderr, "vk-offscreen: no graphics queue\n");
        return 2;
    }
    float prio = 1.0f;
    VkDeviceQueueCreateInfo qi = {.sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                                  .queueFamilyIndex = c.qfam,
                                  .queueCount = 1,
                                  .pQueuePriorities = &prio};
    VkDeviceCreateInfo di = {.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
                             .queueCreateInfoCount = 1,
                             .pQueueCreateInfos = &qi};
    double t0 = now_ms();
    CHECK(vkCreateDevice(c.pd, &di, NULL, &c.dev));
    printf("vkCreateDevice: %.1f ms\n", now_ms() - t0);
    vkGetDeviceQueue(c.dev, c.qfam, 0, &c.queue);
    VkCommandPoolCreateInfo pi = {.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
                                  .queueFamilyIndex = c.qfam};
    CHECK(vkCreateCommandPool(c.dev, &pi, NULL, &c.pool));

    // The target image (device local) and the readback buffer (host visible, coherent).
    VkImageCreateInfo ici = {
        .sType = VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,
        .imageType = VK_IMAGE_TYPE_2D,
        .format = VK_FORMAT_R8G8B8A8_UNORM,
        .extent = {W, H, 1},
        .mipLevels = 1,
        .arrayLayers = 1,
        .samples = VK_SAMPLE_COUNT_1_BIT,
        .tiling = VK_IMAGE_TILING_OPTIMAL,
        .usage = VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT |
                 VK_IMAGE_USAGE_TRANSFER_DST_BIT,
        .initialLayout = VK_IMAGE_LAYOUT_UNDEFINED,
    };
    VkImage img;
    CHECK(vkCreateImage(c.dev, &ici, NULL, &img));
    VkMemoryRequirements ir;
    vkGetImageMemoryRequirements(c.dev, img, &ir);
    VkDeviceMemory img_mem = alloc_for(&c, ir, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
    CHECK(vkBindImageMemory(c.dev, img, img_mem, 0));

    VkBufferCreateInfo bci = {.sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
                              .size = W * H * 4,
                              .usage = VK_BUFFER_USAGE_TRANSFER_DST_BIT};
    VkBuffer buf;
    CHECK(vkCreateBuffer(c.dev, &bci, NULL, &buf));
    VkMemoryRequirements br;
    vkGetBufferMemoryRequirements(c.dev, buf, &br);
    VkDeviceMemory buf_mem = alloc_for(
        &c, br, VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT);
    CHECK(vkBindBufferMemory(c.dev, buf, buf_mem, 0));
    uint8_t *px;
    CHECK(vkMapMemory(c.dev, buf_mem, 0, VK_WHOLE_SIZE, 0, (void **)&px));

    int fails = 0;

    // Step A: clear and copy.
    memset(px, 0xAA, W * H * 4);
    struct clear_arg ca = {img, buf};
    double ta = submit(&c, rec_clear, &ca);
    const int want_a[4] = {0, 64, 128, 255};
    int bad = 0;
    for (int y = 0; y < H; y++)
        for (int x = 0; x < W; x++)
            bad += !pixel_is(px, x, y, want_a);
    printf("step A clear+copy: %.2f ms, %d of %d pixels wrong: %s\n", ta, bad, W * H,
           bad ? "FAIL" : "PASS");
    if (bad) {
        print_pixel("pixel", px, 0, 0);
        print_pixel("pixel", px, W / 2, H / 2);
        fails++;
    }

    if (!skip_draw) {
        // Step B: render pass clear plus one triangle.
        VkImageViewCreateInfo vci = {
            .sType = VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,
            .image = img,
            .viewType = VK_IMAGE_VIEW_TYPE_2D,
            .format = VK_FORMAT_R8G8B8A8_UNORM,
            .subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1},
        };
        VkImageView view;
        CHECK(vkCreateImageView(c.dev, &vci, NULL, &view));
        VkAttachmentDescription att = {
            .format = VK_FORMAT_R8G8B8A8_UNORM,
            .samples = VK_SAMPLE_COUNT_1_BIT,
            .loadOp = VK_ATTACHMENT_LOAD_OP_CLEAR,
            .storeOp = VK_ATTACHMENT_STORE_OP_STORE,
            .stencilLoadOp = VK_ATTACHMENT_LOAD_OP_DONT_CARE,
            .stencilStoreOp = VK_ATTACHMENT_STORE_OP_DONT_CARE,
            .initialLayout = VK_IMAGE_LAYOUT_UNDEFINED,
            .finalLayout = VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
        };
        VkAttachmentReference ref = {0, VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL};
        VkSubpassDescription sub = {.pipelineBindPoint = VK_PIPELINE_BIND_POINT_GRAPHICS,
                                    .colorAttachmentCount = 1,
                                    .pColorAttachments = &ref};
        // Make the attachment writes visible to the copy that follows the pass.
        VkSubpassDependency dep = {
            .srcSubpass = 0,
            .dstSubpass = VK_SUBPASS_EXTERNAL,
            .srcStageMask = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
            .dstStageMask = VK_PIPELINE_STAGE_TRANSFER_BIT,
            .srcAccessMask = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT,
            .dstAccessMask = VK_ACCESS_TRANSFER_READ_BIT,
        };
        VkRenderPassCreateInfo rci = {.sType = VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO,
                                      .attachmentCount = 1,
                                      .pAttachments = &att,
                                      .subpassCount = 1,
                                      .pSubpasses = &sub,
                                      .dependencyCount = 1,
                                      .pDependencies = &dep};
        VkRenderPass rp;
        CHECK(vkCreateRenderPass(c.dev, &rci, NULL, &rp));
        VkFramebufferCreateInfo fci = {.sType = VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO,
                                       .renderPass = rp,
                                       .attachmentCount = 1,
                                       .pAttachments = &view,
                                       .width = W,
                                       .height = H,
                                       .layers = 1};
        VkFramebuffer fb;
        CHECK(vkCreateFramebuffer(c.dev, &fci, NULL, &fb));

        VkShaderModule vs = shader(&c, vert_spv, sizeof(vert_spv));
        VkShaderModule fs = shader(&c, frag_spv, sizeof(frag_spv));
        VkPipelineShaderStageCreateInfo st[2] = {
            {.sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
             .stage = VK_SHADER_STAGE_VERTEX_BIT,
             .module = vs,
             .pName = "main"},
            {.sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
             .stage = VK_SHADER_STAGE_FRAGMENT_BIT,
             .module = fs,
             .pName = "main"},
        };
        VkPipelineVertexInputStateCreateInfo vin = {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO};
        VkPipelineInputAssemblyStateCreateInfo ia = {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
            .topology = VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST};
        VkViewport vp = {0, 0, W, H, 0, 1};
        VkRect2D sc = {{0, 0}, {W, H}};
        VkPipelineViewportStateCreateInfo vps = {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO,
            .viewportCount = 1,
            .pViewports = &vp,
            .scissorCount = 1,
            .pScissors = &sc};
        VkPipelineRasterizationStateCreateInfo rs = {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
            .polygonMode = VK_POLYGON_MODE_FILL,
            .cullMode = VK_CULL_MODE_NONE,
            .frontFace = VK_FRONT_FACE_CLOCKWISE,
            .lineWidth = 1.0f};
        VkPipelineMultisampleStateCreateInfo ms = {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
            .rasterizationSamples = VK_SAMPLE_COUNT_1_BIT};
        VkPipelineColorBlendAttachmentState cba = {
            .colorWriteMask = VK_COLOR_COMPONENT_R_BIT | VK_COLOR_COMPONENT_G_BIT |
                              VK_COLOR_COMPONENT_B_BIT | VK_COLOR_COMPONENT_A_BIT};
        VkPipelineColorBlendStateCreateInfo cb = {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
            .attachmentCount = 1,
            .pAttachments = &cba};
        VkPipelineLayoutCreateInfo lci = {.sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO};
        VkPipelineLayout layout;
        CHECK(vkCreatePipelineLayout(c.dev, &lci, NULL, &layout));
        VkGraphicsPipelineCreateInfo gci = {
            .sType = VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO,
            .stageCount = 2,
            .pStages = st,
            .pVertexInputState = &vin,
            .pInputAssemblyState = &ia,
            .pViewportState = &vps,
            .pRasterizationState = &rs,
            .pMultisampleState = &ms,
            .pColorBlendState = &cb,
            .layout = layout,
            .renderPass = rp,
        };
        VkPipeline pipe;
        t0 = now_ms();
        CHECK(vkCreateGraphicsPipelines(c.dev, VK_NULL_HANDLE, 1, &gci, NULL, &pipe));
        printf("vkCreateGraphicsPipelines: %.1f ms\n", now_ms() - t0);

        memset(px, 0xAA, W * H * 4);
        struct draw_arg da = {img, buf, rp, fb, pipe};
        double tb = submit(&c, rec_draw, &da);
        const int tri[4] = {255, 128, 0, 255}, bg[4] = {26, 26, 26, 255};
        int inside = 0, outside = 0, other = 0;
        for (int y = 0; y < H; y++)
            for (int x = 0; x < W; x++) {
                if (pixel_is(px, x, y, tri))
                    inside++;
                else if (pixel_is(px, x, y, bg))
                    outside++;
                else
                    other++;
            }
        // The triangle covers (1.6 * 1.6 / 2) / 4 = 32 % of the viewport: about 20972 pixels.
        int ok = pixel_is(px, W / 2, H / 2 + 20, tri) && pixel_is(px, 2, 2, bg) &&
                 pixel_is(px, W - 3, 2, bg) && other == 0 && inside > 19000 && inside < 23000;
        printf("step B draw+copy: %.2f ms, triangle %d, background %d, other %d pixels: %s\n",
               tb, inside, outside, other, ok ? "PASS" : "FAIL");
        if (!ok) {
            print_pixel("centre", px, W / 2, H / 2 + 20);
            print_pixel("corner", px, 2, 2);
            fails++;
        }
        if (ppm) {
            FILE *f = fopen(ppm, "wb");
            if (f) {
                fprintf(f, "P6\n%d %d\n255\n", W, H);
                for (int i = 0; i < W * H; i++)
                    fwrite(px + 4 * i, 1, 3, f);
                fclose(f);
                printf("wrote %s\n", ppm);
            } else {
                perror(ppm);
            }
        }
        vkDestroyPipeline(c.dev, pipe, NULL);
        vkDestroyPipelineLayout(c.dev, layout, NULL);
        vkDestroyShaderModule(c.dev, vs, NULL);
        vkDestroyShaderModule(c.dev, fs, NULL);
        vkDestroyFramebuffer(c.dev, fb, NULL);
        vkDestroyRenderPass(c.dev, rp, NULL);
        vkDestroyImageView(c.dev, view, NULL);
    }

    vkUnmapMemory(c.dev, buf_mem);
    vkDestroyBuffer(c.dev, buf, NULL);
    vkFreeMemory(c.dev, buf_mem, NULL);
    vkDestroyImage(c.dev, img, NULL);
    vkFreeMemory(c.dev, img_mem, NULL);
    vkDestroyCommandPool(c.dev, c.pool, NULL);
    vkDestroyDevice(c.dev, NULL);
    vkDestroyInstance(inst, NULL);
    printf("vk-offscreen: %s\n", fails ? "FAIL" : "PASS");
    return fails ? 1 : 0;
}
