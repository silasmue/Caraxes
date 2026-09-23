<!-- SPDX-License-Identifier: MIT -->
# Testing: the Leandro v1 gates against the vfio-user device

One way to put the guest-side test material into a Caraxes VM and to run the v1 gates
against the synthetic device, with v1's verdict rules, so that "the cuda gate passed"
means what "the gpu gate passed" meant for Leandro v1.

```sh
VM=vm0 tools/caraxes-vm.sh sync-tests         # copy, build, record (idempotent)
VM=vm0 tools/caraxes-gate.sh cuda              # every CUDA test, per-test table
VM=vm0 tools/caraxes-gate.sh cuda cuda-init    # one test alone (a clean device trace)
VM=vm0 tools/caraxes-gate.sh gpu               # v1's gpu gate, guest-side stages
VM=vm0 tools/caraxes-gate.sh vdisplay          # not applicable yet (exit 2)
VM=vm0 tools/caraxes-gate.sh display           # not applicable yet (exit 2)
VM=vm0 tools/caraxes-gate.sh nvml              # the Caraxes nvidia-smi gate (nvml.md)
```

Files: `tools/caraxes-vm.sh` (`sync_tests`), `tools/caraxes-gate.sh` (`gate_cuda`,
`gate_gpu`, `gate_display_like`, the verdict helpers `g_*`), `tools/caraxes-guest-tests.sh`
(the guest half, installed as `~/caraxes-tests/caraxes/run-tests.sh`).

## 1. What ../Leandro-Test contains

`/home/silas/git/Leandro-Test` (commit `e046e04`, a dirty working tree when this was
written) is the v1 acceptance and measurement repository. Caraxes reads it and never writes
into it. Its parts, by role:

| Part | Path | Runs on | Role |
|---|---|---|---|
| The gates | `scripts/test.sh` (`gpu`, `vdisplay`, `display`, `gates`, `check`) | host, driving a guest over ssh | acceptance: binary PASS/FAIL |
| Verdict contract | `scripts/lib/common.sh:640-835` (`lea_gate_*`) | host | the JSON line, exit codes, safety net |
| Rig and provisioning | `scripts/lib/rig.sh`, `scripts/lib/provision.sh`, `scripts/lib/config.sh`, `scripts/showcase.sh`, `scripts/build.sh` | host | v1 VM lifecycle with vhost-user-nvrm and virtio_nvrm |
| CUDA probes (C) | `probe/c/*.c`, `probe/Makefile` | guest | `nvprobe` (staged driver API, stages 0-4), `managedprobe`, `hostregprobe`, `oomprobe`, `vrampress`, `forkprobe`, raw-RM `rmdirect`, `ctrlping`, `ioctlping`, OpenCL/EGL/Vulkan probes |
| PTX | `probe/kernels/kernels.cu`, `kernels.ptx` (sm_75, checked in) | guest | the vecAdd and spin kernels `nvprobe` JITs |
| PyTorch probes | `probe/python/{rlprobe,torchprobe,convburn,convoom,mmsweep,pinwin,streamprobe,vramcap,mtprobe,framecopy,drmfd}.py` | guest | torch workloads; `rlprobe` mean10 and `convburn` acc are the torch stage's correctness values |
| Decoders | `probe/python/{traceread,guestdiff,answerdiff,ioctlmatrix,classcoverage,maptax,uvmtax,nvkmsdecode,rfbshot,input_device}.py` | host | trace analysis, not copied |
| Matrix probes | `probe/matrix/*.sh` (27), `probe/run/matrix-guest.sh`, `scripts/ioctl-matrix.sh` | guest half and host half | one probe per library, each with its own `matrix-criterion` and `CRITERION:` line |
| Suites | `probe/suites/*.py` (8), `probe/run/suites.sh` | guest | breadth over torch, CuPy, RAPIDS; a table, never a gate |
| Guest workloads | `scripts/guest/*.c`, `*.sh` | guest | display probes (`drm-modeset.c`, `vkprobe.c`, `fencetime.c`, `atomicflip.c`, `fbprobe.c`, ...), `nvrm-setup.sh`, benchmarks |
| Nix, deploy, tests | `nix/`, `deploy/`, `tests/`, `probe/test/`, `lea` | host | packaging, MeisterStack agent, CLI tests |

How v1 gets its material into a guest:
- `lea_guest_setup` (`scripts/lib/provision.sh:808-880`) builds the probes on the host
  (`make -C probe all-probes`), stages them flat into one directory with the NVIDIA
  userspace, and sends it with `lea_guest_tar` (`provision.sh:187-195`, tar over ssh, not
  scp, so symlinks survive) into `~/gpu`, only when a manifest hash changed.
