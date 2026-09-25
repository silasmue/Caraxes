// SPDX-License-Identifier: MIT
/*
 * gsp-abi-sizes.c -- print the compiled sizes and field offsets of the
 * CPU<->GSP message-queue and boot structures of the vendored NVIDIA open
 * kernel modules, as a Markdown table.
 *
 * Built by caraxes/tools/gsp-abi-sizes.sh against
 * vendor/open-gpu-kernel-modules with the include directories and defines
 * of the driver's own build (src/nvidia/Makefile). Every number printed
 * here comes from sizeof/offsetof in a translation unit that includes the
 * headers the same way src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c does
 * (kernel_gsp.c:60-99), so the layouts are the ones nv-kernel.o is built
 * with -- not re-derived by hand.
 *
 * The field lists below were generated once from clang's
 * -fdump-record-layouts over this same translation unit and then written
 * out explicitly. A field that disappears or is renamed in a later driver
 * version makes this file fail to compile, which is the point: that is a
 * version change somebody has to look at. A field that is ADDED is not
 * caught here; the sizeof line shows it.
 *
 * Nothing in here touches hardware. It is a host program that only prints.
 */

/*
 * Version switch. GSP_ABI selects the field lists: 610 (default) for the
 * 610.57.04 layouts this file was written against, 615 for 615.71.09 and later,
 * where the message queue, the queue element, the GSP arguments and the
 * GSP-FMC boot parameters were redesigned and GSP_SET_SYSTEM_INFO was replaced
 * by GMCAPI_CMD_GSP_INIT (docs/caraxes/blackwell-gb206.md section 3).
 * 595 (any major below 610) drops what 610.57.04 added: the MCTP/NVDM
 * element header and GSP_MSG_QUEUE_ENCRYPTION_TAG (the element then carries
 * authTagBuffer, aadBuffer, elemCount and the RPC header), vbiosRevision,
 * vbiosOemRevision, bSysL2CacheCoherentMode, bBusResetRequired, regionTag,
 * bIsCxlDevice and bindataArgs (docs/caraxes/guest-driver-versions.md).
 * 570 (any major below 580; 570.144 is the firmware Linux 7.2's nova-core
 * speaks) drops in addition what 595 has over r570: bPdiValid/pdi, the
 * extended element-size fields of MESSAGE_QUEUE_INIT_ARGUMENTS,
 * gspMsgQueueGetMaxRpcSize, the later GspSystemInfo fields, the rmctrl fields
 * of rpc_gsp_rm_control_v03_00, rpc_init_done_v17_00.bIsD3Hot,
 * sysmemHeapArgs/rmStateMonitorBufferArgs and the GSP-FMC memory parameters;
 * it adds the r570-only fields (grCapsBits, the fb_* block, the displayless
 * block, the ecidInfo array, bRouteDispIntrsToCPU) and the CPU sequencer
 * structs that 610 removed. Each difference cites the 570.144 header.
 * gsp-abi-sizes.sh sets it from the tree's version.mk or from --abi; the 610
 * output is byte-identical to the file before the switches were added.
 */
#ifndef GSP_ABI
#define GSP_ABI 610
#endif

/* ---- the translation unit of kernel_gsp.c, reduced to what is needed ---- */
#include "nvtypes.h"
#include "gpu/gsp/kernel_gsp.h"   /* KernelGsp, GspStaticConfigInfo, PACKED_REGISTRY_TABLE */
#include "vgpu/rpc.h"             /* rpc_headers.h, rpc_global_enums.h */

#define RPC_STRUCTURES
#define RPC_GENERIC_UNION
#include "g_rpc-structures.h"
#undef RPC_STRUCTURES
#undef RPC_GENERIC_UNION

#define RPC_MESSAGE_STRUCTURES
#define RPC_MESSAGE_GENERIC_UNION
#include "g_rpc-message-header.h"
#undef RPC_MESSAGE_STRUCTURES
#undef RPC_MESSAGE_GENERIC_UNION

#include "gpu/gsp/message_queue_priv.h" /* GSP_MSG_QUEUE_ELEMENT, _checkSum32 */
#if GSP_ABI >= 615
#include "msgq/msgq_priv.h"             /* msgqTxHeader (MSGQ v2: no msgqRxHeader) */
#include "gmcapi/gmcapi_base.h"         /* GMCAPI_HEADER (615: replaces the init RPCs) */
#else
#include "msgq/msgq_priv.h"             /* msgqTxHeader, msgqRxHeader */
#endif
#if GSP_ABI >= 610
#include "mctp_format.h"                /* as message_queue_cpu.c:49 */
#include "nvdm_format.h"                /* as message_queue_cpu.c:50 */
#endif
#include "gpu/gsp/gsp_static_config.h"  /* GspStaticConfigInfo, GspSystemInfo */
#include "gpu/gsp/gsp_init_args.h"      /* GSP_ARGUMENTS_CACHED */
#include "gsp/gsp_fw_wpr_meta.h"        /* GspFwWprMeta */
#include "gsp/gspifpub.h"               /* GSP_FMC_INIT_PARAMS */
#include "libos_init_args.h"            /* LibosMemoryRegionInitArgument */
#include "vgpu/vgpu_version.h"          /* VGX_MAJOR/MINOR_VERSION_NUMBER */
#if GSP_ABI < 595
#include "rmgspseq.h"                   /* GSP_SEQUENCER_BUFFER_CMD (570.144 rmgspseq.h:141-152) */
#endif
#include "nvVer.h"                      /* NV_VERSION_STRING, as kernel_gsp.c:62 */

/* The driver headers are freestanding; libc is only needed for output. */
int printf(const char *, ...);

#define OFF(T, m)  ((unsigned long)__builtin_offsetof(T, m))
#define MSZ(T, m)  ((unsigned long)sizeof(((T *)0)->m))

static void section(const char *title)
{
    printf("\n### %s\n", title);
}

static void type_head(const char *name, unsigned long size, unsigned long align)
{
    printf("\n#### `%s`: sizeof %lu (0x%lx), align %lu\n\n", name, size, size, align);
    printf("| offset | hex | size | field |\n|---:|---:|---:|---|\n");
}

static void row(unsigned long off, unsigned long size, const char *field, const char *note)
{
    printf("| %lu | 0x%04lx | %lu | `%s`%s |\n", off, off, size, field, note);
}

static void row_flex(unsigned long off, const char *field)
{
    printf("| %lu | 0x%04lx | flex | `%s` (flexible array) |\n", off, off, field);
}

static void konst(const char *name, unsigned long long v)
{
    printf("| `%s` | %llu | 0x%llx |\n", name, v, v);
}

#define SECTION(s)   section(s)
#define TYPE(T)      type_head(#T, (unsigned long)sizeof(T), (unsigned long)_Alignof(T))
#define F(T, m)      row(OFF(T, m), MSZ(T, m), #m, "")
#define FX(T, m)     row_flex(OFF(T, m), #m)
#define K(name)      konst(#name, (unsigned long long)(name))

