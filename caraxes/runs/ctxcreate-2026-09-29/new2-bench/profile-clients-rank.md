# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new2-bench/profile.1.tsv, scope clients, cost guest

1510 rows, 174633.9 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 273251 | 22694.41 | 83.1 | 9688.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 109653 | 13494.16 | 123.1 | 20990.4 | - | - | - | bar2area.rs | - |
| 3 | window | bar2 memfd: sync scan (units: pages compared) | bar2 memfd: sync scan (units: pages compared) | 109653 | 13205.06 | 120.4 | 20989.4 | - | - | - | bar2area.rs | - |
| 4 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 13155.45 | 159.8 | 62979.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 5 | access | bar3:R:0xfea000:- | - | 167033 | 12443.64 | 74.5 | 21012.2 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 6 | shadow | gather alloc | gather alloc | 3261 | 9381.21 | 2876.8 | 18768.4 | - | - | - | shadow.rs (worker) | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 26333 | 8627.85 | 327.6 | 10568.9 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | host | 0x29:0x0 | RM_FREE | 129317 | 7915.63 | 61.2 | 21354.4 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 9 | shadow | map | map | 155954 | 4741.44 | 30.4 | 15317.9 | - | - | - | shadow.rs (worker) | - |
| 10 | window | gather: window of mremaps (units: runs) | gather: window of mremaps (units: runs) | 3261 | 4725.79 | 1449.2 | 7373.0 | - | - | - | - | - |
| 11 | rpc | 0xa | FREE | 9934 | 4688.62 | 472.0 | 9147.5 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 12 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 155954 | 4620.77 | 29.6 | 6587.8 | 0x0=40 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | shadow | unmap | unmap | 149295 | 4456.89 | 29.9 | 11938.4 | - | - | - | shadow.rs (worker) | - |
| 14 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 149295 | 4425.48 | 29.6 | 11936.8 | 0x0=13 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | shadow | object alloc | object alloc | 116670 | 4311.06 | 37.0 | 5364.8 | - | - | - | shadow.rs (worker) | - |
| 16 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 116670 | 4267.26 | 36.6 | 5364.4 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 17 | shadow | object free | object free | 115876 | 3387.45 | 29.2 | 3926.4 | - | - | - | shadow.rs (worker) | - |
| 18 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 2828 | 3294.69 | 1165.0 | 3777.6 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 3085.40 | 37.5 | 1949.2 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 20 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 2547.67 | 30.9 | 7463.5 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 21 | window | gather: window unmap (units: pages) | gather: window unmap (units: pages) | 3261 | 2395.48 | 734.6 | 10667.9 | - | - | - | - | - |
| 22 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 6081 | 2382.39 | 391.8 | 361600.7 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | window | gather: host descriptor (units: pages) | gather: host descriptor (units: pages) | 3261 | 2247.89 | 689.3 | 6819.6 | - | - | - | - | - |
| 24 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 1128 | 1930.59 | 1711.5 | 7141.5 | 0x0=8 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 2820 | 1576.72 | 559.1 | 89465.6 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 26 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 26328 | 1152.99 | 43.8 | 45.7 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 27 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 72192 | 1073.46 | 14.9 | 6628.7 | - | - | - | bar1pool.rs | - |
| 28 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 26333 | 1022.99 | 38.8 | 69.2 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 29 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 72684 | 985.02 | 13.6 | 3032.2 | - | - | - | bar1pool.rs | - |
| 30 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 26333 | 912.18 | 34.6 | 27.4 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 31 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 174 | 856.85 | 4924.4 | 11071.2 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 2828 | 821.54 | 290.5 | 9099.9 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | rpc | 0x4c | GSP_RM_CONTROL | 6313 | 818.01 | 129.6 | 5951.8 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 34 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 2828 | 508.00 | 179.6 | 3345.9 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 572 | 496.18 | 867.4 | 2374.6 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 161 | 326.69 | 2029.1 | 5945.9 | 0x0=1 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 37 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 5640 | 304.39 | 54.0 | 12.7 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 38 | shadow | gather free | gather free | 3260 | 266.38 | 81.7 | 2101.6 | - | - | - | shadow.rs (worker) | - |
| 39 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 618 | 254.21 | 411.3 | 4259.9 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 40 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 149 | 187.18 | 1256.2 | 1535.6 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | rpc | 0x67 | GSP_RM_ALLOC | 10064 | 175.45 | 17.4 | 2225.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 42 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 602 | 153.62 | 255.2 | 4365.1 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 43 | host | 0x2a:0xc36f0108 | RM_CONTROL NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN | 2828 | 136.34 | 48.2 | 4048.2 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | access | cfg:R:0x0:0x4 | - | 4151 | 133.65 | 32.2 | 11.1 | - | pci-config | - | config.rs | - |
| 45 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 781 | 128.15 | 164.1 | 5803.2 | 0x0=9 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 793 | 126.94 | 160.1 | 9032.0 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 161 | 124.25 | 771.7 | 2668.5 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 48 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 564 | 123.70 | 219.3 | 3436.6 | 0x0=4 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 49 | window | bar2 memfd: sync drift check | bar2 memfd: sync drift check | 2072 | 119.53 | 57.7 | 1913.4 | - | - | - | bar2area.rs | - |
| 50 | access | bar0:W:0x712000:- | NV_PRAMIN | 2880 | 117.44 | 40.8 | 20.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 51 | host | 0x4e:0x0 | RM_MAP_MEMORY | 799 | 116.12 | 145.3 | 508.8 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 52 | access | cfg:R:0x0:0x60 | - | 8227 | 115.21 | 14.0 | 0.5 | - | pci-config | - | config.rs | - |
| 53 | control | 0xa06c0105 | NVA06C_CTRL_CMD_PREEMPT | 268 | 113.96 | 425.2 | 4266.4 | 0x0=2 | C | echo | cuda.rs | host (forwarded to the mirror object; the guest sees NV_OK) |
| 54 | access | cfg:R:0x0:0x68 | - | 8227 | 113.83 | 13.8 | 0.4 | - | pci-config | - | config.rs | - |
| 55 | access | cfg:R:0x0:0x78 | - | 8227 | 110.13 | 13.4 | 144.7 | - | pci-config | - | config.rs | - |
| 56 | host | 0x2a:0x801714 | RM_CONTROL NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS | 161 | 101.08 | 627.8 | 828.4 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 57 | access | bar0:R:0xbb0000:0xbb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 2820 | 93.85 | 33.3 | 10.0 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 58 | window | bar2 memfd: sync table walk | bar2 memfd: sync table walk | 109653 | 87.50 | 0.8 | 979.2 | - | - | - | bar2area.rs | - |
| 59 | access | bar0:W:0x715000:- | NV_PRAMIN | 2880 | 86.36 | 30.0 | 16.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:W:0x70a000:- | NV_PRAMIN | 2880 | 85.84 | 29.8 | 65.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 1327 | 896684 | 69933.34 |
| window | 28 | 1128132 | 38334.01 |
| host | 59 | 580670 | 33287.86 |
| shadow | 6 | 544316 | 26544.43 |
| rpc | 5 | 26332 | 5682.28 |
| control | 68 | 6391 | 796.30 |
| alloc | 17 | 10066 | 55.72 |
