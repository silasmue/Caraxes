# Windows deferred contexts

Status: experimental, off by default. Context lifecycle passes; graphics pool support is missing. The current host path refuses bootstrap pool acknowledgements, and Windows reports Code 43. No D3D11 rendering pass.

## Contract

`CARAXES_DEFERRED_CONTEXT=1` handles the captured legacy 584-byte NV5080
registration for GR0 INITIALIZE_CTX, PROMOTE_CTX and EVICT_CTX. It requires
host execution, shadow VA spaces and `CARAXES_SHADOW_USERD=1`. Capture mode
must be off.

- Registrations retain exact client, engine object, API handle, channel
  generation, target TSG and shadow root. Flags must select implicit deletion.
- A registration identifies its owning management channel. Allocation flags
  alone cannot distinguish management from ordinary GR channels.
- An idle host management channel is retired before software takeover.
  Initial subchannel bindings and captured nonexecuting 2D/notifier setup are
  previewed from at most 32 entries before retirement. FIFO_IDLE_CHANNELS must
  then succeed for the exact
  host channel; GPGet is rechecked against its last published GPPut.
- The software interpreter stops at the deferred method. Following methods
  cannot execute until the operation completes. Unknown work fails closed.
- INITIALIZE prepares the target's host-owned GR context with an empty private
  USERD. PROMOTE enables later submission after initialization succeeds.
- Target submission is blocked before PROMOTE and during/after EVICT. EVICT
  idles the exact host channel, harvests its final USERD, frees the host TSG and
  synchronizes the software cursor. One target channel per TSG is supported;
  shared-group eviction is unsupported.
- Guest physical context addresses are validated against the VM reservation,
  but are not passed to host RM. The host driver's GR object constructor owns
  the actual context allocation, initialization and promotion.
- Each API is consumed once. Freed or replaced targets invalidate registrations.
  Failed host FREE retains the tracked object and prevents replacement execution.

This is a shadow-context lifecycle model, not support for arbitrary deferred
RM controls. The legacy guest context buffer is not installed over the host's
internally managed context. Management supports the captured C597 one-word
report release after ordered completion. Rendering commands and compute launches
remain unsupported on management channels.

## Related fixes

- Preserve guest engine handles through initial and late host object allocation.
- Remove an individually freed engine object from both registry and host mirror.
- Reject duplicate live engine handles. A failed host FREE keeps its handle busy.
- Remove child channel records and tokens when their TSG is freed.

## Validation

- ABI615: 528 unit tests passed, one ignored; two integration tests passed.
- Tests cover captured payloads, malformed input, client isolation, generation
  reuse, teardown, deferred ordering, exact-once completion and takeover bindings.
- No D3D11 or desktop success is established by these software tests.

## P5 hardware result

- Base 73483c8d, isolated LUT geometry diagnostic profile.
- Binary SHA-256: 5fd978a34a353e817cd9000a341ebf6822d67f8ea9a31be7fc9ebc1edbe46906.
- Windows first entered recovery after earlier interrupted boots. QMP Enter,
  then Enter selected Continue; no reset or repair operation was selected.
- INITIALIZE 0x40000002 and PROMOTE 0x40000003 registrations succeeded for
  target chid 17, generation 18, through management chid 4.
- Initial-prefix validation stopped at subchannel 3 method 0x888. The NVIDIA
  cl902d.h name is SET_PIXELS_FROM_MEMORY_SAFE_OVERLAP; the full binding and
  parameter sequence still need capture before deciding what state to preserve.
- No deferred method completed. The host management channel had already been
  retired before validation failed; move prefix validation before retirement.
- GPGet equality proves FIFO fetch progress, not completion of arbitrary engine
  work. A supported takeover prefix must contain only proven state setup, or
  the transition needs an explicit completion fence.
- No new host Xid observed. All owned test VMs stopped. No rendering pass.
- Artifacts: alpha-2026-09-27/windows-plain/P5-deferred-context/.

## Follow-up before P6

- Preview the initial prefix without changing the registry or freeing host objects.
- Reject unsupported prefixes once per channel generation and keep them off the
  submission path. Include bound class, method, data and up to 256 raw dwords.
- After a valid preview, require host FIFO_IDLE_CHANNELS success with a 100-ms
  timeout and pushbuffer/cache/all-engine flags. GPGet alone is not a fence.
- Apply recovered bindings only after host retirement and generation revalidation.
- ABI615 remains 522 unit tests plus two integration tests passed, one ignored.

## P6 prefix capture

- Base 772f2abd; binary SHA-256:
  39bfe22179d5cece876b2378d3bf746e322468f41027b10814a40758bfa0a592.
