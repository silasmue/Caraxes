# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new-bench/profile.1.tsv, scope clients, cost guest

1510 rows, 196421.1 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 292519 | 25954.33 | 88.7 | 1952.2 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 110495 | 14460.21 | 130.9 | 21921.8 | - | - | - | bar2area.rs | - |
| 3 | window | bar2 memfd: sync scan (units: pages compared) | bar2 memfd: sync scan (units: pages compared) | 110495 | 14156.16 | 128.1 | 21919.0 | - | - | - | bar2area.rs | - |
| 4 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 13527.39 | 164.3 | 116064.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 5 | access | bar3:R:0xfea000:- | - | 167033 | 13324.06 | 79.8 | 21939.7 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 6 | shadow | gather alloc | gather alloc | 3267 | 10143.46 | 3104.8 | 16376.5 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x29:0x0 | RM_FREE | 129323 | 8955.70 | 69.3 | 32676.1 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 26333 | 8843.80 | 335.8 | 10759.7 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 9 | shadow | object alloc | object alloc | 116686 | 6051.77 | 51.9 | 14662.4 | - | - | - | shadow.rs (worker) | - |
| 10 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 116686 | 6010.41 | 51.5 | 14659.4 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 11 | shadow | map | map | 155811 | 5888.28 | 37.8 | 10303.3 | - | - | - | shadow.rs (worker) | - |
| 12 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 155811 | 5809.45 | 37.3 | 10302.6 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | shadow | unmap | unmap | 148844 | 5006.36 | 33.6 | 12691.2 | - | - | - | shadow.rs (worker) | - |
| 14 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 148844 | 4977.10 | 33.4 | 12690.3 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | rpc | 0xa | FREE | 9934 | 4809.87 | 484.2 | 9720.0 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 16 | window | gather: window of mremaps (units: runs) | gather: window of mremaps (units: runs) | 3267 | 4610.17 | 1411.1 | 5399.7 | - | - | - | - | - |
| 17 | shadow | object free | object free | 115876 | 4179.75 | 36.1 | 10048.4 | - | - | - | shadow.rs (worker) | - |
| 18 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 2828 | 3598.51 | 1272.5 | 10096.4 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 6087 | 3549.71 | 583.2 | 414717.8 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 3298.82 | 40.1 | 2378.1 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 21 | window | gather: host descriptor (units: pages) | gather: host descriptor (units: pages) | 3267 | 3250.36 | 994.9 | 14046.6 | - | - | - | - | - |
| 22 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 2826.24 | 34.3 | 2655.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 23 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 72684 | 2787.82 | 38.4 | 9453.0 | - | - | - | bar1pool.rs | - |
| 24 | window | gather: window unmap (units: pages) | gather: window unmap (units: pages) | 3267 | 2271.34 | 695.2 | 4099.4 | - | - | - | - | - |
| 25 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 1128 | 1878.98 | 1665.8 | 3871.2 | 0x0=8 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 2820 | 1723.59 | 611.2 | 204893.0 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 27 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 26328 | 1158.84 | 44.0 | 43.5 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 28 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 72192 | 1145.54 | 15.9 | 14873.1 | - | - | - | bar1pool.rs | - |
| 29 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 26333 | 995.95 | 37.8 | 81.2 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 30 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 184 | 835.52 | 4540.9 | 9542.7 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 2828 | 828.70 | 293.0 | 4436.6 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 26333 | 806.88 | 30.6 | 1514.6 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 33 | rpc | 0x4c | GSP_RM_CONTROL | 6313 | 779.40 | 123.5 | 9416.6 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 34 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 2828 | 657.97 | 232.7 | 4223.7 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 572 | 534.80 | 935.0 | 3316.5 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | shadow | gather free | gather free | 3266 | 419.37 | 128.4 | 3719.0 | - | - | - | shadow.rs (worker) | - |
| 37 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 161 | 294.50 | 1829.2 | 1927.6 | 0x0=1 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 38 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 5640 | 291.30 | 51.6 | 13.8 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 39 | host | 0x4e:0x0 | RM_MAP_MEMORY | 794 | 288.31 | 363.1 | 10590.7 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 40 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 776 | 277.95 | 358.2 | 9893.0 | 0x0=9 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 643 | 249.95 | 388.7 | 3416.8 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 42 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 149 | 239.01 | 1604.1 | 4637.9 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 43 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 564 | 206.49 | 366.1 | 7206.4 | 0x0=4 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | access | cfg:R:0x0:0x4 | - | 4499 | 166.67 | 37.0 | 11.9 | - | pci-config | - | config.rs | - |
| 45 | host | 0xc9:0x0 | REGISTER_FD | 794 | 157.54 | 198.4 | 10231.8 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | rpc | 0x67 | GSP_RM_ALLOC | 10064 | 154.02 | 15.3 | 534.7 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 47 | host | 0x2a:0xc36f0108 | RM_CONTROL NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN | 2828 | 145.13 | 51.3 | 1962.4 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 48 | access | cfg:R:0x0:0x78 | - | 8917 | 143.69 | 16.1 | 36.5 | - | pci-config | - | config.rs | - |
| 49 | access | cfg:R:0x0:0x60 | - | 8917 | 133.03 | 14.9 | 78.3 | - | pci-config | - | config.rs | - |
| 50 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 844 | 131.79 | 156.1 | 3396.9 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 51 | access | cfg:R:0x0:0x68 | - | 8917 | 122.11 | 13.7 | 1.0 | - | pci-config | - | config.rs | - |
| 52 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 161 | 121.18 | 752.7 | 821.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 53 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 602 | 117.67 | 195.5 | 589.1 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 54 | window | bar2 memfd: sync drift check | bar2 memfd: sync drift check | 2230 | 112.80 | 50.6 | 3079.2 | - | - | - | bar2area.rs | - |
| 55 | window | bar2 memfd: sync table walk | bar2 memfd: sync table walk | 110495 | 106.10 | 1.0 | 1230.3 | - | - | - | bar2area.rs | - |
| 56 | host | 0x2a:0x801714 | RM_CONTROL NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS | 161 | 105.16 | 653.2 | 699.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 57 | access | bar0:W:0x702000:- | NV_PRAMIN | 2880 | 104.86 | 36.4 | 38.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 58 | access | bar0:R:0xbb0000:0xbb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 2820 | 92.98 | 33.0 | 14.7 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 59 | access | bar0:W:0x715000:- | NV_PRAMIN | 2880 | 91.48 | 31.8 | 20.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:W:0x712000:- | NV_PRAMIN | 2880 | 88.32 | 30.7 | 29.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 1327 | 919030 | 75283.19 |
| window | 28 | 1126166 | 42900.59 |
| host | 59 | 579587 | 40007.26 |
| shadow | 6 | 543750 | 31689.00 |
| rpc | 5 | 26332 | 5743.48 |
| control | 68 | 6391 | 757.13 |
| alloc | 17 | 10066 | 40.45 |
