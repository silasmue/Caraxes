<!-- SPDX-License-Identifier: MIT -->
# Channel migration and the deferred API

The Windows KMD runs some RM controls inside a channel's stream instead of at once. It
registers the control on a deferred API object (`NV50_DEFERRED_API_CLASS`, 0x5080) under
the channel and triggers it later with a software method in the pushbuffer. It prepares
every application GR context this way, and under contention it moves a busy channel to a
larger ring the same way. VFIO Leandro runs each registered control where GSP-RM would run
it: a context request on the device's management path; a ring migration in the host
channel's own stream through the host RM's 0x5080 object, or in the software executor.

## The driver's objects

| Control | Id | Layout |
|---|---|---|
| `NV5080_CTRL_CMD_DEFERRED_API` | 0x50800101 | legacy: `hApiHandle`, `cmd`, `flags`, `hClientVA`, `hDeviceVA`, a union of context, TLB, PTE, ZCULL, PM and preemption bundles (584 bytes as Windows sends it) |
| `NV5080_CTRL_CMD_DEFERRED_API_V2` | 0x50800103 | the same head; the union adds `DisableChannels` and `ChannelInfoUpdate` |
| `NV5080_CTRL_CMD_DEFERRED_API_INTERNAL` | 0x50800104 | V2's layout, forwarded from the kernel RM to the physical RM |
| `NV5080_CTRL_CMD_REMOVE_API` | 0x50800102 | drops a registration |

- Header: `src/common/sdk/nvidia/inc/ctrl/ctrl5080.h`; code:
  `src/nvidia/src/kernel/gpu/deferred_api.c` (615.71.09).
- The kernel RM forwards V2 as INTERNAL (`defapiCtrlCmdDeferredApiV2_IMPL`); for a ring
  migration it first registers the new USERD memory with GSP (`memRegisterWithGsp`).
  GSP-RM stores every layout in one table (`defapiCtrlCmdDeferredApiInternal_IMPL`,
  `_Class5080AddDeferredApi`).
- The pushbuffer triggers a registration with the 0x5080 software method 0x0200, data = the
  API handle (`_class5080DeferredApiV2`). `defapiIsSwMethodStalling_IMPL` makes the method
  non-stalling for `NV2080_CTRL_CMD_FIFO_UPDATE_CHANNEL_INFO` only.
- Flags select implicit deletion after the method ran, and optionally a wait for a TLB flush.

## Deferred contexts

- The KMD allocates an application's GR channel with `NVOS04_FLAGS_SKIP_CTXBUFFER_ALLOC`
  and sends `GPU_INITIALIZE_CTX`, `GPU_PROMOTE_CTX` and `GPU_EVICT_CTX` for it as deferred
  APIs on its management channel (chid 4 in the runs)
  ([research: windows-deferred-context](../research/windows-deferred-context-2026-09-28.md)).
- The device (`crates/vfio-user-nvgpu/src/ctrl/deferred_context.rs`, Windows launcher
  switch `CARAXES_DEFERRED_CONTEXT=1` with `CARAXES_SHADOW_USERD=1`):
  - A registration keeps client, engine object, API handle, channel generation, target
    TSG and shadow root; each API runs once.
  - INITIALIZE builds the target's host mirror with a host-owned GR context and an empty
    private USERD. The guest's context addresses are checked against the reservation and
    never passed to the host: the host RM's GR object constructor owns the context.
  - PROMOTE opens the target for submission. Before PROMOTE and from EVICT on it is held.
  - EVICT idles the exact host channel (`FIFO_IDLE_CHANNELS`), harvests its USERD and frees
    the host TSG.
- The context belongs to its TSG: it moves to a live member when its channel is freed
  (c5deec2a), and an eviction whose host context is gone frees the host TSG (6c566092)
  ([research: windows-findings](../research/windows-findings.md), P29b, P30).
- The preparation runs on the shadow worker. The deferred method stays pending, the
  management channel is held and replayed by the slice clock until the build is committed
  ([caraxes/execution.md](../vfio/execution.md) §9.12). Before, 57 preparations of
  390 to 432 ms each held the device lock for about 23 s (C83).
- A video channel's 0x5080 object stays an inert host object (1dc497c5, P29b).

## Ring migration

