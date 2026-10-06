<!-- SPDX-License-Identifier: MIT -->
# Windows workload abort investigation

The intermittent Fire Strike failure remains unexplained. Four later complete
runs have not reproduced it. The changes below repair diagnostic tools, VAS
ownership and interrupt masking, and extend doorbell offload. None establishes
the cause of that abort or a benchmark performance gain.

## Earlier failures

- C174 fails before GT2 reports `READY`; C175 fails before Physics reports it.
  Both wait about 60 seconds and report `NO_RESPONSE` / `PROCESS_EXITED`.
- The product log reports a process death near the new workload's start but
  omits PIDs. It cannot distinguish that child from a delayed callback for the
  previous process. Child exit status and redirected output are absent from
  these archives.
- The runner sends workload settings after `READY`. Failed slots retain the
  previous workload's primary result: GT1 in C174's GT2 slot, GT2 in C175's
  Physics slot. Strict validation rejects each whole run.
- Top-level status fields and missing `tmp.txt` also occur in complete archives.
  They do not identify the fault. Spin is not established as the cause: the
  hybrid control fails too. Earlier event searches found no matching current
  application crash or display reset; that does not prove a normal process exit.

See [the overnight investigation](night-performance-2026-10-06.md) for the memory
fix and earlier controls. C178–C181 use the same immutable backend and VMM.

## VAS lifecycle fix

- Before this change, freeing a VAS owner could delete mirrored TSGs and channels
  that another client still used through a DUP alias. The registry then retained
  that alias, leaving the registry and mirror inconsistent.
- The mirror now uses the registry's deterministic surviving-handle plan before
  teardown. It retains host handles and changes their guest ownership key, also
  updating shadow-only TSGs that have no raw host VAS. Pending builds settle first.
- The regression covers both owner-handle and whole-client FREE, raw and
  shadow-only state, two successive surviving clients, and final-owner teardown
  selection. It exercises production planning and mirror map updates without
  GPU RM calls. The full suite passes: 928 unit tests, one ignored test and two
  integration tests. The release build passes.
- The destructive path requires a raw host VAS. Current Windows zero-based
  compressed FB normally uses shadow-only state, where the old code left stale
  ownership keys instead. Neither path is established as the cause of C174/C175.
- A separate edge remains: freeing a VAS handle while an original-owner channel
  still references it can leave that channel with an unresolved registry key.
  This change does not qualify that lifecycle or host-RM free-failure recovery.

## Slow startup

- C181 observes the 3DMark root process about two seconds after Steam and its JVM
  about six seconds after Steam. Window readiness takes 75.146 seconds, leaving
  about 69 seconds after the JVM starts. The first window still has a blank body;
  this is not complete UI readiness.
- A SystemInfo scanner overlaps that interval for about 41 seconds and exits 0.
  SystemInfo is a profiling candidate, not an established cause. WMI timestamps
  have the provider-event limitation described below.
- Independent D3D11 device creation takes 584 ms in C180 and 792 ms in C181.
  That fixture does not explain the application delay or exclude slower
  application-specific GPU queries. VM boot latency has not been measured with
  an early readiness poll.
- C182 measures the first successful SSH response 23.6 seconds after the owned
  launcher starts. The VAS candidate renders three D3D11 readbacks; device
  creation takes 983 ms. Its 3DMark window appears after 54.966 seconds, with a
  later screenshot confirming loaded Home controls. The longest SystemInfo
  scanner overlaps startup for about 26 seconds and exits 0. These timings do
  not establish a speed gain across different sessions and instrumentation.
- A 69.8-second profile bracket around that application start counts 786,206
  trapped register accesses: 186,491 ordinary accesses, 329,326 routed interrupt
  tree writes, 262,377 display-status reads and 8,012 display-status writes.
  Whole-device counts include desktop/background clients. RPC service sums to
  2.05 seconds; power control `0x2080a613` takes 1.49 seconds over 360 replies.
  Nested RPC/control/host/access times overlap and must not be added.
- The automatic guest-token set covers ChIDs 0–47. Runlist 0 / ChID 48 accounts
  for 52,133 trapped doorbells in this bracket. Those writes fall outside the
  eventfd set. The full doorbell access row has 52,178 writes, 592 ms of service
  and 7.68 seconds of interval attribution. Extending coverage must preserve the
  single-reply FD budget and the previously covered engine/runlist set.
- Ordinary accesses have 21.85 seconds of `gap_ns` attribution. These are short
  end-to-end intervals between completed accesses, including current service,
  guest work, transport and scheduling. They are not exclusive trap overhead;
  routed accesses have no such interval measurement. The many traps are a
  concrete optimization target, not a complete explanation of the startup wait.
