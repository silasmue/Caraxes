# Windows deferred contexts

Status: experimental, off by default. Hardware validation pending.

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
  Private USERD GPGet must equal its last published GPPut. Initial subchannel
  bindings are recovered from at most 32 SET_OBJECT/NOP entries.
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