- Preview rejected before retiring the host channel. The complete first entry
  is 43 dwords: object bindings, 2D state setters and GR/compute notifier addresses.
  No launch, semaphore action, notify trigger or deferred invocation occurs in it.
- Subchannel 5 binds value 1. This is the channel-local ENG_SW class ID assigned
  by kchannelRegisterChild, not the external class number 0x5080. The software
  interpreter must retain this ID-to-object association across allocations/frees.
- Next: allow the captured nonexecuting setup and retain software object IDs.
  Unsupported later 2D/compute work must still stop, since its old state is not
  transferred into a software rendering implementation.
- VM stopped. No new host Xid; deferred execution and rendering remain unvalidated.
- Artifacts: alpha-2026-09-27/windows-plain/P6-prefix-preview/.

## P7: fenced takeover and first execution packet

- Base 47c19704; binary SHA-256:
  3a12821cd227582737985d82424902ae860b6b7828a1b2a475a338edd5fad47a.
- ABI615: 524 unit tests and two integration tests passed; one ignored.
- Windows recovery required selecting Continue. Normal driver startup followed.
- Exact host FIFO_IDLE_CHANNELS completed with NV_OK in 1247 us, before the
  management host TSG was freed. The initial setup prefix and local SW ID passed.
- The next 20-dword packet contains, in order:
  - Legacy acquire at VA 0x120233540, payload 2.
  - A140 binding and SET_REFERENCE(0).
  - Deferred INITIALIZE API 0x40000002.
  - SET_REFERENCE(0), then deferred PROMOTE API 0x40000003.
  - C597 REPORT_SEMAPHORE at VA 0x120233560, payload 3, flags 0x1000f010.
- Strict execution stopped at the first SET_REFERENCE (host method 0x50).
  No deferred API completed and the following release was withheld.
- SET_REFERENCE must update USERD.Reference at offset 0x48. The captured 3D
  release requests one word after preceding writes, pipeline ALL; support must
  preserve the INITIALIZE/PROMOTE completion boundary before publishing it.
- ACPI powerdown followed by the stop helper shut down the VM. No new host Xid;
  the trace's RC Xid 45 is the emulator's explicit guest failure for method 0x50.
- Artifacts: alpha-2026-09-27/windows-plain/P7-software-object-id/.
- Rendering remains unvalidated.

## P8 interrupted; lifecycle correction

- Base 9a47c54b; binary SHA-256:
  d8988140d5e3ee1644dc1df743c583b58f1cb8edda49e41a9204e11467f935ea.
- SET_REFERENCE and the exact captured C597 release are implemented. Software
  tests verify no release before both deferred completions, including failure.
- Review found that EVICT could free a busy target without an explicit idle and
  then allow its next doorbell to recreate a context without INIT/PROMOTE.
- P8 was stopped before deferred registration/execution. This run provides no
  runtime validation of the new reference/report code. No new host Xid observed.
- The correction adds a dispatcher gate for all experimental GR skip-context
  targets, including before registration and after eviction. Only a live,
  promoted context can submit. EVICT closes the gate before attempting teardown.
- Target eviction requires exact-channel FIFO_IDLE_CHANNELS success, final
  private USERD harvest, and checked synchronization of the software GPGet.
  Failed eviction keeps submissions held; successful eviction cannot replay
  already executed entries or bypass the next context lifecycle.
- Artifacts: alpha-2026-09-27/windows-plain/P8-ordered-report/.

## P9: context lifecycle passes; preemption bind blocks D3D

- Base 79869d2f; binary SHA-256:
  d1c3be3a9140419a818c9a9a366c7a76f75da0a0c360d22970e6623a4b148f79.
- 24 deferred APIs completed: eight INITIALIZE/PROMOTE/EVICT cycles. Sixteen
  ordered C597 releases completed; no strict interpreter failure.
- Nine exact-channel idles returned NV_OK: one takeover and eight evictions,
  1286–3516 us. These are diagnostic control timings, not workload latency.
- NVIDIA RTX 2070, driver 32.0.16.1714, reports PnP error code 0.
- The NVIDIA-only D3D11 gate reaches device creation and fails with HRESULT
  0x8876017c after 3618 ms. No triangle/readback stage ran. Wrapper exit code 0
  is not a rendering pass; its captured output explicitly reports failure.
- Every context attempt fails GR_CTXSW_PREEMPTION_BIND (0x20801211) with
  NV_ERR_INVALID_OBJECT_HANDLE (0x33). Windows passes a live TSG handle
  0xff0e0000; the current public-control rule accepts only a direct channel.