- C182's process collector stops cleanly with 480 records and task exit 0.
  Post-health remains Problem 0 with no Display 4101; owned processes stop,
  NVIDIA remains bound and the host has no new Xid. No scored benchmark or
  dedicated cross-client DUP hardware trigger is exercised.
- The token fix first selects the old 48-wide runlist set, then spends its spare
  FD budget on ChIDs 48 and 49 where the reported channel count permits. Tight
  budgets retain their old engine coverage. Tests cover every one-message
  budget, partial extension to 49, and lower reported counts. Focused tests,
  the full suite and the release build pass. C184 confirms that ChID 48 no longer
  traps: its 103-second startup bracket has only 14 trapped doorbells, with
  87,246 guest eventfd writes. Window readiness still takes 84.352 seconds.
  Different bracket lengths and host activity prevent a direct speed comparison.
- C183 stops before its D3D fixture or benchmark: a CPU-only Nix QEMU 10.1.5
  test VM appears while the packet admits only 10.2.4. This is a runtime
  admission stop, not a Windows workload abort. Its GPU processes stop normally;
  the unrelated test VMs remain untouched. A fresh packet pins both reviewed
  executables and still excludes GPU, guest-image and tap conflicts.

## Display and interrupt traffic

- C185 enables the existing `CARAXES_DISPLAY_PAGE=exact-ro` mode on the unchanged
  C184 backend. The VMM maps the FE status page read-only; writes still trap and
  apply synchronously. It uses ordinary register values without the legacy
  tagged writable page, watchdog or kick-page protocol.
- Windows reaches SSH after 23.7 seconds, renders the three D3D11 readbacks and
  opens a 3DMark window after 73.541 seconds. A later screenshot confirms loaded
  Home controls. Pre/post health is Problem 0 with no Display 4101. The process
  collector stops cleanly with 508 events and task exit 0; owned GPU processes
  stop, NVIDIA stays bound and the host has no new Xid.
- The 95.3-second startup bracket has zero routed display-status reads and
  11,038 FE writes. It still has 460,210 interrupt-tree writes, 110,166
  SET-generated MSI re-arms and 55,250 vector raises. These device-wide counters
  include background clients. This qualifies the mapped page for this startup
  smoke test; scored rendering, reset and multi-guest behavior remain unqualified.
- The SET re-arm condition ignored the top-level interrupt enable mask in both
  exact and tagged modes. A pending leaf under a disabled top subtree could send
  an MSI when the guest enabled that leaf again or an unrelated top subtree.
  The fix requires the pending subtree to be top-enabled. It preserves ordered
  ACK/SET/CLEAR writes and the separate explicit re-arm path.
- Both regression tests fail with the old predicate and pass with the fix. The
  full suite passes: 930 unit tests, one ignored test and two integration tests;
  the release build passes. C187 qualifies this fix with the mapped FE page in
  one complete Fire Strike run; see the hardware checks below.
  Aggregate re-arm counts cannot say how many MSI writes the fix will remove.
- A relaxed counter now records SET attempts with pending leaf bits and no
  top-enabled pending subtree. It appears in the profile and exit summary.
  Initial host raises retain their existing delivery contract; this counter
  and fix apply only to SET-generated re-arms, not every MSI source.
  The full suite and release build pass. C188's first preflight stops before
  VM creation because another host build consumes the RAM reserve. A later
  attempt runs with the required headroom: its 135.4-second startup bracket has
  491,600 interrupt writes, 13,998 FE writes and zero display-status read traps.
  The masked-SET counter stays at zero, independently confirmed by the exit
  summary. This bug does not explain the hot traffic in that run. Backend
  interrupt service totals 6.62 seconds; this excludes the VMM and transport.
  Window readiness takes 97.765 seconds and a screenshot confirms loaded Home
  controls. The collector stops cleanly, guest health stays Problem 0 with no
  Display 4101, all owned processes stop and NVIDIA remains bound without a new
  Xid. Host load differs; no startup speed or FPS gain is inferred.
- A writable shared interrupt page cannot retain repeated same-value stores.
  Offloading the remaining writes requires an ordered register model, for example
  in the VMM, with synchronous read visibility and correct MSI delivery. Do not
  qualify dropping or merging these stores from access counts alone.
- The current Cloud Hypervisor path is KVM MMIO write exit, PCI bus dispatch,
  client mutex, then a synchronous vfio-user `REGION_WRITE`. Its read-only memory
  mapping means a local VMM handler would still take the KVM exit, but could
  remove the socket round trip. Measure mutex wait and transport latency apart
  from backend register service before choosing that larger change.
- C189 measures that wrapper with an opt-in diagnostic VMM built from
  `b675232c1`. All 34 PCI tests, PCI library clippy and the release build pass.
  The diagnostic adds timestamps and bounded histograms; normal operation keeps
  the original call path. Earlier read-only mapping and BAR-move behavior stays.
