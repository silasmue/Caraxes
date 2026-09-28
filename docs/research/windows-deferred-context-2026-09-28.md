# Windows deferred contexts

Status: experimental, off by default. P9 completed eight context lifecycles; D3D11 rendering still fails.

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