/* kernel_gsp.c:3219-3237 (_kgspGenerateInitArgId) is static in a .c file,
 * so it is replicated here, byte for byte in behaviour. */
static NvU64 init_arg_id(const char *name)
{
    NvU64 id = 0;
    for (unsigned i = 0; i < sizeof(NvU64) && name[i] != '\0'; i++)
        id = (id << 8) | (NvU8)name[i];
    return id;
}

static void constants(void)
{
    /* message_queue_cpu.c:82-91 */
#if GSP_ABI >= 615
    /* 615: message_queue_cpu.c:82-83 */
    const unsigned long hdr      = GSP_MSG_QUEUE_ELEMENT_SIZE_NO_ENCRYPTION;
    const unsigned long hdr_cc   = GSP_MSG_QUEUE_ELEMENT_SIZE_WITH_ENCRYPTION;
#elif GSP_ABI >= 610
    const unsigned long hdr      = OFF(GSP_MSG_QUEUE_ELEMENT, payload);
    const unsigned long hdr_cc   = hdr + sizeof(GSP_MSG_QUEUE_ENCRYPTION_TAG);
#else
    /* before 610: NV_OFFSETOF(GSP_MSG_QUEUE_ELEMENT, rpc), CC tag inline
     * (595.91.07 message_queue_cpu.c:73) */
    const unsigned long hdr      = OFF(GSP_MSG_QUEUE_ELEMENT, rpc);
    const unsigned long hdr_cc   = hdr;
#endif
    const unsigned long elem_min = RM_PAGE_SIZE;
    const unsigned long elem_max = RM_PAGE_SIZE * 16;
#if GSP_ABI >= 595
    /* kernel_gsp.c:3186 -> message_queue_priv.h:112-115 */
    const unsigned long max_rpc  = gspMsgQueueGetMaxRpcSize(elem_max, hdr);
#else
    /* 570.144: no gspMsgQueueGetMaxRpcSize; GSP_MSG_QUEUE_RPC_SIZE_MAX =
     * SIZE_MAX - HDR_SIZE (message_queue_priv.h:90-96) */
    const unsigned long max_rpc  = GSP_MSG_QUEUE_RPC_SIZE_MAX;
#endif
    /* msgq.c:236-238 with hdrAlign = 4 and entryAlign = RM_PAGE_SHIFT
     * (message_queue_cpu.c:90-91, passed at :178-179) */
#if GSP_ABI >= 615
    /* 615: msgq.c msgqTxCreate: entryOff = ALIGN_UP(sizeof(msgqTxHeader), 1 << entryAlign) */
    const unsigned long entry_off = NV_ALIGN_UP(sizeof(msgqTxHeader), 1u << RM_PAGE_SHIFT);
#else
    const unsigned long rx_off    = NV_ALIGN_UP(sizeof(msgqTxHeader), 1u << 4);
    const unsigned long entry_off = NV_ALIGN_UP(rx_off + sizeof(msgqRxHeader), 1u << RM_PAGE_SHIFT);
#endif

    section("Derived constants (computed by the compiler from the same headers)");
    printf("\n| name | decimal | hex |\n|---|---:|---:|\n");
    konst("queueElementHdrSize (no CC)", hdr);
    konst("queueElementHdrSize (CC)", hdr_cc);
    konst("queueElementSizeMin", elem_min);
    konst("queueElementSizeMax", elem_max);
    konst("maxRpcSize = SizeMax - HdrSize", max_rpc);
    konst("max payload after rpc_message_header_v", max_rpc - sizeof(rpc_message_header_v));
    konst("hdr + GspStaticConfigInfo (must be <= maxRpcSize)",
          sizeof(rpc_message_header_v) + sizeof(GspStaticConfigInfo));
    konst("hdr + GspSystemInfo (must be <= maxRpcSize)",
          sizeof(rpc_message_header_v) + sizeof(GspSystemInfo));
#if GSP_ABI < 615
    konst("msgq rxHdrOff (hdrAlign 4)", rx_off);
#endif
    konst("msgq entryOff (entryAlign 12)", entry_off);
    konst("msgq msgCount for a 0x40000 queue", (0x40000 - entry_off) / elem_min);
#if GSP_ABI >= 610
    konst("mctpHeader = mctpCreateTransportHeader(1,1,0,0,0)", mctpCreateTransportHeader(1, 1, 0, 0, 0));
    konst("nvdmHeader = mctpCreateNvdmHeader(NVDM_TYPE_RM_RPC)", mctpCreateNvdmHeader(NVDM_TYPE_RM_RPC));
    K(MCTP_MSG_HEADER_VENDOR_ID_NV);
    K(MCTP_MSG_HEADER_TYPE_VENDOR_PCI);
    K(NVDM_TYPE_RM_RPC);
#endif
#if GSP_ABI >= 615
    K(MSGQ_VERSION_MAJOR);
    K(MSGQ_VERSION_MINOR);
    K(MCTP_MAGIC);
    K(MCTP_MAGIC_SEEN);
    konst("GMCAPI_CMD_GSP_INIT", GMCAPI_CMD_GSP_INIT);
#else
    K(MSGQ_VERSION);
    K(MSGQ_FLAGS_SWAP_RX);
#endif
    K(MSGQ_MSG_SIZE_MIN);
    konst("header_version (rpc_common.c:155-156)",
          DRF_DEF(_VGPU, _MSG_HEADER_VERSION, _MAJOR, _TOT) |
          DRF_DEF(_VGPU, _MSG_HEADER_VERSION, _MINOR, _TOT));
    K(NV_VGPU_MSG_SIGNATURE_VALID);
    K(NV_VGPU_MSG_RESULT_SUCCESS);
    K(NV_VGPU_MSG_RESULT_RPC_PENDING);
    K(NV_VGPU_MSG_UNION_INIT);
    konst("DRF_BASE(NV_VGPU_MSG_RESULT__VMIOP)", DRF_BASE(NV_VGPU_MSG_RESULT__VMIOP));
    K(VGX_MAJOR_VERSION_NUMBER);
    K(VGX_MINOR_VERSION_NUMBER);
    K(NV_VGPU_MSG_FUNCTION_FREE);
    K(NV_VGPU_MSG_FUNCTION_SET_GUEST_SYSTEM_INFO);
    K(NV_VGPU_MSG_FUNCTION_DUP_OBJECT);
    K(NV_VGPU_MSG_FUNCTION_SET_GUEST_SYSTEM_INFO_EXT);
    K(NV_VGPU_MSG_FUNCTION_GET_GSP_STATIC_INFO);
    K(NV_VGPU_MSG_FUNCTION_GSP_SET_SYSTEM_INFO);
    K(NV_VGPU_MSG_FUNCTION_SET_REGISTRY);
    K(NV_VGPU_MSG_FUNCTION_GSP_RM_CONTROL);
    K(NV_VGPU_MSG_FUNCTION_GSP_RM_ALLOC);
    K(NV_VGPU_MSG_EVENT_GSP_INIT_DONE);
    K(NV_VGPU_MSG_EVENT_OS_ERROR_LOG);
    K(NV_VGPU_MSG_EVENT_POST_EVENT);
    K(NV_VGPU_MSG_EVENT_UCODE_LIBOS_PRINT);
    K(NV_VGPU_MSG_EVENT_GSP_LOCKDOWN_NOTICE);
    K(GSP_FW_WPR_META_MAGIC);
    K(GSP_FW_WPR_META_REVISION);
    K(GSP_FW_WPR_META_VERIFIED);
    K(LIBOS_MEMORY_REGION_INIT_ARGUMENTS_MAX);
    K(LIBOS_MEMORY_REGION_CONTIGUOUS);
    K(LIBOS_MEMORY_REGION_LOC_SYSMEM);
    konst("id8(\"LOGINIT\")", init_arg_id("LOGINIT"));
    konst("id8(\"LOGRM\")", init_arg_id("LOGRM"));
    konst("id8(\"RMARGS\")", init_arg_id("RMARGS"));
}

