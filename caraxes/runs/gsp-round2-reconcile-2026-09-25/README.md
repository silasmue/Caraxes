<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# The reconciliation of the C table and the D models: runs of 2026-09-25

The GPU runs for commit 1a96d24 (one RC journal, one set of scheduling flags, the five
shadowing C rows removed; docs/caraxes/gsp-plan/c-guest-state.md §8 "Reconciliation"). Host
driver 615.71.09. `run.sh <phase> <binary> 610|615 [gates|probe]`: 610 on vm3 (the environment
of caraxes/runs/gsp-rpc-2026-09-24/run.sh gate610: 1 GiB at reservation slot 5, tap caraxes3),
615 on vm5-615 (the environment of caraxes/runs/gsp-round2-c-2026-09-24/run.sh: 1 GiB at slot
2, tap caraxes5); both the host-execution configuration with token pool 64, hybrid reactor, no
BAR1 area. It waits in five-minute steps while a process of the VM runs, never overwrites a
phase and brings the VM down at its end. Binaries built from 1a96d24: 610
`target/reconcile` (sha256 218c362e), 615 `target/reconcile615` (0b5646ee); `setup.txt` of each
phase names sha256 and environment.

| Phase | Guest | What | Result | Device pid | Host Xid |
|---|---|---|---|---|---|
| probe-615 | 615.71.09 | `CARAXES_GOE_SELFTEST=5`, `CARAXES_RC_SELFTEST=20`; goe-probe, RC journal probe | goe-probe: 11 records drained (9 `CARAXES-TEST` self-test, 2 `GPU-RC` of the SIMULATED RCs); journal probe: count 3, V2 records 0-2 with the RmProtoBuf header (group 1, type 131), record 3 empty, count 0 after RC_SET_CLEAN_ERROR_HISTORY | 1745596 | 0 |
| gate-615 | 615.71.09 | nvml and cuda gates | nvml pass; cuda: every scored stage passes but cuda-managed (801, managed memory, as in every gate of the day), cuda-gdb skipped | 1763462 | 0 |
| probe-610 | 610.57.04 | RC journal probe | probe error: `/dev/nvidiactl` missing (nothing had opened the GPU; the 610 image has no nvidia-smi on PATH); vm3 was busy for 40 min before (`busy.txt`) | see `device-pid.txt` | 0 |
| gate-610 | 610.57.04 | nvml and cuda gates | nvml fail on power-number (N/A, as in every 610 gate); cuda as 615: all scored stages pass but cuda-managed, cuda-gdb skipped | 1948012 | 0 |
| probe-610-2 | 610.57.04 | RC journal probe | probe error: the fix called nvidia-smi, which the 610 image lacks | see `device-pid.txt` | 0 |
| probe-610-3 | 610.57.04 | RC journal probe (device nodes made with mknod) | count 1 (the SIMULATED RC, also logged by the guest as `NVRM: Xid ... 0, Caraxes: SIMULATED RC 1`), V2 record 0 with the header, record 1 empty, count 0 after the clean | 2005662 | 0 |

**The journal round trip.** A guest user process cannot log an Xid itself:
`INTERNAL_LOG_OOB_XID` is INTERNAL (flags 0xc0, g_subdevice_nvoc.c:6616-6628 of 610.57.04) and
sent by the guest kernel only for its own Xids (gpu.c:7655-7703). The records therefore come
from the device's self-test (`CARAXES_RC_SELFTEST`, a SIMULATED RC of no channel, boot.rs
`rc_selftest`), which goes the way of every device RC: one journal record, OS_ERROR_LOG (the
guest's Xid line), and on 615 one operational-event record. The probe
(caraxes/tools/rc-journal-probe.c) reads it back through the guest kernel with
RC_GET_ERROR_COUNT, RC_GET_ERROR_V2 and RC_SET_CLEAN_ERROR_HISTORY; the device trace shows the
C rows reading rc.rs's journal and the D model clearing it (`probe-615/device-controls.txt`).
`LOG_OOB_XID` itself into the same journal is covered by boot.rs
`the_d_models_rc_reaches_the_guest` (the 615 transport harness, both builds).

**Xid.** No host Xid line in any phase's window (`host-xid.txt`); the host watch saw no alert.
