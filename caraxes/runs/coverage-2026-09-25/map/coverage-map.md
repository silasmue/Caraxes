## (a) Per workload

Cost: trapped accesses by guest-seen time, the rest by device time (profile-rank.py). "New controls": sent in this workload and in neither gate's window.

| workload | rows | cost ms | controls (ids) | new controls | RPCs | trapped accesses | host RM calls | top rows by cost |
|---|---|---|---|---|---|---|---|---|
| boot | 3616 | 13264 | 44 | 39 | 96 | 69750 | 309 | host RM_ALLOC_MEMORY 18x 10000.6 ms; host RM_FREE 14x 485.3 ms; access bar0:W:0x840000:0x840184 (bar0-other) 8512x 320.7 ms; access bar0:W:0x840000:0x8401c4 (bar0-other) 6272x 219.8 ms |
| setup | 61 | 46 | 0 | 0 | 0 | 1026 | 0 | access cfg:R:0x0:0x4 (pci-config) 116x 10.7 ms; access cfg:R:0x0:0x78 (pci-config) 230x 10.2 ms; access cfg:R:0x0:0x60 (pci-config) 230x 9.5 ms; access cfg:R:0x0:0x68 (pci-config) 230x 8.4 ms |
| gate-nvml | 177 | 2361 | 52 | 0 | 145 | 1676 | 111 | rpc GSP_RM_CONTROL 125x 738.5 ms; control (not in controls.tsv: GSS legacy or BINAPI) 1x 678.1 ms; host RM_CONTROL 0x2080a079 1x 678.1 ms; access bar0:W:0x110000:0x110c00 (gsp-queue-doorbell) 147x 71.6 ms |
| gate-cuda | 190 | 62009 | 35 | 0 | 3886 | 85249 | 70955 | window bar2 memfd: syncs (units: dirty pages) 26748x 9509.0 ms; access bar3:R:0xfea000:- (bar2-flush-read) 17262x 8358.7 ms; window bar1 pool: page moved in, cold (units: bytes) 10240x 6349.7 ms; window bar1 pool: page moved out, warm (units: bytes) 9728x 6117.3 ms |
| bench-init | 112 | 1400 | 26 | 0 | 187 | 2124 | 874 | window bar1 pool: page moved out, warm (units: bytes) 512x 193.8 ms; access bar3:R:0xfea000:- (bar2-flush-read) 472x 190.8 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 185.2 ms; window bar2 memfd: syncs (units: dirty pages) 822x 179.8 ms |
| bench-short-auto | 112 | 1693 | 26 | 0 | 187 | 2144 | 951 | window bar1 pool: page moved in, cold (units: bytes) 512x 241.8 ms; window bar2 memfd: syncs (units: dirty pages) 830x 197.3 ms; access bar3:R:0xfea000:- (bar2-flush-read) 477x 194.4 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 186.6 ms |
| bench-rl | 122 | 1508 | 31 | 0 | 194 | 2350 | 3326 | window bar1 pool: page moved out, warm (units: bytes) 512x 303.1 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 226.0 ms; access bar3:R:0xfea000:- (bar2-flush-read) 525x 186.2 ms; window bar2 memfd: syncs (units: dirty pages) 911x 179.3 ms |
| bench-ppo | 122 | 1434 | 31 | 0 | 194 | 2410 | 6240 | access bar3:R:0xfea000:- (bar2-flush-read) 540x 198.4 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 195.9 ms; window bar2 memfd: syncs (units: dirty pages) 935x 181.2 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 163.6 ms |
| smi-q | 176 | 3051 | 48 | 0 | 1010 | 9570 | 1248 | rpc GSP_RM_CONTROL 910x 723.2 ms; access bar0:W:0x110000:0x110c00 (gsp-queue-doorbell) 1030x 503.0 ms; control (not in controls.tsv: GSS legacy or BINAPI) 10x 210.7 ms; host RM_CONTROL 0x2080a079 10x 210.5 ms |
| smi-dmon | 70 | 2252 | 23 | 1 | 568 | 2377 | 5086 | rpc GSP_RM_CONTROL 544x 577.0 ms; access bar0:W:0x110000:0x110c00 (gsp-queue-doorbell) 568x 319.0 ms; control (not in controls.tsv: GSS legacy or BINAPI) 98x 225.9 ms; host RM_CONTROL 0x20809004 98x 225.3 ms |
| graphs | 112 | 1291 | 26 | 0 | 187 | 2173 | 1201 | access bar3:R:0xfea000:- (bar2-flush-read) 486x 176.4 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 166.3 ms; window bar2 memfd: syncs (units: dirty pages) 843x 165.9 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 164.6 ms |
| streams | 111 | 1533 | 26 | 0 | 184 | 2184 | 999 | window bar2 memfd: syncs (units: dirty pages) 848x 207.2 ms; access bar3:R:0xfea000:- (bar2-flush-read) 491x 189.2 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 177.7 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 174.7 ms |
| managed | 112 | 1321 | 26 | 0 | 187 | 2124 | 786 | window bar1 pool: page moved in, cold (units: bytes) 512x 205.3 ms; access bar3:R:0xfea000:- (bar2-flush-read) 472x 183.2 ms; window bar2 memfd: syncs (units: dirty pages) 822x 180.9 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 175.7 ms |
| texsurf | 112 | 1353 | 26 | 0 | 187 | 2144 | 788 | access bar3:R:0xfea000:- (bar2-flush-read) 477x 184.0 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 179.4 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 162.6 ms; window bar2 memfd: syncs (units: dirty pages) 830x 158.2 ms |
| zerocopy | 112 | 1221 | 26 | 0 | 187 | 2182 | 773 | access bar3:R:0xfea000:- (bar2-flush-read) 490x 201.9 ms; window bar2 memfd: syncs (units: dirty pages) 848x 199.5 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 176.6 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 162.7 ms |
| query | 122 | 1924 | 31 | 0 | 197 | 2184 | 984 | window bar2 memfd: syncs (units: dirty pages) 840x 259.4 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 213.4 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 211.5 ms; access bar3:R:0xfea000:- (bar2-flush-read) 477x 204.6 ms |
| torch-matmul | 122 | 1615 | 31 | 0 | 194 | 2341 | 3575 | access bar3:R:0xfea000:- (bar2-flush-read) 521x 202.1 ms; window bar2 memfd: syncs (units: dirty pages) 906x 199.3 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 188.3 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 178.1 ms |
| torch-conv | 122 | 1688 | 31 | 0 | 214 | 2785 | 7271 | window bar1 pool: page moved in, cold (units: bytes) 512x 211.6 ms; access bar3:R:0xfea000:- (bar2-flush-read) 630x 205.6 ms; window bar2 memfd: syncs (units: dirty pages) 1086x 205.3 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 161.3 ms |
| torch-attention | 112 | 1211 | 26 | 0 | 184 | 2281 | 3749 | access bar3:R:0xfea000:- (bar2-flush-read) 516x 186.6 ms; window bar2 memfd: syncs (units: dirty pages) 888x 169.5 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 162.2 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 161.0 ms |
| multiproc | 115 | 2432 | 27 | 0 | 371 | 4354 | 2695 | access bar3:R:0xfea000:- (bar2-flush-read) 977x 353.2 ms; window bar2 memfd: syncs (units: dirty pages) 1691x 339.3 ms; window bar1 pool: page moved out, warm (units: bytes) 1024x 331.8 ms; window bar1 pool: page moved in, cold (units: bytes) 1024x 326.4 ms |
| oom | 113 | 1394 | 26 | 0 | 187 | 2373 | 2195 | window bar1 pool: page moved out, warm (units: bytes) 512x 280.9 ms; access bar3:R:0xfea000:- (bar2-flush-read) 536x 191.8 ms; window bar2 memfd: syncs (units: dirty pages) 923x 168.9 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 162.7 ms |
| nvrtc | 112 | 1413 | 26 | 0 | 187 | 2124 | 844 | access bar3:R:0xfea000:- (bar2-flush-read) 472x 183.9 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 170.2 ms; window bar2 memfd: syncs (units: dirty pages) 822x 166.9 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 166.7 ms |
| vulkan | 114 | 979 | 26 | 14 | 391 | 3905 | 1594 | access bar3:R:0xfea000:- (bar2-flush-read) 863x 142.7 ms; window bar2 memfd: syncs (units: dirty pages) 1528x 114.2 ms; access bar0:W:0x110000:0x110c00 (gsp-queue-doorbell) 391x 81.7 ms; access bar0:W:0xb83000:0xb830b0 (mmu-invalidate) 274x 54.4 ms |
| idle | 14 | 287 | 0 | 0 | 0 | 0 | 115318 | host RM_GET_EVENT_DATA 115318x 287.4 ms; window intr page: scans with work 188400x 0.0 ms; window interrupts: vector raises 108611x 0.0 ms; window interrupts: MSIs written 99339x 0.0 ms |
| kill9 | 126 | 7273 | 33 | 0 | 387 | 4322 | 2427 | host RM_CONTROL NVA06C_CTRL_CMD_PREEMPT 58x 3232.9 ms; window bar1 pool: page moved in, cold (units: bytes) 1024x 583.3 ms; window bar2 memfd: syncs (units: dirty pages) 1667x 518.4 ms; access bar3:R:0xfea000:- (bar2-flush-read) 951x 429.3 ms |
| illegal | 113 | 1517 | 27 | 1 | 188 | 2128 | 850 | window bar2 memfd: syncs (units: dirty pages) 823x 199.7 ms; access bar3:R:0xfea000:- (bar2-flush-read) 472x 193.3 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 190.0 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 185.3 ms |
| after-errors | 122 | 1436 | 31 | 0 | 197 | 2184 | 935 | access bar3:R:0xfea000:- (bar2-flush-read) 477x 197.2 ms; window bar2 memfd: syncs (units: dirty pages) 840x 188.9 ms; window bar1 pool: page moved in, cold (units: bytes) 512x 188.8 ms; window bar1 pool: page moved out, warm (units: bytes) 512x 172.8 ms |