static void libos_log_header(void)
{
    /* There is no C struct for it: the buffer is an NvU64 array. */
    section("LIBOS log buffer header (no struct; NvU64 words)");
    printf("\n| offset | hex | size | field |\n|---:|---:|---:|---|\n");
    printf("| 0 | 0x0000 | %lu | put pointer, in NvU64 entries (liblogdecode.h:156, liblogdecode.c:2543) |\n",
           (unsigned long)sizeof(NvU64));
    printf("| 8 | 0x0008 | %lu | buffer PA, written by the CPU before boot (kernel_gsp.c:3649); also ring slot 0 |\n",
           (unsigned long)sizeof(NvU64));
    printf("| 8 | 0x0008 | ... | ring of `size/8 - 1` NvU64 slots starting at word 1 (liblogdecode.c:1231,1279) |\n");
}

int main(void)
{
    printf("<!-- generated by caraxes/tools/gsp-abi-sizes.sh; driver %s -->\n", NV_VERSION_STRING);
    SECTION("Message queue transport (msgq library, one header per queue)");
    TYPE(msgqTxHeader);
#if GSP_ABI >= 615
    F(msgqTxHeader, versionMajor);
    F(msgqTxHeader, versionMinor);
#else
    F(msgqTxHeader, version);
#endif
    F(msgqTxHeader, size);
    F(msgqTxHeader, msgSize);
    F(msgqTxHeader, msgCount);
#if GSP_ABI < 615
    F(msgqTxHeader, writePtr);
    F(msgqTxHeader, flags);
    F(msgqTxHeader, rxHdrOff);
#endif
    F(msgqTxHeader, entryOff);
#if GSP_ABI >= 615
    F(msgqTxHeader, reserved);
#else

    TYPE(msgqRxHeader);
    F(msgqRxHeader, readPtr);
#endif

    SECTION("Queue element and RPC header");
    TYPE(GSP_MSG_QUEUE_ELEMENT);
#if GSP_ABI >= 615
    F(GSP_MSG_QUEUE_ELEMENT, mctpMagic);
    F(GSP_MSG_QUEUE_ELEMENT, mctpPayloadSize);
#endif
#if GSP_ABI < 610
    /* Before 610: CC tag inline, element count in the element, the RPC header
     * as a member (595.91.07 message_queue_priv.h:43-51). */
    F(GSP_MSG_QUEUE_ELEMENT, authTagBuffer);
    F(GSP_MSG_QUEUE_ELEMENT, aadBuffer);
    F(GSP_MSG_QUEUE_ELEMENT, checkSum);
    F(GSP_MSG_QUEUE_ELEMENT, seqNum);
    F(GSP_MSG_QUEUE_ELEMENT, elemCount);
    F(GSP_MSG_QUEUE_ELEMENT, rpc);
#else
    F(GSP_MSG_QUEUE_ELEMENT, mctpHeader);
    F(GSP_MSG_QUEUE_ELEMENT, nvdmHeader);
#endif
#if GSP_ABI >= 615
    F(GSP_MSG_QUEUE_ELEMENT, noEncryption.nvdmPayloadSize);
    F(GSP_MSG_QUEUE_ELEMENT, noEncryption.reserved);
    FX(GSP_MSG_QUEUE_ELEMENT, noEncryption.payload);
    F(GSP_MSG_QUEUE_ELEMENT, withEncryption.encryptionTag);
    F(GSP_MSG_QUEUE_ELEMENT, withEncryption.nvdmPayloadSize);
    F(GSP_MSG_QUEUE_ELEMENT, withEncryption.reserved);
    FX(GSP_MSG_QUEUE_ELEMENT, withEncryption.payload);
#elif GSP_ABI >= 610
    F(GSP_MSG_QUEUE_ELEMENT, checkSum);
    F(GSP_MSG_QUEUE_ELEMENT, seqNum);
    FX(GSP_MSG_QUEUE_ELEMENT, payload);
#endif
#if GSP_ABI >= 610

    TYPE(GSP_MSG_QUEUE_ENCRYPTION_TAG);
#if GSP_ABI < 615
    F(GSP_MSG_QUEUE_ENCRYPTION_TAG, encryptedSize);
    F(GSP_MSG_QUEUE_ENCRYPTION_TAG, reserved);
#endif
    F(GSP_MSG_QUEUE_ENCRYPTION_TAG, authTagBuffer);
#if GSP_ABI >= 615

    TYPE(GMCAPI_HEADER);
    F(GMCAPI_HEADER, command);
    F(GMCAPI_HEADER, size);
    F(GMCAPI_HEADER, sequence);
    F(GMCAPI_HEADER, request.max_response_size);
    F(GMCAPI_HEADER, response.status);
    F(GMCAPI_HEADER, reserved);
    FX(GMCAPI_HEADER, data);
#else
    F(GSP_MSG_QUEUE_ENCRYPTION_TAG, aadBuffer);
#endif
#endif /* GSP_ABI >= 610 */

    TYPE(rpc_message_header_v);
    F(rpc_message_header_v, header_version);
    F(rpc_message_header_v, signature);
    F(rpc_message_header_v, length);
    F(rpc_message_header_v, function);
    F(rpc_message_header_v, rpc_result);
    F(rpc_message_header_v, rpc_result_private);
    F(rpc_message_header_v, sequence);
    F(rpc_message_header_v, u); /* struct */
    F(rpc_message_header_v, u.spare);
    F(rpc_message_header_v, u.cpuRmGfid);
    FX(rpc_message_header_v, rpc_message_data);

    SECTION("Init RPCs: CPU to GSP and replies");
    TYPE(rpc_set_guest_system_info_v03_00);
    F(rpc_set_guest_system_info_v03_00, vgxVersionMajorNum);
    F(rpc_set_guest_system_info_v03_00, vgxVersionMinorNum);
    F(rpc_set_guest_system_info_v03_00, guestDriverVersionBufferLength);
    F(rpc_set_guest_system_info_v03_00, guestVersionBufferLength);
    F(rpc_set_guest_system_info_v03_00, guestTitleBufferLength);
    F(rpc_set_guest_system_info_v03_00, guestClNum);
    F(rpc_set_guest_system_info_v03_00, guestDriverVersion);
    F(rpc_set_guest_system_info_v03_00, guestVersion);
    F(rpc_set_guest_system_info_v03_00, guestTitle);

    TYPE(rpc_set_guest_system_info_ext_v25_1B);
    F(rpc_set_guest_system_info_ext_v25_1B, guestDriverBranch);
    F(rpc_set_guest_system_info_ext_v25_1B, domain);
    F(rpc_set_guest_system_info_ext_v25_1B, bus);
    F(rpc_set_guest_system_info_ext_v25_1B, device);
    F(rpc_set_guest_system_info_ext_v25_1B, gridBuildCsp);

#if GSP_ABI >= 615
    /* 615: rpc_gsp_set_system_info_v17_00 is gone (GSP_SET_SYSTEM_INFO deprecated,
     * rpc_global_enums.h:82); system info travels NVKV-encoded in GMCAPI_CMD_GSP_INIT. */
#else
    TYPE(rpc_gsp_set_system_info_v17_00);
    F(rpc_gsp_set_system_info_v17_00, data);
#endif

    TYPE(PACKED_REGISTRY_TABLE);
    F(PACKED_REGISTRY_TABLE, size);
    F(PACKED_REGISTRY_TABLE, numEntries);
    F(PACKED_REGISTRY_TABLE, entries);

    TYPE(PACKED_REGISTRY_ENTRY);
    F(PACKED_REGISTRY_ENTRY, nameOffset);
    F(PACKED_REGISTRY_ENTRY, type);
    F(PACKED_REGISTRY_ENTRY, data);
    F(PACKED_REGISTRY_ENTRY, length);

    TYPE(rpc_get_gsp_static_info_v14_00);
    F(rpc_get_gsp_static_info_v14_00, data);

    TYPE(rpc_init_done_v17_00);
#if GSP_ABI >= 595
    F(rpc_init_done_v17_00, bIsD3Hot);
#else
    /* 570.144: NvU32 not_used (generated/g_rpc-structures.h:1544-1547) */
    F(rpc_init_done_v17_00, not_used);
#endif

    SECTION("RM API passthrough RPCs");
    TYPE(rpc_gsp_rm_alloc_v03_00);
    F(rpc_gsp_rm_alloc_v03_00, hClient);
    F(rpc_gsp_rm_alloc_v03_00, hParent);
    F(rpc_gsp_rm_alloc_v03_00, hObject);
    F(rpc_gsp_rm_alloc_v03_00, hClass);
    F(rpc_gsp_rm_alloc_v03_00, status);
    F(rpc_gsp_rm_alloc_v03_00, paramsSize);
    F(rpc_gsp_rm_alloc_v03_00, flags);
    F(rpc_gsp_rm_alloc_v03_00, reserved);
    FX(rpc_gsp_rm_alloc_v03_00, params);

    TYPE(rpc_gsp_rm_control_v03_00);
    F(rpc_gsp_rm_control_v03_00, hClient);
    F(rpc_gsp_rm_control_v03_00, hObject);
    F(rpc_gsp_rm_control_v03_00, cmd);
    F(rpc_gsp_rm_control_v03_00, status);
    F(rpc_gsp_rm_control_v03_00, paramsSize);
#if GSP_ABI >= 595
    F(rpc_gsp_rm_control_v03_00, rmapiRpcFlags);
    F(rpc_gsp_rm_control_v03_00, rmctrlFlags);
    F(rpc_gsp_rm_control_v03_00, rmctrlAccessRight);
    F(rpc_gsp_rm_control_v03_00, reserved0);
#else
    /* 570.144: one NvU32 flags, params at 24 (generated/g_rpc-structures.h:1388-1397) */
    F(rpc_gsp_rm_control_v03_00, flags);
#endif
    FX(rpc_gsp_rm_control_v03_00, params);

    TYPE(rpc_free_v03_00);
    F(rpc_free_v03_00, params); /* struct */
    F(rpc_free_v03_00, params.hRoot);
    F(rpc_free_v03_00, params.hObjectParent);
    F(rpc_free_v03_00, params.hObjectOld);
    F(rpc_free_v03_00, params.status);

    TYPE(rpc_dup_object_v03_00);
    F(rpc_dup_object_v03_00, params); /* struct */
    F(rpc_dup_object_v03_00, params.hClient);
    F(rpc_dup_object_v03_00, params.hParent);
    F(rpc_dup_object_v03_00, params.hObject);
    F(rpc_dup_object_v03_00, params.hClientSrc);
    F(rpc_dup_object_v03_00, params.hObjectSrc);
    F(rpc_dup_object_v03_00, params.flags);
    F(rpc_dup_object_v03_00, params.status);

    SECTION("Events GSP to CPU that the boot path accepts");
    TYPE(rpc_os_error_log_v17_00);
    F(rpc_os_error_log_v17_00, exceptType);
    F(rpc_os_error_log_v17_00, preemptiveRemovalPreviousXid);
    F(rpc_os_error_log_v17_00, runlistId);
    F(rpc_os_error_log_v17_00, chid);
    F(rpc_os_error_log_v17_00, errString);

    TYPE(rpc_post_event_v17_00);
    F(rpc_post_event_v17_00, hClient);
    F(rpc_post_event_v17_00, hEvent);
    F(rpc_post_event_v17_00, notifyIndex);
    F(rpc_post_event_v17_00, data);
    F(rpc_post_event_v17_00, info16);
    F(rpc_post_event_v17_00, status);
    F(rpc_post_event_v17_00, eventDataSize);
    F(rpc_post_event_v17_00, bNotifyList);
    FX(rpc_post_event_v17_00, eventData);

    TYPE(rpc_ucode_libos_print_v1E_08);
    F(rpc_ucode_libos_print_v1E_08, ucodeEngDesc);
    F(rpc_ucode_libos_print_v1E_08, libosPrintBufSize);
    FX(rpc_ucode_libos_print_v1E_08, libosPrintBuf);

    TYPE(rpc_gsp_lockdown_notice_v17_00);
    F(rpc_gsp_lockdown_notice_v17_00, bLockdownEngaging);
#if GSP_ABI < 595
    /* 570.144: GSP_RUN_CPU_SEQUENCER, removed in 610 (generated/g_rpc-structures.h:1414-1420) */
    TYPE(rpc_run_cpu_sequencer_v17_00);
    F(rpc_run_cpu_sequencer_v17_00, bufferSizeDWord);
    F(rpc_run_cpu_sequencer_v17_00, cmdIndex);
    F(rpc_run_cpu_sequencer_v17_00, regSaveArea);
    FX(rpc_run_cpu_sequencer_v17_00, commandBuffer);

    /* 570.144 arch/nvalloc/common/inc/rmgspseq.h:78-152 */
    TYPE(GSP_SEQUENCER_BUFFER_CMD);
    F(GSP_SEQUENCER_BUFFER_CMD, opCode);
    F(GSP_SEQUENCER_BUFFER_CMD, payload);
    F(GSP_SEQUENCER_BUFFER_CMD, payload.regWrite);
    F(GSP_SEQUENCER_BUFFER_CMD, payload.regModify);
    F(GSP_SEQUENCER_BUFFER_CMD, payload.regPoll);
    F(GSP_SEQUENCER_BUFFER_CMD, payload.delayUs);
    F(GSP_SEQUENCER_BUFFER_CMD, payload.regStore);

    TYPE(GSP_SEQ_BUF_PAYLOAD_REG_POLL);
    F(GSP_SEQ_BUF_PAYLOAD_REG_POLL, addr);
    F(GSP_SEQ_BUF_PAYLOAD_REG_POLL, mask);
    F(GSP_SEQ_BUF_PAYLOAD_REG_POLL, val);
    F(GSP_SEQ_BUF_PAYLOAD_REG_POLL, timeout);
    F(GSP_SEQ_BUF_PAYLOAD_REG_POLL, error);
#endif

    SECTION("GET_GSP_STATIC_INFO reply payload");
    TYPE(GspStaticConfigInfo);
#if GSP_ABI < 595
    /* 570.144: grCapsBits first (gpu/gsp/gsp_static_config.h:80) */
    F(GspStaticConfigInfo, grCapsBits);
#endif
    F(GspStaticConfigInfo, gidInfo); /* struct */
    F(GspStaticConfigInfo, gidInfo.index);
    F(GspStaticConfigInfo, gidInfo.flags);
    F(GspStaticConfigInfo, gidInfo.length);
    F(GspStaticConfigInfo, gidInfo.data);
    F(GspStaticConfigInfo, SKUInfo); /* struct */
    F(GspStaticConfigInfo, SKUInfo.BoardID);
    F(GspStaticConfigInfo, SKUInfo.chipSKU);
    F(GspStaticConfigInfo, SKUInfo.chipSKUMod);
    F(GspStaticConfigInfo, SKUInfo.skuConfigVersion);
    F(GspStaticConfigInfo, SKUInfo.project);
    F(GspStaticConfigInfo, SKUInfo.projectSKU);
    F(GspStaticConfigInfo, SKUInfo.CDP);
    F(GspStaticConfigInfo, SKUInfo.projectSKUMod);
    F(GspStaticConfigInfo, SKUInfo.businessCycle);
    F(GspStaticConfigInfo, fbRegionInfoParams); /* struct */
    F(GspStaticConfigInfo, fbRegionInfoParams.numFBRegions);
    F(GspStaticConfigInfo, fbRegionInfoParams.fbRegion);
#if GSP_ABI >= 595
    F(GspStaticConfigInfo, bPdiValid);
    F(GspStaticConfigInfo, pdi);
#endif
    /* 570.144: no bPdiValid/pdi, sriovCaps follows fbRegionInfoParams
     * (gpu/gsp/gsp_static_config.h:83-85) */
    F(GspStaticConfigInfo, sriovCaps); /* struct */
    F(GspStaticConfigInfo, sriovCaps.totalVFs);
    F(GspStaticConfigInfo, sriovCaps.firstVfOffset);
    F(GspStaticConfigInfo, sriovCaps.vfFeatureMask);
    F(GspStaticConfigInfo, sriovCaps.FirstVFBar0Address);
    F(GspStaticConfigInfo, sriovCaps.FirstVFBar1Address);
    F(GspStaticConfigInfo, sriovCaps.FirstVFBar2Address);
    F(GspStaticConfigInfo, sriovCaps.bar0Size);
    F(GspStaticConfigInfo, sriovCaps.bar1Size);
    F(GspStaticConfigInfo, sriovCaps.bar2Size);
    F(GspStaticConfigInfo, sriovCaps.b64bitBar0);
    F(GspStaticConfigInfo, sriovCaps.b64bitBar1);
    F(GspStaticConfigInfo, sriovCaps.b64bitBar2);
    F(GspStaticConfigInfo, sriovCaps.bSriovEnabled);
    F(GspStaticConfigInfo, sriovCaps.bSriovHeavyEnabled);
    F(GspStaticConfigInfo, sriovCaps.bEmulateVFBar0TlbInvalidationRegister);
    F(GspStaticConfigInfo, sriovCaps.bClientRmAllocatedCtxBuffer);
    F(GspStaticConfigInfo, sriovCaps.bNonPowerOf2ChannelCountSupported);
    F(GspStaticConfigInfo, sriovCaps.bVfResizableBAR1Supported);
    F(GspStaticConfigInfo, sriovMaxGfid);
    F(GspStaticConfigInfo, engineCaps);
    F(GspStaticConfigInfo, poisonFuseEnabled);
    F(GspStaticConfigInfo, fb_length);
#if GSP_ABI < 595
    /* 570.144 gpu/gsp/gsp_static_config.h:93-97 */
    F(GspStaticConfigInfo, fbio_mask);
    F(GspStaticConfigInfo, fb_bus_width);
    F(GspStaticConfigInfo, fb_ram_type);
    F(GspStaticConfigInfo, fbp_mask);
    F(GspStaticConfigInfo, l2_cache_size);
#endif
    F(GspStaticConfigInfo, gpuNameString);
    F(GspStaticConfigInfo, gpuShortNameString);
#if GSP_ABI < 595
    /* 570.144 gpu/gsp/gsp_static_config.h:101 */
    F(GspStaticConfigInfo, gpuNameString_Unicode);
#endif
    F(GspStaticConfigInfo, bGpuInternalSku);
    F(GspStaticConfigInfo, bIsQuadroGeneric);
    F(GspStaticConfigInfo, bIsQuadroAd);
    F(GspStaticConfigInfo, bIsNvidiaNvs);
    F(GspStaticConfigInfo, bIsVgx);
    F(GspStaticConfigInfo, bGeforceSmb);
    F(GspStaticConfigInfo, bIsTitan);
    F(GspStaticConfigInfo, bIsTesla);
    F(GspStaticConfigInfo, bIsMobile);
    F(GspStaticConfigInfo, bIsGc6Rtd3Allowed);
    F(GspStaticConfigInfo, bIsGc8Rtd3Allowed);
    F(GspStaticConfigInfo, bIsGcOffRtd3Allowed);
    F(GspStaticConfigInfo, bIsGcoffLegacyAllowed);
    F(GspStaticConfigInfo, bIsMigSupported);
    F(GspStaticConfigInfo, RTD3GC6TotalBoardPower);
    F(GspStaticConfigInfo, RTD3GC6PerstDelay);
    F(GspStaticConfigInfo, bar1PdeBase);
    F(GspStaticConfigInfo, bar2PdeBase);
    F(GspStaticConfigInfo, bVbiosValid);
    F(GspStaticConfigInfo, vbiosSubVendor);
    F(GspStaticConfigInfo, vbiosSubDevice);
#if GSP_ABI >= 610
    F(GspStaticConfigInfo, vbiosRevision);
    F(GspStaticConfigInfo, vbiosOemRevision);
#endif
    F(GspStaticConfigInfo, bPageRetirementSupported);
    F(GspStaticConfigInfo, bSplitVasBetweenServerClientRm);
    F(GspStaticConfigInfo, bClRootportNeedsNosnoopWAR);
#if GSP_ABI < 595
    /* 570.144 gpu/gsp/gsp_static_config.h:145-147 */
    F(GspStaticConfigInfo, displaylessMaxHeads);
    F(GspStaticConfigInfo, displaylessMaxResolution);
    F(GspStaticConfigInfo, displaylessMaxPixels);
#endif
    F(GspStaticConfigInfo, hInternalClient);
    F(GspStaticConfigInfo, hInternalDevice);
    F(GspStaticConfigInfo, hInternalSubdevice);
    F(GspStaticConfigInfo, bSelfHostedMode);
    F(GspStaticConfigInfo, bAtsSupported);
#if GSP_ABI >= 610
    F(GspStaticConfigInfo, bSysL2CacheCoherentMode);
#endif
    F(GspStaticConfigInfo, bIsGpuUefi);
    F(GspStaticConfigInfo, bIsEfiInit);
    F(GspStaticConfigInfo, ecidInfo); /* struct */
#if GSP_ABI >= 615
    F(GspStaticConfigInfo, ecidInfo.info);
#elif GSP_ABI < 595
    /* 570.144: EcidManufacturingInfo ecidInfo[MAX_GROUP_COUNT]
     * (gpu/gsp/gsp_static_config.h:64-69, :164) */
    F(GspStaticConfigInfo, ecidInfo[0].ecidLow);
    F(GspStaticConfigInfo, ecidInfo[0].ecidHigh);
    F(GspStaticConfigInfo, ecidInfo[0].ecidExtended);
#else
    F(GspStaticConfigInfo, ecidInfo.ecidLow);
    F(GspStaticConfigInfo, ecidInfo.ecidHigh);
    F(GspStaticConfigInfo, ecidInfo.ecidExtended);
#endif
    F(GspStaticConfigInfo, fwWprLayoutOffset); /* struct */
    F(GspStaticConfigInfo, fwWprLayoutOffset.nonWprHeapOffset);
    F(GspStaticConfigInfo, fwWprLayoutOffset.frtsOffset);
#if GSP_ABI >= 610
    F(GspStaticConfigInfo, bBusResetRequired);
#endif

    TYPE(NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO);
    F(NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO, base);
    F(NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO, limit);
    F(NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO, reserved);
    F(NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO, performance);
    F(NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO, supportCompressed);
    F(NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO, supportISO);
    F(NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO, bProtected);
    F(NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO, blackList);
#if GSP_ABI >= 610
    F(NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO, regionTag);
#endif

    SECTION("GSP_SET_SYSTEM_INFO payload");
    TYPE(GspSystemInfo);
    F(GspSystemInfo, gpuPhysAddr);
    F(GspSystemInfo, gpuPhysFbAddr);
    F(GspSystemInfo, gpuPhysInstAddr);
    F(GspSystemInfo, gpuPhysIoAddr);
    F(GspSystemInfo, nvDomainBusDeviceFunc);
    F(GspSystemInfo, simAccessBufPhysAddr);
    F(GspSystemInfo, notifyOpSharedSurfacePhysAddr);
    F(GspSystemInfo, pcieAtomicsOpMask);
    F(GspSystemInfo, consoleMemSize);
    F(GspSystemInfo, maxUserVa);
    F(GspSystemInfo, pciConfigMirrorBase);
    F(GspSystemInfo, pciConfigMirrorSize);
    F(GspSystemInfo, PCIDeviceID);
    F(GspSystemInfo, PCISubDeviceID);
    F(GspSystemInfo, PCIRevisionID);
    F(GspSystemInfo, pcieAtomicsCplDeviceCapMask);
    F(GspSystemInfo, oorArch);
    F(GspSystemInfo, clPdbProperties);
    F(GspSystemInfo, Chipset);
    F(GspSystemInfo, bGpuBehindBridge);
    F(GspSystemInfo, bFlrSupported);
    F(GspSystemInfo, b64bBar0Supported);
    F(GspSystemInfo, bMnocAvailable);
    F(GspSystemInfo, chipsetL1ssEnable);
    F(GspSystemInfo, bUpstreamL0sUnsupported);
    F(GspSystemInfo, bUpstreamL1Unsupported);
    F(GspSystemInfo, bUpstreamL1PorSupported);
    F(GspSystemInfo, bUpstreamL1PorMobileOnly);
    F(GspSystemInfo, bSystemHasMux);
    F(GspSystemInfo, upstreamAddressValid);
    F(GspSystemInfo, FHBBusInfo); /* struct */
    F(GspSystemInfo, FHBBusInfo.deviceID);
    F(GspSystemInfo, FHBBusInfo.vendorID);
    F(GspSystemInfo, FHBBusInfo.subdeviceID);
    F(GspSystemInfo, FHBBusInfo.subvendorID);
    F(GspSystemInfo, FHBBusInfo.revisionID);
    F(GspSystemInfo, chipsetIDInfo); /* struct */
    F(GspSystemInfo, chipsetIDInfo.deviceID);
    F(GspSystemInfo, chipsetIDInfo.vendorID);
    F(GspSystemInfo, chipsetIDInfo.subdeviceID);
    F(GspSystemInfo, chipsetIDInfo.subvendorID);
    F(GspSystemInfo, chipsetIDInfo.revisionID);
    F(GspSystemInfo, acpiMethodData); /* struct */
    F(GspSystemInfo, acpiMethodData.bValid);
    F(GspSystemInfo, acpiMethodData.dodMethodData); /* struct */
    F(GspSystemInfo, acpiMethodData.dodMethodData.status);
    F(GspSystemInfo, acpiMethodData.dodMethodData.acpiIdListLen);
    F(GspSystemInfo, acpiMethodData.dodMethodData.acpiIdList);
    F(GspSystemInfo, acpiMethodData.jtMethodData); /* struct */
    F(GspSystemInfo, acpiMethodData.jtMethodData.status);
    F(GspSystemInfo, acpiMethodData.jtMethodData.jtCaps);
    F(GspSystemInfo, acpiMethodData.jtMethodData.jtRevId);
    F(GspSystemInfo, acpiMethodData.jtMethodData.bSBIOSCaps);
    F(GspSystemInfo, acpiMethodData.muxMethodData); /* struct */
    F(GspSystemInfo, acpiMethodData.muxMethodData.tableLen);
    F(GspSystemInfo, acpiMethodData.muxMethodData.acpiIdMuxModeTable);
    F(GspSystemInfo, acpiMethodData.muxMethodData.acpiIdMuxPartTable);
    F(GspSystemInfo, acpiMethodData.muxMethodData.acpiIdMuxStateTable);
    F(GspSystemInfo, acpiMethodData.capsMethodData); /* struct */
    F(GspSystemInfo, acpiMethodData.capsMethodData.status);
    F(GspSystemInfo, acpiMethodData.capsMethodData.optimusCaps);
    F(GspSystemInfo, hypervisorType);
#if GSP_ABI >= 595
    F(GspSystemInfo, virtualConfigBits);
#endif
    /* 570.144: no virtualConfigBits, bIsUnixHdmiFrlComplianceEnabled,
     * bIsCmcBasedHws .. pf0DeviceControl2Reg; bRouteDispIntrsToCPU before
     * hostPageSize (gpu/gsp/gsp_static_config.h:170-220) */
    F(GspSystemInfo, bIsPassthru);
    F(GspSystemInfo, sysTimerOffsetNs);
    F(GspSystemInfo, gspVFInfo); /* struct */
    F(GspSystemInfo, gspVFInfo.totalVFs);
    F(GspSystemInfo, gspVFInfo.firstVFOffset);
    F(GspSystemInfo, gspVFInfo.FirstVFBar0Address);
    F(GspSystemInfo, gspVFInfo.FirstVFBar1Address);
    F(GspSystemInfo, gspVFInfo.FirstVFBar2Address);
    F(GspSystemInfo, gspVFInfo.b64bitBar0);
    F(GspSystemInfo, gspVFInfo.b64bitBar1);
    F(GspSystemInfo, gspVFInfo.b64bitBar2);
    F(GspSystemInfo, bIsPrimary);
#if GSP_ABI >= 595
    F(GspSystemInfo, bIsUnixHdmiFrlComplianceEnabled);
#endif
    F(GspSystemInfo, isGridBuild);
    F(GspSystemInfo, pcieConfigReg); /* struct */
    F(GspSystemInfo, pcieConfigReg.linkCap);
    F(GspSystemInfo, gridBuildCsp);
    F(GspSystemInfo, bPreserveVideoMemoryAllocations);
    F(GspSystemInfo, bTdrEventSupported);
    F(GspSystemInfo, bFeatureStretchVblankCapable);
    F(GspSystemInfo, bEnableDynamicGranularityPageArrays);
    F(GspSystemInfo, bClockBoostSupported);
#if GSP_ABI < 595
    F(GspSystemInfo, bRouteDispIntrsToCPU);
#endif
    F(GspSystemInfo, hostPageSize);
#if GSP_ABI >= 595
    F(GspSystemInfo, bIsCmcBasedHws);
    F(GspSystemInfo, bGspNocatEnabled);
    F(GspSystemInfo, bS0ixSupport);
    F(GspSystemInfo, bWindowChannelAlwaysMapped);
    F(GspSystemInfo, pciePowerControlValue);
    F(GspSystemInfo, bPciePowerControlPresent);
    F(GspSystemInfo, pf0DeviceControl2Reg);
#endif
#if GSP_ABI >= 610
    F(GspSystemInfo, bIsCxlDevice);
#endif

    TYPE(MUX_METHOD_DATA_ELEMENT);
    F(MUX_METHOD_DATA_ELEMENT, acpiId);
    F(MUX_METHOD_DATA_ELEMENT, mode);
    F(MUX_METHOD_DATA_ELEMENT, status);

    SECTION("Boot arguments (RMARGS region, libos init args, WPR meta)");
    TYPE(GSP_ARGUMENTS_CACHED);
#if GSP_ABI >= 615
    F(GSP_ARGUMENTS_CACHED, magic);
    F(GSP_ARGUMENTS_CACHED, size);
    F(GSP_ARGUMENTS_CACHED, flags);
#endif
    F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments); /* struct */
