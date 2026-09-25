<!-- SPDX-License-Identifier: MIT -->
# Workload gate gate

Written by caraxes/runs/workload-2026-09-24/run.sh; how to read it: docs/caraxes/workload-gate.md.

```
# workload gate 2026-09-25T02:59:50+02:00 targets=[native v2] reps=3 full=0 metrics=[short-auto rl ppo-full llama llama-nograph transformer convnet]
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
d0315158041c83ce822fd75dd22354fe34f7ec63ada962dff7d2ca6b76276b6b  /mnt/vmstore/caraxes/vfio-user-nvgpu-v615-57693d8
bac378f31ff769d5a89cf33f1a6ba077381ed500443d5905e24957a4e79fccc5  /mnt/vmstore/caraxes/vm1/cloud-hypervisor-0006
CARAXES_BAR1_PASSTHROUGH=pool
CARAXES_BOOT_MODEL=1
CARAXES_DMA=register
CARAXES_DOORBELL_POLL=hybrid
CARAXES_DOORBELL_TOKENS=64
CARAXES_DRIVER=615
CARAXES_FB_MIB=1024
CARAXES_GATE_FB_MIB=1024
CARAXES_HOST_EXEC=1
CARAXES_HOST_RM=1
CARAXES_INTR_PAGE=1
CARAXES_IP=192.168.102.22
CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test
CARAXES_MAC=02:00:00:00:00:22
CARAXES_MIRROR_CHANNELS=1
CARAXES_MIRROR_GR_SHADOW=1
CARAXES_RESERVE_SLOT=3
CARAXES_RESERVE_VRAM=1
CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-57693d8
CARAXES_TAP=caraxes2
LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
d09f9b031652725a1b37caf8526e481b741b74c2
BENCH_TIMEOUT=900
LLAMA_REPS=5
PPO_FULL_SECONDS=420
WORKLOAD_SECONDS=60
# targets now [native v2] metrics [short-auto rl ppo-full llama llama-nograph transformer convnet]
host watch: 767 samples, nvidia-smi max 0.057s mean 0.025s, 1 kernel log lines, 1 Xid lines, 0 alerts
host Xid lines since 2026-09-25 02:59:50: 1
```

## Per-run result lines