### New controls per workload (not sent in either gate's window)

- **boot** (39): 0x00802046 NV0080_CTRL_CMD_INTERNAL_KGR_INIT_BUG4208224_WAR [echo]; 0x2080013f NV2080_CTRL_CMD_GPU_GET_OEM_BOARD_INFO [host-forwarded]; 0x2080017e NV2080_CTRL_CMD_GPU_GET_VMMU_SEGMENT_SIZE [host-forwarded]; 0x208001b0 NV2080_CTRL_CMD_GPU_GET_CONSTRUCTED_FALCON_INFO [host-forwarded]; 0x20800301 NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION [echo]; 0x20800810 NV2080_CTRL_CMD_BIOS_GET_INFO_V2 [synthesized]; 0x20800a1c NV2080_CTRL_CMD_INTERNAL_MEMSYS_GET_STATIC_CONFIG [synthesized]; 0x20800a1f NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_CAPS [host-forwarded]; 0x20800a22 NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER [host-forwarded]; 0x20800a26 NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS [host-forwarded]; 0x20800a2a NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_INFO [host-forwarded]; 0x20800a2c NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_ZCULL_INFO [host-forwarded]; 0x20800a2e NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_ROP_INFO [host-forwarded]; 0x20800a30 NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_PPC_MASKS [refused]; 0x20800a32 NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO [host-forwarded]; 0x20800a34 NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER [host-forwarded]; 0x20800a36 NV2080_CTRL_CMD_INTERNAL_GPU_GET_CHIP_INFO [echo]; 0x20800a3d NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_FECS_RECORD_SIZE [synthesized]; 0x20800a3f NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_FECS_TRACE_DEFINES [refused]; 0x20800a41 NV2080_CTRL_CMD_INTERNAL_GPU_GET_USER_REGISTER_ACCESS_MAP [synthesized]; 0x20800a48 NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_PDB_PROPERTIES [synthesized]; 0x20800a59 NV2080_CTRL_CMD_INTERNAL_GMMU_GET_STATIC_INFO [synthesized]; 0x20800a5c NV2080_CTRL_CMD_INTERNAL_INTR_GET_KERNEL_TABLE [synthesized]; 0x20800a61 NV2080_CTRL_CMD_INTERNAL_FIFO_GET_NUM_CHANNELS [synthesized]; 0x20800a6c NV2080_CTRL_CMD_INTERNAL_MEMSYS_L2_INVALIDATE_EVICT [echo]; 0x20800a80 NV2080_CTRL_CMD_INTERNAL_PERF_GPU_BOOST_SYNC_GET_INFO [synthesized]; 0x20800a9f NV2080_CTRL_CMD_INTERNAL_GMMU_COPY_RESERVED_SPLIT_GVASPACE_PDES_TO_SERVER [echo]; 0x20800aac NV2080_CTRL_CMD_INTERNAL_BIF_GET_STATIC_INFO [synthesized]; 0x20800afe NV2080_CTRL_CMD_INTERNAL_INIT_USER_SHARED_DATA [guest-derived]; 0x20800aff NV2080_CTRL_CMD_INTERNAL_USER_SHARED_DATA_SET_DATA_POLL [echo]; 0x20800b03 NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER_V2 [host-forwarded]; 0x20800b05 NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_THROTTLE_CTRL [refused]; 0x20801112 NV2080_CTRL_CMD_FIFO_GET_DEVICE_INFO_TABLE [synthesized]; 0x20801803 NV2080_CTRL_CMD_BUS_GET_PCI_BAR_INFO [guest-derived]; 0x20802a06 NV2080_CTRL_CMD_CE_UPDATE_CLASS_DB [synthesized]; 0x20802a0d NV2080_CTRL_CMD_CE_UPDATE_PCE_LCE_MAPPINGS_V2 [echo]; 0x20802a0f NV2080_CTRL_CMD_INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_TYPE [synthesized]; 0x20803601 NV2080_CTRL_CMD_GSP_GET_FEATURES [synthesized]; 0x402c0101 NV402C_CTRL_CMD_I2C_GET_PORT_INFO [synthesized]
- **smi-dmon** (1): 0x20802096 NV2080_CTRL_CMD_PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2 [host-forwarded]
- **vulkan** (14): 0x00801707 NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES [host-forwarded]; 0x00801b02 NV0080_CTRL_CMD_NVENC_GET_CAPS_V2 [synthesized]; 0x00801c02 NV0080_CTRL_CMD_NVDEC_GET_CAPS_V2 [synthesized]; 0x20800301 NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION [echo]; 0x20800aff NV2080_CTRL_CMD_INTERNAL_USER_SHARED_DATA_SET_DATA_POLL [echo]; 0x20801208 NV2080_CTRL_CMD_GR_CTXSW_ZCULL_BIND [guest-derived]; 0x20801315 NV2080_CTRL_CMD_FB_GET_GPU_CACHE_INFO [host-forwarded]; 0x20802a02 NV2080_CTRL_CMD_CE_GET_CE_PCE_MASK [host-forwarded]; 0x20802a07 NV2080_CTRL_CMD_CE_GET_PHYSICAL_CAPS [host-forwarded]; 0x20810107 (not in controls.tsv) [-]; 0x90960106 NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_SIZE [synthesized]; 0x90960107 NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY [guest-derived]; 0xa06f0103 NVA06F_CTRL_CMD_GPFIFO_SCHEDULE [synthesized]; 0xa06f0104 NVA06F_CTRL_CMD_BIND [guest-derived]
- **illegal** (1): 0x83de030c NV83DE_CTRL_CMD_DEBUG_READ_ALL_SM_ERROR_STATES [refused]

