// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// Single-queue Vulkan ALU and device-local streaming diagnostic. No WSI or GPU
// calls from --self-test. Every timed command buffer is pre-recorded and fenced.
#include <errno.h>
#include <inttypes.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <vulkan/vulkan.h>
#include "shaders.inc"

#define ALU_WORDS 65536u
#define LOCAL_SIZE 256u
#define WAIT_NS UINT64_C(10000000000)
#define MAX_ALLOCATION_BYTES (UINT64_C(384) << 20)

struct options {
    int stream;
    uint32_t mib, iterations, samples, warmup, seconds;
    int mib_given, iterations_given;
    int calibrate;
};

enum calibration_api { CALIBRATION_NONE, CALIBRATION_KHR, CALIBRATION_EXT };

struct gpu {
    VkInstance instance;
    VkPhysicalDevice physical;
    VkPhysicalDeviceProperties properties;
    VkPhysicalDeviceMemoryProperties memory;
    VkDevice device;
    VkQueue queue;
    uint32_t family, timestamp_bits;
    VkCommandPool command_pool;
    VkBuffer src, dst, stage;
    VkDeviceMemory src_memory, dst_memory, stage_memory;
    uint32_t src_type, dst_type, stage_type;
    uint32_t *mapped;
    VkDescriptorSetLayout set_layout;
    VkDescriptorPool descriptor_pool;
    VkDescriptorSet descriptor_set;
    VkPipelineLayout pipeline_layout;
    VkPipeline pipeline;
    VkQueryPool queries;
    VkCommandBuffer timed;
    VkFence timed_fence;
    VkDeviceSize bytes;
    uint64_t allocation_bytes;
    const char *calibration_extension;
    PFN_vkGetCalibratedTimestampsKHR calibrated_timestamps;
};

struct params { uint32_t count, iterations; };

static uint64_t now_ns(void)
{
    struct timespec ts;
    if (clock_gettime(CLOCK_MONOTONIC, &ts) != 0) {
        perror("clock_gettime");
        exit(2);
    }
    return (uint64_t)ts.tv_sec * UINT64_C(1000000000) + (uint64_t)ts.tv_nsec;
}

static void failure(int code, const char *message)
{
    printf("{\"type\":\"error\",\"message\":\"%s\"}\n", message);
    fprintf(stderr, "vk-unit-timing: %s\n", message);
    exit(code);
}

static void vk_check(VkResult r, const char *call, int line)
{
    if (r == VK_SUCCESS) return;
    fprintf(stderr, "vk-unit-timing: %s failed: %d at line %d\n", call, r, line);
    printf("{\"type\":\"error\",\"vk_result\":%d,\"line\":%d}\n", r, line);
    exit(r == VK_TIMEOUT || r == VK_ERROR_DEVICE_LOST ? 3 : 2);
}
#define VK(call) vk_check((call), #call, __LINE__)

static uint32_t parse_u32(const char *text, uint32_t min, uint32_t max)
{
    if (!text[0] || text[0] == '-') failure(2, "invalid numeric argument");
    errno = 0;
    char *end = NULL;
    unsigned long n = strtoul(text, &end, 10);
    if (errno || *end || n < min || n > max) failure(2, "out-of-range numeric argument");
    return (uint32_t)n;
}

static void usage(void)
{
    puts("usage: vk-unit-timing --mode alu|stream [--iterations 32|256|1024]"
         " [--mib 16|64] [--samples 1..32] [--warmup 1..16] [--seconds 1..120] [--calibrate]\n"
         "ALU uses 65536 u32 elements; stream uses two device-local buffers of --mib each.\n"
         "Defaults: alu, iterations=256, mib=16, samples=7, warmup=2, seconds=120.\n"
         "--calibrate requires DEVICE and Linux CLOCK_MONOTONIC calibration domains; emits raw pairs.\n"
         "--self-test is CPU-only. Output is JSONL. Exit 0=PASS, 1=validation/budget,"
         " 2=setup, 3=timeout/device lost.");
}

static struct options parse(int argc, char **argv)
{
    struct options o = {.iterations = 256, .mib = 16, .samples = 7,
                        .warmup = 2, .seconds = 120};
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--mode") && i + 1 < argc) {
            const char *value = argv[++i];
            if (!strcmp(value, "stream")) o.stream = 1;
            else if (!strcmp(value, "alu")) o.stream = 0;
            else failure(2, "mode must be alu or stream");
        } else if (!strcmp(argv[i], "--mib") && i + 1 < argc) {
            o.mib = parse_u32(argv[++i], 16, 64);
            if (o.mib != 16 && o.mib != 64) failure(2, "mib must be 16 or 64");
            o.mib_given = 1;
        } else if (!strcmp(argv[i], "--iterations") && i + 1 < argc) {
            o.iterations = parse_u32(argv[++i], 32, 1024);
            if (o.iterations != 32 && o.iterations != 256 && o.iterations != 1024)
                failure(2, "iterations must be 32, 256 or 1024");
            o.iterations_given = 1;
        } else if (!strcmp(argv[i], "--samples") && i + 1 < argc) {
            o.samples = parse_u32(argv[++i], 1, 32);
        } else if (!strcmp(argv[i], "--warmup") && i + 1 < argc) {
            o.warmup = parse_u32(argv[++i], 1, 16);
        } else if (!strcmp(argv[i], "--seconds") && i + 1 < argc) {
            o.seconds = parse_u32(argv[++i], 1, 120);
        } else if (!strcmp(argv[i], "--calibrate")) {
            o.calibrate = 1;
        } else {
            usage();
            failure(2, "unrecognized or incomplete argument");
        }
    }
    if (o.stream && o.iterations_given) failure(2, "--iterations is ALU-only");
    if (!o.stream && o.mib_given) failure(2, "--mib is stream-only");
    return o;
}

