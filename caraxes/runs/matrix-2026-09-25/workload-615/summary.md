<!-- SPDX-License-Identifier: MIT -->
# Workload gate workload-615

Written by caraxes/runs/workload-2026-09-24/run.sh; how to read it: docs/caraxes/workload-gate.md.

```
# workload gate 2026-09-25T05:09:24+02:00 targets=[v2] reps=3 full=0 metrics=[short-auto rl ppo-full llama llama-nograph transformer convnet]
# host driver 615.71.09
NVRM version: NVIDIA UNIX Open Kernel Module for x86_64  615.71.09  Release Build  (root@)  
# llama llama.cpp e9fa0781f1c25fc4fe8c86be1edc6970661ad6f0 (/mnt/vmstore/caraxes/llama/src)
# llama nvcc Build cuda_13.3.r13.3/compiler.38244171_0
# llama host compiler g++-15 (GCC) 15.3.0
# llama GGML_CUDA_NO_VMM=OFF CMAKE_CUDA_ARCHITECTURES=75-real built 2026-09-24T15:51:52+02:00
# llama 147765a7c77ffd27308a5e7cef96a9c321d3d4220a66f95ec47d7286e2e9e5c3  llama-bench
# llama 01dd70979e68625846e9a5edcba531d3798498b866aa1c925cc25d60f63fe680  lib/libcublas.so.13
# llama 93c275af70b6d39fbd9b78cbc925ad9a9a0a81189aaec1521a951119ebe72da3  lib/libcublasLt.so.13
# llama 9f92e4c3f7e14d0fed69ef876a45d4255a2dc88cb552a673a90ce881c26282fe  lib/libcudart.so.13
# model /mnt/vmstore/caraxes/models/Llama-3.2-1B-Instruct-Q4_K_M.gguf sha256 6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83
38ebc3575d731172ba38ed28e815fcdf9fd66ef8a8912bed9b0e7a0d4165cf59  /mnt/vmstore/caraxes/vfio-user-nvgpu-v615-fbe4dfb
bac378f31ff769d5a89cf33f1a6ba077381ed500443d5905e24957a4e79fccc5  /mnt/vmstore/caraxes/vm1/cloud-hypervisor-0006
CARAXES_ALLOC_LAZY=1
CARAXES_BAR1_PASSTHROUGH=pool
CARAXES_BAR1_POOL_MIB=32
CARAXES_BAR2_MEMFD=1
CARAXES_BOOT_MODEL=1
CARAXES_DMA=register
CARAXES_DOORBELL_POLL=hybrid
CARAXES_DOORBELL_TOKENS=64
CARAXES_DRIVER=615
CARAXES_FB_MIB=2048
CARAXES_GATE_FB_MIB=2048
CARAXES_HOST_EXEC=1
CARAXES_HOST_RM=1
CARAXES_INTR_PAGE=1
CARAXES_IP=192.168.102.22
CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test
CARAXES_MAC=02:00:00:00:00:22
CARAXES_MIRROR_CHANNELS=1
CARAXES_MIRROR_GR_SHADOW=1
CARAXES_MSI_REARM_IOEVENTFD=1
CARAXES_RESERVE_SLOT=0
CARAXES_RESERVE_VRAM=1
CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-fbe4dfb
CARAXES_TAP=caraxes2
LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
374dfa69b4975a688c5d514b43a1bda1a3aafcf9
BENCH_TIMEOUT=900
LLAMA_REPS=5
PPO_FULL_SECONDS=420
WORKLOAD_SECONDS=60
# targets now [v2] metrics [short-auto rl ppo-full llama llama-nograph transformer convnet]
host watch: 435 samples, nvidia-smi max 0.168s mean 0.023s, 1 kernel log lines, 1 Xid lines, 0 alerts
host Xid lines since 2026-09-25 05:09:24: 1
```

## Per-run result lines

