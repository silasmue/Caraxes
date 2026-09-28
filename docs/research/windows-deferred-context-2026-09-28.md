# Windows deferred contexts

Status: experimental, off by default. P5 reached registration; execution is unvalidated.

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
  Initial subchannel bindings are previewed from at most 32 SET_OBJECT/NOP
  entries before retirement. FIFO_IDLE_CHANNELS must then succeed for the exact
  host channel; GPGet is rechecked against its last published GPPut.
- The software interpreter stops at the deferred method. Following methods
  cannot execute until the operation completes. Unknown work fails closed.
- INITIALIZE prepares the target's host-owned GR context with an empty private
  USERD. PROMOTE enables later submission after initialization succeeds.
- EVICT frees the exact host TSG and waits for teardown. The first version
  accepts one target channel per TSG; shared-group eviction is unsupported.
- Guest physical context addresses are validated against the VM reservation,
  but are not passed to host RM. The host driver's GR object constructor owns
  the actual context allocation, initialization and promotion.
- Each API is consumed once. Freed or replaced targets invalidate registrations.
  Failed host FREE retains the tracked object and prevents replacement execution.

This is a shadow-context lifecycle model, not support for arbitrary deferred
RM controls. The legacy guest context buffer is not installed over the host's
internally managed context. 3D methods and compute launches on management
channels remain unsupported until explicitly implemented.

## Related fixes

- Preserve guest engine handles through initial and late host object allocation.
- Remove an individually freed engine object from both registry and host mirror.
- Reject duplicate live engine handles. A failed host FREE keeps its handle busy.
- Remove child channel records and tokens when their TSG is freed.

## Validation

- ABI615: 522 unit tests passed, one ignored; two integration tests passed.
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
