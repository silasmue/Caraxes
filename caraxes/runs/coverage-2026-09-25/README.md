<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Coverage map, 2026-09-25 (vm2)

The runs behind [docs/caraxes/coverage-map.md](../../../docs/caraxes/coverage-map.md). The
environment is `run.sh`'s:
- vm2, 610 guest, 1 GiB at reservation slot 3, host RM and execution;
- the night environment (`../night-2026-09-24/env.sh`) plus the recommended configuration:
  interrupt page, token pool 64, hybrid reactor, BAR1 pool 32 MiB, BAR2 memfd, MSI re-arm
  ioeventfd, lazy plans.

The devices are built from this branch:
- `target/cov/release` for the overhead parts;
- `target/cov-instr/release`, built with `-C instrument-coverage`, for `map` and `extra`.

vm0, vm3, vm5 and vm7 were up during every part (`*/other-vms-at-start.txt`).

| Directory | What | Result |
|---|---|---|
| `overhead-off/`, `overhead-on/` | short-auto x5 without and with `CARAXES_PROFILE=1` | medians 0.020 and 0.075 ms per launch, 2 reps per side at about 5 ms (GR held by another VM); `overhead-on/profile/` is the first profile with one row per BAR0 register (51131 rows) |
| `aborted-map/` | the first `map`, stopped during the nvml gate to fold BAR0 rows per page | nothing measured |
| `map/` | the workload series, one SIGUSR2 dump per workload boundary (`marks.tsv`, `profile/dev.<n>.tsv.xz`), `work/<workload>.txt`, `inventory.txt`; the cuda gate's own run directory in `gate-cuda-run/` (device traces xz) | 25 workload windows; nvml gate fails on power N/A (known for the 610 guest), cuda gate 10 pass, 1 fail (cuda-managed, 801), 1 declared unsupported (cuda-gdb); host Xid 109 (kill -9) and 31 (illegal store) on the device's pid |
| `extra/` | streams and two processes without `cuMemAllocAsync`, NVRTC with its builtins, the control sweep (`work/ctlsweep-table-out.txt`) | 111 of 119 swept controls reached the device; 0 host Xid |
| `overhead-on-2/`, `overhead-off-2/` | short-auto x10 each, profile on first | medians 0.022 and 0.021 ms per launch: within noise |

Tables, all regenerated from the records with `CARAXES_PROFILE_ABI=gsp-610.57.04`:
- `combined-coverage-map.md` and `combined-coverage.json`: `coverage-map.py map extra`, sections
  (a) to (c) of the doc;
- `map/coverage-map.md` and `map/coverage.json`: `coverage-map.py map`, the input of the sweep's
  `--only`;
- `sweep-reach.md`: `sweep-reach.py`;
- `ctlsweep-table.h`: `../../tools/ctlsweep-gen.py --priv --controls caraxes/abi/gsp-610.57.04/controls.tsv --only map/coverage.json`;
- `llvm-functions.md` and `.tsv`, `llvm-map-totals.txt`: `llvm-cov.sh tests` then
  `llvm-cov.sh report`. The raw LLVM profiles and exports are in
  `/mnt/vmstore/caraxes/coverage-2026-09-25/`.

Host state read:
- `nvidia-smi` of the host watch and of the nvml gate;
- `journalctl -k -g Xid`.

Nothing on the host was changed. The sweep's host-forwarded controls are getters, sent through
the device's allowlist (public.rs) on its own unprivileged host client.

Reproduce, from the worktree root:

```sh
caraxes/runs/coverage-2026-09-25/run.sh map        # then extra; REPS=10 run.sh overhead-on|off 2
bash caraxes/runs/coverage-2026-09-25/llvm-cov.sh tests && bash caraxes/runs/coverage-2026-09-25/llvm-cov.sh report
cd caraxes/runs/coverage-2026-09-25 && CARAXES_PROFILE_ABI=gsp-610.57.04 python3 coverage-map.py map extra --json combined-coverage.json > combined-coverage-map.md
```

`run.sh` needs:
- the git-ignored `caraxes/reference/rtx2070/rom.bin` and `synth-vbios.bin`, `local.env` and
  `target/bench`, linked from the main checkout;
- `sudo -n setcap` for the device copy.

Do not edit `run.sh` while it runs.