- Its 75.9-second startup bracket has 408,410 backend interrupt writes and
  2.715 seconds of register service. Nearby VMM snapshots contain 403,123 writes,
  13.606 seconds of synchronous request time and 0.024 seconds of mutex wait:
  means 33.75 and 0.060 microseconds per write. Request median falls in 16–32 us,
  p95 in 32–64 us. The wrapper excludes the KVM exit and preceding PCI bus path.
  Requests include socket transport, backend work and scheduling, not just CPU
  service. Snapshot endpoints differ by 0.50 and 1.14 seconds; do not subtract
  the two timings as an exact transport cost.
- Passive desktop idle still has 3,863 interrupt writes per second over 70.9
  seconds. Nearby VMM samples average 24.57 us per request and 0.053 us of mutex
  wait. Display-status read traps remain zero. The client lock is not the main
  wait in these windows. This supports removing the synchronous hop from the
  interrupt path; it does not explain the complete launcher delay or predict FPS.
- C189 reaches a 3DMark window in 53.128 seconds, verifies three D3D11 readbacks
  and loaded Home controls, and passes pre/post health and teardown with NVIDIA
  bound and no new Xid. The host load differs from C188; no speed gain is inferred.
  Process collection and benchmark recording are off for this timing run.
- Physical non-stall events are engine-wide and include host clients. The existing
  `CARAXES_INTR_GATE_MS` delays and combines these hints outside its recent-host-
  doorbell window, retaining held vectors and bypassing RC events. C190 tests a
  1 ms hold and 10 ms hot window on unchanged C189 binaries. It passes the three
  D3D readbacks, loaded Home controls, pre/post health and clean teardown. Seven
  host events arm and disarm without refusal. The final gate summary records
  39,743 immediate events, 119,319 held and 115,871 held-vector deliveries: little
  combining. Idle has 3,576 interrupt writes/s versus C189's 3,863; host activity
  differs, so no gain is promoted. The VMM request still averages 23.83 us.
  Guest register writes stay ordered. Leave the gate off: an old submission can
  incur extra completion latency, and the nominal hold excludes scheduler delay.
- The periodic tree-hole check searched all 28 watched addresses for each of 512
  words while holding the page mutex. `57dfceac` replaces that search and the
  trapped-write lookup with a direct decoder. An exhaustive full-page comparison
  covers registers, holes and unaligned bytes. All 931 unit tests, one ignored
  test, two integration tests and the release build pass. Interrupt behavior is
  unchanged.
- C191 qualifies that decoder with three D3D11 readbacks, loaded 3DMark Home
  controls, pre/post Problem 0, no Display 4101 and clean teardown with NVIDIA
  bound and no new Xid. Gate stays off. Its 148.9-second idle bracket still has
  3,818 interrupt writes/s. Nearby VMM samples average 31.59 us per request;
  host load varies, so no latency or FPS improvement is established. This small
  change does not remove the synchronous hop.
- The next larger proposal is a negotiated local exact tree in the VMM, with one
  model owner and a hidden, versioned host-cause transport. Preserve immediate
  mapped-read visibility, ordered writes, pending/re-pending, MSI availability,
  host hints and reset epochs. All tree accesses must stay with the owner after
  activation; individual writes cannot fall back to a second backend model.
  First measure this path during rendering, then compare register/event behavior
  and qualify stock desktop/D3D, reset and unprofiled Fire Strike separately.
- Standard ioeventfds carry counts for an offset and optional fixed data match,
  not arbitrary register masks and ordering. This VMM refuses `ioregionfd` and
  disallows ioeventfd areas overlapping its mapped pages. Neither is a ready
  replacement for exact interrupt-tree writes.
- A local handler needs a negotiated shared-state contract or full VMM ownership
  of the interrupt model. That includes pending/enabled/re-pended bits, host
  hints and muting, reset epochs, and MSI enable/delivery. Updating visible
  memfd words alone races backend host raises. Qualification must cover multiple
  vCPUs, BAR moves, reset/reconnect, and fallback for unsupported clients.

## GSP reply queue pressure

- C182 and C184 backend logs contain bursts of held replies: the status queue
  has 48 of its 63 slots occupied, leaving too little room for a 16-slot
  continuation record (`0x47`). These are transport continuation messages.
- The guest's trapped `MSGQ_TAIL0` write already retries held replies immediately;
  command processing and enqueue also retry. A new guest request is not a known
  prerequisite for draining the queue. Existing logs do not measure hold age or
  identify these bursts as the startup delay.
- Search the live shared `gpu.log` during a run, or its archived copy after
  teardown. A run-directory backend log does not exist yet during execution;
  an empty search there cannot exclude queue pressure.