```
native/convnet-1: # CONVNET images_per_s=506.49 step_med_ms=63.122 step_p10_ms=61.376 step_p90_ms=65.024 steps=950 first_step_ms=1981.0 loss_first=7.0050 loss_last=0.0032 amp=0 channels_last=0 batch=32 size=224 mem_peak_mib=3211 oom_retries=0 finite=1 device=NVIDIA_GeForce_RTX_2070
native/convnet-2: # CONVNET images_per_s=521.73 step_med_ms=61.167 step_p10_ms=59.633 step_p90_ms=63.395 steps=979 first_step_ms=1691.3 loss_first=7.0050 loss_last=0.0031 amp=0 channels_last=0 batch=32 size=224 mem_peak_mib=3211 oom_retries=0 finite=1 device=NVIDIA_GeForce_RTX_2070
native/convnet-3: # CONVNET images_per_s=505.75 step_med_ms=63.206 step_p10_ms=61.527 step_p90_ms=65.046 steps=949 first_step_ms=1807.5 loss_first=7.0050 loss_last=0.0032 amp=0 channels_last=0 batch=32 size=224 mem_peak_mib=3211 oom_retries=0 finite=1 device=NVIDIA_GeForce_RTX_2070
native/llama-1: # LLAMA pp512_tps=8474.52 pp512_sd=965.95 tg128_tps=294.70 tg128_sd=0.67 reps=5 ngl=99 fa=-1 model=llama_1B_Q4_K_-_Medium model_mib=763 wall_s=3.7 build=e9fa0781f libs=dist gpu=NVIDIA_GeForce_RTX_2070
native/llama-2: # LLAMA pp512_tps=8837.37 pp512_sd=320.83 tg128_tps=290.90 tg128_sd=5.20 reps=5 ngl=99 fa=-1 model=llama_1B_Q4_K_-_Medium model_mib=763 wall_s=3.6 build=e9fa0781f libs=dist gpu=NVIDIA_GeForce_RTX_2070
native/llama-3: # LLAMA pp512_tps=8270.05 pp512_sd=945.21 tg128_tps=294.58 tg128_sd=0.46 reps=5 ngl=99 fa=-1 model=llama_1B_Q4_K_-_Medium model_mib=763 wall_s=3.5 build=e9fa0781f libs=dist gpu=NVIDIA_GeForce_RTX_2070
native/llama-nograph-1: # LLAMA tg128_tps=294.34 tg128_sd=3.81 reps=5 ngl=99 fa=-1 model=llama_1B_Q4_K_-_Medium model_mib=763 wall_s=3.2 build=e9fa0781f libs=dist gpu=NVIDIA_GeForce_RTX_2070
native/llama-nograph-2: # LLAMA tg128_tps=287.77 tg128_sd=12.25 reps=5 ngl=99 fa=-1 model=llama_1B_Q4_K_-_Medium model_mib=763 wall_s=3.3 build=e9fa0781f libs=dist gpu=NVIDIA_GeForce_RTX_2070
native/llama-nograph-3: # LLAMA tg128_tps=292.94 tg128_sd=7.69 reps=5 ngl=99 fa=-1 model=llama_1B_Q4_K_-_Medium model_mib=763 wall_s=3.2 build=e9fa0781f libs=dist gpu=NVIDIA_GeForce_RTX_2070
native/ppo-full-1: # PPO steps_per_s=118799 solve_s=149.6 episodes=140892 reward100=500.0 envs=128 rollout=256
native/ppo-full-2: # PPO steps_per_s=116200 solve_s=204.19 episodes=138727 reward100=500.0 envs=128 rollout=256
native/ppo-full-3: # PPO steps_per_s=116313 solve_s=87.13 episodes=138782 reward100=500.0 envs=128 rollout=256
native/rl-1: stage4: 50 episodes in 1.2s, mean10=56.9
native/rl-2: stage4: 50 episodes in 1.2s, mean10=56.9
native/rl-3: stage4: 50 episodes in 1.3s, mean10=56.9
native/short-auto-1: MSPERLAUNCH=0.006
native/short-auto-2: MSPERLAUNCH=0.006
native/short-auto-3: MSPERLAUNCH=0.006
native/transformer-1: # TRANSFORMER steps_per_s=12.673 step_med_ms=78.950 step_p10_ms=76.875 step_p90_ms=80.781 tokens_per_s=103815 steps=761 first_step_ms=351.4 loss_first=8.3697 loss_last=0.0222 amp=0 batch=32 seq=256 layers=6 dmodel=256 params_m=5.85 mem_peak_mib=1382 oom_retries=0 finite=1 device=NVIDIA_GeForce_RTX_2070
native/transformer-2: # TRANSFORMER steps_per_s=12.647 step_med_ms=79.195 step_p10_ms=76.918 step_p90_ms=80.930 tokens_per_s=103608 steps=759 first_step_ms=329.5 loss_first=8.3697 loss_last=0.0225 amp=0 batch=32 seq=256 layers=6 dmodel=256 params_m=5.85 mem_peak_mib=1382 oom_retries=0 finite=1 device=NVIDIA_GeForce_RTX_2070
native/transformer-3: # TRANSFORMER steps_per_s=12.666 step_med_ms=78.896 step_p10_ms=76.948 step_p90_ms=80.848 tokens_per_s=103760 steps=760 first_step_ms=254.1 loss_first=8.3697 loss_last=0.0230 amp=0 batch=32 seq=256 layers=6 dmodel=256 params_m=5.85 mem_peak_mib=1382 oom_retries=0 finite=1 device=NVIDIA_GeForce_RTX_2070
v2/convnet-1: # CONVNET images_per_s=465.34 step_med_ms=34.298 step_p10_ms=33.191 step_p90_ms=35.521 steps=1746 first_step_ms=1413.4 loss_first=7.0384 loss_last=0.0009 amp=0 channels_last=0 batch=16 size=224 mem_peak_mib=567 oom_retries=1 finite=1 device=Leandro_VFIO_RTX_2070-1G
v2/convnet-2: # CONVNET images_per_s=468.00 step_med_ms=34.180 step_p10_ms=33.013 step_p90_ms=35.254 steps=1755 first_step_ms=1543.1 loss_first=7.0384 loss_last=0.0009 amp=0 channels_last=0 batch=16 size=224 mem_peak_mib=567 oom_retries=1 finite=1 device=Leandro_VFIO_RTX_2070-1G
v2/convnet-3: # CONVNET images_per_s=464.63 step_med_ms=34.427 step_p10_ms=33.331 step_p90_ms=35.497 steps=1743 first_step_ms=1587.1 loss_first=7.0384 loss_last=0.0009 amp=0 channels_last=0 batch=16 size=224 mem_peak_mib=567 oom_retries=1 finite=1 device=Leandro_VFIO_RTX_2070-1G
v2/llama-1: # LLAMA failed=1 rc=1 wall_s=4.7
v2/llama-2: # LLAMA failed=1 rc=1 wall_s=5.6
v2/llama-3: # LLAMA failed=1 rc=1 wall_s=5.4
v2/llama-nograph-1: # LLAMA failed=1 rc=1 wall_s=4.3
v2/llama-nograph-2: # LLAMA failed=1 rc=1 wall_s=4.9
v2/llama-nograph-3: # LLAMA failed=1 rc=1 wall_s=4.0
v2/ppo-full-1: # PPO steps_per_s=75551 solve_s=233.67 episodes=103802 reward100=494.2 envs=128 rollout=256
v2/ppo-full-2: # PPO steps_per_s=73592 solve_s=174.55 episodes=102147 reward100=492.0 envs=128 rollout=256
v2/ppo-full-3: # PPO steps_per_s=74056 solve_s=239.79 episodes=102538 reward100=500.0 envs=128 rollout=256
v2/rl-1: stage4: 50 episodes in 2.5s, mean10=56.9
v2/rl-2: stage4: 50 episodes in 2.1s, mean10=56.9
v2/rl-3: stage4: 50 episodes in 2.4s, mean10=56.9
v2/short-auto-1: MSPERLAUNCH=0.020
v2/short-auto-2: MSPERLAUNCH=0.023
v2/short-auto-3: MSPERLAUNCH=0.016
v2/transformer-1: # TRANSFORMER steps_per_s=42.930 step_med_ms=23.236 step_p10_ms=22.320 step_p90_ms=24.358 tokens_per_s=87921 steps=2576 first_step_ms=362.7 loss_first=8.3663 loss_last=0.0023 amp=0 batch=8 seq=256 layers=6 dmodel=256 params_m=5.85 mem_peak_mib=408 oom_retries=2 finite=1 device=Leandro_VFIO_RTX_2070-1G
v2/transformer-2: no result line, # WALL wall_ms=8209 rc=1
v2/transformer-3: # TRANSFORMER steps_per_s=42.842 step_med_ms=23.266 step_p10_ms=22.343 step_p90_ms=24.454 tokens_per_s=87741 steps=2571 first_step_ms=431.0 loss_first=8.3663 loss_last=0.0023 amp=0 batch=8 seq=256 layers=6 dmodel=256 params_m=5.85 mem_peak_mib=408 oom_retries=2 finite=1 device=Leandro_VFIO_RTX_2070-1G
```