- `lea_guest_cc` (`provision.sh:202-212`) ships single C sources from `scripts/guest/` and
  compiles them in the guest ("the guest's glibc is not this host's").
- `probe/run/suites.sh` tars `probe/suites` into `~/gpu/suites` per run.
- `scripts/ioctl-matrix.sh:976-994` stages `scripts/lib`, `probe/matrix`, `probe/bin` and
  the tracer into `~/matrix` and runs `probe/run/matrix-guest.sh` there per probe.

## 2. sync-tests: what is copied, from where, to where

`tools/caraxes-vm.sh sync-tests` (VM from `$VM`, default `vm0`) uses rsync over the VM's
ssh key (`/mnt/vmstore/leandro/vm/id_leandro`, user `leandro`). It is idempotent: rsync
sends only changes and `--delete` removes files that left the source; `make` rebuilds only
stale probes.

Guest layout (`CARAXES_GUEST_TESTS`, default `/home/leandro/caraxes-tests`):

```
caraxes-tests/
  leandro-test/            Leandro-Test's own layout, so its scripts find each other unmodified
    probe/Makefile, README.md
    probe/c/               all C sources
    probe/kernels/         kernels.cu, kernels.ptx
    probe/matrix/          all 27 matrix probes
    probe/suites/          the 8 suites and their README
    probe/run/matrix-guest.sh
    probe/python/          the 11 GPU workloads (no host decoders)
    probe/bin/             built in the guest by sync-tests
    scripts/guest/         the guest workloads (display probes, nvrm-setup.sh, bench-one.sh, ...)
    scripts/lib/           config.sh, common.sh, matrix.sh (what the matrix probes source)
    lib/libnvrm_trace.so   the v1 tracer (crates/nvrm-trace, host-built), where matrix-guest.sh expects it
    DRIVER_VERSION         from this tree (610.57.04); the matrix payload reads it from its root
    README.md, LICENSE
  probe -> leandro-test/probe
  caraxes/run-tests.sh     tools/caraxes-guest-tests.sh
  SYNC.txt                 what the sync found in the guest (below)
  out/<run>/               raw results of each gate run; fetched into caraxes/runs/
```

The task proposed `probe/` and `leandro-test/` as sibling copies. The probe sources live
only in Leandro-Test now: this tree's `probe/` holds only git-ignored leftovers (host-built
binaries in `probe/bin/`, v1 native traces in `probe/traces/`), so `probe/` in the guest is
a link into `leandro-test/probe` and v1's relative paths keep working.

Not copied: the host-side decoders in `probe/python`, `probe/test/`, `probe/run/{trace,
drmtrace,suites}.sh` (host drivers), `scripts/*.sh` other than `scripts/guest/` and the
three libraries, `nix/`, `deploy/`, `tests/`, `lea`, the core's `tests/tools` (only the
display gates use `edid-verify.c` and `vdisp-frame.c`).

