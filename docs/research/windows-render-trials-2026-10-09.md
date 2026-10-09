<!-- SPDX-License-Identifier: MIT -->
# Windows render trials, 2026-10-09

The corrected build completes stock Windows Fire Strike. Neither bounded server
polling nor a larger BAR1 pool shows a graphics FPS gain in these diagnostics.
Defaults remain unchanged.

## Configuration

- Backend source `f7c431d6`, SHA256
  `82a4ee78d611948ff8f9ed64e9403a7601912b9d066b1f8f2216c3da72e1e22b`.
- Paired Cloud Hypervisor SHA256
  `7f3682ea1422c9c7335b07483960ec819c49da6405669a12cd73f8d5e582198b`.
- Windows signed driver 617.14, transport ABI 615; physical TU106 RTX 2070,
  host driver 615.71.09. Eight vCPUs, 8 GiB RAM, 4096 MiB framebuffer reservation.
- Fire Strike 1.1, preset P, fullscreen 1920x1080, VSync off, UI Demo off.
  SystemInfo collection and monitoring on.
- VM boost, compression `slice`, BAR2 memfd, exact read-only FE page and local
  IRQ. Socket BAR transport, HEAD ioeventfd and host-control offlock disabled.
  Doorbell polling remains hybrid.
- Backend server TID CPU 2, doorbell reader 11, host events 10, local IRQ 0/8.
  Eight vCPUs exclude 2/3/10/11. Server 2 shares a physical core with events 10.
  Other host tasks and IRQs remain unrestricted; Floppy retains priority.
- Profiling, bounded host timeline, process exit trace and physical clock samples
  enabled in both arms. These are instrumented local diagnostics.

## Results

| Case | Server idle polling | BAR1 pool | GT1 FPS | GT2 FPS | Graphics score |
|---|---|---|---:|---:|---:|
| C220 | `0` | 32 MiB | 19.563662 | 33.734764 | 5696 |
| C221 | `hybrid:100` | 32 MiB | 19.422684 | 32.376999 | 5584 |
| C222 | `0` | 64 MiB | 18.948196 | 32.193451 | 5486 |

- Each archive has four positive, distinct primary results. All four observed
  workload lifetimes exit 0. Signed NVIDIA ProblemCode 0, no Display 4101 since boot,
  no new host Xid, clean owned teardown. Host NVIDIA remains bound.
- C220 graphics clock samples with GPU utilization at least 80% range 1830–1860 MHz;
  memory 7000 MHz. The physical card boosts during rendering.
- C221 had one SSH connection timeout. The guest recovered and completed the run;
  the observer resumed with an explicit 232.923-second clock-sampling gap. Its
  archive and process lifetimes qualify; phase clock coverage is incomplete.
- C220's observer initially failed to read its root-owned downloaded archive.
  The fresh stable archive was retained and independently qualified afterward.
- C222 retains the 96 MiB host BAR1 floor and dynamic window admission. Its larger
  pool takes another 32 MiB from the same framebuffer reservation. GT1/GT2 are
  3.15%/4.57% below C220; combined FPS is higher, 21.2145 versus 19.646364.
- No graphics gain is demonstrated. Single trials, changing host activity and
  incomplete C221 clock coverage cannot establish causal regressions. No new scene
  recording or default change follows this result.

## Cost evidence

C220's broad before/after profile bracket includes benchmark setup, rendering
and later idle. Device-level counter deltas, without summing client scopes:

| Operation | Count | Total |
|---|---:|---:|
| GSP HEAD writes | 33027 | 49.005 s |
| Physical A613 host controls | 7990 | 34.389 s |
| Physical A612 host controls | 7984 | 4.494 s |

HEAD handling contains host calls; the totals overlap. They are not additive or
proof of the critical path. The 8192-entry host timeline evicts 8498 older rows by
the later snapshot, so retained spans cannot reconstruct every early GT1 call.
Physical ioctl time remains substantial and server polling cannot shorten it.

## Result qualification

`tools/windows/validate-firestrike-diagnostic.py` binds actual saved flags and
backend/VMM bytes to the frozen packet, checks the fresh archive and exact
configuration, and requires four successful process lifetimes plus driver and
teardown health. WMI stop names can be truncated; FILETIME/sequence ordering and
the corresponding start identity distinguish later PID reuse. C220 PID11932's
later sshd exit255 is excluded from its earlier successful workload lifetime.

CPU checks accept C220/C221/C222 and reject a nonzero exit, intervening same-PID start,
missing stop, duplicate result node, identical FPS values and changed VMM identity.
Raw archives and process command lines remain private. This does not validate
pixels against native Windows or establish an uninstrumented speedup.

## C262: clock and TSG corrections

C262 completes standard Fire Strike with source `c8e8ea4d` and the corrected
native UI/trace flow. Backend SHA256:
`b6d36f87a730382d76b6c81719869191cb087ad931424adb2aa92cebcac1a96e`.

| GT1 FPS | GT2 FPS | Physics FPS | Combined FPS | Graphics score |
|---:|---:|---:|---:|---:|
| 18.486073 | 29.838354 | 25.100178 | 17.217127 | 5250 |

- All four workload lifetimes exit 0; native trace self-test exits 37. Driver
  ProblemCode and Display 4101 stay 0; exact owned cleanup passes in 671 seconds.
- Backend/VM policy is now nice10/SCHED_OTHER, with bounded marker captures.
  C220's nice0 policy and older source are not a matched comparator.
- The 282.166-second backend observation retains every selected host span.
  A612/A613 union occupancy is 39.236 seconds, with A613 median 3.478 ms. This
  includes launch/result handling and establishes neither GPU execution nor
  critical-path delay. Global marker contention loses 2282 sequence IDs.
- No speedup is established. C264 is the matched clock-only comparison, using
  identical UI, tracing, affinity, resource limits and host policy.

## Next measurement

After the matched comparison, test the existing host-control offlock option as
a separate variant. It can release the device lock during physical A612/A613
calls; it preserves ordered GSP replies and does not shorten the physical calls.
Defaults remain unchanged until qualified evidence supports a change.

Retain the baseline pool and polling settings with the bounded backend markers.
Record physical doorbell writes, host notifier observations and local cause
publication against the same clock as A612/A613 controls. Distinguish event gaps
from execution time; notifier events alone do not establish GPU completion.
See [latency attribution](latency-attribution-2026-10-09.md).

Evidence: `/mnt/vmstore/caraxes/3dmark-trials-2026-10-09/`, frozen C220/C221/C222 packets,
curated qualification reports, profile snapshots and observer recovery records.
