<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Guest idle CPU and host control timing

The virtual GSP runs on the host. Its polling, queue processing and emulation
must not become extra guest work. Normal driver housekeeping and display
interrupt handling still run in the guest. Incorrect completion state,
repeated notifications or delayed replies can make that driver poll or retry.
Low trap counts alone do not exclude a busy loop over shared memory.

The October 9 four-vCPU headless hardware test did not reproduce high guest
CPU. It did expose persistent NVIDIA interrupts after a verified GPU job.
The tested backend was the pinned C209 baseline, not the new diagnostic build.
The ioctl error fix and diagnostics make no FPS or idle CPU improvement claim.
See the [hardware results](idle-hardware-2026-10-09.md) for evidence and limits.

## Source findings

These are diagnostic leads, not established causes of the reported idle load.
The first hardware result must identify the busy NVIDIA driver path and the
request/status sequence feeding it.

- `public.rs::Guest::IdleChannels` caps host waits at 50 ms and maps unexpected
  host failures to timeout. Check whether a busy driver path repeatedly asks
  for idle and receives that status; do not report success for unfinished work.
- `rusd.rs` implements the shared-data page behind `CARAXES_RUSD=1`; `boot.rs`
  refreshes its polled timestamps on synchronous controls. Deferred host
  controls take a different path. The requested mask is stored but not used,
  and the header's `pollIntervalMs` is not scheduled. Record the flag and check
  requested poll sections and freshness if the driver repeatedly queries data.
  A successful registration reply alone does not prove a useful shared page.
  The vendored NVIDIA `gpu_user_shared_data.c::_gpushareddataSendDataPollRpc`
  caches the mask/interval after success and suppresses identical resends.
  Missing scheduled publication is a confirmed implementation gap, but does
  not by itself prove a guest retry loop. `cl00de.h` bounds inconsistent reads
  to ten attempts; the inspected kernel thermal consumer makes one control
  fallback when the timestamp is invalid. Proprietary userspace behavior and
  the actual hot guest stack still need measurement.
- `ctrl.rs::begin_control` can echo an unsupported request with `NV_OK` under
  `CARAXES_ACK_UNANSWERED=1`. Record diagnostic answer labels and the flag:
  this success does not establish a modeled side effect or valid output.
  If a hot driver path waits for the promised state, implement that command's
  semantics before treating the acknowledgment as compatibility support.
- `um_page.rs::publish_bounded` limits publication frequency to one update per
  microsecond. It does not guarantee one-microsecond freshness while the host
  reactor sleeps. If driver stacks show timer polling, compare the mapped
  usermode clock with the live trapped timer path under the same workload.
- `mirror.rs::ensure_events` subscribes to host engine non-stall notifiers.
  These are engine-wide: host desktop work can notify an idle guest.
  `launch.rs` already documents this and provides `CARAXES_INTR_GATE_MS`.
  The gate delays/coalesces events; it does not establish guest ownership.
- Event registration is lazy. Test idle both before GPU use and after a short,
  verified GPU job has activated mirrored channels and host events.
- `intr.rs::raise_in_generation` coalesces an already-pending vector. This
  prevents duplicate MSI delivery before an ACK, but a guest that ACKs quickly
  can still receive a series of unrelated engine notifications.
- `display/engine.rs::Ticker::run` skips a head without a period or without
  `want`/`vblank`. With no active heads it parks for 50 ms. This host wakeup is
  separate from guest interrupt handling; verify the actual head state.
- The historical [trap census](trap-census-2026-09-30.md) measured Linux GNOME
  idle at 421 traps/s and 1.1% observed vCPU load. That older result does not
  establish the current baseline or explain intermittent high CPU episodes.

## Controlled idle comparison

Floppy has priority. Run a bounded, low-priority hardware diagnostic only while
the host resource floors are met. Keep the NVIDIA host driver bound, use one
owned host-backed VM and retain the existing 8 GiB RAM / 4 GiB framebuffer
limits. Record source, binary hashes, driver, vCPU count, clocks, poll mode and
interrupt gate setting.

1. Boot the stock-driver Linux image into `multi-user.target`, without the
   guest helper module. Initialize the GPU outside the measured window.
   Verify that no compositor or display head remains active.
2. Capture 60 seconds of settled headless idle. Execute one short, verified
   existing compute probe, let the guest settle, then capture 60 seconds again.
   Record which host event subscriptions are armed in each phase.
   If CPU remains high, identify guest driver stacks before changing GSP
   replies. Correlate the hot stack with command/status counters and reply
   publication; process rankings alone cannot identify the responsible reply.
3. Repeat with GNOME Wayland on the same image and VM settings. Keep the
   desktop still. Treat the codec service state as a separate factor;
   Sunshine is not assumed to explain the reported CPU load.