## Diagnostic changes

- `tools/windows/3dmark-launch.ps1` replaces the launch helper's stale process
  snapshot. The old helper sampled after 15 seconds, waited another 20 seconds
  and tested the old value. C180 reported a setup failure although Steam started
  3DMark later. The new interactive helper rechecks until the real window is
  ready, with a monotonic deadline and no duplicate launch of an existing
  current-session 3DMark. This is separate from the earlier workload aborts.
- `tools/windows/process-trace.ps1` records both WMI process streams through a
  .NET concurrent queue. PowerShell writes the JSONL. The first diagnostic's
  `Get-Event` / `Remove-Event` path failed when two distinct events shared an
  event identifier; the collector stopped before that benchmark started.
- Readiness requires an owned `cmd.exe` start, stop and exit status 37, also
  checked directly. Clean completion requires task exit 0, an explicit stop or
  deadline, and no cleanup errors. Raw exit status remains unsigned, in decimal
  and hexadecimal.
- Target command-line lookup is best effort. Fast startup probes often exit
  before the live query. Stop names are truncated and parent PIDs may be zero.
  Seven C178 workload PIDs and three C180 workload PIDs had earlier owners of a
  different name. A join by PID across the entire recording would be wrong.
  Match the compatible name and process instance; reject ambiguous matches.
- WMI timestamps describe provider event creation. They are not exact process
  birth/exit times, and callback order may differ. A callback still in flight at
  collector shutdown may escape the final drain; retain this coverage limit.
- Frozen collection helpers still request VM teardown on a collection exception.
  C181 confirmed this gap when its owned collector was cancelled. Change that
  behavior in a fresh packet before further abort reproduction; preserve the
  original packets and collect failure health/logs before normal teardown.

## Hardware checks

Stock Windows 617.14, host driver 615.71.09, host-backed RTX 2070, 4 GiB guest
FB, 8 GiB RAM, eight vCPUs, hybrid doorbell reader and finite VM boost policy.
The CPU-role masks match the earlier control; other host work remains unrestricted.
Backend scored diagnostics are off; the guest process collector is an additional
diagnostic and these runs are not a performance comparison.

| Session | Complete Fire Strike runs | Process evidence |
| --- | ---: | --- |
| C178 | 2 | Run 2: four separate workload instances, each exit 0; run 1 has no working collector coverage |
| C180 | 1 | Four separate workload instances, each exit 0; collector task exit 0 and clean stop marker |
| C187 | 1 | Four separate workload instances, each exit 0; mapped FE page and top-mask fix; monitoring on |

- C178's corrected collector has 1,007 records and a clean 900-second deadline.
  It includes four scored children and 31 separate startup probes, all with
  compatible, instance-matched zero-status stops and no collector fatal events.
- The 31 probes launch the stock Fire Strike executable, receive `1 Ready`,
  send `exit`, receive `1000 Bye` and exit 0 without a probe kill. They do not
  render, submit benchmark settings, or reproduce the Java runner's full path.
- C180's repository collector has 1,261 records, four `READY` / `EXIT_OK` pairs
  in the product log and a strict complete archive. Fresh D3D11 readbacks and
  pre/post guest health pass: Problem 0, no Display 4101 since boot.
- C180 stopped cleanly with NVIDIA bound and no new host Xid. SystemInfo scan
  and hardware monitoring were restored to on. C178 ran with monitoring off
  and could not restore it before the resource guard stopped the session.
- C177, C179 and C180's first start did not perform a benchmark: runtime
  admission or disk-space checks stopped them. Keep those stops separate from
  workload failures. Only owned inactive compiler/debug outputs were relocated,
  with verified contents and original paths preserved by symlinks.
- C181 verified the new interactive launcher: task exit 0, window readiness after
  75 seconds. Its later UI navigation selected Storage Benchmark, so the owned
  collector was cancelled and no Fire Strike result is accepted. Window presence
  alone does not prove that the requested benchmark page is ready. Confirm the
  page before arming a run.
- C187 validates all four primary results with the declared monitoring-on
  configuration: graphics 5,181, GT1 18.019 FPS, GT2 30.048 FPS and Combined
  17.512 FPS. These remain in the earlier performance range. No gain is promoted.
  The frozen C148 validator first rejects the changed monitoring bit; a fresh
  strict validator expects monitoring on and retains every other gate. The
  original rejection, archive and validation-policy hashes remain preserved.
- C187's 84 physical samples at at least 80% utilization have a median SM clock
  of 1,830 MHz and a peak of 1,860 MHz; memory remains at 7,000 MHz. Idle returns
  to 1,410 MHz. These physical samples
  also include host activity; they do not isolate guest GPU utilization.