#if GSP_ABI >= 615
    F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.flags);
#endif
    F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.sharedMemPhysAddr);
    F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.pageTableEntryCount);
    F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.cmdQueueOffset);
    F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.statQueueOffset);
#if GSP_ABI >= 595
    F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.queueElementHdrSize);
    F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.queueElementSizeMin);
    F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.queueElementSizeMax);
    F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.queueHeaderAlign);
    F(GSP_ARGUMENTS_CACHED, messageQueueInitArguments.queueElementAlign);
#endif
    F(GSP_ARGUMENTS_CACHED, srInitArguments); /* struct */
    F(GSP_ARGUMENTS_CACHED, srInitArguments.oldLevel);
    F(GSP_ARGUMENTS_CACHED, srInitArguments.flags);
#if GSP_ABI >= 615
    F(GSP_ARGUMENTS_CACHED, srInitArguments.reserved);
    F(GSP_ARGUMENTS_CACHED, reserved2);
#else
    F(GSP_ARGUMENTS_CACHED, srInitArguments.bInPMTransition);
#endif
    F(GSP_ARGUMENTS_CACHED, gpuInstance);
#if GSP_ABI < 615
    F(GSP_ARGUMENTS_CACHED, bDmemStack);
#endif
    F(GSP_ARGUMENTS_CACHED, profilerArgs); /* struct */
    F(GSP_ARGUMENTS_CACHED, profilerArgs.pa);
    F(GSP_ARGUMENTS_CACHED, profilerArgs.size);
