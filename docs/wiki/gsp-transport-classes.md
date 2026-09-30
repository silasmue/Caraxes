<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GSP transport classes

Every routed control was classified by the structure of its command (call, data, pointers,
handles, side effects) into what could leave the trapped path and what must stay on it,
with isolation as a factor of the verdict. The classes rank the transport work by mechanism
instead of by workload.

## The verdicts

| Verdict | Rows | Rule | Mechanism |
|---|---:|---|---|
| ZT-PAGE | 101 | a getter answered from chip data, costume or display configuration, not computed from the request | could be published on a read-only page; a stock driver has no read path for it, so the gain (about 1.5 ms per CUDA process) does not pay for a driver change |
| ZT-QUEUE | 155 | device-only: acknowledged without effect, this VM's device state read or set, or computed from the request (AUX, mode checks) | the polled queue |
| HOST | 103 | a host RM round trip: static getters, telemetry, GSS-legacy ids | caches: a device cache per card for static answers, a TTL cache for telemetry, and, behind the open switch `CARAXES_LEGACY_CACHEABLE=1`, the reply's CACHEABLE bit that makes a stock guest cache ten static GSS-legacy answers itself (`rpc.c`, `rpcRmApiControl_GSP`) |
| TRAP | 32 | a state change carrying guest addresses (`DMA_SET_PAGE_DIRECTORY`, `GPU_PROMOTE_CTX`, display pushbuffers), embedded handles, a change to what a zero-trap window serves (`GPFIFO_SCHEDULE`, binds, flushes), or an act on the VM's host TSG (`PREEMPT`, `SET_TIMESLICE`) | synchronous by necessity, validated in order before the reply |
| REFUSE | 490 | 379 absent on TU106, 111 isolation | never optimised |

- Table: `caraxes/abi/gsp-615.71.09/transport.tsv` (881 rows, with the GSS-legacy and BINAPI
  ids the profiles saw); tool: `caraxes/tools/gsp-transport.py`.

## What the records say

- Controls are lifecycle traffic (profiles of 2026-09-25): 272 RPCs per 615 boot, 187 per
  CUDA process (45 of them controls), 101 per `nvidia-smi -q`, 1352 per GNOME start, 0 per
  submission and 0 per frame (one `BUS_FLUSH_WITH_SYSMEMBAR` in Mutter's window).
- The trap census of 2026-09-30 counts 5 to 40 RPC/s on Linux (SotTR 5 to 36) and 45 RPC/s in
  a Windows NVENC stream, 49 at a Windows desktop start; the device time behind a queue-head
  write is 67 to 1520 us ([research: trap-census](../research/trap-census-2026-09-30.md)).
- Per RPC the transport costs more than the device: four trapped accesses on BAR0 page
  0x110000, 108 us median, against 3 us of device work (2026-09-25). For HOST rows the host call (0.2 to
  80 ms) dominates.
- The largest saving is in the HOST class: 1852 ms of GSS-legacy forwards, 375 ms of static
  host getters and 373 ms of telemetry over the profiles; the polled queue would save about
  130 us per RPC (24 ms per CUDA process, 175 ms per GNOME start).

## The polled queue, in short

1. BAR0 page 0x110000 becomes a read-only memory area of the device's memfd; `QUEUE_HEAD`
   and `MSGQ_TAIL` writes become ioeventfds (a store to a read-only slot still exits).
2. The reactor takes exactly as many elements as the eventfd counted, syncs the BAR2 memfd
   before the first, answers device-only rows itself and hands HOST rows to a worker.
3. A `QUEUE_HEAD` write of 0 (reload) needs a datamatch-0 ioeventfd registered first; the
   falcon data ports on the page must never be read by the guest while mapped; FLR restores
   the boot values.
4. The first step, the `QUEUE_HEAD` ioeventfd alone (about 30 us per RPC), is built as the
   open switch `CARAXES_RPC_IOEVENTFD` ([gsp-message-queue](gsp-message-queue.md)); with the
   falcon status words served from memory it saves four traps per RPC. The Cloud Hypervisor
   fork maps read-only areas today (the display page, the Windows interrupt page).

## Depth

- [caraxes/gsp-transport-classes.md](../caraxes/gsp-transport-classes.md): the method, the
  per-window counts, the top controls per class, the ranked list and the open questions.
- [performance-shape](performance-shape.md): where the transport sits among the other costs.