- C187 passes pre/post health, three D3D11 readbacks and clean teardown with no
  new Xid. The process collector has 1,288 records and clean task exit 0.
  A complete archive establishes rendered benchmark results, not independently
  recorded moving-scene evidence. Capture remains disabled.

Private evidence: `C178-run2-process-audit.json`, final C178 trace files,
`C178-startup30-*`, `C180-run1-process-audit.json`, `C180-process-trace-final/`
and `C180-root-closeout.json` under the local Windows artifact directory. Raw
archives and process metadata contain private machine data and remain local.

## Next steps

- Qualify the VAS fix on hardware without claiming it resolves the earlier
  no-`READY` failure. Profile application startup separately from scored runs.
- Measure masked SET attempts with the new counter. Profile VMM mutex/transport
  time separately from backend register service before moving the model.
- Extend mapped FE qualification to reboot/reset and multiple guests. Compare
  matching monitoring, start/idle phases and host clocks before promoting a score.
- Repeat with SystemInfo monitoring on. Require four matching primary results,
  compatible child start/stop instances, post-health and clean teardown.
- On a reproduced failure, retain current child exit status, product log,
  application events and driver/backend health before changing GPU behavior.
- Keep the original no-`READY` failure open. Successful later starts do not
  identify its cause or justify marking Windows graphics fully supported.

## Render profile and server polling

- C192 uses the qualified C191 backend and diagnostic VMM with the same IRQ,
  register, boost and affinity settings. One instrumented Fire Strike publishes
  all four primary results. Its score is excluded from performance comparisons;
  no strict scored validation or child-exit audit is performed. Pre/post health
  stays Problem 0 with no Display 4101. Available host checks pass with no new
  Xid or VA-update error; 19 other backend ERROR lines remain. All owned processes
  and samplers stop, and NVIDIA remains bound.
- The 324.9-second launch/render/collection bracket has 1,326,750 interrupt writes
  (4,084/s), 5.514 seconds of backend interrupt service, 37,659 FE writes and zero
  display-status read traps. Nearby VMM snapshots span 321.9 seconds: 1,316,115
  interrupt requests, 38.157 seconds of request time and 0.069 seconds of client
  mutex wait, averaging 28.99 us and 0.053 us. Median/p95 fall in 16–32/32–64 us.
  Snapshot endpoints differ by +1.27/−1.79 seconds. These are elapsed, overlapping
  device-wide measurements, not exclusive CPU cost or the complete frame stall.
- Six passive stills identify SystemInfo wait, GT1 loading, an actual GT1 scene
  (frame 419, time 25.28), GT2 loading, Physics rendering and Combined loading.
  The initially labelled `gt1-observed` counter interval crosses the GT1/GT2
  transition. Its files are preserved and its phase label is rejected in
  `C192-phase-review.json`. A separate GT1 interval is inferred from the scene's
  displayed time, rather than two captured phase boundaries; its VMM endpoints
  differ by up to 2.31 seconds. This is not continuous moving-scene evidence.
- In that inferred 10.76-second interval, all 935 ordinary doorbell traps are
  runlist 0 / ChID 55 (914) or runlist 2 / ChID 56 (21), identified by existing
  per-token profile rows without byte traces. Both exceed the automatic ChID 0–49
  set. The offer contains 250 datamatch entries across runlists 0, 1, 7, 2 and 3;
  widening every runlist would exceed the FD budget. The 86 eventfd wakeups
  represent 340 guest writes; posted-ring counts can differ because RingGate
  coalesces repeated queued values. Control `0x2080a613` has 391 host calls taking
  2.114 seconds, and `0x2080a612` has 392 taking 0.366 seconds. Whole-device counts
  include desktop/SystemInfo work. The opaque controls' safe caching semantics
  remain unproven; do not cache them from request size alone. IRQ handling is a
  cost, not a complete explanation of low FPS. Physical samples during this run reach
  1,845 MHz with memory at 7,000 MHz; host activity is included.
- Host engine events include other host clients. The existing coalescing, pending
  `again` state, hint withdrawal and optional gate do not identify a fresh event's
  owner. `GP_GET == GP_PUT` proves fetch, not execution completion; an empty event
  drain also includes read/status failures. Neither condition is a safe reason
  to drop a completion hint. A reliable per-channel completion signal remains
  a prerequisite for guest-work-based suppression.
- `6b792bc7` adds an opt-in server wait experiment:
  `CARAXES_VFIO_IDLE_POLL=0|hybrid:1..1000|spin` (hybrid budget in microseconds).
  Default remains blocking. Readiness polling peeks without consuming header
  bytes or ancillary descriptors; normal receive/reply ordering remains. Full
  spin can consume one host CPU while idle. It retains every KVM write exit and
  synchronous socket request; it does not implement the local VMM IRQ proposal.
