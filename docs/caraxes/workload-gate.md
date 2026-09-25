<!-- SPDX-License-Identifier: MIT -->
# Workload gate: real workloads against native ("verrücktere Kernels")

Written 2026-09-24, prepared without a VM or the GPU. The runs are for tonight's window after
the closing series and the host driver upgrade to 615.71.09 ([cutover-615.md](cutover-615.md)).
Operator's order: real workloads on top of the microbenchmarks of [benchmark.md](benchmark.md),
against native and v1. The question is whether the zero-trap submission path (the P21 BAR1 area,
the doorbell token pool and the pinned reactor, benchmark.md sections of 2026-09-24,
[doorbell-poll.md](doorbell-poll.md)) holds under sustained load. The gate also shows where the
device itself becomes the limit. Today the device needs about 3.6 us (p50) from a guest
doorbell to the host doorbell for each ring.

**Status.** First run on 2026-09-25 (night series, 615 guest on vm2 at 1 GiB): section 7. The
table of section 5 is filled from it.

## 1. The command for the window

```
caraxes/tools/build-llama.sh all        # once, on the host, no GPU (already done if dist/ exists)
caraxes/runs/workload-2026-09-24/run.sh --targets "native v2"
```

This runs in about 70 minutes: two targets, 3 reps, restart per rep. `--full` runs the full-length
variant (about 2 h 30 min), `--reps N` changes the number of reps, and
`--metrics "..."` selects a subset. For example, `--metrics "llama transformer" --reps 1` is a
quick first look. The output is `caraxes/runs/workload-2026-09-24/gate-HHMM/` (or `full-HHMM/`):
the raw file of each run, `series.txt`, `setup.txt`, `hostwatch.txt`, `host-xid.txt`,
`v2-device-start.txt`, `v2-device-summary.txt` and `summary.md`.

What the runner does, in order:

1. Reads the host driver version from `/proc/driver/nvidia/version`. **v1 is dropped** unless the
   host runs 610.57.04. The v1 guest and `vhost-user-nvrm` are 610.57.04, and v1 forwards
   ioctls verbatim (cutover-615.md (e)). `FORCE_V1=1` overrides this; do not use it on the
   615 host.
2. Drops `llama` if `build-llama.sh` has not produced `/mnt/vmstore/caraxes/llama/dist/llama-bench`
   and `/mnt/vmstore/caraxes/models/llama-gate.gguf`.
3. Sets up v2 in the closing configuration:
   - `source caraxes/runs/mapping-2026-09-24/env.sh`: vm2, slot 0, 2 GiB profile, host RM, host
     execution, mirror channels, GR shadow, reserved VRAM, registered DMA;
   - `CARAXES_SERVER`: the newest `/mnt/vmstore/caraxes/vfio-user-nvgpu-closing-610-*` (today
     `81054f0`). An exported `CARAXES_SERVER` overrides it, for example
     `target/release/vfio-user-nvgpu` for a fresh build from main;
   - `LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh`, `CARAXES_BAR1_PASSTHROUGH=1`,
     `CARAXES_DOORBELL_TOKENS=64` and `CARAXES_DOORBELL_POLL=spin`.

   The frozen binary contains the host-layout selection (`hostabi.rs`, `e36f582`), so it serves
   the 615 host (cutover-615.md (c) 1).
4. Starts the host watch (`bench-hostwatch.sh`, one `nvidia-smi` query every 5 s). With v2 in the
   set, `SLOW=5` and `KILL=$W/gpu.pid`: a host that takes longer than 5 s to answer stops the
   device.
5. Stages the probes into `target/bench/bin` (native) and `~/bench` (guests). `v2-restart`, then
   `stage-llama` copies llama-bench, its libraries and the model into `~/bench-llama`. It skips
   the copy when the sha256 sums already match.
6. `bench-run.sh series` with `V2_RESTART=1` (and `V1_RESTART=1`). The rep loop is outermost, so
   the targets interleave inside every metric (benchmark.md §2).
7. Stops the host watch, collects the device log lines, the host Xid lines since the start and
   `summary.md`. `summary.md` holds the setup, one result line per run and the
   `bench-summary.py` tables.