### Top 8 rows per workload

**boot**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 18 | 10000.64 | 555591.0 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 2 | host | 0x29:0x0 | RM_FREE | 14 | 485.33 | 34666.7 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 3 | access | bar0:W:0x840000:0x840184 | NV_PSEC (SEC2 falcon) | 8512 | 320.65 | 37.7 | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 4 | access | bar0:W:0x840000:0x8401c4 | NV_PSEC (SEC2 falcon) | 6272 | 219.81 | 35.0 | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 5 | host | 0x2a:0x410103 | RM_CONTROL NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR | 162 | 98.12 | 605.7 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | access | bar0:W:0x70d000:- | NV_PRAMIN | 1000 | 56.58 | 56.6 | bar0.rs, boot.rs | NV_PRAMIN |
| 7 | access | bar0:W:0x70e000:- | NV_PRAMIN | 960 | 46.00 | 47.9 | bar0.rs, boot.rs | NV_PRAMIN |
| 8 | access | bar0:W:0x713000:- | NV_PRAMIN | 960 | 44.23 | 46.1 | bar0.rs, boot.rs | NV_PRAMIN |

**setup**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | access | cfg:R:0x0:0x4 | - | 116 | 10.72 | 92.4 | config.rs | - |
| 2 | access | cfg:R:0x0:0x78 | - | 230 | 10.20 | 44.3 | config.rs | - |
| 3 | access | cfg:R:0x0:0x60 | - | 230 | 9.47 | 41.2 | config.rs | - |
| 4 | access | cfg:R:0x0:0x68 | - | 230 | 8.43 | 36.7 | config.rs | - |
| 5 | access | cfg:R:0x0:0x34 | - | 116 | 5.03 | 43.4 | config.rs | - |
| 6 | access | cfg:R:0x0:0x4c | - | 2 | 0.20 | 102.3 | config.rs | - |
| 7 | access | cfg:R:0x0:0x0 | - | 2 | 0.11 | 54.9 | config.rs | - |
| 8 | access | cfg:R:0x0:0xe8 | - | 2 | 0.11 | 53.7 | config.rs | - |

**gate-nvml**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | rpc | 0x4c | GSP_RM_CONTROL | 125 | 738.49 | 5907.9 | boot.rs (service_queues), gsprpc.rs | - |
| 2 | control | 0x2080a079 | (not in controls.tsv: GSS legacy or BINAPI) | 1 | 678.07 | 678072.8 | - | host (GSS legacy, forwarded opaquely; command and size from a native host trace) |
| 3 | host | 0x2a:0x2080a079 | RM_CONTROL 0x2080a079 | 1 | 678.06 | 678056.8 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 4 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 147 | 71.57 | 486.9 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 5 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 147 | 19.30 | 131.3 | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 6 | control | 0x20809004 | (not in controls.tsv: GSS legacy or BINAPI) | 4 | 9.44 | 2360.4 | - | host (GSS legacy, forwarded opaquely; command and size from a native host trace) |
| 7 | host | 0x2a:0x20809004 | RM_CONTROL 0x20809004 | 4 | 9.40 | 2350.9 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | access | bar0:R:0x110000:0x110804 | NV_PGSP (GSP falcon) | 147 | 9.16 | 62.3 | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |

**gate-cuda**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 26748 | 9508.98 | 355.5 | bar2area.rs | - |
| 2 | access | bar3:R:0xfea000:- | - | 17262 | 8358.67 | 484.2 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 3 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 10240 | 6349.69 | 620.1 | bar1pool.rs | - |
| 4 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 9728 | 6117.33 | 628.8 | bar1pool.rs | - |
| 5 | host | 0x4e:0x0 | RM_MAP_MEMORY | 10891 | 3598.05 | 330.4 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | shadow | gather alloc | gather alloc | 602 | 2684.14 | 4458.7 | shadow.rs (worker) | - |
| 7 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 400 | 2272.67 | 5681.7 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | host | 0x29:0x0 | RM_FREE | 5430 | 1962.73 | 361.5 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