- Next: capture the full 112-byte request, then implement context-owner
  resolution and the requested mode semantics. Recording an unsupported mode
  alone is insufficient evidence that the host context implements it.
- VM stopped after ACPI shutdown. Host reports no new Xid. QEMU's screenshot
  shows the lock screen on its display; it is not proof of NVIDIA rendering.
- ABI615: 528 unit tests plus two integration tests passed, one ignored.
  ABI610: 525 unit tests plus two integration tests passed.
- Artifacts: alpha-2026-09-27/windows-plain/P9-context-lifecycle/.

## P10: GFXP pool request captured

- Base b9632a8e; binary SHA-256:
  cee7b7177d6743095781eb653713c1ba3c459108de08ee2582ff30ad719f7dac.
- Seven binds captured. Flags 2 select graphics mode; gfxp mode 2 requests
  GFXP_POOL. Compute mode is not selected. Padding and route info are zero.
- First request's nonzero buffers: CONTEXT_POOL 0x120800000,
  CONTEXT_POOL_CONTROL 0x12010d000, CONTEXT_POOL_CONTROL_CPU
  0xfffff38f73ca3000. The last value is a guest Windows kernel address and must
  never be forwarded as a host CPU pointer.
- BIND follows successful deferred INIT/PROMOTE. Accepting a TSG and merely
  recording this request would not establish a working pool or preemption mode.
- Next diagnostic reads the existing owned host context's actual modes. In
  parallel, inspect pool support and whether advertised capability bits require
  correction so the stock guest can choose an implemented mode.
- VM stopped. No new host Xid. No new D3D probe in this capture-only run.
- Artifacts: alpha-2026-09-27/windows-plain/P10-preemption-capture/,
  including preemption.json with all seven exact payloads.

## P11: host context mode verified

- Base 63066e66; binary SHA-256:
  711790cb2d5ad66db65953ce655ebb1475c4df5dd5a1bdee5198e59498ab7129.
- Read-only GR_GET_CTXSW_MODES on each exact owned, promoted host context
  returns NV_OK, graphics WFI (0), compute WFI (0), zcull 1.
- This does not implement Windows' requested pooled graphics preemption (2).
  No guest handles, virtual addresses or CPU pointers were forwarded by the query.
- Separate bug: recorded.rs supplies estimated pool sizing and acknowledges
  INITIALIZE/ADD/REMOVE under the bootstrap assumption that no GR work executes.
  Those overrides also ran in host-backed mode. The host path now refuses these
  controls before recorded overrides; no unconstructed pool is reported ready.
- Next run tests whether the stock driver handles that refusal with a supported
  fallback. If it requires pooled preemption, an actual pool implementation is needed.
- VM stopped; no new host Xid.
- Artifacts: alpha-2026-09-27/windows-plain/P11-host-preemption-modes/.

## P12: no stock-driver fallback from refused pool sizing

- Base 2a4a83d1; binary SHA-256:
  93499676180a4abcf2d6ca5ec421ca1eff14b6a7306deca67d9e36bf29c63846.
- Pool sizing returns NOT_SUPPORTED at 24.83 s. Windows unloads GSP at
  26.16 s and reports NVIDIA PnP code 43. It does not select WFI in this run.
- The P9 code-0 result therefore relied on bootstrap pool acknowledgements;
  it was not a working graphics implementation. Keep those acknowledgements
  disabled for host execution while actual pool support is developed.
- Public GPU headers expose pool control-block offsets through 0x60 on Turing,
  but not enough initialization/ownership semantics to justify the old estimated
  sizing formula or writing a guessed freelist.
- RM pool QUERY/INITIALIZE/ADD/REMOVE controls are kernel-only. Flag 0x40 means
  ROUTE_TO_PHYSICAL; it lacks PRIVILEGED (0x4) or NON_PRIVILEGED (0x8). The
  access check requires KERNEL privilege; CAP_SYS_ADMIN grants USER_ROOT only.
- A separate public mode setter exists. Whether it can construct native pool
  state on an owned empty context still needs investigation; mode success alone
  would not prove the guest pool lifecycle or BIND contract is implemented.
- VM stopped; no new host Xid. No rendering probe after the explicit driver failure.
- Artifacts: alpha-2026-09-27/windows-plain/P12-host-pool-refusal/.

## P13: public mode setter on an empty owned context

- Base d72adeb7 plus archived diagnostic.patch; binary SHA-256:
  b7bfef979d3f577097443e848ce5f22a5798776a7d4eaa9b7e59e7490d74f875.