- The same change repairs short-header reception: the server completes a
  fragmented header and owns received descriptors before a fallible read, closing
  them on mid-header EOF. Boundary EOF stays clean. Three socketpair tests cover
  fragmented headers/SCM_RIGHTS, EOF descriptor cleanup and polling fallback.
  Vendor tests pass 17/17 via an isolated manifest with recorded dev-lock
  provenance; production Cargo.lock is unchanged. Backend tests pass 931 library,
  one parser and two integration tests, with one ignored; release build passes.
  The initial settings-registry failure is retained and the new entry is tested.
  C193 qualifies startup/D3D11 and passive desktop idle with full server spin.
  Stock Windows stays Problem 0 with no Display 4101; device creation takes
  655 ms and three verified readbacks take 23 ms. Available checks pass with no
  new Xid or VA-update errors; 15 other backend ERROR lines remain. Exact owned
  processes are stopped and NVIDIA remains bound. No 3DMark render is performed.
- C193's 60.73-second idle bracket has 228,811 interrupt writes (3,767/s) and
  0.952 seconds of backend service. Nearby VMM snapshots cover 60.38 seconds:
  227,786 requests, 7.265 seconds of request time and 0.013 seconds of mutex wait,
  averaging 31.90 us and 0.058 us. Snapshot endpoints differ by about two seconds.
  The whole backend consumes 68.18 CPU seconds over 61.48 wall seconds, including
  all its threads. Spin provides no demonstrated latency gain over C191's
  31.59 us idle sample and remains off by default. Different host loads prevent
  treating these runs as proof of a regression. No FPS gain is promoted.
- Two C193 admission attempts stop below the unchanged 45 GiB artifact floor,
  before creating a VM/backend. The successful third attempt follows verified
  relocation of rebuildable release/graph/dependency caches to the root disk,
  preserving their paths with symlinks. Frozen binaries, packets and results
  retain their bytes. These are resource stops, not guest boot failures.

Artifacts are under `/mnt/vmstore/caraxes/windows-3dmark-2026-10-04`:
`C192-root-closeout.json`, `C192-latency-whole-render-diagnostic.json`,
`C192-phase-review.json`, `C193-vfio-poll-build-checks.json`,
`C193-latency-idle-spin.json`, `C193-idle-cpu.json`, `C193-root-closeout.json`
and `C193-cache-relocation.json`.

## Parallel trap reduction candidates

- `b53f534e` adds `CARAXES_GUEST_TOKEN_EXTRAS=runlist:chid,...` (decimal;
  unset or `0` keeps the previous offer). Sparse datamatches consume only spare
  FDs after the unchanged automatic token list. Pool, MSI-rearm and RPC-head
  reservations still share the 253-FD limit. Malformed fields, unselected
  runlists and out-of-range ChIDs fail before host allocation. With the C192
  settings, `0:55,2:56` fits behind all 250 base tokens. Excess valid requests
  remain trapped and the log lists the actual additions. A mapped usermode
  doorbell is incompatible. Default coverage and guest token values are unchanged.
- Guest-token readers resolve a wake against the current BootModel. No explicit
  reconnect-epoch discard of queued counts was found. This is a pre-existing
  limitation of all static guest-token eventfds; reset/channel reuse needs its
  own qualification before claiming stale-wake isolation across reconnect.
- `031b435c` adds an inactive exact read-only window USERD page builder. Every
  dword, including holes and assembly/ARMED state, matches the ordinary display
  model; all 32 eligible pages and a real window-4 pushbuffer update are tested.
  Only emulated per-guest state is published. Every write must stay trapped.
  The time-dependent core scan-line page is excluded. No runtime mapping or
  refresh call-site is added.
- The display release probe measures about 45.5 us per full-page refresh and
  43 ns per one-word publication. It is a local model probe, not a production
  hot-path benchmark. A full scan after every PUT could erase the savings from
  mapped reads; use it only for initialization/reset. Activation requires an
  audit and incremental publication at every channel-state mutation, under the
  current display owner, before write/control completion.
- In the separate VMM tree, `4dfa2bc66` adds an inactive exact IRQ core. Ten
  standalone tests pass; thirteen tests with the actual backend oracle compare
  4,000 ordered transitions, every page word and MSI count. Byte lanes, repeated
  SET, ACK-again, TOP masking, hints, watchdog, availability and reset are covered.
  It is not registered in the VMM runtime and removes no traps yet.
- IRQ activation still needs negotiated single ownership, reliable cause
  transport, read-only page/MSI adapters and reset/BAR lifecycle handling.
  Preserve host pending-bitset drain grouping: splitting two raises into two
  folds can create an extra ACK-again MSI. Initial activation is restricted to
  fresh exact reset; importing live tagged state is unimplemented. A transport
  fault must stop the device, without falling back to another mutable tree.