**bench-init**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 193.77 | 378.5 | bar1pool.rs | - |
| 2 | access | bar3:R:0xfea000:- | - | 472 | 190.80 | 404.2 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 3 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 185.23 | 361.8 | bar1pool.rs | - |
| 4 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 822 | 179.85 | 218.8 | bar2area.rs | - |
| 5 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 20 | 67.14 | 3357.2 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 187 | 66.03 | 353.1 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 7 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 8 | 63.21 | 7900.6 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | host | 0x29:0x0 | RM_FREE | 130 | 56.90 | 437.7 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

**bench-short-auto**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 241.84 | 472.4 | bar1pool.rs | - |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 830 | 197.27 | 237.7 | bar2area.rs | - |
| 3 | access | bar3:R:0xfea000:- | - | 477 | 194.43 | 407.6 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 4 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 186.57 | 364.4 | bar1pool.rs | - |
| 5 | shadow | gather alloc | gather alloc | 38 | 133.45 | 3512.0 | shadow.rs (worker) | - |
| 6 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 20 | 122.58 | 6128.9 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 8 | 87.11 | 10888.9 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 187 | 80.01 | 427.9 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |

**bench-rl**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 303.07 | 591.9 | bar1pool.rs | - |
| 2 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 226.03 | 441.5 | bar1pool.rs | - |
| 3 | access | bar3:R:0xfea000:- | - | 525 | 186.15 | 354.6 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 4 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 911 | 179.27 | 196.8 | bar2area.rs | - |
| 5 | host | 0x29:0x0 | RM_FREE | 191 | 82.31 | 430.9 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | rpc | 0xa | FREE | 71 | 68.45 | 964.1 | boot.rs (service_queues), gsprpc.rs | - |
| 7 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 56 | 55.41 | 989.5 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 194 | 47.06 | 242.6 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |

**bench-ppo**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | access | bar3:R:0xfea000:- | - | 540 | 198.41 | 367.4 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 2 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 195.86 | 382.5 | bar1pool.rs | - |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 935 | 181.19 | 193.8 | bar2area.rs | - |
| 4 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 163.64 | 319.6 | bar1pool.rs | - |
| 5 | shadow | gather alloc | gather alloc | 40 | 111.37 | 2784.1 | shadow.rs (worker) | - |
| 6 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 194 | 57.23 | 295.0 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 7 | host | 0x29:0x0 | RM_FREE | 212 | 53.20 | 250.9 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 20 | 47.37 | 2368.3 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

**smi-q**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | rpc | 0x4c | GSP_RM_CONTROL | 910 | 723.24 | 794.8 | boot.rs (service_queues), gsprpc.rs | - |
| 2 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 1030 | 502.98 | 488.3 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 3 | control | 0x2080a079 | (not in controls.tsv: GSS legacy or BINAPI) | 10 | 210.66 | 21066.3 | - | host (GSS legacy, forwarded opaquely; command and size from a native host trace) |
| 4 | host | 0x2a:0x2080a079 | RM_CONTROL 0x2080a079 | 10 | 210.52 | 21051.7 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 5 | control | 0x20809004 | (not in controls.tsv: GSS legacy or BINAPI) | 40 | 85.51 | 2137.7 | - | host (GSS legacy, forwarded opaquely; command and size from a native host trace) |
| 6 | host | 0x2a:0x20809004 | RM_CONTROL 0x20809004 | 40 | 85.34 | 2133.4 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 1030 | 75.61 | 73.4 | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 8 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 1030 | 75.58 | 73.4 | bar2area.rs | - |

**smi-dmon**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | rpc | 0x4c | GSP_RM_CONTROL | 544 | 577.01 | 1060.7 | boot.rs (service_queues), gsprpc.rs | - |
| 2 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 568 | 318.96 | 561.6 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 3 | control | 0x20809004 | (not in controls.tsv: GSS legacy or BINAPI) | 98 | 225.85 | 2304.6 | - | host (GSS legacy, forwarded opaquely; command and size from a native host trace) |
| 4 | host | 0x2a:0x20809004 | RM_CONTROL 0x20809004 | 98 | 225.34 | 2299.4 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 5 | control | 0x20802087 | NV2080_CTRL_CMD_PERF_GET_VID_ENG_PERFMON_SAMPLE | 80 | 109.78 | 1372.2 | nvml.rs | host (forwarded as-is, public control) |
| 6 | host | 0x2a:0x20802087 | RM_CONTROL NV2080_CTRL_CMD_PERF_GET_VID_ENG_PERFMON_SAMPLE | 80 | 109.41 | 1367.6 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | control | 0x2080a097 | (not in controls.tsv: GSS legacy or BINAPI) | 69 | 92.24 | 1336.8 | - | host (GSS legacy, forwarded opaquely; command and size from a native host trace) |
| 8 | host | 0x2a:0x2080a097 | RM_CONTROL 0x2080a097 | 69 | 91.77 | 1330.0 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

**graphs**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | access | bar3:R:0xfea000:- | - | 486 | 176.40 | 363.0 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 2 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 166.32 | 324.8 | bar1pool.rs | - |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 843 | 165.88 | 196.8 | bar2area.rs | - |
| 4 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 164.63 | 321.5 | bar1pool.rs | - |
| 5 | host | 0x29:0x0 | RM_FREE | 134 | 79.03 | 589.8 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | rpc | 0xa | FREE | 71 | 72.08 | 1015.3 | boot.rs (service_queues), gsprpc.rs | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 187 | 50.70 | 271.1 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 20 | 43.91 | 2195.6 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

**streams**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 848 | 207.24 | 244.4 | bar2area.rs | - |
| 2 | access | bar3:R:0xfea000:- | - | 491 | 189.17 | 385.3 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 3 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 177.75 | 347.2 | bar1pool.rs | - |
| 4 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 174.70 | 341.2 | bar1pool.rs | - |
| 5 | shadow | gather alloc | gather alloc | 39 | 163.70 | 4197.3 | shadow.rs (worker) | - |
| 6 | host | 0x29:0x0 | RM_FREE | 155 | 60.86 | 392.6 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 184 | 58.63 | 318.7 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 20 | 54.65 | 2732.5 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

**managed**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 205.33 | 401.0 | bar1pool.rs | - |
| 2 | access | bar3:R:0xfea000:- | - | 472 | 183.22 | 388.2 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 822 | 180.86 | 220.0 | bar2area.rs | - |
| 4 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 175.72 | 343.2 | bar1pool.rs | - |
| 5 | host | 0x29:0x0 | RM_FREE | 132 | 68.03 | 515.4 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | rpc | 0xa | FREE | 71 | 63.11 | 888.9 | boot.rs (service_queues), gsprpc.rs | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 187 | 61.33 | 328.0 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 20 | 54.39 | 2719.6 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

