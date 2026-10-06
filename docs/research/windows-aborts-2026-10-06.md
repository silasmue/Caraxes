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
- A writable shared interrupt page cannot retain repeated same-value stores.
  Offloading the remaining writes requires an ordered register model, for example
  in the VMM, with synchronous read visibility and correct MSI delivery. Do not
  qualify dropping or merging these stores from access counts alone.
- The current Cloud Hypervisor path is KVM MMIO write exit, PCI bus dispatch,
  client mutex, then a synchronous vfio-user `REGION_WRITE`. Its read-only memory
  mapping means a local VMM handler would still take the KVM exit, but could
  remove the socket round trip. Measure mutex wait and transport latency apart
  from backend register service before choosing that larger change.
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
- The physical GPU reaches 1,830–1,860 MHz during C187's high-utilization samples;
  memory remains at 7,000 MHz. Idle returns to 1,410 MHz. These physical samples
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