#if GSP_ABI >= 595
    F(GSP_ARGUMENTS_CACHED, sysmemHeapArgs); /* struct */
    F(GSP_ARGUMENTS_CACHED, sysmemHeapArgs.pa);
    F(GSP_ARGUMENTS_CACHED, sysmemHeapArgs.size);
    F(GSP_ARGUMENTS_CACHED, rmStateMonitorBufferArgs); /* struct */
    F(GSP_ARGUMENTS_CACHED, rmStateMonitorBufferArgs.pa);
    F(GSP_ARGUMENTS_CACHED, rmStateMonitorBufferArgs.size);
#else
    /* 570.144: ends after profilerArgs (gpu/gsp/gsp_init_args.h:45-57) */
#endif
#if GSP_ABI >= 610
    F(GSP_ARGUMENTS_CACHED, bindataArgs); /* struct */
    F(GSP_ARGUMENTS_CACHED, bindataArgs.radix3);
    F(GSP_ARGUMENTS_CACHED, bindataArgs.size);
#endif
#if GSP_ABI >= 615
    F(GSP_ARGUMENTS_CACHED, vbiosOverrideArgs.pa);
    F(GSP_ARGUMENTS_CACHED, vbiosOverrideArgs.size);
#endif

    TYPE(MESSAGE_QUEUE_INIT_ARGUMENTS);