**texsurf**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | access | bar3:R:0xfea000:- | - | 477 | 183.99 | 385.7 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 2 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 179.40 | 350.4 | bar1pool.rs | - |
| 3 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 162.59 | 317.5 | bar1pool.rs | - |
| 4 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 830 | 158.16 | 190.6 | bar2area.rs | - |
| 5 | shadow | gather alloc | gather alloc | 38 | 124.03 | 3264.0 | shadow.rs (worker) | - |
| 6 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 187 | 61.12 | 326.8 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 7 | host | 0x29:0x0 | RM_FREE | 151 | 58.08 | 384.6 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 8 | 55.37 | 6920.7 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

**zerocopy**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | access | bar3:R:0xfea000:- | - | 490 | 201.89 | 412.0 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 848 | 199.46 | 235.2 | bar2area.rs | - |
| 3 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 176.63 | 345.0 | bar1pool.rs | - |
| 4 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 162.74 | 317.8 | bar1pool.rs | - |
| 5 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 187 | 73.37 | 392.3 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 6 | host | 0x29:0x0 | RM_FREE | 135 | 46.04 | 341.0 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | rpc | 0xa | FREE | 71 | 40.63 | 572.2 | boot.rs (service_queues), gsprpc.rs | - |
| 8 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 171 | 38.96 | 227.8 | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |

**query**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 840 | 259.39 | 308.8 | bar2area.rs | - |
| 2 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 213.39 | 416.8 | bar1pool.rs | - |
| 3 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 211.54 | 413.2 | bar1pool.rs | - |
| 4 | access | bar3:R:0xfea000:- | - | 477 | 204.62 | 429.0 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 5 | shadow | gather alloc | gather alloc | 38 | 180.95 | 4761.8 | shadow.rs (worker) | - |
| 6 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 20 | 78.12 | 3906.2 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 197 | 76.20 | 386.8 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | host | 0x29:0x0 | RM_FREE | 168 | 71.68 | 426.7 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

**torch-matmul**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | access | bar3:R:0xfea000:- | - | 521 | 202.09 | 387.9 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 906 | 199.31 | 220.0 | bar2area.rs | - |
| 3 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 188.26 | 367.7 | bar1pool.rs | - |
| 4 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 178.09 | 347.8 | bar1pool.rs | - |
| 5 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 8 | 82.49 | 10311.6 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 20 | 74.70 | 3734.9 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 194 | 63.17 | 325.6 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 20 | 53.14 | 2656.9 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

**torch-conv**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 211.60 | 413.3 | bar1pool.rs | - |
| 2 | access | bar3:R:0xfea000:- | - | 630 | 205.62 | 326.4 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 1086 | 205.25 | 189.0 | bar2area.rs | - |
| 4 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 161.34 | 315.1 | bar1pool.rs | - |
| 5 | shadow | gather alloc | gather alloc | 40 | 130.49 | 3262.3 | shadow.rs (worker) | - |
| 6 | host | 0x29:0x0 | RM_FREE | 944 | 53.33 | 56.5 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 214 | 52.50 | 245.3 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | shadow | object alloc | object alloc | 879 | 51.90 | 59.0 | shadow.rs (worker) | - |

**torch-attention**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | access | bar3:R:0xfea000:- | - | 516 | 186.59 | 361.6 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 888 | 169.53 | 190.9 | bar2area.rs | - |
| 3 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 162.21 | 316.8 | bar1pool.rs | - |
| 4 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 161.04 | 314.5 | bar1pool.rs | - |
| 5 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 184 | 46.95 | 255.2 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 6 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 20 | 43.72 | 2185.9 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | host | 0x29:0x0 | RM_FREE | 235 | 43.31 | 184.3 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 188 | 34.34 | 182.7 | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |

**multiproc**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | access | bar3:R:0xfea000:- | - | 977 | 353.16 | 361.5 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 1691 | 339.27 | 200.6 | bar2area.rs | - |
| 3 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 1024 | 331.78 | 324.0 | bar1pool.rs | - |
| 4 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 1024 | 326.35 | 318.7 | bar1pool.rs | - |
| 5 | shadow | gather alloc | gather alloc | 65 | 167.33 | 2574.3 | shadow.rs (worker) | - |
| 6 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 371 | 119.13 | 321.1 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 7 | host | 0x29:0x0 | RM_FREE | 295 | 103.92 | 352.3 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | rpc | 0xa | FREE | 142 | 90.16 | 634.9 | boot.rs (service_queues), gsprpc.rs | - |

**oom**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 280.89 | 548.6 | bar1pool.rs | - |
| 2 | access | bar3:R:0xfea000:- | - | 536 | 191.82 | 357.9 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 923 | 168.91 | 183.0 | bar2area.rs | - |
| 4 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 162.71 | 317.8 | bar1pool.rs | - |
| 5 | host | 0x29:0x0 | RM_FREE | 486 | 76.05 | 156.5 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 200 | 66.96 | 334.8 | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 7 | rpc | 0xa | FREE | 71 | 62.92 | 886.3 | boot.rs (service_queues), gsprpc.rs | - |
| 8 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 187 | 51.03 | 272.9 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |

**nvrtc**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | access | bar3:R:0xfea000:- | - | 472 | 183.90 | 389.6 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 2 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 170.18 | 332.4 | bar1pool.rs | - |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 822 | 166.94 | 203.1 | bar2area.rs | - |
| 4 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 166.65 | 325.5 | bar1pool.rs | - |
| 5 | shadow | gather alloc | gather alloc | 38 | 106.64 | 2806.4 | shadow.rs (worker) | - |
| 6 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 20 | 82.25 | 4112.6 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | host | 0x29:0x0 | RM_FREE | 150 | 72.40 | 482.7 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | rpc | 0xa | FREE | 71 | 63.86 | 899.5 | boot.rs (service_queues), gsprpc.rs | - |

**vulkan**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | access | bar3:R:0xfea000:- | - | 863 | 142.67 | 165.3 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 1528 | 114.19 | 74.7 | bar2area.rs | - |
| 3 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 391 | 81.71 | 209.0 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 4 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 274 | 54.39 | 198.5 | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 5 | window | bar1 pool: page moved out, cold (units: bytes) | bar1 pool: page moved out, cold (units: bytes) | 131 | 45.43 | 346.8 | bar1pool.rs | - |
| 6 | rpc | 0x4c | GSP_RM_CONTROL | 277 | 43.72 | 157.9 | boot.rs (service_queues), gsprpc.rs | - |
| 7 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 131 | 43.41 | 331.4 | bar1pool.rs | - |
| 8 | host | 0x29:0x0 | RM_FREE | 282 | 41.57 | 147.4 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