- Stable backend validation passes 936 library, two parser and two integration
  tests, with one ignored; release build and whitespace checks pass. AST graph
  update completes with the existing header parse warnings. The display and
  local IRQ candidates remain inactive during sparse-token hardware qualification.

Private evidence: `C194-build-checks.json`, `C194-graphify-update.log` and
`local-irq-foundation-2026-10-06/metadata.json` in the Windows artifact directory.

## Sparse-token Windows qualification

- C196 runs stock Windows with `0:55,2:56`, all 250 automatic entries and both
  additions registered. Device creation takes 635 ms; three verified D3D11
  readbacks take 24 ms. Pre/post health stays Problem 0 with no Display 4101.
  The 3DMark window becomes ready after 49.4 seconds. An earlier snapshot request
  precedes readiness and fails with "UI window unavailable"; its evidence is
  retained, followed by a successful fresh snapshot.
- One diagnostic Fire Strike completes all four primary results: graphics 5,092,
  GT1 17.766 FPS and GT2 29.367 FPS. This instrumented run has no strict child-exit
  audit and is excluded from scored comparisons. No gain or moving-scene proof
  is promoted. The process query's name filter is incomplete; continuing
  SystemInfo activity alone does not identify which rendering phase is active.
- Across the 375.2-second launch/render/collection/idle bracket, 17,131 ordinary
  doorbells still trap: runlist 0 / ChID 51 (16,215), runlist 2 / ChID 52 (910)
  and runlist 2 / ChID 50 (6). The offer's new values have no trapped writes, but
  no per-value eventfd counters establish that they were used. Automatic ChID
  allocation differs from C192. There are 29,995 eventfd wakeups representing
  67,824 guest writes. Static values from one trace are not general coverage.
- The same bracket has 1,543,931 interrupt writes (4,115/s), 5.792 seconds of
  backend service, 36,523 FE writes and zero display-status read traps. Nearby
  VMM snapshots span 372.4 seconds: 1,534,294 requests taking 43.681 seconds plus
  0.086 seconds of client mutex wait, averaging 28.47 us and 0.056 us. Endpoint
  differences are +1.21/−1.58 seconds. The local IRQ owner remains the next
  route-level change; this is not a complete explanation of the low FPS.
- Available checks pass with no new Xid or VA-update error; 19 other backend
  ERROR lines remain. All owned processes and samplers stop; NVIDIA remains bound.
  C194 and C195 fail admission before creating any VM because of package path
  and checksum-basename errors. Those frozen packets are preserved. C196 uses a
  corrected sibling package with identical binary bytes and unchanged guards.
- `b1e25965` separately fixes the vfio-user client's inverted RESET flag test.
  A socketpair regression tests real `DEVICE_GET_INFO` replies with and without
  the flag; all 18 isolated vendor tests pass. Production Cargo.lock is unchanged.
  C196 uses the previous immutable VMM/backend and does not test this correction.
  The VMM's relative vendor path currently resolves into the main checkout,
  rather than this worktree. Pin an isolated matching copy before implementing
  the local-IRQ protocol; do not alter the main checkout through that symlink.

Private evidence: `C196-root-closeout.json`, `C196-doorbell-delta.json`,
`C196-latency-whole-render-diagnostic.json`, `C194-admission-stop.json`,
`C195-admission-stop.json` and `C197-reset-vendor-validation.json`.

## Incremental read-only window page

- `3087ab82` wires `CARAXES_DISPLAY_WINDOW_RO=window:0..31`; unset or `0`
  keeps the existing path. It requires host execution, the boot model and
  display. Legacy writable kick pages are rejected in either attachment order.
  The selected page belongs to the same VM's interrupt memfd; every store
  remains trapped. No physical BAR is exposed.
- Allocation and pushbuffer controls publish PUT/GET. Kicks publish accepted
  PUT before engine work and canonical assembly/ARMED words before the
  triggering UPDATE can notify. GET advances after parsing finishes; an earlier
  completion may observe the prior consumed pointer. Never publish GET early,
  allowing the guest to overwrite unread pushbuffer words. Notification-time
  ordering is source-reviewed; no dedicated interleaving test was added.
- The mapped decoder preserves all page words, reserved holes, byte reads,
  partial-write behavior and current FREE persistence. Regression pushbuffers
  include methods 0, 4 and 0x100: raw method deltas must not replace register
  pointers or otherwise-zero readbacks. Full refresh runs only at attachment
  and reset, with changed dwords published on ordinary operations.
- Reset errors propagate and erase the old image. Terminal handling detaches
  the display, disables MSI, retires the model and rejects later VFIO requests.
  Host memory stays allocated until normal worker-join teardown. A later
  client requires a fresh backend/memfd: closing the socket cannot revoke a
  read mapping retained by the departing VMM.