#if GSP_ABI >= 615
    F(MESSAGE_QUEUE_INIT_ARGUMENTS, flags);
#endif
    F(MESSAGE_QUEUE_INIT_ARGUMENTS, sharedMemPhysAddr);
    F(MESSAGE_QUEUE_INIT_ARGUMENTS, pageTableEntryCount);
    F(MESSAGE_QUEUE_INIT_ARGUMENTS, cmdQueueOffset);
    F(MESSAGE_QUEUE_INIT_ARGUMENTS, statQueueOffset);
#if GSP_ABI >= 595
    F(MESSAGE_QUEUE_INIT_ARGUMENTS, queueElementHdrSize);
    F(MESSAGE_QUEUE_INIT_ARGUMENTS, queueElementSizeMin);
    F(MESSAGE_QUEUE_INIT_ARGUMENTS, queueElementSizeMax);
    F(MESSAGE_QUEUE_INIT_ARGUMENTS, queueHeaderAlign);
    F(MESSAGE_QUEUE_INIT_ARGUMENTS, queueElementAlign);
#else
    /* 570.144: the four fields above only (gpu/gsp/gsp_init_args.h:29-34) */
#endif
#if GSP_ABI >= 615
    F(MESSAGE_QUEUE_INIT_ARGUMENTS, reserved);