```
v2/convnet-1: no result line, # WALL wall_ms=4498 rc=134
v2/convnet-2: # CONVNET images_per_s=524.45 step_med_ms=60.891 step_p10_ms=59.203 step_p90_ms=63.024 steps=984 first_step_ms=2574.9 loss_first=7.0050 loss_last=0.0030 amp=0 channels_last=0 batch=32 size=224 mem_peak_mib=1459 oom_retries=0 finite=1 device=Leandro_VFIO_RTX_2070-2G
v2/convnet-3: # CONVNET images_per_s=528.43 step_med_ms=60.401 step_p10_ms=58.689 step_p90_ms=62.636 steps=991 first_step_ms=2536.0 loss_first=7.0050 loss_last=0.0031 amp=0 channels_last=0 batch=32 size=224 mem_peak_mib=1459 oom_retries=0 finite=1 device=Leandro_VFIO_RTX_2070-2G
v2/llama-1: # LLAMA pp512_tps=8149.83 pp512_sd=1397.54 tg128_tps=272.09 tg128_sd=8.65 reps=5 ngl=99 fa=-1 model=llama_1B_Q4_K_-_Medium model_mib=763 wall_s=4.5 build=e9fa0781f libs=dist gpu=Leandro_VFIO_RTX_2070-2G
v2/llama-2: # LLAMA pp512_tps=8747.46 pp512_sd=574.26 tg128_tps=254.19 tg128_sd=21.87 reps=5 ngl=99 fa=-1 model=llama_1B_Q4_K_-_Medium model_mib=763 wall_s=5.9 build=e9fa0781f libs=dist gpu=Leandro_VFIO_RTX_2070-2G
v2/llama-3: # LLAMA pp512_tps=7816.37 pp512_sd=1522.28 tg128_tps=282.74 tg128_sd=8.82 reps=5 ngl=99 fa=-1 model=llama_1B_Q4_K_-_Medium model_mib=763 wall_s=5.3 build=e9fa0781f libs=dist gpu=Leandro_VFIO_RTX_2070-2G
v2/llama-nograph-1: # LLAMA tg128_tps=256.64 tg128_sd=8.56 reps=5 ngl=99 fa=-1 model=llama_1B_Q4_K_-_Medium model_mib=763 wall_s=3.6 build=e9fa0781f libs=dist gpu=Leandro_VFIO_RTX_2070-2G
v2/llama-nograph-2: # LLAMA tg128_tps=235.39 tg128_sd=38.91 reps=5 ngl=99 fa=-1 model=llama_1B_Q4_K_-_Medium model_mib=763 wall_s=4.7 build=e9fa0781f libs=dist gpu=Leandro_VFIO_RTX_2070-2G
v2/llama-nograph-3: # LLAMA tg128_tps=257.69 tg128_sd=12.87 reps=5 ngl=99 fa=-1 model=llama_1B_Q4_K_-_Medium model_mib=763 wall_s=3.6 build=e9fa0781f libs=dist gpu=Leandro_VFIO_RTX_2070-2G
v2/ppo-full-1: # PPO steps_per_s=74756 solve_s=135.34 episodes=103136 reward100=500.0 envs=128 rollout=256
v2/ppo-full-2: # PPO steps_per_s=74714 solve_s=237.95 episodes=103061 reward100=500.0 envs=128 rollout=256
v2/ppo-full-3: # PPO steps_per_s=72869 solve_s=251.93 episodes=101490 reward100=499.4 envs=128 rollout=256
v2/rl-1: stage4: 50 episodes in 1.8s, mean10=56.9
v2/rl-2: stage4: 50 episodes in 2.1s, mean10=56.9
v2/rl-3: stage4: 50 episodes in 1.8s, mean10=56.9
v2/short-auto-1: MSPERLAUNCH=0.016
v2/short-auto-2: MSPERLAUNCH=0.016
v2/short-auto-3: MSPERLAUNCH=0.016
v2/transformer-1: # TRANSFORMER steps_per_s=12.787 step_med_ms=78.257 step_p10_ms=76.488 step_p90_ms=79.902 tokens_per_s=104754 steps=768 first_step_ms=446.8 loss_first=8.3697 loss_last=0.0222 amp=0 batch=32 seq=256 layers=6 dmodel=256 params_m=5.85 mem_peak_mib=1382 oom_retries=0 finite=1 device=Leandro_VFIO_RTX_2070-2G
v2/transformer-2: # TRANSFORMER steps_per_s=12.797 step_med_ms=78.215 step_p10_ms=76.347 step_p90_ms=79.743 tokens_per_s=104831 steps=768 first_step_ms=683.5 loss_first=8.3697 loss_last=0.0222 amp=0 batch=32 seq=256 layers=6 dmodel=256 params_m=5.85 mem_peak_mib=1382 oom_retries=0 finite=1 device=Leandro_VFIO_RTX_2070-2G
v2/transformer-3: # TRANSFORMER steps_per_s=12.785 step_med_ms=78.166 step_p10_ms=76.445 step_p90_ms=79.890 tokens_per_s=104732 steps=768 first_step_ms=647.4 loss_first=8.3697 loss_last=0.0222 amp=0 batch=32 seq=256 layers=6 dmodel=256 params_m=5.85 mem_peak_mib=1382 oom_retries=0 finite=1 device=Leandro_VFIO_RTX_2070-2G
```

## Tables