Timeouts: every run is bounded by `BENCH_TIMEOUT`, which defaults to the longest workload plus
480 s (900 s in the default set).

## 2. The workloads

Every probe is a harness metric in `caraxes/tools/bench-run.sh`. It prints one machine-readable
`# NAME key=value ...` line, which `bench-summary.py` parses. The 60 s workloads run for 60 s by
default and for 420 s under `--full` (`WORKLOAD_SECONDS`). All times are measured inside the
target, as in benchmark.md.

| Metric | Probe | What it exercises | Result line |
|---|---|---|---|
| `short-auto` | `nvprobe 4`, 1000-cycle kernel, 100 launches with `cuCtxSynchronize` | per-submission cost with nothing to hide it (micro) | `MSPERLAUNCH=` |
| `rl` | Leandro-Test `rlprobe.py`, 50 REINFORCE episodes | small H2D/D2H at every step, a tiny net (micro/RL) | `stage4:`, `~N/s` |
| `ppo-full` | `caraxes/tools/pporl.py --seconds 420` | the operator's PPO CartPole: 128 CPU environments; per step one H2D, about ten small kernels, one synchronous D2H; per rollout of 256 steps an update of 32 minibatches with a few hundred kernels each. Sustained submission at a rate of thousands per second for 7 minutes | `# PPO steps_per_s= solve_s= episodes= reward100=` |
| `llama` | `caraxes/tools/llamaprobe.py`, which runs llama.cpp `llama-bench -p 512 -n 128 -r 5 -ngl 99 -fa auto` | pp512: batched quantized GEMMs (MMQ) over 512 tokens, device-bound. tg128: one token per step, a few hundred small kernels per token. ggml captures them in a CUDA graph and launches one graph per token, then does a synchronous read-back. This half is submission- and latency-bound | `# LLAMA pp512_tps= tg128_tps= ...` |
| `llama-nograph` | the same with `GGML_CUDA_DISABLE_GRAPHS=1`, tg128 only (`-p 0 -n 128`) | token generation with one launch per kernel instead of one graph launch per token: several hundred submissions per token, each followed by the device's per-ring work. Compared with `llama`'s tg128, it separates the cost of submissions from the cost of the kernels | `# LLAMA tg128_tps= ...` |
| `transformer` | `caraxes/tools/transformer.py`: GPT-style, 6 layers, d_model 256, 8 heads, sequence 256, batch 32, vocabulary 4096, AdamW (fused), fp32 (`TRANSFORMER_ARGS=--amp` for fp16 autocast) | one training step = forward, backward, clip, optimizer: several hundred kernels of mixed size (cuBLAS GEMMs, fused SDPA attention, layer norm, GELU, the fused AdamW), ending in `loss.item()` | `# TRANSFORMER steps_per_s= step_med_ms= tokens_per_s= ...` |
| `convnet` | `caraxes/tools/convnet.py`: ResNet-18 (written out; the venvs have no torchvision), synthetic 224 px images, batch 32, SGD with momentum, fp32, `cudnn.benchmark` (`CONVNET_ARGS=--amp --channels-last` for tensor cores) | cuDNN convolutions (forward, data and weight gradient) and batch norm: long kernels of hundreds of microseconds each, a small H2D of the labels and `loss.item()` per step. The workload where the device should be the limit | `# CONVNET images_per_s= step_med_ms= ...` |

Probe details:

- `pporl.py` is the operator's script, unchanged. The metric `ppo` (60 s) stays for the short run.
- `transformer.py` and `convnet.py` generate their data on the device once, so the loop moves no
  data from the host except what is listed above. Timing starts after a warm-up of 5 steps. The
  first step is reported on its own (`first_step_ms`): it includes cuBLAS handle creation, the
  first allocations and, for convnet, cuDNN's autotuning of every convolution shape. It is the
  step with the most mapping changes, which is v2's expensive operation (benchmark.md §5, 20 to
  55 ms of device CPU per mapping change).
- Each step ends in one synchronous read-back, as real training loops do (logging the loss). Steps
  do not overlap, so the median step time is a clean per-step figure.
