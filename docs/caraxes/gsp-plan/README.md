<!-- SPDX-License-Identifier: MIT -->
# GSP plan: preparing a full implementation of the GSP surface on 615.71.09

The research programme after D19 and P23 ([../README.md](../README.md)): everything a GSP-RM
stand-in must answer for a stock 615.71.09 kernel RM on TU106, from source, compared with what
`crates/vfio-user-nvgpu` does today, with a recipe, a truth source and an isolation rating per
item. [../gsp-surface.md](../gsp-surface.md) is the inventory this plan starts from. One row
per document.

| File | Contents |
|---|---|
| [rpc-functions.md](rpc-functions.md) | All 232 RPC functions of 615 (sizes compiled from both trees; only additions, no layout change), who sends each and when; a 615 GSP client on TU106 uses 14 (7 at every boot, `DUP_OBJECT` in regular use, 6 on some paths) plus the GMCAPI commands `GSP_INIT` and `GSP_SUSPEND`; the device answers 7 and both commands; the gaps (`ALLOC_MEMORY` first, sent at every boot by the new operational-event import) with recipes and open questions |
| [events.md](events.md) | The 45 events of 615 (36 dispatched) and the GMCAPI GSP-to-CPU commands: payloads (`RC_TRIGGERED` grew from 48 to 64 bytes under the same version name), kernel handlers, which a TU106 Linux client needs, what the device sends (POST_EVENT for hotplug only), contained recipes for host-mirrored errors, events never to send, measurements M1-M10 |
| [classes.md](classes.md) | The classes whose allocation reaches GSP: 143 a GSP client can send (of 149 flagged) plus the kernel's own 0x90CD/0x90D0 of the event import, 48 on TU106, 36 allocated by a stock stack; alloc parameter layouts 610/615 (only the TSG params changed), how real GSP-RM refuses a foreign class, the device's echo, recipes, FREE and DUP_OBJECT gaps |
| [boot-exchange.md](boot-exchange.md) | The 615 boot exchange in order (queue construction to `GSP_INIT` and the first RPCs), teardown and reload, suspend and resume (`GSP_RESUME_DONE`), GC6/RTD3 (not used on a desktop TU106), what `NV_ABI_STABLE` covers and what the tree says about stability (the RM RPC layer is an opt-in "unstable interface"), the per-step device status and the gap list B1-B7 |