Sources: v2=/home/silas/git/Caraxes/.claude/worktrees/agent-ad0bd7a6ec1b6cc05/caraxes/runs/matrix-2026-09-25/workload-615/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 0/21
Host 1-min load average at run start, median [min-max]: v2 4.34 [1.63-5.61] n=21

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 | v2 / None |
|---|---|---|---|
| ms/launch | ms (low is better) | 0.016 [0.016-0.016] n=3 | - |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 | v2 / None |
|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=3 | - |
| process wall (incl. torch import) | s (low is better) | 7.62 [7.54-7.75] n=3 | - |
| stage4 (50 training episodes) | s (low is better) | 1.80 [1.80-2.10] n=3 | - |
| H2D+D2H transfers per second | 1/s (high is better) | 1556 [1354-1566] n=3 | - |

### (W1) PPO CartPole, full length (pporl, 420 s)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| environment steps per second | 1/s (high is better) | 74714 [72869-74756] n=3 | - |
| time to solved (mean100 >= 475) | s (low is better) | 238 [135-252] n=3 | - |
| reward100 at the end (not a time) |  | 500 [499-500] n=3 | - |

### (W2) llama.cpp inference, all layers on the GPU (llama-bench: pp = prompt processing, tg = token generation)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| pp512 (prompt processing, 512 tokens) | tokens/s (high is better) | 8150 [7816-8747] n=3 | - |
| tg128 (token generation, 128 tokens) | tokens/s (high is better) | 272 [254-283] n=3 | - |
| llama-bench process wall (load, warm-up, all reps) | s (low is better) | 5.30 [4.50-5.90] n=3 | - |

### (W2) llama.cpp token generation with CUDA graphs off (GGML_CUDA_DISABLE_GRAPHS=1): one launch per kernel

| value | unit | v2 | v2 / None |
|---|---|---|---|
| tg128 (token generation, 128 tokens) | tokens/s (high is better) | 257 [235-258] n=3 | - |
| llama-bench process wall (load, warm-up, all reps) | s (low is better) | 3.60 [3.60-4.70] n=3 | - |

### (W3) GPT-style training loop, 6 layers, d_model 256, AdamW (transformer.py)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| training steps per second (batch 32, seq 256, amp 0) | 1/s (high is better) | 12.8 [12.8-12.8] n=3 | - |
| median step time | ms (low is better) | 78.2 [78.2-78.3] n=3 | - |
| p90 step time | ms (low is better) | 79.9 [79.7-79.9] n=3 | - |
| tokens per second | 1/s (high is better) | 104754 [104732-104831] n=3 | - |
| first step (cuBLAS setup, allocations) | ms (low is better) | 647 [447-684] n=3 | - |
| peak allocated memory | MiB | 1382 [1382-1382] n=3 | - |
| loss at the end (not a time) |  | 0.022 [0.022-0.022] n=3 | - |

### (W4) ResNet-18 training, synthetic 224 px images, cuDNN (convnet.py)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| images per second (batch 32, 224 px, amp 0) | 1/s (high is better) | 526 [524-528] n=2 | - |
| median step time | ms (low is better) | 60.6 [60.4-60.9] n=2 | - |
| p90 step time | ms (low is better) | 62.8 [62.6-63.0] n=2 | - |
| first step (cuDNN autotuning) | ms (low is better) | 2555 [2536-2575] n=2 | - |
| peak allocated memory | MiB | 1459 [1459-1459] n=2 | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 | v2 / None |
|---|---|---|---|
| convnet device CPU | ms (low is better) | 12670 [710-13220] n=3 | - |
| convnet VMM CPU | ms (low is better) | 131060 [8750-131830] n=3 | - |
| llama device CPU | ms (low is better) | 1530 [1430-1590] n=3 | - |
| llama VMM CPU | ms (low is better) | 19940 [17770-20740] n=3 | - |
| llama-nograph device CPU | ms (low is better) | 1960 [1930-2300] n=3 | - |
| llama-nograph VMM CPU | ms (low is better) | 15040 [15030-17690] n=3 | - |
| ppo-full device CPU | ms (low is better) | 403190 [403020-407420] n=3 | - |
| ppo-full VMM CPU | ms (low is better) | 833980 [833730-837130] n=3 | - |
| rl device CPU | ms (low is better) | 2840 [2810-3050] n=3 | - |
| rl VMM CPU | ms (low is better) | 13460 [12510-15060] n=3 | - |
| short-auto device CPU | ms (low is better) | 530 [490-560] n=3 | - |
| short-auto VMM CPU | ms (low is better) | 430 [410-1140] n=3 | - |
| transformer device CPU | ms (low is better) | 11430 [11410-11480] n=3 | - |
| transformer VMM CPU | ms (low is better) | 128080 [127600-129780] n=3 | - |

### (8) Device process memory (over all runs)

| value | unit | v2 | v2 / None |
|---|---|---|---|
| device HWM | MiB (low is better) | 9224 [9224-9224] n=21 | - |
| device RSS after run | MiB (low is better) | 5137 [5133-5139] n=21 | - |