4. Use equivalent native/reference idle data before interpreting percentages.
   Do not stop the host desktop to manufacture a quiet baseline.
5. In Windows, collect user, privileged, interrupt and DPC CPU time, process
   CPU deltas, and DWM/display state. Disconnecting Moonlight or closing an
   application does not prove headless operation. Preserve PCI identity for
   the comparison. If kernel CPU is high, take a short stack trace in a
   separate diagnostic window to identify the driver code doing the work.

Collect guest and host data separately:

```sh
# Inside the Linux guest, write to a fresh evidence path.
python3 /path/to/idle-census.py --seconds 60 --threads > /path/to/guest-idle.json

# On the host, use only the exact backend/VM PIDs recorded by the owner harness.
python3 tools/graphics/idle-census.py --seconds 60 \
  --pid "$backend_pid" --pid "$vm_pid" --threads > /path/to/host-idle.json
```

The collector reads `/proc`; it does not access the GPU or change services.
It retains raw endpoint counters, per-second CPU accounting, process/thread
rankings, interrupt deltas and read failures. `100%` means one CPU; aggregate
values may exceed 100%. Guest ticks are not added twice. Process and thread
times overlap and must not be summed. Exited/new processes and reused PIDs
are excluded from process deltas. Sequential read spans expose timestamp
uncertainty; the collector's own PID is recorded. CPU time alone does not
identify a driver stack or attribute an interrupt to a guest command.

Take backend profile snapshots immediately before and after each window,
using SIGUSR2 only on the owned backend PID. Compare counter deltas for
doorbells, host events, IRQ raises/MSIs/ACKs, display ACKs and GSP/BAR accesses.
Include local IRQ/shared-page counters: socket traps do not cover them.
Do not run screenshot, foregrounding or UI tasks during these windows.

Interpret the paired data before changing policy:

| Observation | Next check |
| --- | --- |
| Guest kernel/IRQ CPU rises with host events while guest doorbells stay idle | Engine-wide notifications, pending/ACK transitions, guest ISR stacks |
| Guest CPU rises with GSP HEAD/BAR polling but few MSIs | Reply publication, queue state, timeout/retry path, timer freshness |
| Guest CPU is high only with an active display | Vblank cadence, head teardown, display status and compositor scheduling |
| Host reader spins while guest CPU stays low | Host doorbell poll policy; keep this separate from driver idle CPU |
| Guest steal time or host vCPU runnable delay rises | Host scheduling contention; do not attribute it to GSP emulation |

A doorbell-age heuristic cannot safely drop completions. A channel may wait
for GPU work submitted earlier. Any future notification filter needs a
validated per-guest completion signal, correct rearm/reset behavior, and
coverage for graphics, compute and video engines. Keep current completion
delivery intact while collecting evidence.

## Exact host ioctl timeline

Enable alongside the aggregate profiler:

```sh
CARAXES_PROFILE=1
CARAXES_PROFILE_HOST_TIMELINE=1
CARAXES_PROFILE_OUT=/absolute/evidence/gpu-profile
```

- Retains the latest 8,192 completed `0x2080a612` / `0x2080a613` control calls.
  No ioctl payloads, guest addresses or per-call file writes are recorded.
- `<prefix>.<tag>.host.tsv` contains monotonic begin/end timestamps, sequence,
  command, status, drop counts, and bracketed Unix-clock calibration at both
  anchor and snapshot. Check calibration drift before joining wall-clock data.
- Snapshots overlap and are non-consuming. Deduplicate by process instance
  and sequence. Rows and eviction follow callback recording order; concurrent
  calls can overlap or record out of end-time order. Use `end_ns` for observed
  completion order. In-progress or hung calls have no row yet.
- Each TSV is individually published by rename. The aggregate and timeline
  files are not an atomic pair; collect and hash both after the dump completes.
- Timeline mode refuses startup if another observer already owns the global
  ioctl slot. Ordinary profiling retains the existing observer behavior.
- Profiling off installs no observer and reads no ioctl clocks. Ordinary
  profiling allocates no timeline buffer. Timeline mode adds bounded storage
  and bookkeeping, so measure its overhead before using it for comparisons.

`nvrm-abi` now snapshots syscall errno before callbacks can overwrite it.
A regression test uses `/dev/null` and observers that deliberately change
errno. Both observer modes must still return the original `ENOTTY` error.
Timed intervals now end before argument decoding and observer execution;
failed calls include the errno snapshot. Older profile durations included
argument decoding, so a small duration change is not a speedup claim.

These timestamps measure host ioctl latency. They do not identify firmware
lock time, actual GPU execution, a guest RPC reply or a matching guest request.
They are a correlation aid for the next backend analysis, not an end-to-end
completion trace.
