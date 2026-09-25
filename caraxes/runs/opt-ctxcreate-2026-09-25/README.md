<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# cuCtxCreate, 2026-09-25 (vm3, 610 guest)

Design, analysis and numbers: [docs/caraxes/ctx-create.md](../../../docs/caraxes/ctx-create.md);
the hold refinement: [docs/caraxes/bar1-pool.md](../../../docs/caraxes/bar1-pool.md) §9.1.

**Environment.** vm3 only (1 GiB at reservation slot 5, tap caraxes3), the gate610 environment
of `caraxes/runs/gsp-rpc-2026-09-24/run.sh` plus the recommended configuration, as
`caraxes/runs/matrix-2026-09-25/env.sh` `mx_guest 610` sets it: host RM, host execution, the GR
shadow, interrupt page, BAR1 pool 32 MiB, token pool 64, hybrid reactor,
`CARAXES_BAR2_MEMFD=1`, `CARAXES_MSI_REARM_IOEVENTFD=1`, `CARAXES_ALLOC_LAZY=1`. Binaries are
frozen as `/mnt/vmstore/caraxes/vfio-user-nvgpu-ctx610-<id>`; each run's `setup.txt` has the
sha256. The host was shared with other agents' VMs (vm0, vm2, vm5, vm1 and a v1 VM); each run's
`other-vms-at-start.txt` lists the device processes that were up.

| Directory | Binary | What |
|---|---|---|
| `before` | d96cc59 (the start of the day, the §9 hold on) | census, init and churn |
| `part1`, `part1-allocburn` | 3880068 (part 1: the hold refinement) | census, init and churn; 20 x convburn plus allocburn |
| `wip2`, `wip3` | uncommitted steps of part 2 (prefetch keyed on TSG roots only; with context shares) | census, init and churn |
| `before-2` | d96cc59 | the same, for the scatter |
| `wip4` | part 2 plus a retire of host channels at the guest's free (freed with the TSG), not kept | census, init and churn |
| `part2` | 56aacd2 (part 2: prefetch, build phases) | census, init and churn |
| `after`, `after-allocburn` | 9f712dc (the end state) | census, init and churn, the cuda gate; 20 x convburn plus allocburn |
| `before-3`, `after-2`, `after-noprefetch` | d96cc59; 9f712dc; 9f712dc with `CARAXES_SHADOW_PREFETCH=0` | census, init and churn, back to back |

Per run directory: `setup.txt`, `other-vms-at-start.txt`, `device-start.txt`, `device-pid.txt`,
`device-summary.txt`, `device-mirror-summary.txt` (the mirror's exit line: build phases, host
frees, prefetch, untrapped-GPPut counters), `hostwatch.txt`, `host-xid.txt` (the host's Xid lines
with the device pid); for the census: `census.txt`, `census-trace.txt.xz`, `census-table.md`,
`census.json`, `census-rpc-cost.md` (host RM call histogram in section 3), `phases.md`
(`phases.py`), `census-device-lines.txt`; `v2/` with bench-cu init and churn (3 reps); for the
loops `loop.txt`, `pool-per-iter.txt`, `device-events.txt`. The cuda gate's record of `after`
(75 MB of device traces and transcript) is in
`/mnt/vmstore/caraxes/opt-ctxcreate-2026-09-25/after/gate/`; `after/gate-cuda.txt` has its table.

Scripts:
- `run.sh measure|allocburn LABEL` with `BIN=` the frozen binary (`GATE=1` adds the cuda gate,
  `N=` the loop's iterations); refuses to start while a vm3 process runs, retries a refused
  reservation every five minutes, brings vm3 down at the end;
- `series.sh`: the closing series (`after` to `after-noprefetch`);
- `phases.py TRACE`: the split of each cuCtxCreate and cuCtxDestroy window (trapped, first-doorbell
  builds by phase, untrapped gaps, background worker, host channel frees);
- `compare.py RUN ...`: the rows of ctx-create.md per run.