**idle**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | host | 0x52:0x0 | RM_GET_EVENT_DATA | 115318 | 287.37 | 2.5 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 2 | window | intr page: scans with work | intr page: scans with work | 188400 | 0.00 | 0.0 | intr_page.rs | - |
| 3 | window | interrupts: vector raises | interrupts: vector raises | 108611 | 0.00 | 0.0 | intr.rs | - |
| 4 | window | interrupts: MSIs written | interrupts: MSIs written | 99339 | 0.00 | 0.0 | intr.rs | - |
| 5 | window | intr page: MSIs for raises | intr page: MSIs for raises | 99336 | 0.00 | 0.0 | intr_page.rs | - |
| 6 | window | intr page: acks applied | intr page: acks applied | 99336 | 0.00 | 0.0 | intr_page.rs | - |
| 7 | window | msi re-arm eventfd: wakeups (units: guest writes) | msi re-arm eventfd: wakeups (units: guest writes) | 99022 | 0.00 | 0.0 | device.rs, intr_page.rs | - |
| 8 | window | interrupts: raises coalesced while pending | interrupts: raises coalesced while pending | 8830 | 0.00 | 0.0 | intr.rs | - |

**kill9**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 58 | 3232.88 | 55739.3 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 2 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 1024 | 583.31 | 569.6 | bar1pool.rs | - |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 1667 | 518.42 | 311.0 | bar2area.rs | - |
| 4 | access | bar3:R:0xfea000:- | - | 951 | 429.33 | 451.5 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 5 | shadow | gather alloc | gather alloc | 60 | 321.00 | 5350.0 | shadow.rs (worker) | - |
| 6 | window | bar1 pool: page moved out, cold (units: bytes) | bar1 pool: page moved out, cold (units: bytes) | 512 | 191.37 | 373.8 | bar1pool.rs | - |
| 7 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 40 | 183.85 | 4596.2 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 171.87 | 335.7 | bar1pool.rs | - |

**illegal**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 823 | 199.74 | 242.7 | bar2area.rs | - |
| 2 | access | bar3:R:0xfea000:- | - | 472 | 193.33 | 409.6 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 3 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 190.02 | 371.1 | bar1pool.rs | - |
| 4 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 185.25 | 361.8 | bar1pool.rs | - |
| 5 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 188 | 81.64 | 434.3 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 6 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 20 | 78.69 | 3934.3 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 8 | 72.13 | 9016.6 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | host | 0x29:0x0 | RM_FREE | 135 | 71.41 | 528.9 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

**after-errors**

| # | kind | key | name | count | cost ms | mean us | module | label |
|---|---|---|---|---|---|---|---|---|
| 1 | access | bar3:R:0xfea000:- | - | 477 | 197.20 | 413.4 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 840 | 188.90 | 224.9 | bar2area.rs | - |
| 3 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 512 | 188.84 | 368.8 | bar1pool.rs | - |
| 4 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 512 | 172.81 | 337.5 | bar1pool.rs | - |
| 5 | shadow | gather alloc | gather alloc | 38 | 139.71 | 3676.5 | shadow.rs (worker) | - |
| 6 | host | 0x29:0x0 | RM_FREE | 168 | 70.71 | 420.9 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | rpc | 0xa | FREE | 71 | 61.22 | 862.3 | boot.rs (service_queues), gsprpc.rs | - |
| 8 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 197 | 56.10 | 284.8 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |

## (b) The union

Controls of controls.tsv (gsp-610.57.04) that reach GSP, by the device's provenance, and whether any workload of this run (boot and teardown included) sent them:

| provenance | answered by the device | touched | untouched |
|---|---|---|---|
| synthesized | yes | 26 | 54 |
| host-forwarded | yes | 40 | 30 |
| guest-derived | yes | 8 | 66 |
| echo | yes | 16 | 48 |
| refused | yes | 7 | 468 |
| unhandled | no | 0 | 66 |

Controls seen that controls.tsv does not list (GSS legacy, BINAPI): 35: 0x20808159, 0x20808162, 0x2080852a, 0x2080852c, 0x2080852e, 0x2080852f, 0x20808536, 0x20808542, 0x20808546, 0x20809001, 0x20809004, 0x20809009, 0x20809019, 0x20809038, 0x20809064, 0x2080a014, 0x2080a026, 0x2080a060, 0x2080a06e, 0x2080a079, 0x2080a080, 0x2080a081, 0x2080a084, 0x2080a097, 0x2080a0a4, 0x2080a0a7, 0x2080a0a8, 0x2080a0f2, 0x2080a612, 0x2080a618, 0x2080a63c, 0x20810107, 0x20810108, 0x20810110, 0x20810111