- Isolated build temporarily uses the P11 bootstrap replies to reach the probe.
  All SKIP_CTX target submissions are blocked, including after PROMOTE. Neither
  that bootstrap behavior nor the diagnostic LUT profile is merged into baseline.
- P13 did not reach the probe: startup stopped progressing before deferred
  registration. QGA was unavailable; the owned VM was stopped. Cause unconfirmed.
- P13b reached an exact single-member, prepared/promoted host TSG. Its private
  USERD had GPGet/GPPut zero, no publication and no doorbells. FIFO_IDLE returned
  NV_OK in 1321 us. GET reported graphics WFI (0).
- SET_CTXSW_PREEMPTION_MODE on the owned host TSG, flags 2 / mode 2 / route zero,
  returned NV_ERR_INVALID_ARGUMENT (0x1f). GET remained WFI. Restoring mode 0
  returned NV_OK and GET confirmed WFI. No target workload was submitted.
- This rules out that exact setter request as a shortcut; it does not identify
  the rejected field or prove that all other channel/buffer configurations fail.
- A separate public OGKM vGPU path allocates and maps five ordinary GFXP buffers,
  then BINDs mode 1. Its teardown BINDs WFI before unmapping/freeing. SET alone is
  not documented to allocate those buffers.
- Existing host measurements, IDs 0x0b..0x0f, give size/alignment pairs:
  0x10f300/0x1000, 0xe1000/0x100, 0x20000/0x100,
  0x3e3a80/0x1000, 0x82000/0x100. Preserve page rounding and memory floors.
- A production ordinary-GFXP path would also need per-TSG ownership, host VA
  exclusions, checked BIND/WFI, retryable cleanup and an independently validated
  guest pool model. Host mode 1 is not evidence that guest pool mode 2 works.
- Artifacts: windows-plain/P13-build/, P13-empty-host-mode/ and
  P13b-empty-host-mode/ under alpha-2026-09-27/.

## Owned-buffer foundation

80113455 adds a floor-accounted HostGfxp allocation, aligned host virtual
reservation, and retry-preserving buffer release. Physical buffers explicitly
use 4 KiB pages; accounting rounds up the required size. These helpers do not
implement pool lifecycle or change the guest-visible preemption mode.

The same review found that HostMemory::free_object removed its ledger entry
and children before RM_FREE returned. On refusal, Mirror retained its object
but could never retry through HostMemory. The ledger now changes only after
successful RM_FREE. A failure-injection test covers failure followed by retry.

Validation: ABI615 531 unit tests and two integration tests passed, one ignored.

## P14: ordinary GFXP with real host buffers passes

- Base 80113455 plus isolated diagnostic.patch; binary SHA-256:
  97ea37f2344b9a819a0ab9e996385f42db49ad7db4eaa9b7e59e7490d74f875.
- Same empty-target restrictions as P13, plus exactly one host channel on the
  shadow root and a drained mapping worker. FIFO_IDLE returned NV_OK in 985 us.
- On the member channel, SET2 returns INVALID_ARGUMENT and GET remains WFI.
  SET1 succeeds and GET reports ordinary GFXP (1); SET0/GET restores WFI.
- Five physical buffers allocated with floor accounting, rounded 4 KiB pages,
  aligned RM-selected VAs and nondeferred maps: 6,778,880 bytes total.
- BIND1 on the exact host TSG returns NV_OK and GET confirms mode 1. BIND WFI
  with zero addresses returns NV_OK; GET confirms mode 0. All five buffers are
  then unmapped and freed successfully.
- Host 2D (902D), 3D (C597) and compute (C5C0) objects exist on the target.
- No target workload or guest pool equivalence was tested. The temporary probe
  blocks submissions and finishes cleanup before the event thread can enqueue
  another mapping plan. Production requires persistent private VA exclusions.
- A first P14 build used a member handle for BIND; review corrected it to the
  group handle before any hardware run. That unused binary is separately marked
  NOT-RUN in P14-unrun-channel-bind-build/.
- VM stopped; no new host Xid. Artifacts: windows-plain/P14-build/ and
  P14-owned-gfxp-bind/ under alpha-2026-09-27/.

## Persistent owned-GFXP lifecycle

- ec3d1fa8 tracks five host buffers per exact TSG, with an eight-TSG limit and
  a 64 MiB limit per set. No production control enables this path yet.
- First bind requires a single prepared/promoted channel, exact generation and
  shadow root, private USERD at cursor zero and a successful host idle. A drained
  worker precedes allocation; all host-selected VA spans enter private exclusions.
- BIND1 and GET1 must succeed before target submission reopens. Partial state
  remains tracked with submission blocked; retry is through teardown, not bind.
