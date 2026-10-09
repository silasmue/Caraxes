<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Windows idle after completion and RUSD corrections

Windows boots and the existing D3D11 probe verifies three GPU-rendered frames.
Guest idle remains above the near-zero target. These runs establish compatibility
and diagnostic behavior, not an FPS improvement or a before/after comparison.

## Tested build

- Source: `f7c431d60eac44fcfaea5f3fca95e5e523ff69a6`.
- Backend SHA256: `82a4ee78d611948ff8f9ed64e9403a7601912b9d066b1f8f2216c3da72e1e22b`.
- Cloud Hypervisor SHA256: `7f3682ea1422c9c7335b07483960ec819c49da6405669a12cd73f8d5e582198b`.
- One host-backed TU106/RTX 2070 guest: four vCPUs, 8 GiB RAM, 4 GiB framebuffer.
- Signed NVIDIA `32.0.16.1714`, guest transport ABI 615, host driver 615.71.09.
- RUSD enabled; host-control worker disabled; hybrid doorbell polling; VM boost
  policy; compression slice; exact read-only display FE; local IRQ; socket BAR.
- Sunshine stopped for measurement and restarted afterward. Startup type unchanged.
- Host NVIDIA driver stayed bound. Each owner retained exact process pidfds,
  guest/slot locks, RAM/disk floors, and removed its foreground TPM and fresh TAP.

## Results

CPU percentages average the four individual raw processor instances. Privileged
time includes interrupt and DPC time; those columns must not be added to it.

| Case/window | Settle | Busy | User | Privileged | Interrupt | DPC |
| --- | --- | --- | --- | --- | --- | --- |
| C217 before probe | 20 s | 36.729% | 26.222% | 10.515% | 2.025% | 0.260% |
| C217 after probe | 20 s | 12.129% | 5.324% | 6.843% | 0.987% | 0.234% |
| C219 later idle | 120 s | 5.158% | 1.771% | 3.400% | 0.944% | 0.116% |

Each accounting interval is about 30 seconds. C219 is a separate boot without
another renderer probe. Earlier high load falls with time, but early-window
process attribution was not collected. Do not assign that transient to Defender,
Steam or the NVIDIA driver from the later process list.

C219 matched 169 process lifetimes across endpoints. The largest measured
process contributions, normalized to the whole guest, were System 0.453%,
MsMpEng 0.375%, sihost 0.233%, steamwebhelper 0.194%, and Steam 0.155%.
NVDisplay.Container and DWM each contributed 0.013%; the second NVIDIA container
contributed zero. Process CPU sums do not reconcile fully with raw processor
accounting. IRQ/DPC time, different read spans, exited/new processes and virtual
time accounting limit interpretation. The unexplained difference is not proof
of NVIDIA work. No guest kernel stack trace was collected.

The C217 D3D11 probe selected NVIDIA RTX 2070, created the device successfully,
and verified frames 0, 1 and 2 through readback. Device creation took 582 ms;
the three-frame smoke took 59 ms. This is a correctness probe, not throughput.
C217 and C219 ended with ProblemCode 0, a signed driver, no Display 4101 since
boot, no new host Xid, and no owned VMM/backend/TPM left running.

## Hypotheses narrowed

- Idle replies now preserve actual host errors and require host engine completion.
  Software slices/acquires/deferred work cannot become a false successful idle.
  All host channels must resolve, without truncation.
- RUSD now honors masks and intervals independently of RPCs, handles the verified
  570/595/610/615 layouts, and retires registrations on deinit/unmap/translation
  changes. Only supported virtual model sections receive fresh timestamps.
- CPU qualification: 996 backend library tests passed; two existing tests ignored.
  C217 accepted RUSD init and both poll requests. This does not directly measure
  guest page freshness or establish a CPU reduction from the correction.
- C217 retained 20 completed A612/A613 calls with no timeline drops or invalid
  spans: 3.585 ms A612 and 37.640 ms A613 in total. No new selected calls appeared
  between the second and third snapshots, covering the probe and later idle.
  These selected controls cannot explain that window's sustained CPU by duration
  alone. Other ioctls, in-progress calls and firmware execution are not covered.
- Aggregate profile deltas spanning the probe also contain settling. Window rows
  with `ns` in their names store nanoseconds in `count`, not an access count.
  Use the actual counter units; do not sum nested scopes or label mixed deltas idle.

## Next diagnostic

1. Take a short guest CPU stack trace after a long settle. Separate nvlddmkm,
   HAL/timers, DPC/ISR, other kernel services and time attributed to trapped access.
2. Correlate that window with exact IRQ raise/ACK transitions and BAR operations.
   Windows raw interrupt counts include every device and timer, not only NVIDIA.
3. Compare RUSD on/off with the same binary and settled conditions only if the
   hot stack shows telemetry queries. Keep modeled values separate from host clocks.
4. Investigate engine-wide host notifications if the stack and event counts support
   it. Do not drop completions based on doorbell age or equal fetch pointers.
5. Resume rendering performance work with A612/A613 timing under a real workload.
   Their low activity during idle does not clear their cost during 3DMark.

## Evidence

`/mnt/vmstore/caraxes/host-timeline-2026-10-09/` retains the pinned C215 build,
CPU qualification, independent C header oracle reference, C217/C219 raw counters,
host profiles/timelines, owner identities, health, resource logs and closeout.

- C215: collector aborted before sampling on Sunshine and localized output;
  its TAP cleanup needed a separately recorded correction after CH changed the MAC.
- C216: Sunshine preparation returned a misleading nonzero PowerShell exit after
  an empty process lookup; no accepted sample.
- C218: longer settling exceeded the old SSH timeout; no accepted sample.
- C217/C219: completed bounded diagnostics. Earlier failed packets remain intact.

See [Linux idle results](idle-hardware-2026-10-09.md) and
[source findings and timeline contract](idle-and-host-timeline-2026-10-09.md).