- rpc: 10 exercised: SET_GUEST_SYSTEM_INFO, FREE, DUP_OBJECT, GET_GSP_STATIC_INFO, UPDATE_BAR_PDE, GSP_SET_SYSTEM_INFO, SET_REGISTRY, GSP_RM_CONTROL, GSP_RM_ALLOC, INIT_GSP_TRACE_CRASH_BUFFER
- event: 0 exercised: 
- alloc: 21 exercised: NV01_ROOT, NV01_MEMORY_VIRTUAL, NV01_EVENT_OS_EVENT, NV01_EVENT_KERNEL_CALLBACK_EX, NV01_DEVICE_0, NV20_SUBDEVICE_0, NV2081_BINAPI, NV20_SUBDEVICE_DIAG, NV40_I2C, GT200_DEBUGGER, FERMI_TWOD_A, FERMI_CONTEXT_SHARE_A, GF100_ZBC_CLEAR, GF100_SUBDEVICE_INFOROM, FERMI_VASPACE_A, KEPLER_CHANNEL_GROUP_A, KEPLER_INLINE_TO_MEMORY_B, TURING_CHANNEL_GPFIFO_A, TURING_A, TURING_DMA_COPY_A, TURING_COMPUTE_A
- host: 112 exercised: RM_ALLOC_MEMORY 0x27:0x0, RM_FREE 0x29:0x0, RM_CONTROL 0x2a:0x20800102, RM_CONTROL 0x2a:0x2080012f, RM_CONTROL 0x2a:0x2080013f, RM_CONTROL 0x2a:0x2080014b, RM_CONTROL 0x2a:0x20800156, RM_CONTROL 0x2a:0x20800157, RM_CONTROL 0x2a:0x20800168, RM_CONTROL 0x2a:0x2080017e, RM_CONTROL 0x2a:0x208001a4, RM_CONTROL 0x2a:0x208001b0, RM_CONTROL 0x2a:0x20800301, RM_CONTROL 0x2a:0x20800513, RM_CONTROL 0x2a:0x20801206, RM_CONTROL 0x2a:0x20801213, RM_CONTROL 0x2a:0x2080121b, RM_CONTROL 0x2a:0x20801227, RM_CONTROL 0x2a:0x20801228, RM_CONTROL 0x2a:0x2080122a, RM_CONTROL 0x2a:0x2080122b, RM_CONTROL 0x2a:0x2080122d, RM_CONTROL 0x2a:0x20801230, RM_CONTROL 0x2a:0x20801234, RM_CONTROL 0x2a:0x20801237, RM_CONTROL 0x2a:0x20801239, RM_CONTROL 0x2a:0x2080123c, RM_CONTROL 0x2a:0x20801303, RM_CONTROL 0x2a:0x2080130e, RM_CONTROL 0x2a:0x20801315, RM_CONTROL 0x2a:0x20801320, RM_CONTROL 0x2a:0x20801322, RM_CONTROL 0x2a:0x20801344, RM_CONTROL 0x2a:0x20801357, RM_CONTROL 0x2a:0x20801813, RM_CONTROL 0x2a:0x20801819, RM_CONTROL 0x2a:0x20801823, RM_CONTROL 0x2a:0x20801829, RM_CONTROL 0x2a:0x2080182a, RM_CONTROL 0x2a:0x2080182b, RM_CONTROL 0x2a:0x20801830, RM_CONTROL 0x2a:0x20802068, RM_CONTROL 0x2a:0x20802087, RM_CONTROL 0x2a:0x20802096, RM_CONTROL 0x2a:0x20802a02, RM_CONTROL 0x2a:0x20802a03, RM_CONTROL 0x2a:0x20802a0a, RM_CONTROL 0x2a:0x20803801, RM_CONTROL 0x2a:0x20808159, RM_CONTROL 0x2a:0x20808162, RM_CONTROL 0x2a:0x2080852a, RM_CONTROL 0x2a:0x2080852c, RM_CONTROL 0x2a:0x2080852e, RM_CONTROL 0x2a:0x2080852f, RM_CONTROL 0x2a:0x20808542, RM_CONTROL 0x2a:0x20808546, RM_CONTROL 0x2a:0x20809001, RM_CONTROL 0x2a:0x20809004, RM_CONTROL 0x2a:0x20809009, RM_CONTROL 0x2a:0x20809019, RM_CONTROL 0x2a:0x20809038, RM_CONTROL 0x2a:0x20809064, RM_CONTROL 0x2a:0x2080a014, RM_CONTROL 0x2a:0x2080a026, RM_CONTROL 0x2a:0x2080a060, RM_CONTROL 0x2a:0x2080a06e, RM_CONTROL 0x2a:0x2080a079, RM_CONTROL 0x2a:0x2080a080, RM_CONTROL 0x2a:0x2080a081, RM_CONTROL 0x2a:0x2080a084, RM_CONTROL 0x2a:0x2080a097, RM_CONTROL 0x2a:0x2080a0a4, RM_CONTROL 0x2a:0x2080a0a7, RM_CONTROL 0x2a:0x2080a0a8, RM_CONTROL 0x2a:0x2080a0f2, RM_CONTROL 0x2a:0x2080a612, RM_CONTROL 0x2a:0x2080a63c, RM_CONTROL 0x2a:0x20810107, RM_CONTROL 0x2a:0x20810108, RM_CONTROL 0x2a:0x20810110, RM_CONTROL 0x2a:0x20810111, RM_CONTROL 0x2a:0x410103, RM_CONTROL 0x2a:0x800294, RM_CONTROL 0x2a:0x801707, RM_CONTROL 0x2a:0xa06c0103, RM_CONTROL 0x2a:0xa06c0105, RM_CONTROL 0x2a:0xa06f0103, RM_CONTROL 0x2a:0xc36f0108, RM_ALLOC 0x2b:0x2080, RM_ALLOC 0x2b:0x2081, RM_ALLOC 0x2b:0x40, RM_ALLOC 0x2b:0x41, RM_ALLOC 0x2b:0x50a0, RM_ALLOC 0x2b:0x79, RM_ALLOC 0x2b:0x80, RM_ALLOC 0x2b:0x902d, RM_ALLOC 0x2b:0x90f1, RM_ALLOC 0x2b:0xa06c, RM_ALLOC 0x2b:0xa140, RM_ALLOC 0x2b:0xc361, RM_ALLOC 0x2b:0xc46f, RM_ALLOC 0x2b:0xc597, RM_ALLOC 0x2b:0xc5b5, RM_ALLOC 0x2b:0xc5c0, RM_MAP_MEMORY 0x4e:0x0, RM_UNMAP_MEMORY 0x4f:0x0, RM_GET_EVENT_DATA 0x52:0x0, RM_MAP_MEMORY_DMA 0x57:0x0, RM_UNMAP_MEMORY_DMA 0x58:0x0, REGISTER_FD 0xc9:0x0, ALLOC_OS_EVENT 0xce:0x0, other 0xd9:0x0
- shadow: 6 exercised: gather alloc, gather free, map, object alloc, object free, unmap
- window: 21 exercised: bar1 pool: page moved in, cold (units: bytes), bar1 pool: page moved out, cold (units: bytes), bar1 pool: page moved out, warm (units: bytes), bar1 pool: warm-move barriers (units: pages), bar2 memfd: syncs (units: dirty pages), bar2 memfd: table walks, interrupts: MSIs written, interrupts: raises coalesced while pending, interrupts: vector raises, intr page: LEAF_EN_CLEAR applied, intr page: LEAF_EN_SET applied, intr page: MSI re-arms (set store while pending), intr page: MSIs by the watchdog, intr page: MSIs for raises, intr page: TOP_EN_CLEAR applied, intr page: TOP_EN_SET applied, intr page: acks applied, intr page: scans with work, intr page: triggers applied, msi re-arm eventfd: wakeups (units: guest writes), token pool: eventfd rings (units: guest writes)

## (c) Candidates outside the benchmarks

"In the benchmarks" is the same row's cost in bench-init, short-auto, rl and ppo together; a row with 0 there is a path the benchmarks do not reach at all.

