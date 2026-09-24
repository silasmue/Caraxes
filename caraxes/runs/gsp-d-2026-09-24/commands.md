<!-- SPDX-License-Identifier: MIT -->
# Category D runs on vm0 (2026-09-24)

The runs of docs/caraxes/gsp-plan/d-device-model.md §10: the display gate before and after the
rule table (R0, 5eac90a) and the display models (R1, ca598ec), Q1 (DP unplug and plug) and Q3
(kill -9 of a CUDA process). Guest runs only, no host read. vm0 at slot 0 with the 2 GiB
profile, the display page series' environment with the interrupt page and the display page on
(`env.sh`), 610.57.04 guest. Each session brings vm0 up, runs its stages and brings it down.

```sh
# from the worktree root
caraxes/runs/gsp-d-2026-09-24/series.sh before /mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-gspd-base gate
caraxes/runs/gsp-d-2026-09-24/series.sh after  /mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-gspd gate q1
caraxes/runs/gsp-d-2026-09-24/series.sh q3     /mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-gspd q3
caraxes/runs/gsp-d-2026-09-24/series.sh after2 /mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-gspd gate
```

`vfio-user-nvgpu-gspd-base` is the release build of the branch base 0899c72,
`vfio-user-nvgpu-gspd` that of ca598ec (`binary.txt` per session has the sha256).

| Session | Device pid | Result | Host Xid |
|---|---:|---|---|
| before | 402214 | display gate PASS 11/11 (fence median 0.11 ms, present 58.8 FPS) | none |
| after | 436562 | display gate 10/11: `events` FAIL, fence median 1.42 ms (present 50.9 FPS); Q1 | none |
| q3-attempt1 | 502691 | not usable: the gate runner was not synced and the device nodes missing (cuInit 100) | none |
| q3 | 515485 | cuda gate 10 of 12 scored pass (`cuda-managed` fails on `GPU_QUERY_ECC_STATUS` as in every cuda gate of the day); kill -9 window | one Xid 31 at 19:38:37 on pid 508533, the vfio-user-nvgpu of vm1 (another VM, started 19:33:34), not this device |
| after2 | 558559 | display gate PASS 11/11 (fence median 0.14 ms, present 58.7 FPS) | none |

The `after` session's `events` failure is timing, not the change: the device's control answers
in the gate windows of before, after and after2 are identical (every id, status, label and
count; 2605 answers each), the rerun passes, and during `after` two other VMs (vm2 and
vm3-615) ran on the same GPU with a host load of 4 to 5.

`*/dscan.txt` and `*-dscan.txt` list the D controls of each window (`dscan.py`). The full gate
trace windows (15 to 25 MB each compressed) are kept in the VM store,
`/mnt/vmstore/caraxes/vm0/gsp-d-2026-09-24/`; the Q1 and kill -9 windows are here.