- `NV2080_CTRL_CMD_FIFO_UPDATE_CHANNEL_INFO` (0x20801116, `ctrl2080fifo.h`) moves an
  existing channel to a new GPFIFO (`gpFifoOffset`, `gpFifoEntries`) and a new USERD
  (`hUserdMemory`, `userdOffset`). Directly it needs an idle channel; as a deferred API it
  runs at the software method.
- WDDM grows a busy channel's ring this way under contention, with the deferred call as the
  old ring's last entry. Example: chid 22, an application's GR channel, moved to a
  65536-entry GPFIFO at 0x2_2538_0000 and a new USERD
  ([research: windows-findings](../research/windows-findings.md), step 23).
- The device (`register_channel_migration`, `update_channel_info` in
  `ctrl/deferred_context.rs`):
  - V2 and INTERNAL register into the same table as the legacy layout.
  - A host channel: the host RM registers the migration with its own
    `NV5080_CTRL_CMD_DEFERRED_API_V2` on the owner's host 0x5080 object, same API handle,
    naming the host channel, a second private USERD and the guest's GPFIFO VA
    (`mirror.rs` `begin_migration`). The host RM moves the channel in its own stream at the
    software method. Both rings are published until the new ring's GPGet moves
    (`settle_migrations`).
  - A software channel: the executor switches rings after the method's entry
    (`channel.rs` `RingSwitch`).
  - A direct 0x20801116 moves an idle channel now (`mirror.rs` `migrate_now`,
    `channel.rs` `switch_ring`); a refusal is `NV_ERR_INVALID_STATE`.
  - Refused: a migration between a host channel and a software one, and a host channel's
    new USERD outside guest system memory.
- Every migration in C116 to C118 went from 8192 to 65536 entries, 16 to 106 ms after its
  registration. The switch is exact: in C118 both switched at the old ring's end (GPGet 8017
  = the last published GPPut 8017; 3334 = 3334)
  ([research: windows-findings](../research/windows-findings.md), step 23).

## The bug it fixed (KNOWN BUG #1)

Evidence: [research: windows-findings](../research/windows-findings.md), step 23.

- Symptom: Heaven or Superposition beside a second GPU client (Steam, Edge) crashed 90 to
  250 s into a run (0xC0000005 in `d3d11.dll`, or 0xC0000409) with a burst of
  LiveKernelEvent 141, or hung; no TDR, no FLR, no host Xid. About half of the runs.
- Cause: every failure followed, within 0 to 65 s, a refused
  `NV5080_CTRL_CMD_DEFERRED_API_INTERNAL` whose nested command was the ring migration (13
  across C101 to C114). The device modelled only the legacy context requests and marked
  the owner for a management takeover, which cannot happen past a channel's start
  (GPGet at most 32). The refused method never completed.
- Ruled out: the shader cache, the guest's files, host builds, device stalls (every hold
  under 80 ms in passing and failing runs).
- Fix: deferred-v2 (196b3f78), following `deferred_api.c`.

| Run | Load | Result | Migrations |
|---|---|---|---|
| C116 | present-probe beside three Heavens (the C112 repro) | 3 of 3 pass; 154.6, 154.3, 152.5 fps | 3 |
| C117 | Steam and Edge running | 4 of 4 pass; Heaven 158.3, 153.9 fps; Superposition 12247, 12175 | 3 |
| C118 | the gate candidate, Steam and Edge | stages 12/12; Heaven 155.9, 153.3 fps; Superposition 12228, 12160 | 2 |

- 0 deferred refusals, 0 hangs, 0 crashes, 0 host Xid, 0 FLR in all three.
- Open: KNOWN BUG #2, a 3DMark DX12 constant-cache fault at VA 0 whose first instance came
  1.3 s after an exact migration; the second had none near it
  ([research: windows-findings](../research/windows-findings.md), step 23).

## Depth

- [research/windows-findings.md](../research/windows-findings.md): P29b to P31, steps 12, 19, 23.
- [research/windows-deferred-context-2026-09-28.md](../research/windows-deferred-context-2026-09-28.md):
  the context lifecycle contract and its runs.
- [channels-and-submission.md](channels-and-submission.md), [software-executor.md](software-executor.md),
  [execution-mirror.md](execution-mirror.md).