| # | kind | key | name | count | cost ms (all non-benchmark workloads) | mean us | in the benchmarks, ms | top workloads | module | label |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 44698 | 13094.8 | 293.0 | 737.6 | gate-cuda 9509, kill9 518, multiproc 339 | bar2area.rs | - |
| 2 | access | bar3:R:0xfea000:- | - | 26570 | 11783.7 | 443.5 | 769.8 | gate-cuda 8359, kill9 429, multiproc 353 | fb.rs, bar2area.rs (the memfd sync point) | - |
| 3 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 19075 | 9673.4 | 507.1 | 816.7 | gate-cuda 6350, kill9 583, multiproc 326 | bar1pool.rs | - |
| 4 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 17920 | 9001.0 | 502.3 | 879.3 | gate-cuda 6117, multiproc 332, oom 281 | bar1pool.rs | - |
| 5 | shadow | gather alloc | gather alloc | 1119 | 4260.1 | 3807.1 | 314.0 | gate-cuda 2684, kill9 321, query 181 | shadow.rs (worker) | - |
| 6 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 1104 | 3931.2 | 3560.8 | 83.4 | kill9 3233, gate-cuda 539, streams 30 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | host | 0x4e:0x0 | RM_MAP_MEMORY | 11261 | 3652.5 | 324.3 | 8.1 | gate-cuda 3598, multiproc 15, kill9 8 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 9260 | 3371.2 | 364.1 | 250.3 | gate-cuda 1317, smi-q 503, smi-dmon 319 | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 9 | host | 0x29:0x0 | RM_FREE | 9488 | 3048.6 | 321.3 | 256.8 | gate-cuda 1963, kill9 126, multiproc 104 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 743 | 2697.6 | 3630.7 | 128.3 | gate-cuda 2273, kill9 96, torch-matmul 53 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 11 | rpc | 0x4c | GSP_RM_CONTROL | 3690 | 2692.2 | 729.6 | 22.9 | gate-nvml 738, smi-q 723, smi-dmon 577 | boot.rs (service_queues), gsprpc.rs | - |
| 12 | rpc | 0xa | FREE | 2774 | 2303.2 | 830.3 | 208.6 | gate-cuda 1405, kill9 102, multiproc 90 | boot.rs (service_queues), gsprpc.rs | - |
| 13 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 743 | 2218.8 | 2986.3 | 263.5 | gate-cuda 1208, kill9 184, multiproc 85 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 1859 | 1883.1 | 1013.0 | 111.4 | gate-cuda 1333, kill9 101, multiproc 54 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | shadow | map | map | 9545 | 1722.4 | 180.5 | 34.6 | gate-cuda 1440, kill9 60, torch-conv 42 | shadow.rs (worker) | - |
| 16 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 9545 | 1714.8 | 179.7 | 34.3 | gate-cuda 1437, kill9 57, torch-conv 41 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 17 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 8868 | 1606.3 | 181.1 | 127.2 | gate-cuda 977, kill9 78, oom 67 | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 18 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 17736 | 1354.6 | 76.4 | 86.8 | gate-cuda 930, kill9 90, multiproc 34 | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 19 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 299 | 1332.4 | 4456.1 | 193.7 | gate-cuda 581, kill9 153, torch-matmul 82 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | shadow | object alloc | object alloc | 7438 | 1210.2 | 162.7 | 57.4 | gate-cuda 922, kill9 57, torch-conv 52 | shadow.rs (worker) | - |
| 21 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 7438 | 1203.5 | 161.8 | 57.0 | gate-cuda 917, kill9 57, torch-conv 51 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 9260 | 1077.6 | 116.4 | 77.6 | gate-cuda 579, kill9 88, smi-q 76 | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 23 | host | 0xc9:0x0 | REGISTER_FD | 11261 | 910.9 | 80.9 | 1.9 | gate-cuda 841, query 25, torch-attention 16 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 24 | control | 0x2080a079 | (not in controls.tsv: GSS legacy or BINAPI) | 11 | 888.7 | 80794.1 | 0.0 | gate-nvml 678, smi-q 211 | - | host (GSS legacy, forwarded opaquely; command and size from a native host trace) |
| 25 | host | 0x2a:0x2080a079 | RM_CONTROL 0x2080a079 | 11 | 888.6 | 80779.4 | 0.0 | gate-nvml 678, smi-q 211 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | shadow | object free | object free | 7476 | 712.1 | 95.3 | 36.9 | gate-cuda 531, torch-conv 31, kill9 23 | shadow.rs (worker) | - |
| 27 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 6299 | 686.7 | 109.0 | 4.2 | gate-cuda 657, multiproc 9, streams 4 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 28 | access | bar0:R:0x110000:0x110804 | NV_PGSP (GSP falcon) | 9260 | 673.9 | 72.8 | 39.4 | gate-cuda 360, smi-q 53, kill9 41 | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 29 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 8868 | 625.0 | 70.5 | 39.6 | gate-cuda 426, kill9 30, vulkan 25 | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 30 | window | bar1 pool: page moved out, cold (units: bytes) | bar1 pool: page moved out, cold (units: bytes) | 1155 | 561.8 | 486.4 | 0.0 | gate-cuda 325, kill9 191, vulkan 45 | bar1pool.rs | - |
| 31 | access | bar0:R:0x110000:0x110808 | NV_PGSP (GSP falcon) | 9260 | 554.9 | 59.9 | 37.8 | gate-cuda 279, smi-q 49, kill9 33 | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 32 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 8868 | 539.9 | 60.9 | 36.0 | gate-cuda 376, kill9 24, torch-conv 13 | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 33 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 743 | 441.3 | 594.0 | 46.4 | gate-cuda 253, kill9 38, torch-matmul 21 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x52:0x0 | RM_GET_EVENT_DATA | 155612 | 430.2 | 2.8 | 30.6 | idle 287, gate-cuda 89, smi-dmon 12 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | shadow | unmap | unmap | 4640 | 420.2 | 90.6 | 36.5 | gate-cuda 274, torch-conv 36, kill9 13 | shadow.rs (worker) | - |
| 36 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 4640 | 417.6 | 90.0 | 36.3 | gate-cuda 272, torch-conv 36, kill9 13 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 150 | 358.6 | 2390.9 | 38.3 | gate-cuda 191, nvrtc 27, kill9 24 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | control | 0x20809004 | (not in controls.tsv: GSS legacy or BINAPI) | 142 | 320.8 | 2259.2 | 0.0 | smi-dmon 226, smi-q 86, gate-nvml 9 | - | host (GSS legacy, forwarded opaquely; command and size from a native host trace) |
| 39 | host | 0x2a:0x20809004 | RM_CONTROL 0x20809004 | 142 | 320.1 | 2254.1 | 0.0 | smi-dmon 225, smi-q 85, gate-nvml 9 | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 40 | access | cfg:R:0x0:0x4 | - | 3248 | 216.1 | 66.5 | 0.0 | gate-cuda 181, smi-q 29, gate-nvml 6 | config.rs | - |