static uint32_t seed(uint32_t i) { return i * 1664525u + 1013904223u; }

static uint32_t expected(uint32_t i, const struct options *o)
{
    uint32_t x = seed(i);
    if (o->stream) return x ^ 0x5a5a5a5au;
    x ^= i * 0x9e3779b9u;
    for (uint32_t j = 0; j < o->iterations; ++j) {
        x ^= x >> 16;
        x *= 0x7feb352du;
        x ^= x >> 15;
        x *= 0x846ca68bu;
        x ^= x >> 16;
        x += (i ^ j) * 0x9e3779b9u;
    }
    return x;
}

static uint64_t timestamp_delta(uint64_t start, uint64_t end, uint32_t bits)
{
    return bits == 64 ? end - start : (end - start) & ((UINT64_C(1) << bits) - 1);
}

static enum calibration_api calibration_extension(const VkExtensionProperties *extensions,
                                                  uint32_t count)
{
    int ext = 0;
    for (uint32_t i = 0; i < count; i++) {
        if (!strcmp(extensions[i].extensionName, VK_KHR_CALIBRATED_TIMESTAMPS_EXTENSION_NAME))
            return CALIBRATION_KHR;
        if (!strcmp(extensions[i].extensionName, VK_EXT_CALIBRATED_TIMESTAMPS_EXTENSION_NAME))
            ext = 1;
    }
    return ext ? CALIBRATION_EXT : CALIBRATION_NONE;
}

static int calibration_domains(const VkTimeDomainKHR *domains, uint32_t count)
{
    int device = 0, monotonic = 0;
    for (uint32_t i = 0; i < count; i++) {
        device |= domains[i] == VK_TIME_DOMAIN_DEVICE_KHR;
        monotonic |= domains[i] == VK_TIME_DOMAIN_CLOCK_MONOTONIC_KHR;
    }
    // CLOCK_MONOTONIC_RAW cannot be joined to the now_ns() clock.
    return device && monotonic;
}

static int self_test(void)
{
    struct options alu = {.stream = 0, .iterations = 32};
    int ok = expected(0, &alu) == UINT32_C(0xa835d3a3);
    alu.iterations = 256;
    ok &= expected(1, &alu) == UINT32_C(0x244520fe);
    alu.iterations = 1024;
    ok &= expected(17, &alu) == UINT32_C(0x96f5a893);
    ok &= timestamp_delta(250, 5, 8) == 11;
    ok &= timestamp_delta(UINT64_MAX - 3, 2, 64) == 6;
    VkExtensionProperties extensions[2] = {
        {.extensionName = VK_EXT_CALIBRATED_TIMESTAMPS_EXTENSION_NAME},
        {.extensionName = VK_KHR_CALIBRATED_TIMESTAMPS_EXTENSION_NAME},
    };
    ok &= calibration_extension(extensions, 0) == CALIBRATION_NONE;
    ok &= calibration_extension(extensions, 1) == CALIBRATION_EXT;
    ok &= calibration_extension(extensions, 2) == CALIBRATION_KHR;
    VkTimeDomainKHR domains[3] = {VK_TIME_DOMAIN_DEVICE_KHR,
        VK_TIME_DOMAIN_CLOCK_MONOTONIC_RAW_KHR, VK_TIME_DOMAIN_CLOCK_MONOTONIC_KHR};
    ok &= !calibration_domains(domains, 1);
    ok &= !calibration_domains(domains, 2);
    ok &= calibration_domains(domains, 3);
    ok &= !calibration_domains(domains + 2, 1);
    printf("{\"type\":\"self_test\",\"pass\":%s}\n", ok ? "true" : "false");
    return ok ? 0 : 1;
}

static uint32_t memory_type(const struct gpu *g, uint32_t allowed,
                            VkMemoryPropertyFlags flags, VkMemoryPropertyFlags prefer)
{
    for (int preferred = 1; preferred >= 0; preferred--)
        for (uint32_t i = 0; i < g->memory.memoryTypeCount; i++) {
            VkMemoryPropertyFlags actual = g->memory.memoryTypes[i].propertyFlags;
            if ((allowed & (1u << i)) && (actual & flags) == flags &&
                (!preferred || (actual & prefer) == prefer)) return i;
        }
    failure(2, "required Vulkan memory type unavailable");
    return 0;
}