- Memory. The v2 profile has 2 GiB. The tensors saved for the backward pass, measured on the host
  CPU with the full configurations, are about 940 MiB for the transformer and about 720 MiB for
  ResNet-18, plus parameters, gradients, optimizer state and workspaces. The llama model is
  about 770 MiB of weights (Q4_K_M), plus a KV cache and compute buffers of a few hundred MiB.
  If a probe runs out of memory, it halves its batch and starts again. Its line then reports
  `batch=` and `oom_retries=`, and the summary row carries the batch in its name, so a changed
  batch shows up as a separate row and is not silently compared with native.
- `llamaprobe.py` puts `~/bench-llama/lib` (the CUDA 13.3 runtime and cuBLAS the binary was built
  with) in front of the guest's `/opt/nvrm/lib`, where libcuda lives. `LLAMA_ARGS="--libs venv"`
  uses the guest venv's torch CUDA 13.0 runtime and cuBLAS 13.1 instead (section 3).

## 3. The llama.cpp build

`caraxes/tools/build-llama.sh all` builds `llama-bench` on the host. The guest images have no
nvcc (guest-driver-versions.md §8).

| Item | Value |
|---|---|
| Source | llama.cpp `e9fa0781f1c25fc4fe8c86be1edc6970661ad6f0`, the revision of the local checkout `../llama.cpp`. The script uses that checkout (`LLAMA_SRC`) and clones the pinned revision into `/mnt/vmstore/caraxes/llama/src` when there is none; the build of 2026-09-24 ran from such a clone. Built out of tree into `/mnt/vmstore/caraxes/llama/build` |
| Toolkit | CUDA 13.3 (`/opt/cuda`), host compiler gcc-15 (nvcc 13.3 does not accept gcc 16) |
| Device code | `CMAKE_CUDA_ARCHITECTURES=75-real`: SASS for TU106 only, no PTX. A 13.3 build then runs on an older 13.x driver (minor-version compatibility), because nothing needs a JIT of 13.3 PTX |
| Linking | libcuda dynamic (the driver's: 610.57.04 in the guest, the host's own on native). libcudart, libcublas and libcublasLt as `.so` files in `dist/lib`, RUNPATH `$ORIGIN/lib`. ggml's `GGML_STATIC` links with `-static`, which cannot load libcuda. libstdc++ and libgcc are static because the guests' libstdc++ (gcc 13) is older than the host's (gcc 16) |
| glibc | host 2.44, guests (Ubuntu 24.04) 2.39. `build-llama.sh check` fails if any symbol needs a glibc newer than 2.39. The first build failed on one symbol, `sqrtf@GLIBC_2.43`. With `-fno-math-errno`, gcc emits the `sqrtss` instruction instead of the libm call, and the highest symbol is then `GLIBC_2.38` |
| Options | `GGML_CUDA=ON`, `GGML_CUDA_NCCL=OFF`, `GGML_OPENMP=OFF`, `GGML_NATIVE=OFF` with AVX2/FMA/F16C, CUDA graphs and VMM pools at their defaults (on). `LLAMA_NO_VMM=1` also builds `dist-novmm`, which uses the legacy cudaMalloc pool, as a fallback if a target refuses `cuMemCreate`/`cuMemMap` |
| Model | Llama 3.2 1B Instruct, Q4_K_M (public GGUF conversion), in `/mnt/vmstore/caraxes/models/`, with its sha256 in `<file>.sha256` and the source URL in `<file>.source`. `llama-gate.gguf` links to it |
| Download | one plain GET with `curl -q` (no curlrc), no token, no custom header, no identifier |