- Engine-child, channel and TSG frees idle the target, BIND WFI and verify mode 0
  before freeing buffers. Exclusions remain until every child is freed, then the
  merged exclusion set is recomputed without subtracting another owner's range.
- Fixed existing parent-lifetime errors: the shadow TSG stored its guest TSG key
  where its VAS key belonged; drop_vas ignored failed dependent frees; reset cleared
  ownership sets even when resources survived cleanup. All now retain dependencies.
- HostMemory stops LIFO frees at the first failure and keeps gather descriptors
  pinned. A final shutdown with retained owned-GFXP tags skips explicit teardown
  entirely and leaves ordering to native RM client-FD closure. Otherwise newer
  buffers could be freed before older, still-live channels.
- c9531d34 makes a successful virtual allocation return ownership before address
  validation. An invalid VA no longer hides a failed rollback from the context owner.
- ABI615: 535 unit tests, two integration tests passed; one ignored. ABI610:
  532 unit tests and two integration tests passed at ec3d1fa8.
- Next experiment maps the captured Windows pool BIND to owned ordinary GFXP.
  Its legacy pool sizing/INITIALIZE/ADD/REMOVE replies are still incomplete and
  confined to an isolated diagnostic build. A D3D pass would not validate pool
  lifecycle, arbitrary slot changes, preemption stress, or multi-guest graphics.

## P15: first compatibility run and startup race

- Base c9531d34 plus isolated pool/BIND/LUT diagnostic; SHA-256:
  df785f2e72f0544a8efed2b257281e6d0079305a5520aa38bd56d5dbbd1b9e43.
- P15 stalls before deferred registration or owned GFXP. Like P13, sequence 829
  completes but sequence 830 never arrives. Successful P13b/P14 receive it after
  84/110 ms. Shadow synchronization reports no mapping failures.
- Notifier 0x0800000c is GR0 nonstall, mapped to guest vector 4. The existing
  log text for no MSI also includes normal coalescing; it cannot prove masking.
  Code review separately found that PCI MSI enable/eventfd installation does not
  replay already-pending shared interrupts. Causation of this stall is unproven.
- P15b, same binary/configuration, boots with NVIDIA PnP code 0. Each GFXP bind
  is refused because the promoted target already received its first doorbell.
  D3D11CreateDevice still returns 0x8876017c after 3461 ms; no rendering pass.
- Both VMs stopped, no new host Xid. P16 will hold the experimental target until
  owned GFXP is bound, retaining the empty-first-use requirement.
- Pool capture: QUERY maxSlots=4; INIT maxSlots=0, memory caf0000b, offset
  0x440000, size 0x1000; ADD count 4, slots 0..3, same memory/span. Allocation
  0x450000 matches our fabricated query layout rounded to 64 KiB. This demonstrates
  guest consumption of our answer, not native geometry. Public headers define
  INIT.maxSlots as input; the closed Windows driver's zero remains unexplained.
- Future pool translation needs tracked memory identity, allocation bounds,
  initialization and slot lifecycle. Slot-to-context association and control-block
  contents remain unknown; unconditional bootstrap replies are not a valid model.

## P16: first-use gating establishes GFXP, then compute CTA is requested

- Base 178bfd2f plus isolated diagnostic.patch; SHA-256:
  3ceee816bd77b50283f29842649ce7556abedc54556d148c35201e60cfb20d76.
- Diagnostic holds SKIP_CTX GR targets until their exact owned-GFXP state is
  bound. All submission routes converge on the gate; management remains exempt.
  Held work replays through the existing periodic path after successful binding.
- Multiple real BIND1/GET1 operations pass, five private buffers per target.
  The trace confirms target doorbells after binding. NVIDIA reports PnP code 0.
- The next captured BIND differs: flags1 (compute only), gfxp0 (ignored),
  cilp1 (CTA), all nine addresses, padding and routing zero. The narrow graphics
  diagnostic rejects it. This is not a graphics WFI/unbind request.
- D3D11CreateDevice still returns 0x8876017c after 4025 ms. No rendering pass.
  P17 will translate only that exact compute request to the owned host TSG and
  require GET to confirm CTA while preserving ordinary GFXP.
- Shared-MSI correction 178bfd2f has full regression results: ABI610 535 unit
  + two integration passed; ABI615 538 unit + two integration passed, one ignored.
  Use `--features v615` for that build; CARAXES_GSP_ABI alone does not select it.
- Remaining interrupt gap: shared bits already drained into IntrTree and its local
  MSI flag can outlive a failed delivery without availability-transition replay.
  Page mode remains owned by its reactor/watchdog. The startup race is not solved
  merely because this boot succeeded.