static void create_buffer(struct gpu *g, VkDeviceSize bytes, VkBufferUsageFlags usage,
                          VkMemoryPropertyFlags flags, VkBuffer *buffer, VkDeviceMemory *memory,
                          uint32_t *selected_type)
{
    VkBufferCreateInfo bi = {.sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
                             .size = bytes, .usage = usage,
                             .sharingMode = VK_SHARING_MODE_EXCLUSIVE};
    VK(vkCreateBuffer(g->device, &bi, NULL, buffer));
    VkMemoryRequirements req;
    vkGetBufferMemoryRequirements(g->device, *buffer, &req);
    if (req.size > MAX_ALLOCATION_BYTES - g->allocation_bytes)
        failure(2, "buffer allocations exceed 384 MiB");
    g->allocation_bytes += req.size;
    VkMemoryPropertyFlags prefer = (flags & VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
        ? VK_MEMORY_PROPERTY_HOST_CACHED_BIT : 0;
    *selected_type = memory_type(g, req.memoryTypeBits, flags, prefer);
    VkMemoryAllocateInfo ai = {.sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
        .allocationSize = req.size, .memoryTypeIndex = *selected_type};
    VK(vkAllocateMemory(g->device, &ai, NULL, memory));
    VK(vkBindBufferMemory(g->device, *buffer, *memory, 0));
}

static enum calibration_api prepare_calibration(struct gpu *g)
{
    uint32_t count = 0;
    VK(vkEnumerateDeviceExtensionProperties(g->physical, NULL, &count, NULL));
    if (!count || count > 1024) failure(2, "unexpected device-extension count");
    VkExtensionProperties *extensions = calloc(count, sizeof(*extensions));
    if (!extensions) failure(2, "device-extension storage unavailable");
    VK(vkEnumerateDeviceExtensionProperties(g->physical, NULL, &count, extensions));
    enum calibration_api api = calibration_extension(extensions, count);
    free(extensions);
    if (api == CALIBRATION_NONE) failure(2, "requested calibrated timestamps extension absent");
    g->calibration_extension = api == CALIBRATION_KHR
        ? VK_KHR_CALIBRATED_TIMESTAMPS_EXTENSION_NAME : VK_EXT_CALIBRATED_TIMESTAMPS_EXTENSION_NAME;
    // KHR and EXT use compatible aliased structures, domains and signatures.
    const char *name = api == CALIBRATION_KHR
        ? "vkGetPhysicalDeviceCalibrateableTimeDomainsKHR" : "vkGetPhysicalDeviceCalibrateableTimeDomainsEXT";
    PFN_vkGetPhysicalDeviceCalibrateableTimeDomainsKHR get_domains =
        (PFN_vkGetPhysicalDeviceCalibrateableTimeDomainsKHR)vkGetInstanceProcAddr(g->instance, name);
    if (!get_domains) failure(2, "calibratable-domain function unavailable");
    count = 0;
    VK(get_domains(g->physical, &count, NULL));
    if (!count || count > 32) failure(2, "unexpected calibration-domain count");
    VkTimeDomainKHR domains[32];
    VK(get_domains(g->physical, &count, domains));
    if (!calibration_domains(domains, count))
        failure(2, "DEVICE and CLOCK_MONOTONIC calibration domains required");
    return api;
}

static void create_gpu(struct gpu *g, const struct options *o)
{
    VkApplicationInfo app = {.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO,
        .pApplicationName = "owned-vk-unit-timing", .apiVersion = VK_API_VERSION_1_2};
    VkInstanceCreateInfo ii = {.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
                               .pApplicationInfo = &app};
    VK(vkCreateInstance(&ii, NULL, &g->instance));
    uint32_t count = 0;
    VK(vkEnumeratePhysicalDevices(g->instance, &count, NULL));
    if (!count || count > 16) failure(2, "unexpected physical-device count");
    VkPhysicalDevice devices[16];
    VK(vkEnumeratePhysicalDevices(g->instance, &count, devices));
    for (uint32_t i = 0; i < count; i++) {
        VkPhysicalDeviceDriverProperties driver = {
            .sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DRIVER_PROPERTIES};
        VkPhysicalDeviceProperties2 props = {
            .sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2, .pNext = &driver};
        vkGetPhysicalDeviceProperties2(devices[i], &props);
        if (props.properties.vendorID == 0x10de && props.properties.deviceID == 0x1f02 &&
            driver.driverID == VK_DRIVER_ID_NVIDIA_PROPRIETARY) {
            g->physical = devices[i];
            g->properties = props.properties;
            break;
        }
    }
    if (!g->physical) failure(2, "qualified NVIDIA 10de:1f02 Vulkan device absent");
    vkGetPhysicalDeviceMemoryProperties(g->physical, &g->memory);
    g->bytes = o->stream ? (VkDeviceSize)o->mib << 20 : (VkDeviceSize)ALU_WORDS * 4;
    const VkPhysicalDeviceLimits *limit = &g->properties.limits;
    uint32_t words = (uint32_t)(g->bytes / 4);
    uint32_t groups = (words + (o->stream ? 4 * LOCAL_SIZE : LOCAL_SIZE) - 1) /
                      (o->stream ? 4 * LOCAL_SIZE : LOCAL_SIZE);
    if (g->bytes > limit->maxStorageBufferRange || limit->maxPushConstantsSize < 8 ||
        limit->maxPerStageDescriptorStorageBuffers < 2 ||
        limit->maxComputeWorkGroupInvocations < LOCAL_SIZE ||
        limit->maxComputeWorkGroupSize[0] < LOCAL_SIZE ||
        limit->maxComputeWorkGroupCount[0] < groups)
        failure(2, "compute/storage/dispatch device limit too small");
    if (!g->properties.limits.timestampComputeAndGraphics ||
        !isfinite(limit->timestampPeriod) || limit->timestampPeriod <= 0)
        failure(2, "compute timestamps unavailable");
    uint32_t families = 0;
    vkGetPhysicalDeviceQueueFamilyProperties(g->physical, &families, NULL);
    if (!families || families > 32) failure(2, "unexpected queue-family count");
    VkQueueFamilyProperties queues[32];
    vkGetPhysicalDeviceQueueFamilyProperties(g->physical, &families, queues);
    g->family = UINT32_MAX;
    for (int prefer_graphics = 1; prefer_graphics >= 0 && g->family == UINT32_MAX;
         prefer_graphics--)
        for (uint32_t i = 0; i < families; i++) {
            VkQueueFlags flags = queues[i].queueFlags;
            if ((flags & VK_QUEUE_COMPUTE_BIT) && queues[i].timestampValidBits &&
                (!prefer_graphics || (flags & VK_QUEUE_GRAPHICS_BIT))) {
                g->family = i;
                g->timestamp_bits = queues[i].timestampValidBits;
                break;
            }
        }
    if (g->family == UINT32_MAX || g->timestamp_bits > 64)
        failure(2, "compute queue with valid timestamps absent");
    float priority = 1.0f;
    VkDeviceQueueCreateInfo qi = {.sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
        .queueFamilyIndex = g->family, .queueCount = 1, .pQueuePriorities = &priority};
    VkDeviceCreateInfo di = {.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
        .queueCreateInfoCount = 1, .pQueueCreateInfos = &qi};
    enum calibration_api calibration = CALIBRATION_NONE;
    if (o->calibrate) {
        calibration = prepare_calibration(g);
        di.enabledExtensionCount = 1;
        di.ppEnabledExtensionNames = &g->calibration_extension;
    }
    VK(vkCreateDevice(g->physical, &di, NULL, &g->device));
    if (o->calibrate) {
        const char *name = calibration == CALIBRATION_KHR
            ? "vkGetCalibratedTimestampsKHR" : "vkGetCalibratedTimestampsEXT";
        g->calibrated_timestamps =
            (PFN_vkGetCalibratedTimestampsKHR)vkGetDeviceProcAddr(g->device, name);
        if (!g->calibrated_timestamps) failure(2, "calibrated-timestamps function unavailable");
    }
    vkGetDeviceQueue(g->device, g->family, 0, &g->queue);
    VkCommandPoolCreateInfo ci = {.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
        .queueFamilyIndex = g->family};
    VK(vkCreateCommandPool(g->device, &ci, NULL, &g->command_pool));
    create_buffer(g, g->bytes, VK_BUFFER_USAGE_STORAGE_BUFFER_BIT |
                  VK_BUFFER_USAGE_TRANSFER_DST_BIT, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT,
                  &g->src, &g->src_memory, &g->src_type);
    create_buffer(g, g->bytes, VK_BUFFER_USAGE_STORAGE_BUFFER_BIT |
                  VK_BUFFER_USAGE_TRANSFER_SRC_BIT, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT,
                  &g->dst, &g->dst_memory, &g->dst_type);
    create_buffer(g, g->bytes, VK_BUFFER_USAGE_TRANSFER_SRC_BIT |
                  VK_BUFFER_USAGE_TRANSFER_DST_BIT, VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT |
                  VK_MEMORY_PROPERTY_HOST_COHERENT_BIT, &g->stage, &g->stage_memory,
                  &g->stage_type);
    VK(vkMapMemory(g->device, g->stage_memory, 0, g->bytes, 0, (void **)&g->mapped));

    VkDescriptorSetLayoutBinding bindings[2] = {
        {.binding = 0, .descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
         .descriptorCount = 1, .stageFlags = VK_SHADER_STAGE_COMPUTE_BIT},
        {.binding = 1, .descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
         .descriptorCount = 1, .stageFlags = VK_SHADER_STAGE_COMPUTE_BIT},
    };
    VkDescriptorSetLayoutCreateInfo li = {.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
        .bindingCount = 2, .pBindings = bindings};
    VK(vkCreateDescriptorSetLayout(g->device, &li, NULL, &g->set_layout));
    VkDescriptorPoolSize ps = {.type = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, .descriptorCount = 2};
    VkDescriptorPoolCreateInfo pci = {.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO,
        .maxSets = 1, .poolSizeCount = 1, .pPoolSizes = &ps};
    VK(vkCreateDescriptorPool(g->device, &pci, NULL, &g->descriptor_pool));
    VkDescriptorSetAllocateInfo dai = {.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO,
        .descriptorPool = g->descriptor_pool, .descriptorSetCount = 1,
        .pSetLayouts = &g->set_layout};
    VK(vkAllocateDescriptorSets(g->device, &dai, &g->descriptor_set));
    VkDescriptorBufferInfo bi[2] = {{g->src, 0, g->bytes}, {g->dst, 0, g->bytes}};
    VkWriteDescriptorSet writes[2] = {
        {.sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET, .dstSet = g->descriptor_set,
         .dstBinding = 0, .descriptorCount = 1,
         .descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, .pBufferInfo = &bi[0]},
        {.sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET, .dstSet = g->descriptor_set,
         .dstBinding = 1, .descriptorCount = 1,
         .descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, .pBufferInfo = &bi[1]},
    };
    vkUpdateDescriptorSets(g->device, 2, writes, 0, NULL);

    VkPushConstantRange range = {.stageFlags = VK_SHADER_STAGE_COMPUTE_BIT,
                                 .offset = 0, .size = sizeof(struct params)};
    VkPipelineLayoutCreateInfo pli = {.sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO,
        .setLayoutCount = 1, .pSetLayouts = &g->set_layout,
        .pushConstantRangeCount = 1, .pPushConstantRanges = &range};
    VK(vkCreatePipelineLayout(g->device, &pli, NULL, &g->pipeline_layout));
    const uint32_t *code = o->stream ? stream_spv : alu_spv;
    size_t code_size = o->stream ? sizeof(stream_spv) : sizeof(alu_spv);
    VkShaderModuleCreateInfo smi = {.sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
                                    .codeSize = code_size, .pCode = code};
    VkShaderModule module;
    VK(vkCreateShaderModule(g->device, &smi, NULL, &module));
    VkComputePipelineCreateInfo cpi = {.sType = VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO,
        .stage = {.sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                  .stage = VK_SHADER_STAGE_COMPUTE_BIT, .module = module, .pName = "main"},
        .layout = g->pipeline_layout};
    VK(vkCreateComputePipelines(g->device, VK_NULL_HANDLE, 1, &cpi, NULL, &g->pipeline));
    vkDestroyShaderModule(g->device, module, NULL);
    VkQueryPoolCreateInfo qci = {.sType = VK_STRUCTURE_TYPE_QUERY_POOL_CREATE_INFO,
        .queryType = VK_QUERY_TYPE_TIMESTAMP, .queryCount = 2};
    VK(vkCreateQueryPool(g->device, &qci, NULL, &g->queries));
    VkFenceCreateInfo fi = {.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
    VK(vkCreateFence(g->device, &fi, NULL, &g->timed_fence));
    printf("{\"type\":\"meta\",\"mode\":\"%s\",\"vendor_id\":%u,"
           "\"device_id\":%u,\"driver_version_raw\":%u,"
           "\"queue_family\":%u,\"timestamp_bits\":%u,"
           "\"timestamp_period_ns\":%.9g,\"bytes_per_buffer\":%" PRIu64 ","
           "\"logical_bytes_per_dispatch\":%" PRIu64 ",\"workgroups\":%u,"
           "\"total_allocated_bytes\":%" PRIu64 ",\"iterations\":%u,"
           "\"src_memory_type\":%u,\"src_memory_flags\":%u,\"src_heap\":%u,"
           "\"dst_memory_type\":%u,\"dst_memory_flags\":%u,\"dst_heap\":%u,"
           "\"stage_memory_type\":%u,\"stage_memory_flags\":%u,\"stage_heap\":%u,"
           "\"stage_cached\":%s,\"gpu_scope\":\"waw_barrier_plus_dispatch\","
           "\"samples\":%u,\"warmup\":%u,\"seconds\":%u}\n",
           o->stream ? "stream" : "alu", g->properties.vendorID, g->properties.deviceID,
           g->properties.driverVersion, g->family, g->timestamp_bits,
           limit->timestampPeriod, (uint64_t)g->bytes,
           2 * (uint64_t)g->bytes, groups, g->allocation_bytes,
           o->stream ? 0 : o->iterations,
           g->src_type, g->memory.memoryTypes[g->src_type].propertyFlags,
           g->memory.memoryTypes[g->src_type].heapIndex,
           g->dst_type, g->memory.memoryTypes[g->dst_type].propertyFlags,
           g->memory.memoryTypes[g->dst_type].heapIndex,
           g->stage_type, g->memory.memoryTypes[g->stage_type].propertyFlags,
           g->memory.memoryTypes[g->stage_type].heapIndex,
           (g->memory.memoryTypes[g->stage_type].propertyFlags &
            VK_MEMORY_PROPERTY_HOST_CACHED_BIT) ? "true" : "false",
           o->samples, o->warmup, o->seconds);
    if (o->calibrate)
        printf("{\"type\":\"calibration_meta\",\"extension\":\"%s\","
               "\"device_domain\":\"DEVICE\",\"host_domain\":\"CLOCK_MONOTONIC\","
               "\"host_unit\":\"ns\",\"device_unit\":\"ticks\",\"max_deviation_unit\":\"ns\","
               "\"scope\":\"before_and_after_each_sample_batch\","
               "\"limits\":\"Raw pairs only: assess deviation, drift, timestamp wrap and calibration age before correlation; guest CLOCK_MONOTONIC is not the host backend clock; no latency split inferred\"}\n",
               g->calibration_extension);
}

static VkCommandBuffer begin_commands(struct gpu *g)
{
    VkCommandBufferAllocateInfo ai = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
        .commandPool = g->command_pool, .level = VK_COMMAND_BUFFER_LEVEL_PRIMARY,
        .commandBufferCount = 1};
    VkCommandBuffer cb;
    VK(vkAllocateCommandBuffers(g->device, &ai, &cb));
    VkCommandBufferBeginInfo bi = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO};
    VK(vkBeginCommandBuffer(cb, &bi));
    return cb;
}