## Tables

Sources: native=/home/silas/git/Caraxes/.claude/worktrees/agent-a25d59d08c801f257/caraxes/runs/workload-2026-09-24/night/gate/native, v2=/home/silas/git/Caraxes/.claude/worktrees/agent-a25d59d08c801f257/caraxes/runs/workload-2026-09-24/night/gate/v2
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): native 2/21, v2 0/21
Host 1-min load average at run start, median [min-max]: native 3.38 [1.82-5.69] n=21, v2 3.09 [1.82-5.84] n=21

### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | native | v2 | v2 / native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.006 [0.006-0.006] n=3 | 0.020 [0.016-0.023] n=3 | 3.33 |

### (4) rlprobe, 50 REINFORCE episodes

| value | unit | native | v2 | v2 / native |
|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=3 | 56.9 [56.9-56.9] n=3 | 1.00 |
| process wall (incl. torch import) | s (low is better) | 4.58 [4.27-6.15] n=3 | 14.1 [13.1-15.6] n=3 | 3.08 |
| stage4 (50 training episodes) | s (low is better) | 1.20 [1.20-1.30] n=3 | 2.40 [2.10-2.50] n=3 | 2.00 |
| H2D+D2H transfers per second | 1/s (high is better) | 2353 [2166-2471] n=3 | 1188 [1133-1399] n=3 | 0.50 |

### (W1) PPO CartPole, full length (pporl, 420 s)

| value | unit | native | v2 | v2 / native |
|---|---|---|---|---|
| environment steps per second | 1/s (high is better) | 116313 [116200-118799] n=3 | 74056 [73592-75551] n=3 | 0.64 |
| time to solved (mean100 >= 475) | s (low is better) | 150 [87.1-204] n=3 | 234 [175-240] n=3 | 1.56 |
| reward100 at the end (not a time) |  | 500 [500-500] n=3 | 494 [492-500] n=3 | 0.99 |

### (W2) llama.cpp inference, all layers on the GPU (llama-bench: pp = prompt processing, tg = token generation)

| value | unit | native | v2 | v2 / native |
|---|---|---|---|---|
| pp512 (prompt processing, 512 tokens) | tokens/s (high is better) | 8475 [8270-8837] n=3 | n/a | - |
| tg128 (token generation, 128 tokens) | tokens/s (high is better) | 295 [291-295] n=3 | n/a | - |
| llama-bench process wall (load, warm-up, all reps) | s (low is better) | 3.60 [3.50-3.70] n=3 | 5.40 [4.70-5.60] n=3 | 1.50 |

