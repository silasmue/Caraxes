<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Host-control worker diagnostic (6 October 2026)

**No performance gain is accepted.** This is one instrumented Fire Strike pair,
not a moving-scene or child-exit audit. The feature remains default off.

## Why this was tested

C207/C208 moved guest GSP command-HEAD writes to an ioeventfd: 31,709 backend
HEAD-write accesses in C207 became zero in C208. Backend HEAD-read count stayed close
(23,989 versus 24,248), while its cumulative access time moved from 1.851 to
48.690 s. GT1 was 18.02 versus 17.41 FPS and GT2 30.54 versus 30.55 FPS.
This removed the synchronous HEAD-write socket transaction; KVM still handles
the write exit. It did not establish an FPS improvement or remove the synchronous
host-control work. Source: external
`/mnt/vmstore/caraxes/windows-3dmark-2026-10-04/C207-C208-rpc-ioeventfd-offline-diagnosis.json`.

Commit `c0e9eef5` adds `CARAXES_HOST_CONTROL_OFFLOCK=1` for only the exact
54,352-byte A612 and 79,936-byte A613 MsgqV2 controls. The existing host RM
client stays with one ordered worker. The selected guest RPC waits for its
actual host status and output; no response is cached, synthesized or retried.
Normal mode keeps the original synchronous path. Admission requires the
qualified host-exec/GPA/615 setup and trapped RPC HEAD writes; incompatible
cache, selected overrides and ACK-extra modes are refused. With generic
ACK_UNANSWERED enabled, both controls must be explicit exceptions. A613's reply is
admitted as two records atomically, including under held-tail pressure. Epoch
retirement drains accepted work before queue, DMA or device teardown; failures
remain terminal. See `crates/vfio-user-nvgpu/src/host_control.rs`,
`src/boot.rs` and `src/msgq/v2.rs` in that crate.

Validation passed: 980 library, five binary and two protocol tests; two
library tests were ignored. Full backend Clippy completed with 225 warnings
remaining. Logs: external
`/mnt/vmstore/caraxes/windows-3dmark-2026-10-04/host-control-full-backend-tests.log`
and `host-control-full-backend-clippy.log` in the same directory.

## One OFF/ON comparison

The strict C209 (OFF) / C212 (ON) comparator returned success and verified
matching binaries and actual flags apart from the worker switch and case
output paths. Both fresh results completed. C210 stopped before benchmark
startup; C211 failed a host RAM resource guard before benchmark. Neither is
included. Both compared VMs stopped cleanly, with driver ProblemCode 0 and no
new Display 4101 or host Xid in closeout.

| Render-bracket observation | C209 OFF | C212 ON |
| --- | ---: | ---: |
| Bracket | 286.410 s | 267.794 s |
| GT1 / GT2 | 17.711123 / 28.655884 FPS | 17.922169 / 28.531679 FPS |
| GSP HEAD writes, backend count / time | 34,134 / 54.103 s | 32,168 / 10.639 s |
| GSP HEAD-write mean | 1.585 ms | 0.331 ms |
| GSP HEAD reads, backend count / time | 25,714 / 2.413 s | 481,971 / 2.981 s |
| A613 host calls, count / mean | 8,410 / 4.316 ms | 7,850 / 4.396 ms |
| A612 host calls, count / mean | 8,404 / 0.596 ms | 7,844 / 0.618 ms |

In C212, worker execution totaled 42.039 s over 18,004 calls; queue wait
totaled 0.376 s. Ready-to-Device and finish/serialization windows totaled
1.374 and 0.630 s over 15,694 completions. These windows overlap other
profile scopes and cannot be added to the host-control totals. The large
increase in HEAD reads is consistent with guest polling becoming visible
after Device is released; it does not identify the cause of individual polls.

Sources: external `C209-C212-render-comparison.json` and
`C209-C212-gsp-host-control-deltas.json` under
`/mnt/vmstore/caraxes/windows-3dmark-2026-10-04/`, plus the pinned runtime
manifests and render-before/after TSVs named by those reports. Their relaxed,
device-wide snapshots cover unequal brackets and may include background and
result-wait work. A socket transaction can get cheaper while KVM exits remain;
neither the profile nor this single pair supports a causal FPS claim.

Next, measure an authoritative GSP reply-HEAD snapshot before considering
read-only shared reads: queue publication and reset ordering must be proved
before mapping it. Separately split host ioctl time from RM lock/firmware time
for A613; the local legacy SDK provides no public payload or side-effect
contract that would justify caching or an ACK shortcut.