static void dispatch(VkCommandBuffer cb, const struct gpu *g, const struct options *o)
{
    struct params p = {.count = (uint32_t)(g->bytes / 4),
                       .iterations = o->stream ? 0 : o->iterations};
    uint32_t per_group = o->stream ? LOCAL_SIZE * 4 : LOCAL_SIZE;
    uint32_t groups = (p.count + per_group - 1) / per_group;
    vkCmdBindPipeline(cb, VK_PIPELINE_BIND_POINT_COMPUTE, g->pipeline);
    vkCmdBindDescriptorSets(cb, VK_PIPELINE_BIND_POINT_COMPUTE, g->pipeline_layout,
                            0, 1, &g->descriptor_set, 0, NULL);
    vkCmdPushConstants(cb, g->pipeline_layout, VK_SHADER_STAGE_COMPUTE_BIT,
                       0, sizeof(p), &p);
    vkCmdDispatch(cb, groups, 1, 1);
}

static void finish_one(struct gpu *g, VkCommandBuffer cb, uint64_t deadline)
{
    VK(vkEndCommandBuffer(cb));
    VkFenceCreateInfo fi = {.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
    VkFence fence;
    VK(vkCreateFence(g->device, &fi, NULL, &fence));
    VkSubmitInfo si = {.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO,
        .commandBufferCount = 1, .pCommandBuffers = &cb};
    if (now_ns() >= deadline) failure(1, "global wall budget exhausted");
    VK(vkQueueSubmit(g->queue, 1, &si, fence));
    uint64_t now = now_ns();
    if (now >= deadline) failure(1, "global wall budget exhausted during submit");
    uint64_t wait = deadline - now;
    VK(vkWaitForFences(g->device, 1, &fence, VK_TRUE, wait < WAIT_NS ? wait : WAIT_NS));
    vkDestroyFence(g->device, fence, NULL);
    vkFreeCommandBuffers(g->device, g->command_pool, 1, &cb);
}

static void upload(struct gpu *g, uint64_t deadline)
{
    VkCommandBuffer cb = begin_commands(g);
    VkBufferMemoryBarrier host = {.sType = VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER,
        .srcAccessMask = VK_ACCESS_HOST_WRITE_BIT,
        .dstAccessMask = VK_ACCESS_TRANSFER_READ_BIT,
        .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .buffer = g->stage, .size = VK_WHOLE_SIZE};
    vkCmdPipelineBarrier(cb, VK_PIPELINE_STAGE_HOST_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT,
                         0, 0, NULL, 1, &host, 0, NULL);
    VkBufferCopy copy = {.size = g->bytes};
    vkCmdCopyBuffer(cb, g->stage, g->src, 1, &copy);
    VkBufferMemoryBarrier ready = {.sType = VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER,
        .srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT,
        .dstAccessMask = VK_ACCESS_SHADER_READ_BIT,
        .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .buffer = g->src, .size = VK_WHOLE_SIZE};
    vkCmdPipelineBarrier(cb, VK_PIPELINE_STAGE_TRANSFER_BIT,
                         VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT, 0, 0, NULL, 1, &ready, 0, NULL);
    finish_one(g, cb, deadline);
}

static void readback(struct gpu *g, uint64_t deadline)
{
    VkCommandBuffer cb = begin_commands(g);
    VkBufferMemoryBarrier ready = {.sType = VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER,
        .srcAccessMask = VK_ACCESS_SHADER_WRITE_BIT,
        .dstAccessMask = VK_ACCESS_TRANSFER_READ_BIT,
        .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .buffer = g->dst, .size = VK_WHOLE_SIZE};
    vkCmdPipelineBarrier(cb, VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT,
                         VK_PIPELINE_STAGE_TRANSFER_BIT, 0, 0, NULL, 1, &ready, 0, NULL);
    VkBufferCopy copy = {.size = g->bytes};
    vkCmdCopyBuffer(cb, g->dst, g->stage, 1, &copy);
    VkBufferMemoryBarrier host = {.sType = VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER,
        .srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT,
        .dstAccessMask = VK_ACCESS_HOST_READ_BIT,
        .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .buffer = g->stage, .size = VK_WHOLE_SIZE};
    vkCmdPipelineBarrier(cb, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_HOST_BIT,
                         0, 0, NULL, 1, &host, 0, NULL);
    finish_one(g, cb, deadline);
}

static void prepare_timed(struct gpu *g, const struct options *o)
{
    uint64_t begin = now_ns();
    VkCommandBuffer cb = begin_commands(g);
    vkCmdResetQueryPool(cb, g->queries, 0, 2);
    vkCmdWriteTimestamp(cb, VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT, g->queries, 0);
    // This WAW dependency remains inside the TOP-to-BOTTOM GPU scope so its
    // cost is included rather than hidden.
    VkBufferMemoryBarrier prior = {.sType = VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER,
        .srcAccessMask = VK_ACCESS_SHADER_WRITE_BIT,
        .dstAccessMask = VK_ACCESS_SHADER_WRITE_BIT,
        .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .buffer = g->dst, .size = VK_WHOLE_SIZE};
    vkCmdPipelineBarrier(cb, VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT,
                         VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT, 0, 0, NULL,
                         1, &prior, 0, NULL);
    dispatch(cb, g, o);
    vkCmdWriteTimestamp(cb, VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT, g->queries, 1);
    VK(vkEndCommandBuffer(cb));
    g->timed = cb;
    printf("{\"type\":\"record\",\"cpu_record_us\":%.3f}\n",
           (now_ns() - begin) / 1000.0);
}

struct calibration_pair {
    uint64_t call_begin_ns, call_end_ns;
    uint64_t device_ticks, host_ns, max_deviation_ns;
};

static struct calibration_pair capture_calibration(struct gpu *g, uint64_t deadline)
{
    VkCalibratedTimestampInfoKHR info[2] = {
        {.sType = VK_STRUCTURE_TYPE_CALIBRATED_TIMESTAMP_INFO_KHR,
         .timeDomain = VK_TIME_DOMAIN_DEVICE_KHR},
        {.sType = VK_STRUCTURE_TYPE_CALIBRATED_TIMESTAMP_INFO_KHR,
         .timeDomain = VK_TIME_DOMAIN_CLOCK_MONOTONIC_KHR},
    };
    uint64_t values[2] = {0};
    struct calibration_pair pair = {.call_begin_ns = now_ns()};
    if (pair.call_begin_ns >= deadline) failure(1, "wall budget exhausted before calibration");
    VK(g->calibrated_timestamps(g->device, 2, info, values, &pair.max_deviation_ns));
    pair.call_end_ns = now_ns();
    if (pair.call_end_ns >= deadline) failure(1, "wall budget exhausted during calibration");
    pair.device_ticks = values[0];
    pair.host_ns = values[1];
    return pair;
}

static void emit_calibration(const struct calibration_pair *pair, const char *phase,
                             uint32_t index, const char *side)
{
    printf("{\"type\":\"calibration\",\"phase\":\"%s\",\"index\":%u,\"side\":\"%s\","
           "\"cpu_call_begin_ns\":%" PRIu64 ",\"cpu_call_end_ns\":%" PRIu64 ","
           "\"device_ticks\":%" PRIu64 ",\"host_monotonic_ns\":%" PRIu64 ","
           "\"max_deviation_ns\":%" PRIu64 "}\n",
           phase, index, side, pair->call_begin_ns, pair->call_end_ns,
           pair->device_ticks, pair->host_ns, pair->max_deviation_ns);
}

static int measured_batch(struct gpu *g, const struct options *o, uint64_t deadline)
{
    VkSubmitInfo si = {.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO,
        .commandBufferCount = 1, .pCommandBuffers = &g->timed};
    for (uint32_t i = 0; i < o->warmup + o->samples; i++) {
        if (now_ns() >= deadline) failure(1, "global wall budget exhausted");
        VK(vkResetFences(g->device, 1, &g->timed_fence));
        struct calibration_pair before = {0}, after = {0};
        if (o->calibrate) before = capture_calibration(g, deadline);
        uint64_t begin = now_ns();
        if (o->calibrate && begin >= deadline)
            failure(1, "wall budget exhausted before calibrated submission");
        VK(vkQueueSubmit(g->queue, 1, &si, g->timed_fence));
        uint64_t submitted = now_ns();
        if (submitted >= deadline) failure(1, "global wall budget exhausted during submit");
        uint64_t wait = deadline - submitted;
        VK(vkWaitForFences(g->device, 1, &g->timed_fence, VK_TRUE,
                           wait < WAIT_NS ? wait : WAIT_NS));
        uint64_t finished = now_ns();
        uint64_t q[2][2] = {{0}};
        VK(vkGetQueryPoolResults(g->device, g->queries, 0, 2,
                                 sizeof(q), q, sizeof(q[0]),
                                 VK_QUERY_RESULT_64_BIT |
                                 VK_QUERY_RESULT_WITH_AVAILABILITY_BIT));
        uint64_t queried = now_ns();
        if (o->calibrate) after = capture_calibration(g, deadline);
        if (!q[0][1] || !q[1][1]) failure(1, "timestamp unavailable after fence");
        long double wrap_ns = ldexpl((long double)g->properties.limits.timestampPeriod,
                                     (int)g->timestamp_bits);
        if ((long double)(finished - begin) >= wrap_ns)
            failure(1, "CPU interval permits full timestamp wrap");
        uint64_t ticks = timestamp_delta(q[0][0], q[1][0], g->timestamp_bits);
        if (!ticks) failure(1, "zero GPU timestamp delta");
        double gpu_us = ticks * (double)g->properties.limits.timestampPeriod / 1000.0;
        if (finished - begin >= UINT64_C(1000000000) || gpu_us >= 1000000.0)
            failure(1, "one probe job exceeded 1 second");
        const char *phase = i < o->warmup ? "warmup" : "measured";
        uint32_t index = i < o->warmup ? i : i - o->warmup;
        if (o->calibrate) {
            emit_calibration(&before, phase, index, "before");
            emit_calibration(&after, phase, index, "after");
        }
        printf("{\"type\":\"sample\",\"phase\":\"%s\",\"index\":%u,"
               "\"cpu_submit_us\":%.3f,\"cpu_fence_wait_us\":%.3f,"
               "\"cpu_submit_wait_us\":%.3f,\"cpu_query_us\":%.3f,"
               "\"gpu_us\":%.3f,\"gpu_ticks\":%" PRIu64 ","
               "\"gpu_start_ticks\":%" PRIu64 ",\"gpu_end_ticks\":%" PRIu64,
               phase, index,
               (submitted - begin) / 1000.0, (finished - submitted) / 1000.0,
               (finished - begin) / 1000.0, (queried - finished) / 1000.0,
               gpu_us, ticks, q[0][0], q[1][0]);
        if (o->calibrate)
            printf(",\"cpu_begin_ns\":%" PRIu64 ",\"cpu_submitted_ns\":%" PRIu64 ","
                   "\"cpu_fence_finished_ns\":%" PRIu64 ",\"cpu_query_done_ns\":%" PRIu64,
                   begin, submitted, finished, queried);
        puts("}");
    }
    return 1;
}

static void destroy_gpu(struct gpu *g)
{
    vkDestroyFence(g->device, g->timed_fence, NULL);
    vkFreeCommandBuffers(g->device, g->command_pool, 1, &g->timed);
    vkDestroyQueryPool(g->device, g->queries, NULL);
    vkDestroyPipeline(g->device, g->pipeline, NULL);
    vkDestroyPipelineLayout(g->device, g->pipeline_layout, NULL);
    vkDestroyDescriptorPool(g->device, g->descriptor_pool, NULL);
    vkDestroyDescriptorSetLayout(g->device, g->set_layout, NULL);
    vkUnmapMemory(g->device, g->stage_memory);
    vkDestroyBuffer(g->device, g->stage, NULL);
    vkDestroyBuffer(g->device, g->dst, NULL);
    vkDestroyBuffer(g->device, g->src, NULL);
    vkFreeMemory(g->device, g->stage_memory, NULL);
    vkFreeMemory(g->device, g->dst_memory, NULL);
    vkFreeMemory(g->device, g->src_memory, NULL);
    vkDestroyCommandPool(g->device, g->command_pool, NULL);
    vkDestroyDevice(g->device, NULL);
    vkDestroyInstance(g->instance, NULL);
}

int main(int argc, char **argv)
{
    setvbuf(stdout, NULL, _IOLBF, 0);
    if (argc == 2 && !strcmp(argv[1], "--help")) { usage(); return 0; }
    if (argc == 2 && !strcmp(argv[1], "--self-test")) return self_test();
    struct options o = parse(argc, argv);
    uint64_t deadline = now_ns() + (uint64_t)o.seconds * UINT64_C(1000000000);
    // A synchronous driver call may not return to permit deadline checks.
    alarm(o.seconds);
    struct gpu g = {0};
    create_gpu(&g, &o);
    uint32_t words = (uint32_t)(g.bytes / 4);
    for (uint32_t i = 0; i < words; i++) g.mapped[i] = seed(i);
    upload(&g, deadline);
    VkCommandBuffer precondition = begin_commands(&g);
    dispatch(precondition, &g, &o);
    finish_one(&g, precondition, deadline);
    prepare_timed(&g, &o);
    measured_batch(&g, &o, deadline);
    uint64_t readback_start = now_ns();
    readback(&g, deadline);
    uint64_t verification_start = now_ns();
    uint64_t mismatches = 0;
    uint64_t checksum = UINT64_C(1469598103934665603);
    uint32_t first_bad = UINT32_MAX;
    for (uint32_t i = 0; i < words; i++) {
        uint32_t got = g.mapped[i];
        if (got != expected(i, &o)) {
            mismatches++;
            if (first_bad == UINT32_MAX) first_bad = i;
        }
        checksum = (checksum ^ got) * UINT64_C(1099511628211);
        if ((i & 4095u) == 0 && now_ns() >= deadline)
            failure(1, "global wall budget exhausted in validation");
    }
    uint64_t verification_end = now_ns();
    if (verification_end >= deadline) failure(1, "global wall budget exhausted in validation");
    printf("{\"type\":\"validation\",\"checked_words\":%u,"
           "\"mismatches\":%" PRIu64 ",\"first_bad\":%d,"
           "\"checksum_fnv64\":\"%016" PRIx64 "\","
           "\"cpu_readback_us\":%.3f,\"cpu_verify_us\":%.3f,\"pass\":%s}\n",
           words, mismatches, first_bad == UINT32_MAX ? -1 : (int)first_bad,
           checksum, (verification_start - readback_start) / 1000.0,
           (verification_end - verification_start) / 1000.0,
           mismatches ? "false" : "true");
    destroy_gpu(&g);
    alarm(0);
    printf("{\"type\":\"complete\",\"samples\":%u,\"pass\":%s}\n",
           o.samples, mismatches ? "false" : "true");
    return mismatches ? 1 : 0;
}