### (W2) llama.cpp token generation with CUDA graphs off (GGML_CUDA_DISABLE_GRAPHS=1): one launch per kernel

| value | unit | native | v2 | v2 / native |
|---|---|---|---|---|
| tg128 (token generation, 128 tokens) | tokens/s (high is better) | 293 [288-294] n=3 | n/a | - |
| llama-bench process wall (load, warm-up, all reps) | s (low is better) | 3.20 [3.20-3.30] n=3 | 4.30 [4.00-4.90] n=3 | 1.34 |

### (W3) GPT-style training loop, 6 layers, d_model 256, AdamW (transformer.py)

| value | unit | native | v2 | v2 / native |
|---|---|---|---|---|
| training steps per second (batch 32, seq 256, amp 0) | 1/s (high is better) | 12.7 [12.6-12.7] n=3 | n/a | - |
| median step time | ms (low is better) | 79.0 [78.9-79.2] n=3 | 23.3 [23.2-23.3] n=2 | 0.29 |
| p90 step time | ms (low is better) | 80.8 [80.8-80.9] n=3 | 24.4 [24.4-24.5] n=2 | 0.30 |
| tokens per second | 1/s (high is better) | 103760 [103608-103815] n=3 | 87831 [87741-87921] n=2 | 0.85 |
| first step (cuBLAS setup, allocations) | ms (low is better) | 330 [254-351] n=3 | 397 [363-431] n=2 | 1.20 |
| peak allocated memory | MiB | 1382 [1382-1382] n=3 | 408 [408-408] n=2 | 0.30 |
| loss at the end (not a time) |  | 0.022 [0.022-0.023] n=3 | 0.002 [0.002-0.002] n=2 | 0.10 |
| training steps per second (batch 8, seq 256, amp 0) | 1/s (high is better) | n/a | 42.9 [42.8-42.9] n=2 | - |

### (W4) ResNet-18 training, synthetic 224 px images, cuDNN (convnet.py)

| value | unit | native | v2 | v2 / native |
|---|---|---|---|---|
| images per second (batch 32, 224 px, amp 0) | 1/s (high is better) | 506 [506-522] n=3 | n/a | - |
| median step time | ms (low is better) | 63.1 [61.2-63.2] n=3 | 34.3 [34.2-34.4] n=3 | 0.54 |
| p90 step time | ms (low is better) | 65.0 [63.4-65.0] n=3 | 35.5 [35.3-35.5] n=3 | 0.55 |
| first step (cuDNN autotuning) | ms (low is better) | 1808 [1691-1981] n=3 | 1543 [1413-1587] n=3 | 0.85 |
| peak allocated memory | MiB | 3211 [3211-3211] n=3 | 567 [567-567] n=3 | 0.18 |
| images per second (batch 16, 224 px, amp 0) | 1/s (high is better) | n/a | 465 [465-468] n=3 | - |

### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | native | v2 | v2 / native |
|---|---|---|---|---|
| convnet device CPU | ms (low is better) | n/a | 20470 [20050-21020] n=3 | - |
| convnet VMM CPU | ms (low is better) | n/a | 68530 [68500-69470] n=3 | - |
| llama device CPU | ms (low is better) | n/a | 3350 [3130-3480] n=3 | - |
| llama VMM CPU | ms (low is better) | n/a | 3980 [3300-9690] n=3 | - |
| llama-nograph device CPU | ms (low is better) | n/a | 2590 [2250-2700] n=3 | - |
| llama-nograph VMM CPU | ms (low is better) | n/a | 2780 [2700-8090] n=3 | - |
| ppo-full device CPU | ms (low is better) | n/a | 410680 [403040-410950] n=3 | - |
| ppo-full VMM CPU | ms (low is better) | n/a | 423860 [420840-846130] n=3 | - |
| rl device CPU | ms (low is better) | n/a | 6250 [6240-6470] n=3 | - |
| rl VMM CPU | ms (low is better) | n/a | 24170 [23670-26580] n=3 | - |
| short-auto device CPU | ms (low is better) | n/a | 2230 [2090-2810] n=3 | - |
| short-auto VMM CPU | ms (low is better) | n/a | 4960 [4720-7260] n=3 | - |
| transformer device CPU | ms (low is better) | n/a | 29070 [4230-29640] n=3 | - |
| transformer VMM CPU | ms (low is better) | n/a | 67280 [6630-67420] n=3 | - |

### (8) Device process memory (over all runs)

| value | unit | native | v2 | v2 / native |
|---|---|---|---|---|
| device HWM | MiB (low is better) | n/a | 8199 [8199-8199] n=21 | - |
| device RSS after run | MiB (low is better) | n/a | 5137 [5131-5139] n=21 | - |