Build result: see [section 6](#6-preparation-record).

If the guest driver refuses the 13.3 runtime ("CUDA driver version is insufficient"), run
`LLAMA_ARGS="--libs venv"`. That uses the CUDA 13.0 runtime and cuBLAS 13.1 of the guest venv,
which torch uses on the same driver. If the target refuses the VMM pool, build with
`LLAMA_NO_VMM=1` and run with `LLAMA_DIST=/mnt/vmstore/caraxes/llama/dist-novmm`.

## 4. How to read it

- **Ratio** = v2 median / native median (bench-summary.py). For throughput (tokens/s, steps/s,
  images/s) 1.00 is parity and lower is slower; for times it is the other way round.
- **Where the path is the limit.** A workload with kernels much longer than the per-ring cost
  (convnet, pp512, the transformer's larger GEMMs) hides the submission path as long as the guest
  keeps the queue full: the ratio should approach 1.00. A workload that synchronizes after little
  work shows the per-submission and per-sync cost directly: `short-auto`, `rl`, the PPO rollout
  and tg128. `short-auto` and `rl` are the micro rows the gate keeps for comparison. The
  pair tg128 / tg128 without graphs isolates the submission cost: the same kernels, once as
  one graph launch per token and once as several hundred launches per token.
- **Where the device becomes the limit.** The device process spends about 3.6 us per ring
  (doorbell-poll.md). A step of N submissions therefore has a floor of about N x 3.6 us on v2,
  in addition to the GPU time, when the rings are serialized. Check the device CPU rows of group
  (8): device CPU close to wall time means the device process, not the GPU, sets the pace. With
  the spin reactor, one core is always busy, so compare the CPU time with the run's wall time.
  Also check `step_p90_ms` against `step_med_ms`: a wide gap on v2 alone points at stalls in the
  device (mapping changes, reactor wakeups) and not at the GPU.
- **Holds under load** means: every v2 run has `rc=0` and a finite loss; PPO reaches
  `reward100` like native; `host-xid.txt` is empty; the host watch has no alert;
  `v2-device-summary.txt` shows no trapped doorbells beyond the first ring of each channel (token
  pool counters, benchmark.md "Zero-trap submission"); the throughput of the 420 s runs does not
  sag over time (the progress lines every 5 or 10 s in the raw files).
- `first_step_ms` and the llama wall time, compared between targets, measure setup cost (mapping
  changes, module and cuBLAS or cuDNN initialisation), not steady-state throughput.

## 5. Table (the first run, section 7)

v2 is the 615 guest with the 1 GiB profile; where v2 had to halve its batch the rows are not
like for like (per-sample throughput shown, marked). v1 was dropped on the 615 host.

| Workload | Value | native | v2 | v2 / native | v1 | v1 / native |
|---|---|---|---|---|---|---|
| short-auto | ms/launch | 0.006 | 0.020 | 3.3 | - | - |
| rl | stage 4 (50 episodes), s | 1.20 | 2.40 | 2.0 | - | - |
| rl | transfers per second | 2353 | 1188 | 0.50 | - | - |
| ppo-full | environment steps per second | 116313 | 74056 | 0.64 | - | - |
| ppo-full | time to solved, s | 150 | 234 | 1.56 | - | - |
| llama | pp512, tokens/s | 8475 | failed: the model does not load into 791 MiB | - | - | - |
| llama | tg128, tokens/s | 295 | failed | - | - | - |
| llama-nograph | tg128 without CUDA graphs, tokens/s | 293 | failed | - | - | - |
| transformer | tokens per second (native batch 32, v2 batch 8, fp32) | 103760 | 87831 (n=2) | 0.85, not like for like | - | - |
| transformer | median / p90 step, ms | 79.0 / 80.8 (batch 32) | 23.3 / 24.4 (batch 8) | - | - | - |
| convnet | images per second (native batch 32, v2 batch 16, fp32) | 506 | 465 | 0.92, not like for like | - | - |
| convnet | median / p90 step, ms | 63.1 / 65.0 (batch 32) | 34.3 / 35.5 (batch 16) | - | - | - |
| all | device CPU per run, ms (group 8) | - | ppo-full 410680 of 427000 wall; convnet 20470, transformer 29070 of about 65000 | | | |
| all | host Xid, host-watch alerts | 0, 0 | 1 Xid 31 on the device's pid (transformer rep 2), 0 alerts | | | |

## 6. Preparation record

Done on the host on 2026-09-24, 15:37-15:52, after the closing series and its PPO runs had
finished (the compile ran under `nice`, with no measurement in progress). No VM, no GPU.

| Item | Value |
|---|---|
| Model | `/mnt/vmstore/caraxes/models/Llama-3.2-1B-Instruct-Q4_K_M.gguf` (808 MB), `llama-gate.gguf` links to it |
| Model sha256 | `6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83` |
| Model source | `https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf` (one GET, 10 s) |
| Build | `/mnt/vmstore/caraxes/llama/dist` (603 MB: llama-bench plus libcudart, libcublas and libcublasLt 13.3), `BUILD-INFO.txt` beside it |
| llama-bench sha256 | `147765a7c77ffd27308a5e7cef96a9c321d3d4220a66f95ec47d7286e2e9e5c3` |
| `build-llama.sh check` | NEEDED `libcudart.so.13 libcublas.so.13 libcublasLt.so.13 libcuda.so.1 libm libc`, RUNPATH `$ORIGIN/lib`, then `/opt/cuda/targets/x86_64-linux/lib` (absent in the guests; no patchelf on the host to drop it); highest glibc symbol 2.38 (binary), 2.27 (libraries); no libstdc++ in the binary's NEEDED list. libcudart itself needs the system libstdc++, which the guests have |
| Not verified | the binary has not run. Running it would touch the GPU. The first real run is tonight's; if the 610 guest driver refuses the 13.3 runtime, use `LLAMA_ARGS="--libs venv"` (section 3) |

Probe checks without the GPU (`CUDA_VISIBLE_DEVICES=` and `--cpu`, host venv torch 2.13.0):
`transformer.py --cpu` (2 layers, d_model 64, sequence 32, batch 4) and `convnet.py --cpu` (batch 2,
64 px, with and without `--channels-last`) run and print their result lines. The loss falls.
`llamaprobe.py --dry-run` builds the command. `py_compile` passes for all probes and
`bench-summary.py`, and `bash -n` passes for `bench-run.sh`, `build-llama.sh` and `run.sh`.
`bench-summary.py` parses synthetic result lines of every new metric into the tables above.

## 7. First run (2026-09-25, 02:59-04:04, night series)

`caraxes/runs/workload-2026-09-24/night/` (`run.sh`, `env.sh`, `gate/summary.md`):
`run.sh --targets "native v2"`, 3 reps, the default metrics, on the 615 guest (vm2-615, 1 GiB
profile at slot 3, frozen `vfio-user-nvgpu-v615-57693d8`, BAR1 pool, token pool 64, hybrid
reactor, interrupt page on; `WORKLOAD_ENV` of `../run.sh`). The host was quiet: the script waited
for no other device process (it started at once, after the like-for-like series), and no other
VM's device was active in any v2 run. v1 dropped (615 host).

What it shows:

- **Holds under load, mostly.** PPO at 420 s solves on every rep (reward100 492-500, solve in
  175-240 s against 87-204 s native) at 0.64 of native's steps; the token pool counted 20.2
  million guest doorbell writes and 0 trapped pool tokens (`gate/v2-device-exit.txt`); host
  watch 0 alerts, nvidia-smi at most 0.057 s.
- **One device fault.** Transformer rep 2 died with `cudaErrorLaunchFailure`: host Xid 31 at
  03:40:46 on pid 2211803, vm2's device process of that rep, GPC0 `GPCCLIENT_T1_11`, FAULT_PDE
  on a write at 0x100_1e200000 (`gate/host-xid.txt`, `gate/v2/transformer-2.txt`). Reps 1 and 3
  of the same metric ran clean at the same batch. The BAR1 pool moved 3072 GPU-mapped pages
  behind its barrier over the run (135 barriers, 372 TSG preempts), which is the first thing to
  rule out; a PDE fault on a GR write points at a shadow that did not have a page-directory entry
  the guest had, at that moment.
- **The 1 GiB profile is too small for three workloads.** llama-bench cannot load the 763 MiB
  model into the 791 MiB the guest sees (every rep, both variants); the transformer runs at
  batch 8 and ResNet-18 at batch 16 after the probes' OOM halving. Per sample, v2 reaches 0.85
  (transformer) and 0.92 (convnet) of native at those smaller batches. The like-for-like rows
  need the 2 GiB profile (slot 0), which the night's slot scheme did not give vm2.
- **The device is the pace-setter for PPO.** Its CPU per ppo-full run is 411 s of 427 s wall:
  the hybrid reactor and the per-ring work run flat out for the whole run.
