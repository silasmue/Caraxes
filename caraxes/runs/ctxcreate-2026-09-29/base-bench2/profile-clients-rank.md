# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-bench2/profile.1.tsv, scope clients, cost guest

1504 rows, 167968.2 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 270625 | 25273.85 | 93.4 | 2965.3 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 111561 | 15926.98 | 142.8 | 12674.1 | - | - | - | bar2area.rs | - |
| 3 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 15084.71 | 183.2 | 119607.8 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 4 | access | bar3:R:0xfea000:- | - | 167033 | 14987.44 | 89.7 | 12672.6 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 5 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 26333 | 9348.70 | 355.0 | 8731.3 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 6 | shadow | gather alloc | gather alloc | 3240 | 9008.25 | 2780.3 | 12889.2 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x29:0x0 | RM_FREE | 129273 | 8602.19 | 66.5 | 34300.7 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | rpc | 0xa | FREE | 9934 | 4999.57 | 503.3 | 6075.6 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 9 | shadow | map | map | 156083 | 4799.07 | 30.7 | 8850.3 | - | - | - | shadow.rs (worker) | - |
| 10 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 156083 | 4721.18 | 30.2 | 8849.3 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 11 | shadow | object alloc | object alloc | 116670 | 4440.95 | 38.1 | 6916.5 | - | - | - | shadow.rs (worker) | - |
| 12 | shadow | unmap | unmap | 149277 | 4407.34 | 29.5 | 4691.1 | - | - | - | shadow.rs (worker) | - |
| 13 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 116670 | 4393.53 | 37.7 | 6915.8 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 149277 | 4370.35 | 29.3 | 4690.7 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 3838.91 | 46.6 | 7890.8 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 16 | shadow | object free | object free | 115853 | 3736.83 | 32.3 | 9153.4 | - | - | - | shadow.rs (worker) | - |
| 17 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 2828 | 3695.20 | 1306.6 | 4104.3 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 18 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 3399.46 | 41.3 | 6975.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 19 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 72684 | 2523.76 | 34.7 | 1449.4 | - | - | - | bar1pool.rs | - |
| 20 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 6060 | 2467.46 | 407.2 | 263227.6 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 1128 | 1620.94 | 1437.0 | 4634.3 | 0x0=8 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 2820 | 1615.12 | 572.7 | 135993.2 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 23 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 72192 | 1384.57 | 19.2 | 15543.9 | - | - | - | bar1pool.rs | - |
| 24 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 26328 | 1377.43 | 52.3 | 82.5 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 25 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 192 | 1170.01 | 6093.8 | 42136.4 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 26333 | 1076.64 | 40.9 | 6798.1 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 27 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 26333 | 1051.60 | 39.9 | 58.5 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 28 | rpc | 0x4c | GSP_RM_CONTROL | 6313 | 751.07 | 119.0 | 3308.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 29 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 2828 | 627.26 | 221.8 | 1142.7 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 30 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 572 | 528.56 | 924.1 | 1957.1 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 2828 | 480.46 | 169.9 | 3118.7 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | shadow | gather free | gather free | 3239 | 378.97 | 117.0 | 2981.3 | - | - | - | shadow.rs (worker) | - |
| 33 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 5640 | 312.18 | 55.4 | 2.1 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 34 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 161 | 290.49 | 1804.3 | 1954.3 | 0x0=1 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 35 | access | cfg:R:0x0:0x4 | - | 4963 | 228.64 | 46.1 | 1276.4 | - | pci-config | - | config.rs | - |
| 36 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 629 | 224.07 | 356.2 | 3288.5 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 798 | 206.51 | 258.8 | 14276.8 | 0x0=9 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 149 | 188.49 | 1265.1 | 1374.0 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | rpc | 0x67 | GSP_RM_ALLOC | 10064 | 184.02 | 18.3 | 758.0 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 40 | access | cfg:R:0x0:0x68 | - | 9837 | 161.16 | 16.4 | 641.3 | - | pci-config | - | config.rs | - |
| 41 | access | cfg:R:0x0:0x60 | - | 9837 | 160.36 | 16.3 | 69.0 | - | pci-config | - | config.rs | - |
| 42 | access | bar0:W:0x70e000:- | NV_PRAMIN | 2880 | 152.49 | 52.9 | 30.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 43 | access | bar0:W:0x706000:- | NV_PRAMIN | 2880 | 141.69 | 49.2 | 98.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 44 | host | 0x2a:0xc36f0108 | RM_CONTROL NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN | 2828 | 136.27 | 48.2 | 3423.5 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | access | cfg:R:0x0:0x78 | - | 9837 | 131.17 | 13.3 | 2.1 | - | pci-config | - | config.rs | - |
| 46 | host | 0x4e:0x0 | RM_MAP_MEMORY | 816 | 121.64 | 149.1 | 431.0 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 602 | 113.41 | 188.4 | 489.3 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 48 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 161 | 111.65 | 693.5 | 735.7 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 49 | access | bar0:W:0x715000:- | NV_PRAMIN | 2880 | 109.73 | 38.1 | 11.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 50 | host | 0x2a:0x801714 | RM_CONTROL NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS | 161 | 106.03 | 658.6 | 759.1 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 51 | control | 0xa06c0105 | NVA06C_CTRL_CMD_PREEMPT | 268 | 104.38 | 389.5 | 3301.1 | 0x0=2 | C | echo | cuda.rs | host (forwarded to the mirror object; the guest sees NV_OK) |
| 52 | access | bar0:R:0xbb0000:0xbb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 2820 | 101.95 | 36.2 | 29.2 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 53 | access | cfg:R:0x0:0x34 | - | 4962 | 100.99 | 20.4 | 647.8 | - | pci-config | - | config.rs | - |
| 54 | access | bar0:W:0x702000:- | NV_PRAMIN | 2880 | 97.00 | 33.7 | 28.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 55 | access | bar0:W:0x70b000:- | NV_PRAMIN | 2880 | 96.94 | 33.7 | 8.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 56 | access | bar0:W:0x714000:- | NV_PRAMIN | 2880 | 96.31 | 33.4 | 14.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 57 | access | bar0:W:0x70d000:- | NV_PRAMIN | 3000 | 95.61 | 31.9 | 39.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 58 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 564 | 90.57 | 160.6 | 1119.9 | 0x0=4 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 59 | access | bar0:W:0x70f000:- | NV_PRAMIN | 2880 | 89.95 | 31.2 | 69.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:W:0x712000:- | NV_PRAMIN | 2880 | 88.89 | 30.9 | 6.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 1327 | 901240 | 80293.16 |
| host | 59 | 584763 | 34358.15 |
| shadow | 6 | 544362 | 26771.42 |
| window | 22 | 903956 | 19835.41 |
| rpc | 5 | 26332 | 5934.97 |
| control | 68 | 6391 | 727.70 |
| alloc | 17 | 10066 | 47.41 |
