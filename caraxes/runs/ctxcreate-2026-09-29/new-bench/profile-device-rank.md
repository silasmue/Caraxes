# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new-bench/profile.1.tsv, scope device, cost guest

3746 rows, 30423.5 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 37610 | 3314.94 | 88.1 | 1952.2 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 872 | 2446.11 | 2805.2 | 414717.8 | 0x0=872 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 14700 | 1978.58 | 134.6 | 21921.8 | - | - | - | bar2area.rs | - |
| 4 | window | bar2 memfd: sync scan (units: pages compared) | bar2 memfd: sync scan (units: pages compared) | 14700 | 1936.12 | 131.7 | 21919.0 | - | - | - | bar2area.rs | - |
| 5 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 1805.23 | 168.8 | 116064.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 6 | access | bar3:R:0xfea000:- | - | 22069 | 1767.39 | 80.1 | 21939.7 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 7 | shadow | gather alloc | gather alloc | 464 | 1443.01 | 3109.9 | 16376.5 | - | - | - | shadow.rs (worker) | - |
| 8 | host | 0x29:0x0 | RM_FREE | 16628 | 1297.69 | 78.0 | 32676.1 | 0x0=16628 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 9 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 3762 | 1254.36 | 333.4 | 10759.7 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 10 | shadow | object alloc | object alloc | 14701 | 786.05 | 53.5 | 14662.4 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14701 | 780.74 | 53.1 | 14659.4 | 0x0=14701 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | shadow | map | map | 19615 | 752.57 | 38.4 | 10303.3 | - | - | - | shadow.rs (worker) | - |
| 13 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19615 | 742.61 | 37.9 | 10302.6 | 0x0=19615 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | rpc | 0xa | FREE | 1405 | 687.09 | 489.0 | 9720.0 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 15 | window | gather: window of mremaps (units: runs) | gather: window of mremaps (units: runs) | 464 | 655.59 | 1412.9 | 5399.7 | - | - | - | - | - |
| 16 | shadow | unmap | unmap | 18706 | 631.48 | 33.8 | 12691.2 | - | - | - | shadow.rs (worker) | - |
| 17 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18706 | 627.79 | 33.6 | 12690.3 | 0x0=18706 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 18 | shadow | object free | object free | 14721 | 538.23 | 36.6 | 10048.4 | - | - | - | shadow.rs (worker) | - |
| 19 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 401 | 510.14 | 1272.2 | 10096.4 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | window | gather: host descriptor (units: pages) | gather: host descriptor (units: pages) | 464 | 462.73 | 997.3 | 14046.6 | - | - | - | - | - |
| 21 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 428.46 | 40.1 | 2378.1 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 22 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 10239 | 396.42 | 38.7 | 9453.0 | - | - | - | bar1pool.rs | - |
| 23 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 366.74 | 34.3 | 2655.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 24 | window | gather: window unmap (units: pages) | gather: window unmap (units: pages) | 464 | 323.04 | 696.2 | 4099.4 | - | - | - | - | - |
| 25 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 160 | 266.78 | 1667.3 | 3871.2 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | access | bar0:W:0x840000:0x840184 | NV_PSEC (SEC2 falcon) | 8512 | 245.35 | 28.8 | 996.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 27 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 400 | 244.44 | 611.1 | 204893.0 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 28 | access | bar0:W:0x840000:0x8401c4 | NV_PSEC (SEC2 falcon) | 6272 | 169.46 | 27.0 | 14.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 29 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 3760 | 165.70 | 44.1 | 43.5 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 30 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 10240 | 162.30 | 15.8 | 14873.1 | - | - | - | bar1pool.rs | - |
| 31 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 3762 | 142.23 | 37.8 | 81.2 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 32 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 401 | 117.63 | 293.3 | 4436.6 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 3761 | 115.60 | 30.7 | 1514.6 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 34 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 24 | 109.71 | 4571.4 | 9542.7 | 0x0=24 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | rpc | 0x4c | GSP_RM_CONTROL | 919 | 106.14 | 115.5 | 9416.6 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 36 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 401 | 93.45 | 233.0 | 4223.7 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 81 | 75.93 | 937.5 | 3316.5 | 0x0=81 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | shadow | gather free | gather free | 464 | 59.61 | 128.5 | 3719.0 | - | - | - | shadow.rs (worker) | - |
| 39 | access | bar0:R:0x303000:- | NV_PROM_DATA (VBIOS window) | 960 | 58.20 | 60.6 | 14.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 40 | access | bar0:R:0x305000:- | NV_PROM_DATA (VBIOS window) | 960 | 53.15 | 55.4 | 17.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 41 | access | bar0:R:0x30d000:- | NV_PROM_DATA (VBIOS window) | 960 | 51.66 | 53.8 | 2.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 42 | access | bar0:R:0x302000:- | NV_PROM_DATA (VBIOS window) | 960 | 48.55 | 50.6 | 13.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 43 | host | 0x4e:0x0 | RM_MAP_MEMORY | 113 | 41.49 | 367.1 | 10590.7 | 0x0=113 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 800 | 41.40 | 51.7 | 13.8 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 45 | access | bar0:R:0x30e000:- | NV_PROM_DATA (VBIOS window) | 960 | 39.75 | 41.4 | 15.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 46 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 109 | 39.46 | 362.0 | 9893.0 | 0x0=109 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | access | bar0:R:0x31c000:- | NV_PROM_DATA (VBIOS window) | 960 | 38.48 | 40.1 | 12.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 48 | access | cfg:R:0x0:0x4 | - | 917 | 38.19 | 41.6 | 11.9 | - | pci-config | - | config.rs | - |
| 49 | access | bar0:R:0x300000:- | NV_PROM_DATA (VBIOS window) | 960 | 37.03 | 38.6 | 2.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 50 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 20 | 36.59 | 1829.4 | 1927.6 | 0x0=20 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 51 | access | bar0:R:0x310000:- | NV_PROM_DATA (VBIOS window) | 960 | 36.07 | 37.6 | 2.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 52 | access | bar0:R:0x312000:- | NV_PROM_DATA (VBIOS window) | 960 | 35.58 | 37.1 | 2.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 53 | access | bar0:R:0x304000:- | NV_PROM_DATA (VBIOS window) | 960 | 35.47 | 36.9 | 2.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 54 | access | bar0:R:0x301000:- | NV_PROM_DATA (VBIOS window) | 960 | 35.26 | 36.7 | 5.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 55 | access | bar0:W:0x702000:- | NV_PRAMIN | 960 | 34.95 | 36.4 | 38.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 56 | access | bar0:R:0x30b000:- | NV_PROM_DATA (VBIOS window) | 960 | 34.36 | 35.8 | 20.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 57 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 87 | 33.89 | 389.6 | 3416.8 | 0x0=87 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 58 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 21 | 33.89 | 1613.9 | 4637.9 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 59 | access | bar0:R:0x31d000:- | NV_PROM_DATA (VBIOS window) | 960 | 33.60 | 35.0 | 15.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 60 | access | bar0:R:0x309000:- | NV_PROM_DATA (VBIOS window) | 960 | 32.18 | 33.5 | 1.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 61 | access | bar0:R:0x313000:- | NV_PROM_DATA (VBIOS window) | 960 | 31.02 | 32.3 | 46.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 62 | access | bar0:R:0x308000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.74 | 32.0 | 8.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 63 | access | bar0:R:0x306000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.60 | 31.9 | 1.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 64 | access | bar0:R:0x30a000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.51 | 31.8 | 2.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 65 | access | bar0:W:0x715000:- | NV_PRAMIN | 960 | 30.49 | 31.8 | 20.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 66 | access | bar0:R:0x30f000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.40 | 31.7 | 1.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 67 | access | bar0:R:0x311000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.27 | 31.5 | 1.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 68 | access | bar0:R:0x315000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.79 | 31.0 | 1.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 69 | access | bar0:R:0x30c000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.71 | 30.9 | 1.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 70 | access | bar0:R:0x316000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.54 | 30.8 | 1.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 71 | access | bar0:R:0x314000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.47 | 30.7 | 1.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 72 | access | bar0:W:0x712000:- | NV_PRAMIN | 960 | 29.44 | 30.7 | 29.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 73 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 80 | 29.40 | 367.5 | 7206.4 | 0x0=80 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 74 | access | cfg:R:0x0:0x78 | - | 1768 | 28.92 | 16.4 | 36.5 | - | pci-config | - | config.rs | - |
| 75 | access | bar0:R:0x307000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.79 | 30.0 | 1.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 76 | access | bar0:W:0x70d000:- | NV_PRAMIN | 1000 | 28.10 | 28.1 | 18.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 77 | access | bar0:W:0x706000:- | NV_PRAMIN | 960 | 27.76 | 28.9 | 9.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 78 | access | bar0:W:0x711000:- | NV_PRAMIN | 960 | 27.35 | 28.5 | 8.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 79 | access | bar0:W:0x707000:- | NV_PRAMIN | 960 | 27.16 | 28.3 | 20.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 80 | access | cfg:R:0x0:0x60 | - | 1770 | 26.94 | 15.2 | 78.3 | - | pci-config | - | config.rs | - |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 3554 | 182651 | 11938.42 |
| host | 66 | 73816 | 7404.34 |
| window | 28 | 150779 | 5945.94 |
| shadow | 6 | 68671 | 4210.95 |
| rpc | 7 | 3760 | 816.05 |
| control | 68 | 919 | 102.05 |
| alloc | 17 | 1430 | 5.78 |
