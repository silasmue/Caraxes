# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new2-census-user/profile.1.tsv, scope clients, cost guest

1510 rows, 149903.5 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 255084 | 21670.91 | 85.0 | 2410.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 11947.45 | 155.6 | 19806.2 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 98521 | 11578.36 | 117.5 | 11460.3 | - | - | - | bar2area.rs | - |
| 4 | window | bar2 memfd: sync scan (units: pages compared) | bar2 memfd: sync scan (units: pages compared) | 98521 | 11335.59 | 115.1 | 11457.5 | - | - | - | bar2area.rs | - |
| 5 | access | bar3:R:0xfea000:- | - | 151055 | 11325.61 | 75.0 | 11470.5 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 6 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 20351 | 6775.69 | 332.9 | 20343.2 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 7 | host | 0x29:0x0 | RM_FREE | 122770 | 6484.17 | 52.8 | 31909.0 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | shadow | gather alloc | gather alloc | 2247 | 6254.46 | 2783.5 | 10830.1 | - | - | - | shadow.rs (worker) | - |
| 9 | shadow | map | map | 152701 | 4398.50 | 28.8 | 3717.2 | - | - | - | shadow.rs (worker) | - |
| 10 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 152701 | 4350.98 | 28.5 | 3716.3 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 11 | shadow | object alloc | object alloc | 113501 | 4201.14 | 37.0 | 5189.2 | - | - | - | shadow.rs (worker) | - |
| 12 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 113501 | 4160.49 | 36.7 | 5188.4 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | shadow | unmap | unmap | 147039 | 4116.46 | 28.0 | 6577.2 | - | - | - | shadow.rs (worker) | - |
| 14 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 147039 | 4086.03 | 27.8 | 6576.6 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | shadow | object free | object free | 112862 | 3339.08 | 29.6 | 3477.6 | - | - | - | shadow.rs (worker) | - |
| 16 | rpc | 0xa | FREE | 7547 | 3287.97 | 435.7 | 2517.8 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 17 | window | gather: window of mremaps (units: runs) | gather: window of mremaps (units: runs) | 2247 | 3173.02 | 1412.1 | 5239.6 | - | - | - | - | - |
| 18 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 3125.65 | 40.7 | 2421.7 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 19 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 2946.06 | 38.4 | 2114.7 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 20 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 2128 | 2818.05 | 1324.3 | 4381.2 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | window | gather: window unmap (units: pages) | gather: window unmap (units: pages) | 2247 | 1605.09 | 714.3 | 4437.7 | - | - | - | - | - |
| 22 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 4367 | 1547.44 | 354.3 | 348212.6 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | window | gather: host descriptor (units: pages) | gather: host descriptor (units: pages) | 2247 | 1468.39 | 653.5 | 3752.3 | - | - | - | - | - |
| 24 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 2120 | 1214.15 | 572.7 | 76415.0 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 25 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 848 | 1071.42 | 1263.5 | 3573.0 | 0x0=8 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 20346 | 964.83 | 47.4 | 85.7 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 27 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 54272 | 790.07 | 14.6 | 2026.5 | - | - | - | bar1pool.rs | - |
| 28 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 20351 | 780.98 | 38.4 | 8919.9 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 29 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 54771 | 761.66 | 13.9 | 2704.8 | - | - | - | bar1pool.rs | - |
| 30 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 20351 | 743.27 | 36.5 | 71.8 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 31 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 182 | 553.93 | 3043.6 | 8048.9 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | rpc | 0x4c | GSP_RM_CONTROL | 5117 | 501.65 | 98.0 | 1819.0 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 33 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 2128 | 494.06 | 232.2 | 5005.1 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | access | bar0:W:0x1000:0x1478 | NV_PBUS_SW_SCRATCH(i) | 4526 | 474.12 | 104.8 | 344.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PBUS_SW_SCRATCH(i) |
| 35 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 432 | 352.59 | 816.2 | 2035.8 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 2128 | 316.02 | 148.5 | 949.3 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 4240 | 257.91 | 60.8 | 59.9 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 38 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 121 | 204.34 | 1688.8 | 1814.2 | 0x0=1 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 39 | shadow | gather free | gather free | 2247 | 198.58 | 88.4 | 4112.7 | - | - | - | shadow.rs (worker) | - |
| 40 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 573 | 193.45 | 337.6 | 2364.4 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | access | cfg:R:0x0:0x4 | - | 4151 | 167.75 | 40.4 | 48.1 | - | pci-config | - | config.rs | - |
| 42 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 424 | 147.70 | 348.4 | 3618.3 | 0x0=4 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 43 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 114 | 140.45 | 1232.0 | 1333.2 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | rpc | 0x67 | GSP_RM_ALLOC | 7665 | 127.48 | 16.6 | 1872.8 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 45 | access | cfg:R:0x0:0x60 | - | 8227 | 110.81 | 13.5 | 19.9 | - | pci-config | - | config.rs | - |
| 46 | access | cfg:R:0x0:0x68 | - | 8227 | 110.39 | 13.4 | 3711.8 | - | pci-config | - | config.rs | - |
| 47 | access | cfg:R:0x0:0x78 | - | 8227 | 106.35 | 12.9 | 20.4 | - | pci-config | - | config.rs | - |
| 48 | window | bar2 memfd: sync drift check | bar2 memfd: sync drift check | 1917 | 93.07 | 48.5 | 995.6 | - | - | - | bar2area.rs | - |
| 49 | access | bar0:W:0x713000:- | NV_PRAMIN | 2880 | 93.05 | 32.3 | 18.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 50 | access | bar0:W:0x712000:- | NV_PRAMIN | 2880 | 89.39 | 31.0 | 21.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 51 | access | bar0:W:0x711000:- | NV_PRAMIN | 2880 | 89.26 | 31.0 | 109.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 52 | access | bar0:W:0x710000:- | NV_PRAMIN | 2880 | 89.09 | 30.9 | 21.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 53 | access | bar0:W:0x70d000:- | NV_PRAMIN | 3000 | 89.03 | 29.7 | 24.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 54 | access | bar0:W:0x70e000:- | NV_PRAMIN | 2880 | 88.82 | 30.8 | 13.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 55 | access | bar0:R:0xbb0000:0xbb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 2120 | 87.84 | 41.4 | 60.8 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 56 | window | bar2 memfd: sync table walk | bar2 memfd: sync table walk | 98521 | 87.80 | 0.9 | 970.4 | - | - | - | bar2area.rs | - |
| 57 | host | 0x4e:0x0 | RM_MAP_MEMORY | 620 | 87.77 | 141.6 | 581.8 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 58 | access | bar0:W:0x70f000:- | NV_PRAMIN | 2880 | 87.06 | 30.2 | 43.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 59 | access | bar0:W:0x714000:- | NV_PRAMIN | 2880 | 85.01 | 29.5 | 15.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:W:0x715000:- | NV_PRAMIN | 2880 | 84.44 | 29.3 | 15.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 1328 | 823256 | 64671.77 |
| window | 27 | 980013 | 30893.05 |
| host | 59 | 558164 | 27385.98 |
| shadow | 6 | 530597 | 22508.23 |
| rpc | 5 | 20350 | 3917.26 |
| control | 68 | 5195 | 486.30 |
| alloc | 17 | 7674 | 40.95 |