#endif

    TYPE(GSP_SR_INIT_ARGUMENTS);
    F(GSP_SR_INIT_ARGUMENTS, oldLevel);
    F(GSP_SR_INIT_ARGUMENTS, flags);
#if GSP_ABI >= 615
    F(GSP_SR_INIT_ARGUMENTS, reserved);
#else
    F(GSP_SR_INIT_ARGUMENTS, bInPMTransition);
#endif

    TYPE(LibosMemoryRegionInitArgument);
    F(LibosMemoryRegionInitArgument, id8);
    F(LibosMemoryRegionInitArgument, pa);
    F(LibosMemoryRegionInitArgument, size);
    F(LibosMemoryRegionInitArgument, kind);
    F(LibosMemoryRegionInitArgument, loc);

    TYPE(GspFwWprMeta);
    F(GspFwWprMeta, magic);
    F(GspFwWprMeta, revision);
    F(GspFwWprMeta, sysmemAddrOfRadix3Elf);
    F(GspFwWprMeta, sizeOfRadix3Elf);
    F(GspFwWprMeta, sysmemAddrOfBootloader);
    F(GspFwWprMeta, sizeOfBootloader);
    F(GspFwWprMeta, bootloaderCodeOffset);
    F(GspFwWprMeta, bootloaderDataOffset);
    F(GspFwWprMeta, bootloaderManifestOffset);
    F(GspFwWprMeta, sysmemAddrOfSignature);
    F(GspFwWprMeta, sizeOfSignature);
    F(GspFwWprMeta, gspFwHeapFreeListWprOffset);
    F(GspFwWprMeta, unused0);
    F(GspFwWprMeta, unused1);
    F(GspFwWprMeta, gspFwRsvdStart);
    F(GspFwWprMeta, nonWprHeapOffset);
    F(GspFwWprMeta, nonWprHeapSize);
    F(GspFwWprMeta, gspFwWprStart);
    F(GspFwWprMeta, gspFwHeapOffset);
    F(GspFwWprMeta, gspFwHeapSize);
    F(GspFwWprMeta, gspFwOffset);
    F(GspFwWprMeta, bootBinOffset);
    F(GspFwWprMeta, frtsOffset);
    F(GspFwWprMeta, frtsSize);
    F(GspFwWprMeta, gspFwWprEnd);
    F(GspFwWprMeta, fbSize);
    F(GspFwWprMeta, vgaWorkspaceOffset);
    F(GspFwWprMeta, vgaWorkspaceSize);
    F(GspFwWprMeta, bootCount);
    F(GspFwWprMeta, partitionRpcAddr);
    F(GspFwWprMeta, partitionRpcRequestOffset);
    F(GspFwWprMeta, partitionRpcReplyOffset);
    F(GspFwWprMeta, elfCodeOffset);
    F(GspFwWprMeta, elfDataOffset);
    F(GspFwWprMeta, elfCodeSize);
    F(GspFwWprMeta, elfDataSize);
    F(GspFwWprMeta, lsUcodeVersion);
    F(GspFwWprMeta, partitionRpcPadding);
    F(GspFwWprMeta, sysmemAddrOfCrashReportQueue);
    F(GspFwWprMeta, sizeOfCrashReportQueue);
    F(GspFwWprMeta, lsUcodeVersionPadding);
    F(GspFwWprMeta, gspFwHeapVfPartitionCount);
    F(GspFwWprMeta, flags);
