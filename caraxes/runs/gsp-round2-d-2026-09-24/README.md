<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Category D, round 2: runs of 2026-09-24

The runs of round 2 (docs/caraxes/gsp-plan/d-device-model.md §10 "Round 2"). GPU: vm3 only,
one VM at a time, other VMs on the card concurrently; vm3 down after every run.

| Path | What |
|---|---|
| `run.sh` | `SERVER=<binary> run.sh gate610\|gate615 <label> [zbc-probe]`: the environment of `../gsp-rpc-2026-09-24/run.sh` gate610/gate615 (1 GiB profile at reservation slot 5, token pool, hybrid reactor, no BAR1 area), the nvml and cuda gates, then the ZBC probe in the guest (`zbc-probe --set`) |
| `gate610-before/`, `gate615-before/` | parent 57693d8 (binaries 5d18010b, 237030dc); the 610 before run without the probe |
| `gate610-after/`, `gate615-after/` | 55d37b9 (binaries 18dc2fbe, b3262c05), with the probe |
| `*/device-dcontrols-gates.txt` | the D and ZBC controls of the gate window (empty in all four) |
| `*/device-zbc-probe.txt`, `*/zbc-probe.txt` | the probe's controls as the device answered them, and the probe's own output |
| `*/host-xid.txt`, `*/host-xid-device.txt` | every host Xid line of the run's window, and those of the run's device pid (none) |
| `q5-host-sweep.sh`, `q5-host-sweep/` | Measurement Q5: the one read-only sweep of the host's ZBC table (caraxes/tools/zbc-probe.rs without `--set`), every RM call with its status in `sweep.txt` |

The before runs used `run.sh` before its device-pid Xid filter was added; their
`host-xid-device.txt` were written by hand with the pid from `up.txt` (1094752, 1138022).
