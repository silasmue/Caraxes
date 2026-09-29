<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# The shadow's host RM calls, 2026-09-25 (vm0)

Backlog item 2 of [docs/caraxes/optimization-backlog.md](../../../docs/caraxes/optimization-backlog.md).
Design, fail-closed argument and results: [docs/caraxes/memory-model.md](../../../docs/caraxes/memory-model.md) §7.

**Environment.** vm0 only, 1 GiB at reservation slot 0, 610 guest. `run.sh` sources
`caraxes/runs/intr-page-2026-09-24/env.sh` and changes it:
- no display;
- host RM and host execution, the GR shadow;
- token pool 64, hybrid reactor, interrupt page;
- `CARAXES_BAR2_MEMFD=1`, no BAR1 pool.

Binaries are frozen under `/mnt/vmstore/caraxes/vfio-user-nvgpu-opt2-<commit>`, and the sha256
of each is in its run's `setup.txt`.

| Directory | Binary | Switches | What it isolates |
|---|---|---|---|
| `before/` | 4fec5bd | `CARAXES_SHADOW_DEFER_TLB=0 CARAXES_SHADOW_REGION_POOL=0` | the calls of main d466e9d, with the new counters |
| `defer-pool/` | 4fec5bd | defaults | deferred TLB invalidation and the region free list |
| `final/` | f0edbb8 | defaults | whole-region frees, deferral off |
| `gather/`, `gather-2/` | ac950e9 | defaults | gather descriptors. vm3's device was busy during `gather`'s bench runs, so `gather-2` repeats the part with no other VM active |

Per run directory:
- `setup.txt`, `device-start.txt`, `device-summary.txt`, `device-pid.txt`: the environment and the
  device's lines;
- `census.txt`: census-cu's `OP` lines. The census is cuInit x3, cuCtxCreate/cuCtxDestroy x5, and
  the ops x10, as in `caraxes/runs/trap-census-2026-09-25/run.sh`;
- `census-trace.txt.xz`: the device trace of the census processes;
- `census-table.md`, `census.json`: `caraxes/tools/census-table.py`;
- `census-rpc-cost.md`: `caraxes/tools/rpc-cost.py`, whose section 3 has the host RM call
  histogram by kind;
- `calls.md`: `calls.py`, the shadow's host RM calls per census operation and per context window;
- `v2/`: `bench-cu` init and churn, 3 reps each (`caraxes/tools/bench-run.sh series`);
- `cuda-gate.txt` and `gate-cuda/` (not in `before`), `hostwatch.txt`, `host-xid.txt`. The gate
  wrote its record to `caraxes/runs/gate-cuda-2026-09-25[-N]/`, and it was moved here. The
  gate's device traces and transcript (80 MB per gate) are in
  `/mnt/vmstore/caraxes/opt-shadow-maps-2026-09-25/<run>/gate-cuda/`.

Tables over all runs:
- `compare.md`: `compare.py`, the backlog's rows before and after, and every context window;
- `bench-summary.md`: `caraxes/tools/bench-summary.py` over the `v2/` runs.

`tables.sh` regenerates all of them from the traces.

**Results** (memory-model.md §7.5):
- host RM calls per cuCtxCreate: 13171 to 228 (worker apply 770 to 67 ms);
- cuCtxCreate: 815 to 309 ms (native 146);
- cuCtxDestroy: 170 to 107 ms;
- the whole census's shadow host time: 4.8 s to 0.72 s.

**Checks.**
- cuda gate 11 of 12 in `defer-pool`, `final`, `gather` and `gather-2`. cuda-managed fails with
  its known 801.
- Host Xid: 0 lines in every part. The device pids are in `device-pid.txt`.
- Host watch: no alert.

Reproduce, from the worktree root (`caraxes/reference/rtx2070/rom.bin` and `synth-vbios.bin`,
and `target/bench/bin` with `census-cu` and `census.cubin`, as for the trap census):

```sh
caraxes/runs/opt-shadow-maps-2026-09-25/run.sh before|defer-pool|final|gather [label]
caraxes/runs/opt-shadow-maps-2026-09-25/tables.sh
```

`run.sh` refuses to start while a vm0 process runs. It retries a refused reservation after
five minutes, and brings vm0 down at the end.