#if GSP_ABI >= 615
    F(GspFwWprMeta, pagingConfig);
#else
    F(GspFwWprMeta, padding);
#endif
    F(GspFwWprMeta, pmuReservedSize);
    F(GspFwWprMeta, verified);

    SECTION("GSP-FMC boot parameters (Hopper and later only, not used on Turing)");
    TYPE(GSP_FMC_INIT_PARAMS);
    F(GSP_FMC_INIT_PARAMS, regkeys);

    TYPE(GSP_FMC_BOOT_PARAMS);
    F(GSP_FMC_BOOT_PARAMS, initParams); /* struct */
    F(GSP_FMC_BOOT_PARAMS, initParams.regkeys);
    F(GSP_FMC_BOOT_PARAMS, bootGspRmParams); /* struct */
    F(GSP_FMC_BOOT_PARAMS, bootGspRmParams.target);
    F(GSP_FMC_BOOT_PARAMS, bootGspRmParams.gspRmDescSize);
    F(GSP_FMC_BOOT_PARAMS, bootGspRmParams.gspRmDescOffset);
    F(GSP_FMC_BOOT_PARAMS, bootGspRmParams.wprCarveoutOffset);
    F(GSP_FMC_BOOT_PARAMS, bootGspRmParams.wprCarveoutSize);
    F(GSP_FMC_BOOT_PARAMS, bootGspRmParams.bIsGspRmBoot);
#if GSP_ABI >= 595
    F(GSP_FMC_BOOT_PARAMS, bootGspRmParams.bInstInSysMode);
#endif
    F(GSP_FMC_BOOT_PARAMS, gspRmParams); /* struct */
    F(GSP_FMC_BOOT_PARAMS, gspRmParams.target);
    F(GSP_FMC_BOOT_PARAMS, gspRmParams.bootArgsOffset);
    F(GSP_FMC_BOOT_PARAMS, gspSpdmParams); /* struct */
    F(GSP_FMC_BOOT_PARAMS, gspSpdmParams.target);
    F(GSP_FMC_BOOT_PARAMS, gspSpdmParams.payloadBufferOffset);
    F(GSP_FMC_BOOT_PARAMS, gspSpdmParams.payloadBufferSize);
#if GSP_ABI >= 595
    F(GSP_FMC_BOOT_PARAMS, gspRmMemParams); /* struct */
    F(GSP_FMC_BOOT_PARAMS, gspRmMemParams.flushSysmemAddrValLo);
    F(GSP_FMC_BOOT_PARAMS, gspRmMemParams.flushSysmemAddrValHi);
#else
    /* 570.144: no bInstInSysMode, no gspRmMemParams (gsp/gspifpub.h:59-113) */
#endif
    libos_log_header();
    constants();
    return 0;
}