- The first full suite exposed a new reader-constructor deadlock: it acquired
  the Device mutex while the independent exact-rearm test held it. The reader
  now receives its fault flag through Routes without acquiring that mutex.
  The final suite passes 939 library, three parser and two integration tests;
  the isolated reset-failure test also passes. Two tests are ignored in the
  full suite, including that global-Setup test. Release and diff checks pass.
- The VMM must map the selected area read-only. For window 4, add
  `0:0x694000:0x1000` to `x_readonly_mmap_areas`; tree/FE areas remain as before.
  The existing frozen launcher does not add this area automatically. C198 has
  a separate pinned launcher with all three protections. Merely setting the
  backend variable with a writable VMM mapping is not a valid configuration.
- C198's source, release and runtime packet are pinned. Hardware was not
  started: artifact free space was 43.38 GiB, below the retained 45 GiB floor.
  No owned GPU VM remains; NVIDIA stays bound. Do not promote Windows mapping
  stability, trap savings or FPS gains until fresh hardware qualification.

Private evidence: `C198-build-checks.json`, `C198-source-regression-stop.json`,
`C198-hardware-pending-storage.json` and `C198-display-window-runtime/`.

## Local IRQ implementation boundary

- VMM commit `f1c8d8f23` pins our existing vfio-user dependency inside its own
  worktree, eliminating the shared symlink into the main checkout. The lockfile
  is unchanged. The copied library passes 18 tests; CH PCI check and four
  read-only mapping tests pass. This is not a new VMM hardware qualification.
- The reviewed activation plan uses a VMM-owned 4 KiB read-only guest tree
  page and a separate hidden, sequenced backend-to-VMM queue. Preserve each
  complete pending-bitset drain as one Fold. One owner applies queued causes,
  guest ACK/mask/trigger/rearm and MSI delivery in order. Queue faults stop the
  session; no silent drop or fallback to a second tree owner.
- Required wiring: optional VERSION negotiation and session hooks; retire
  backend tree/MSI owners before COMMIT; local page/MSI adapter and independent
  consumer; RPC completion fences; reset epochs, BAR moves, disconnect and
  migration guards. Unsupported clients keep the existing generic path. The
  tested core remains inactive and removes no interrupt-write round trips yet.
- A later VMM-to-backend doorbell queue should capture each actual four-byte
  value and feed existing post_doorbell/RingGate behavior. Preserve BAR2 flush,
  submission holds and reset generations. This avoids static ChID offers and
  per-value FD limits without exposing a raw physical doorbell. It does not
  remove KVM write exits or establish a performance gain by itself.
- Next hardware step: qualify C198 boot/D3D/desktop after storage headroom is
  restored. Next substantial performance step: linked local IRQ transport
  tests, then an instrumented Windows control. Keep full rendering/recording
  for an actual change or measured gain; a sparse-token retest alone has no
  demonstrated benefit.

Private designs: `local-irq-activation-design-2026-10-06/plan.txt` and
`local-doorbell-value-queue-design-2026-10-06/plan.txt`. Old packets, results and
the earlier Claude handoff are unchanged.

## Local IRQ source activation

- The inactive boundary above is superseded by the paired implementation in
  [local-irq-2026-10-06.md](local-irq-2026-10-06.md): negotiated ownership,
  shared-memory causes, private VMM register page and local MSI delivery are
  wired into the real backend and CH device path. Default remains off.
- Backend tests pass 943 library, three binary and two integration cases;
  vendor tests pass 28 and CH PCI tests pass 53. Five linked tests exercise
  the actual backend/CH owner, including reset and progress during a blocked
  unrelated RPC. Frozen source hashes are unchanged throughout verification.
- C199 is the next short Windows qualification, with local IRQ on and the
  optional display window off. C198 remains a separate unlaunched candidate.
  Source tests establish neither guest stability nor an FPS improvement.

## First local IRQ qualification attempt

- C199 starts the paired VMM/backend but its test guard stops before probes:
  the package lacks the launcher's expected `source.json`. Both owned processes
  exit and NVIDIA remains bound. No guest health or performance result accepted.
- Review finds a separate real bug: both backend ownership guards rejected
  CYA2 config-mirror reads, while CH correctly forwards them. Guards now distinguish
  reads from rearm writes and report offset/direction on ownership violations.
- Six actual-backend linked tests pass after the correction, including partial
  and neighboring CYA2 reads without MSI replay. The new case fails with the
  old guard. A CH forwarding regression passes; its full PCI suite is now 54.
- C200 repeats the short qualification with fresh metadata and an identity-ready
  marker. Immutable C199 sources, binaries and failed results are retained.