Built in the guest: `nvprobe managedprobe hostregprobe oomprobe vrampress rmdirect ctrlping
ioctlping`, with `CUDA_HOME` set to the torch venv's `nvidia/cu13` (it carries `cuda.h`)
and `-L/opt/nvrm/lib -lcuda`. Not built: `forkprobe` (needs the toolkit's `lib64/stubs`),
`glinterop`, `oclprobe`, `eglplat`, `vkalloc`, `vkrt` (EGL, OpenCL and Vulkan headers are
not in the image), and `kernels.ptx` stays the checked-in sm_75 file (no nvcc in the guest).

What the baked image has (`SYNC.txt` of 2026-09-23, image
`guest-baked-20260801-610.57.04-20260820-105733.qcow2`):

| Item | Present |
|---|---|
| OS, kernel | Ubuntu 24.04.4 LTS, 6.8.0-136-generic |
| nvidia.ko | 610.57.04, open modules built in the guest (`~/ogkm`) |
| NVIDIA userspace | `/opt/nvrm/lib` (libcuda, NVML, nvvm, ptxjitcompiler, encode, cuvid, OpenCL, ... 610.57.04; stale 610.43.03 copies beside them, the `.so.1` links point to 610.57.04), `/opt/nvrm/bin/nvidia-smi`, GL stack in `/opt/nvrm-gl` |
| CUDA toolkit | none: no `/usr/local/cuda`, no nvcc, no cuda-gdb |
| CUDA driver-API header | `~/gpu/venv/lib/python3.12/site-packages/nvidia/cu13/include/cuda.h`, `CUDA_VERSION 13000` |
| PyTorch | `~/gpu/venv`: torch 2.13.0 with the CUDA 13.0 wheels (cuda-runtime 13.0.96, cuDNN 9.20.0.48, cuBLAS 13.1.1.3, NCCL 2.29.7, triton 3.7.1), numpy 2.5.2 |
| Toolchain | gcc 13.3.0, make, python 3.12.3, strace, rsync |
| Missing | ffmpeg, cupy, cudf, cuml, clinfo, the EGL/GLES/OpenCL/Vulkan development headers |

The guest has no network (no NAT on the `caraxes0` tap, DNS fails), so sync-tests installs
nothing; it records what is missing. Adding NAT touches host network state and needs the
operator.

## 3. The gates

All four new gates need the VM up, nvidia.ko loaded (`tools/caraxes-vm.sh probe`), the
material synced, and the device trace readable. Each of these is a precondition: missing
means skip (exit 2), never fail. A guest runner that differs from
`tools/caraxes-guest-tests.sh` is a skip too, as a stale binary is in v1
(`lea_require_built`, `common.sh:565-630`).

### cuda

One ssh call per test, so each test gets its own window of the device's RPC trace. Scored
tests, in order:

| Test | v1 source | Criterion (v1's own) |
|---|---|---|
| cuda-init | `nvprobe 0` | `stage 0 ok (cuInit)` |
| cuda-context | `nvprobe 1` | `stage 1 ok` |
| cuda-core | `probe/matrix/cuda-core.sh` | `nvprobe 2`: memory written, read back, compared |
| cuda-jit | `probe/matrix/cuda-jit.sh` | `nvprobe 3`: PTX JIT, vecAdd result correct |
| cuda-launch | `probe/matrix/cuda-launch.sh` | `nvprobe 4` with blocking sync, ITERS > 0 |
| cuda-managed | `probe/matrix/cuda-managed.sh` | `managedprobe` stage 2 correct |
| cuda-hostreg | `probe/matrix/cuda-hostreg.sh` | two pinned roundtrips `bad=0 correct` |
| cuda-torch | `probe/matrix/cuda-torch.sh` | `torchprobe 5` with cuDNN, `stage 5 ok` |
| cuda-gdb | `probe/matrix/cuda-gdb.sh` | declares itself unsupported without cuda-gdb (exit 2) |
| rlprobe | `probe/python/rlprobe.py`, `NVRL_PIN=1` | exits 0 and prints `mean10=` |
| convburn | `probe/python/convburn.py` | exits 0 and prints `acc=` |
| robustness | v1 gpu gate stage (`test.sh:334-355`) | two `nvprobe 3` in parallel, SIGKILL mid-run then a correct run, nvidia.ko refcount rises while `/dev/nvidiactl` is open |

Unscored (information, as `probe/suites/README.md` says of the suites): the eight
`probe/suites` programs, run with the venv; a missing module is a skip.

For each failed test the guest runner re-runs the same command once under the v1 tracer
(`LD_PRELOAD=libnvrm_trace.so`, wrapped around the workload only, as `LEA_MATRIX_WRAP`
does) and reports the last RM ioctl whose status was not NV_OK. The host adds, from the
device's trace window, every control or allocation the device did not answer with NV_OK,
named from the vendor headers. The row therefore carries three things: the failing CUDA
call and its error code (from the program), the RM call that caused it (guest tracer), and
the device-side answer. The device counts include the traced re-run (so "x2" for one
failure).

### gpu

v1's `gate_gpu` (`scripts/test.sh:117-447`) on the stages a guest can carry, with v1's
criteria:

| v1 stage | Here | Note |
|---|---|---|
| tables | not run, fact | virtio_nvrm descriptor-table checksum; the stock driver has none |
| smi | run | guest `nvidia-smi` against the host's (read-only NVML), masked exactly as `test.sh:228-232` masks: header line, process list, digit runs, whitespace runs, both card names folded to `CARD` |
| memory | run | `nvprobe 2` |
| kernel | run | `nvprobe 3` correct and `managedprobe` twice to stage 2; v1's pin and pool accounting (`virtio_nvrm` parameters) has no counterpart |
| torch | run | `rlprobe` mean10 and `convburn` acc bit-identical to the native reference |
| robustness | run | as in cuda; v1's `rmmod virtio_nvrm` refusal becomes the nvidia.ko refcount, because an rmmod that succeeded would unload the guest driver |
| own | run | a torch process holding 64 MiB appears in the guest's own `nvidia-smi` process list |
| encode | run | guest half (NVENC 1080p, hardware decode 30 frames); v1's native byte-count comparison needs a host NVENC run |
| counter-check | not run | unloads virtio_nvrm |

The native torch reference is a CUDA workload on the host card. It runs only with
`CARAXES_GATE_NATIVE=1` (host venv `LEA_HOSTVENV`, default `../Leandro-Test/vendor/hostvenv`,
which does not exist today) and is stored in `caraxes/reference/rtx2070/torch-native.txt`;
without it the torch stage fails, as v1's does when `LEA_HOSTVENV` is missing
(`test.sh:315-318`).

Host reads made by `gpu`: `nvidia-smi` and `nvidia-smi --query-gpu=name` on the host
(NVML, read-only, the same reads the nvml gate makes).

### vdisplay, display

Both exit 2 with "not applicable yet". v1's vdisplay gate (`test.sh:452-729`) tests
virtio_nvrm's own virtual display (module parameter `vdisplay=1`, the module's EDID, a
frame read back through nvidia-drm); v1's display gate (`test.sh:734-1126`) needs
nvidia-drm with a connector and modeset, X, Vulkan presentation, Sunshine and Moonlight.
Caraxes serves no display engine and scope stays nvidia.ko and nvidia-uvm until the
compute path works (kickoff.md section 6). The facts in the JSON line record what the
guest has: today no nvidia-modeset or nvidia-drm module and no `/dev/dri` node.

## 4. How the verdict is computed (the v1 gate logic)

What made v1's gates trustworthy, and what these gates keep:

1. **One JSON line, nothing else on stdout** (`common.sh:640-672`):
   `{"gate":"cuda","result":"pass|fail|skip","dur_s":N,"reason":"...","facts":{"stages":[...],"failed":[...],...}}`.
   `reason` is absent on pass. Human text goes to stderr (`lea_gate_begin` moves stdout to
   fd 3 and points stdout at stderr, `common.sh:716-731`). Here `g_begin` does the same and
   also tees the transcript into the run directory.
2. **Exit 0 pass, 1 fail, 2 skip.** Skip only for a precondition (no guest, no material,
   no driver). "A stage could not measure" is a fail inside a running gate, because a
   skipped stage inside a green gate is a success claim without a reader
   (`common.sh:661-664`).
3. **Binary gates.** Any failed stage fails the gate (`lea_gate_finish`,
   `common.sh:812-835`), and the gate never learns to accept red; expected failures live
   in the suites runner, which is not a gate (`probe/suites/README.md`).
4. **A gate that dies still reports**: the EXIT trap emits a fail with the pending status
   (`_lea_gate_atexit`, `common.sh:733-747`); here `g_atexit`.
5. **Every stage compares against the native run** of the same program where v1 did
   (`smi` masked diff, `torch` bit-identical values, `encode` byte count within 4x), and
   masking is checked back verbatim (driver version and card name, `test.sh:212-237`).
6. **A probe must say what it proved**: a matrix probe that exits 0 without a
   `CRITERION:` line fails (`probe/run/matrix-guest.sh`), and exit 2 there is a declared
   reason not to run, never a pass.
7. **The aggregator cross-checks** each gate's JSON result against its exit code and
   counts a mismatch as a fail (`do_gates`, `test.sh:1131-1200`). Not reproduced here yet; run the gates
   one by one.

Caraxes additions: per-test rows for `cuda` (v1's gpu gate stops at the stage level),
the guest-tracer column, the device-trace column, the raw evidence saved per run.

## 5. Evidence

Each run writes `caraxes/runs/gate-<name>-<date>[-N]/`:

| File | Contents |
|---|---|
| `verdict.json` | the JSON line |
| `transcript.log` | the human transcript (stderr) |
| `table.txt` | the per-test table (cuda, gpu) |
| `guest-sync.txt` | the guest's `SYNC.txt` at the time of the run |
| `guest/<test>.log`, `.dmesg` | program output, the guest kernel log of the test's window |
| `guest/<test>.rm.tsv`, `.rm.out` | the traced re-run of a failed test |
| `device/<test>.trace` | the device's RPC events of the test's window (register accesses dropped) |
| `device/<first test>.full.trace` | the first test's window with register accesses |
| `device/window.trace`, `server-stderr.log` | the whole run's RPC events and the server's stderr |
| `smi-*.txt`, `smi.diff` | the gpu gate's smi stage |

Every text file carries an SPDX header; the JSON line cannot.

## 6. How to add a test

1. Put the program where v1 would: a C source in `../Leandro-Test/probe/c` with a
   Makefile rule, a Python workload in `probe/python`, or a matrix probe
   `probe/matrix/<name>.sh` with its header (`matrix-group`, `matrix-criterion`,
   `matrix-status`) and a `lea_matrix_criterion` line. Leandro-Test changes go through
   that repository, not from here.
2. If it is a C probe, add its target to `GUEST_PROBES` in `tools/caraxes-vm.sh`. A new
   Python file under `probe/python` also needs its name in the `sync_tests` include list.
3. Add the test to `tools/caraxes-guest-tests.sh`: its name in `CUDA_TESTS` (scored) or
   `SUITES` (unscored), and a `case` line in `one()`. A `cuda-*` name runs the matrix probe
   of that name automatically. The function prints one `row` with v1's criterion and calls
   `retrace` on failure.
4. `tools/caraxes-vm.sh sync-tests`, then `tools/caraxes-gate.sh cuda <name>` to see it
   alone, then the full gate.

## 7. Results (2026-09-23, vm0)

Device: `vfio-user-nvgpu` with `--boot-model --fb-mib 4096 --host-rm` (TU106, driver
610.57.04, FB 4096 MiB), this tree `e2f1aec` plus the uncommitted crate, Leandro-Test
`e046e04` (dirty). nvidia-uvm loaded with `uvm_disable_hmm=N`.

| Gate | Verdict | Exit | Run directory |
|---|---|---|---|
| nvml | pass (11 of 11 checks), re-run after the other gates | 0 | not saved per run (see [nvml.md](nvml.md)) |
| cuda (cuda-init alone) | fail | 1 | `caraxes/runs/gate-cuda-2026-09-23/` |
| cuda (all) | fail: 0 of 12 scored pass, 11 fail, 1 declared-unsupported | 1 | `caraxes/runs/gate-cuda-2026-09-23-2/` |
| gpu | fail: smi, memory, kernel, torch, robustness, own, encode | 1 | `caraxes/runs/gate-gpu-2026-09-23/` |
| vdisplay | skip, not applicable yet | 2 | `caraxes/runs/gate-vdisplay-2026-09-23/` |
| display | skip, not applicable yet | 2 | `caraxes/runs/gate-display-2026-09-23/` |

cuda, per test (`table.txt` of `gate-cuda-2026-09-23-2`). "RM" is the last failed RM call
of the traced re-run, "device" the non-OK device answers in the test's window:

| Test | Scored | Result | Guest error | RM (guest tracer) | Device |
|---|---|---|---|---|---|
| cuda-init | yes | fail | `ERROR cuInit: 100 no CUDA-capable device is detected` | RM_CONTROL `NV2080_CTRL_CMD_BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS` -> 0x56 | same control 0x56, unanswered |
| cuda-context | yes | fail | `ERROR cuInit: 100` | same | same |
| cuda-core | yes | fail | `ERROR cuInit: 100` | same | same |
| cuda-jit | yes | fail | `ERROR cuInit: 100` | same | same |
| cuda-launch | yes | fail | `ERROR cuInit: 100` | same | same |
| cuda-managed | yes | fail | `ERROR cuInit: 100` | same | same |
| cuda-hostreg | yes | fail | `A cuInit : 100 no CUDA-capable device is detected` | same | same |
| cuda-torch | yes | fail | `AssertionError: no CUDA device visible` | same | same |
| cuda-gdb | yes | skip | `declared-unsupported: ... no cuda-gdb` | - | none |
| rlprobe | yes | fail | `AssertionError: no CUDA card visible` | same | same |
| convburn | yes | fail | `RuntimeError: No CUDA GPUs are available` | same | same |
| robustness | yes | fail | 0 of 3 runs correct (`ERROR cuInit: 100`); refcount 1 -> 2 | - | same, x4 |
| suite-test_async_streams | no | fail | `RuntimeError: No CUDA GPUs are available` | - | same |
| suite-test_cuda_graphs | no | fail | same | - | same |
| suite-test_high_freq_event_polling | no | fail | same | - | same |
| suite-test_multi_process_cuda | no | fail | same | - | same |
| suite-test_nvrtc | no | skip | missing module: cupy | - | none |
| suite-test_uvm_migration | no | skip | missing module: cupy | - | none |
| suite-test_vram_churn | no | fail | `RuntimeError: No CUDA GPUs are available` | - | same |
| suite-rapids_cuml_cudf | no | skip | missing module: cudf | - | none |

gpu, per stage (`gate-gpu-2026-09-23`): smi fails on one masked field only, the Disp.A
column, which is host display state the device does not forward:

```
  host  |   0  NVIDIA GeForce RTX 2070        Off |   00000000:2D:00.0  On |                  N/A |
  guest |   0  Leandro RTX 2070-4G            Off |   00000000:00:05.0 Off |                  N/A |
```

memory, kernel, robustness, own and torch fail at `cuInit` 100 as above; encode fails on
"no ffmpeg in the guest"; torch would also fail for want of a native reference.

Every CUDA program fails at the same place: `cuInit` returns 100
(`CUDA_ERROR_NO_DEVICE`) right after the device answers
`NV2080_CTRL_CMD_BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS` with NOT_SUPPORTED.

### 7a. Results after E5c (2026-09-23, vm2)

Device on vm2 with the E5b part 1 memory model plus the E5c rows (experiments.md, "E5c: CUDA
initialization"). Runs under `caraxes/runs/e5c-2026-09-23/` (the gate's `CARAXES_GATE_RUNS`).

| Test | Result | Guest error / criterion | Run |
|---|---|---|---|
| cuda-init | **pass** | `stage 0 ok (cuInit)` | c1, c2, c3 |
| cuda-context | **pass** | `stage 1 ok (context, sched=auto)` | c2, c3 |
| cuda-core | **pass** | `nvprobe 2: device memory written, read back and compared` | c2, c3, c5 |
| cuda-hostreg | **pass** | `2 pinned-memory roundtrips (HostRegister and HostAlloc) compared byte for byte` | c3 |
| cuda-jit | fail | kernel launch refused by the device (the execution wall) | c3, c5 |
| cuda-launch | fail | timeout (launch) | c3 |
| cuda-managed | fail | `AllocManaged a: 801 operation not supported` (no device refusal) | c3 |
| cuda-torch | fail | stages 0-1 ok, then `H2D/D2H round trip broken` (the copy waits on a kernel) | c3 |
| cuda-gdb | skip | no cuda-gdb | c3 |
| rlprobe, convburn | fail | timeout (kernels) | c3 |
| robustness | fail | `0 of 3 runs correct (WRONG at 1: 0.000000 != 3.000000)` | c3 |

4 of 12 scored tests pass (0 before). Everything that fails now fails at the first kernel
launch or behind it, except cuda-managed. A single test can take 40-120 s on this device
(context creation runs about 200 emulated pushes); use `CARAXES_TEST_TIMEOUT=180` or more.

### 7b. Results after E5b part 2 (2026-09-23, vm2)

Device on vm2 with `CARAXES_HOST_EXEC=1` (real VRAM behind the FB, mirror channels), design in
[execution.md](execution.md). Runs under `caraxes/runs/e5b2-2026-09-23/`.

| Test | Result (default: host channels off) | Note |
|---|---|---|
| nvml gate | **pass** (11/11) | on the reserved VRAM, no Xid |
| `uvm verify 1` | **pass** (`rmStatus 0x0`) | on the reserved VRAM |
| cuda-init | **pass** | on the VRAM store |
| cuda-context | **pass** | on the VRAM store |
| cuda-core | **pass** | device memory written/read/compared, now in real VRAM |
| cuda-hostreg | **pass** | pinned roundtrips |
| cuda-jit, cuda-launch, rlprobe, convburn, robustness, cuda-torch | fail | the kernel-launch wall, as in §7a, now on real VRAM |
| cuda-managed | fail | UVM managed alloc 801 (guest side, unchanged) |

The same four scored tests pass as after E5c. Update 2026-09-23: with
`CARAXES_MIRROR_CHANNELS=1` the four now pass **with CUDA's CE channels executing on the
physical copy engines** (gate `gate-cuda-2026-09-23-13`, 0 RC, 0 Xid): cuda-core writes,
reads back and compares device memory through copies the physical GPU ran, walking the
guest's own page tables. Two device fixes made this work (execution.md §6): the guest reads
the *host* work-submit token back from its notifier page (the device maps it to the guest
channel), and the guest's register `MMU_INVALIDATE` is replayed as a privileged
`MEM_OP MMU_TLB_INVALIDATE` of the guest PDB from a device-owned channel (`DMA_INVALIDATE_TLB`
on the mirror VAS is a no-op, so a fresh mapping had faulted FAULT_PDE). The earlier "CE
first-launch stall" was an artifact of a heavy per-doorbell debug poll, now gated off
(`CARAXES_MIRROR_DIAG=1`). The remaining wall is the GR kernel launch (cuda-jit and beyond),
execution.md §6 wall 2.

## 8. The CUDA path: what the device does not answer yet

The smallest CUDA program in the material is `nvprobe 0` (`cuInit` alone). It ran alone
(`gate-cuda-2026-09-23`, test `cuda-init`) with a clean window. The guest-side RM calls of
that run (`guest/cuda-init.rm.tsv`) are identical, call for call, to the v1 native trace
of the same program on the real card (`probe/traces/lvl0.tsv`, git-ignored, from the v1
matrix work; its driver version is not recorded in the file) for the first 48 calls. Call
49 is where they part.

Observed, in order (the device's window, `device/cuda-init.trace`):

| # | RPC | Object / command | Device answer today | Label | Guest-visible result |
|---|---|---|---|---|---|
| 1 | GSP_RM_ALLOC | NV01_ROOT (client) | NV_OK | synthesized (object recorded, no host object) | none |
| 2 | GSP_RM_ALLOC | NV01_DEVICE_0 (0x80) | NV_OK | synthesized (object recorded, no host object) | none |
| 3 | GSP_RM_ALLOC | NV20_SUBDEVICE_0 (0x2080) | NV_OK | synthesized (object recorded, no host object) | none |
| 4 | GSP_RM_ALLOC | 0x2081 (subdevice BINAPI object) | NV_OK | synthesized (object recorded, no host object) | none |
| 5 | GSP_RM_CONTROL | 0x20810108 (BINAPI) | NV_OK | host, forwarded opaquely | none |
| 6 | GSP_RM_CONTROL | 0x20801823 `BUS_GET_INFO_V2` | NV_OK | host, public control | none |
| 7 | GSP_RM_CONTROL | 0x2080182a `BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS` | **0x56 NOT_SUPPORTED** | unanswered | libcuda frees the device, subdevice and client, calls `UVM_DEINITIALIZE`; `cuInit` returns **100 CUDA_ERROR_NO_DEVICE** |
| 8-11 | GSP_RM_FREE | 0x2081, subdevice, device, client | NV_OK | synthesized | none |

The real card answers row 7 with NV_OK (native trace line 91). It is flagged
`ROUTE_TO_PHYSICAL` (flags 0x40048, `src/nvidia/generated/g_subdevice_nvoc.c:10869`), so
the kernel RM always forwards it; there is no row for it in the device crate.
`UVM_INITIALIZE` in this run: flags 0x0, rmStatus 0x0 (guest tracer). CUDA passes flags 0,
so the HMM condition of E5a applies: `UVM_REGISTER_GPU` will need `uvm_disable_hmm=1` on
this guest (today `N`) until the VM gets a free physical range ([uvm.md](uvm.md)).

Predicted next, not observed: the rest of the native `cuInit` sequence after the
divergence (`probe/traces/lvl0.tsv` lines 91-182, 42 RM calls). Controls flagged
`ROUTE_TO_PHYSICAL` reach the device as GSP_RM_CONTROL; the others are answered by the
guest kernel RM from its static info or trigger internal RPCs of their own. "Row in crate"
means the id or name occurs in `crates/vfio-user-nvgpu` (a hint, not proof that the
answer is right).

| Native order | Call | ROUTE_TO_PHYSICAL | Row in crate | Native status |
|---|---|---|---|---|
| 1 | `GPU_QUERY_ECC_STATUS` 0x2080012f | yes | v610.rs | 0x56 (tolerated natively) |
| 2 | `FB_GET_INFO_V2` 0x20801303 | no | nvml.rs, hostmem.rs | ok |
| 3 | `GPU_GET_ENGINES_V2` 0x20800170 | no | none | ok |
| 4 | `GPU_GET_SIMULATION_INFO` 0x20800119 | no (cacheable) | none | ok |
| 5 | `NV0000 GPU_GET_MEMOP_ENABLE` 0x27b | no | none | ok |
| 6 | `GPU_GET_INFO_V2` 0x20800102 | no | nvml.rs | ok |
| 7 | `GR_GET_INFO` 0x20801201 | no | kgr.rs | ok |
| 8 | `GR_GET_GPC_MASK`, `GR_GET_TPC_MASK` x3, `GR_GET_CAPS_V2` | no | kgr.rs | ok |
| 9 | `CE_GET_ALL_CAPS` 0x20802a0a | no | none | ok |
| 10 | `GPU_GET_SKYLINE_INFO`, `GR_GET_GLOBAL_SM_ORDER` | no | kgr.rs (SM order) | ok |
| 11 | `GRMGR_GET_GR_FS_INFO` 0x20803801 | **yes** | **none** | ok |
| 12 | `NV0080 FB_GET_CAPS_V2` 0x801307 | no (cacheable) | none | ok |
| 13 | alloc `TURING_USERMODE_A` 0xc461, `CLIENT_GET_ADDR_SPACE_TYPE`, map it | alloc | none | ok |
| 14 | alloc `RM_USER_SHARED_DATA` 0xde, `CLIENT_GET_ADDR_SPACE_TYPE`, map it | alloc | none | ok |
| 15 | `GSP_GET_FEATURES` 0x20803601 | yes (cacheable) | ctrl.rs | ok |
| 16 | alloc `FERMI_VASPACE_A` 0x90f1 x2 | alloc | channel.rs (records VA spaces, E5a) | ok |
| 17 | `GPU_GET_SHORT_NAME_STRING`, `GPU_GET_NAME_STRING`, `GPU_GET_GID_INFO`, `GPU_QUERY_COMPUTE_MODE_RULES`, 0x20808159 | no | none | ok |
| 18 | `NV0000 SYSTEM_GET_FABRIC_STATUS`, `RC_RELEASE_WATCHDOG_REQUESTS`, `RC_SOFT_DISABLE_WATCHDOG`, `SYSTEM_GET_P2P_CAPS_MATRIX`, 0x20808162 | no | none | ok |
| 19 | `BUS_GET_C2C_INFO` 0x2080182b | **yes** | **none** | ok |
| 20 | `NVLINK_GET_NVLINK_STATUS` 0x20803002 | no | none | ok |
| 21 | `UVM_REGISTER_GPU` (37), `UVM_PAGEABLE_MEM_ACCESS_ON_GPU` (70) | UVM | E5a path | ok |
| 22 | alloc `NV50_MEMORY_VIRTUAL` 0x50a0 | alloc | none | ok |

So the controls that certainly reach the device and have no row are
`BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS`, `GRMGR_GET_GR_FS_INFO` and `BUS_GET_C2C_INFO`;
`UVM_REGISTER_GPU` then needs the HMM aid. Beyond `cuInit`, the context stage (`nvprobe 1`,
native `lvl1.tsv`, 1258 lines) creates the channel, the compute and copy objects and the
GR context, which the device today only emulates in software for the copy engine
(`channel.rs`: host methods and the `TURING_DMA_COPY_A` subset UVM uses, against the fake
FB); no compute class runs anywhere. That is E5b and E5c.

### 8a. Update after E5c (2026-09-23)

Every control of the tables above is now answered (the divergence at call 49 and the
predicted rows 3-22), and so is the context path; the complete ordered list with its
classification is [cuda-lookahead.md](cuda-lookahead.md). What still reaches the device
unanswered in a CUDA program: nothing in stages 0-2. The wall is not an RPC but a push: the
first `LOAD_INLINE_QMD_DATA(63)` on the GR channel (a kernel launch), whose QMD release
semaphore the following CE copy acquires. The device refuses and logs it
(`compute KERNEL LAUNCH NOT EXECUTED`); experiments.md E5c lists what part 2 must provide.

## 9. Gaps in the v1 material for Caraxes

- The v1 gpu gate's `tables` and counter-check stages, and its pin and pool leak checks,
  belong to virtio_nvrm; Caraxes needs its own leak check (for example device-side object
  counts after SIGKILL) to make the robustness stage as strong as v1's.
- No native reference: the torch stage and the encode comparison need a native run on the
  host card, which is the operator's call; v1's `vendor/hostvenv` does not exist.
- The guest image lacks ffmpeg (encode), CuPy and RAPIDS (three suites), cuda-gdb, and the
  EGL, OpenCL and Vulkan headers (six probes); the guest has no network to install them.
- The smi stage's mask was written for a guest that forwards all host display state; Disp.A
  differs by design here. Whether to mask it is a decision for the D9 cut, not for the gate.
- The display gates presuppose virtio_nvrm's virtual display; Caraxes needs its own display
  engine before any of them applies.
- The native traces used above (`probe/traces/*.tsv`) are untracked leftovers without a
  recorded driver version; a fresh native trace set for 610.57.04 would make the lookahead
  exact.
