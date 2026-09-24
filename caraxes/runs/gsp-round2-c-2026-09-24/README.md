<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GSP plan round 2, category C: runs of 2026-09-24/25

The GPU runs of the round-2 category C work (docs/caraxes/gsp-plan/c-guest-state.md §8
"Round 2"; commits 672d561 TSG key, 5591b9c IDLE_CHANNELS, e52824f the 17 D-plan C rows,
8989bbc the open B rows, 1f8d564 operational-event records). vm5 only, 1 GiB profile at
reservation slot 2 ([top-3 GiB, top-2 GiB)), the host-execution configuration of
`caraxes/runs/doorbell-2026-09-24/env-vm1.sh` otherwise (token pool 64, hybrid reactor, no
BAR1 area); host driver 615.71.09. `setup.txt` of each phase names the binary's sha256 and the
environment. The binaries: "before" is the base 57693d8 (`target/round2c-base*`), "after" the
round's tree at 1f8d564 (default and `--features v615`).

Scripts: `prepare-vm5-615.sh` and `seed-vm5-615.sh` made `/mnt/vmstore/caraxes/vm5-615` (an
overlay on the 615 base image as vm3-615, vm5's seed with a new instance id); `run.sh <phase>
<binary> 610|615 [probe]` (refuses while a vm5 or bench-v1 process runs, waits and retries on a
headroom refusal, brings the VM down, never overwrites a phase; `RUN_GATES=0` skips the
gates); `compare.sh` writes `compare.txt`. No reservation was refused (no `retries.txt`).

## Gates

| Phase | Guest | nvml | cuda (scored) | Host Xid |
|---|---|---|---|---|
| before-610 | 610.57.04 | fail: power-number (N/A) | torch, rlprobe, convburn pass; the seven nvprobe-based tests and robustness fail with empty probe output | 0 |
| after-610 | 610.57.04 | fail: power-number (N/A) | identical to before | 0 |
| before-615 | 615.71.09 | pass | 10 pass, cuda-gdb skipped, cuda-managed fails (801, managed memory unsupported) | 2, not this device (below) |
| after-615 | 615.71.09 | pass | identical to before | 0 |
| probe-615 | 615.71.09, `CARAXES_GOE_SELFTEST=5` | pass | identical | 0 |

The 610 guest's nvprobe stages print nothing and exit 0 (or fail at stage 2) on vm5's 610
overlay both before and after, while torch, rlprobe and convburn run; the same probes pass on
the 615 guest. It is a property of vm5's 610 image or its test build, not of the device
change (identical before and after); gsp-rpc-2026-09-24 ran them on vm3 with passes.

**Host Xid.** before-615's window has two Xid 31 lines (23:32:08 and 23:32:25, CE2 FAULT_PTE)
on pid 1131576. This run's device was pid 1127010 (`before-615/device-pid.txt`); pid 1131576
is the device of another agent's run (`caraxes/runs/xid31-hostreg-2026-09-24/before/device-pid.txt`
in its worktree) on the shared card. No Xid on this device's pid in any phase.

## What the gates reached

- The host TSG map is keyed by the kernel's TSG id on the 615 guest ("host TSG ... for guest
  TSG 0x5c000057 (grpID 5)", `after-615/device-gsp-plan.txt`) and by the guest handle on 610.
- None of the 17 D-plan C rows and none of the new B rows was sent by the gates
  (`device-controls.txt`); nor was an IDLE_CHANNELS RPC, and no RC happened, so no RC record.
  Their answers are covered by the unit tests.

## The operational-event records (probe)

`probe-615` ran the device with `CARAXES_GOE_SELFTEST=5` (a SIMULATED debug-category record
every 5 s once the kernel's import is bound). `probe-615/device-goe.txt`: 71 records written
into the import's registered guest buffer, each followed by `POST_EVENT` and answered by the
kernel's drain with `UPDATE_GET` whose record and vardata get equal the device's put (1/96,
2/192, ... 71/6816): the 615 kernel read every record with the framing it checks.

The guest probe `caraxes/tools/goe-probe.sh` failed twice before it worked, each a probe error,
kept as records: `probe-615` allocated the event buffer under the subdevice (0x36),
`probe-615-2` with kernel-internal backing (0x56, not allowed on Linux); `probe-615-3` used
device memory (the bind needs a deviceless buffer, 0x1F). `probe-615-4` (gates skipped):
deviceless read-only memory, bind after the import; 8 records in 45 s, each `CARAXES-TEST`,
category 0xffff, group attributes 0x4 (SIMULATED), originator 2 (GSP firmware), the GPU UUID of
the guest, one legacy-Xid context with the device's text (`probe-615-4/goe-probe.txt`). So the
records the device writes reach a user tool in the guest through the kernel's own
operational-event log.
