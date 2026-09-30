<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GSP surface

Scope: everything a stock 615.71.09 kernel RM in GSP-client mode can send to GSP-RM, how much of it the device answers, and how it sorts the rest.

## Facts

### The numbers of 615.71.09

| Item | Count | Evidence |
|---|---:|---|
| Exported controls (unique ids, 16 of them kernel-sent without an export entry) | 1397 | [caraxes/abi/gsp-615.71.09/MANIFEST; docs/vfio/gsp-abi-615.md §1] |
| Controls that reach GSP (routed) | 844 | [controls.tsv, column `route` = `gsp`, counted] |
| of which INTERNAL / sent explicitly by the kernel RM | 246 / 260 | [docs/vfio/gsp-surface.md §615.71.09] |
| Routed controls per heuristic category A / B / C / D / E (answered or not) | 122 / 86 / 39 / 390 / 207 | [controls.tsv, column `category`, counted] |
| Routed controls the device answers | 777 | [controls.tsv, column `provenance`, counted; docs/vfio/gsp-surface.md "After the reconciliation"] |
| host-forwarded / synthesized / guest-derived / echo / refused | 69 / 80 / 75 / 65 / 488 | [same] |
| Routed controls unhandled | 67 | [same] |
| unhandled A / B / C / D / E | 50 / 0 / 1 / 16 / 0 | [same] |
| RPC functions / sent by a 615 GSP client / handled by the device | 232 / 14 / 19 (all 14) | [caraxes/abi/gsp-615.71.09/rpc.tsv; docs/vfio/gsp-abi-615.md §4] |
| GMCAPI commands a TU106 client uses (both answered) | 2 (`GSP_INIT`, `GSP_SUSPEND`) | [docs/vfio/gsp-plan/rpc-functions.md; docs/vfio/gsp-plan/README.md] |
| Events / dispatched by the CPU side / sent by the device | 45 / 36 / 4 | [caraxes/abi/gsp-615.71.09/events.tsv] |
| Classes (`RS_ENTRY`) / reaching GSP by flags | 224 / 149 | [caraxes/abi/gsp-615.71.09/classes.tsv] |
| GSP-bound classes in TU106's class database / named / acknowledged / refused | 53 / 21 / 29 / 99 | [same; docs/vfio/gsp-abi-615.md §4] |
| Classes a 615 client can put on the wire / on TU106 / allocated by a stock TU106 stack | 143 / 48 / 36 | [docs/vfio/gsp-plan/classes.md §2] |
| Boot and transport types / tagged `NV_ABI_STABLE` | 41 / 17 (12 structs, 5 enums) | [caraxes/abi/gsp-615.71.09/boot.tsv; docs/vfio/gsp-abi-615.md §2] |

- The 14 RPC functions of a 615 client are `ALLOC_MEMORY`, `FREE`, `MAP_MEMORY_DMA`, `UNMAP_MEMORY_DMA`, `DUP_OBJECT`, `IDLE_CHANNELS`, `GPU_EXEC_REG_OPS`, `UPDATE_BAR_PDE`, `CONTINUATION_RECORD`, `GSP_RM_CONTROL`, `DUMP_PROTOBUF_COMPONENT`, `GSP_RM_ALLOC`, `ECC_NOTIFIER_WRITE_ACK` and `INIT_GSP_TRACE_CRASH_BUFFER` [caraxes/abi/gsp-615.71.09/rpc.tsv, column `gsp_client`].
- The four events the device sends are `GSP_INIT_DONE`, `POST_EVENT`, `RC_TRIGGERED` and `OS_ERROR_LOG` [caraxes/abi/gsp-615.71.09/events.tsv, column `device`].
- The 488 refused routed controls include the 302 controls of the 28 feature families [docs/vfio/gsp-plan/e-families.md; docs/vfio/gsp-abi-615.md §4].
- The one unhandled C control, `NV_EVENT_BUFFER_CTRL_CMD_UPDATE_GET`, is answered by `gspmem.rs` ahead of the dispatcher, where the inventory does not look [docs/vfio/gsp-surface.md "After the GSP plan round 2, category C"].
- The 16 unhandled D controls are the 12 display MODEL rows and the 4 open A rows the heuristic still calls D [docs/vfio/gsp-surface.md "After round 2 of D", "After the GSP plan round 2, category C"].
- The 50 unhandled A controls wait for a capture of the card or the kernel's reading of an INTERNAL answer [docs/vfio/gsp-surface.md "After E and A"].
- No CUDA or Wayland workload has reached a category D control so far; the only D control seen in a Leandro VFIO run is `NV0073_CTRL_CMD_SPECIFIC_HDCP_CTRL` on a DP hot-unplug [docs/vfio/gsp-plan/d-device-model.md summary; run: caraxes/runs/display-cd-2026-09-24/d11-unplug].
- 610 delta: 1377 exported, 829 routed, 763 answered, 66 unhandled (A 50, D 16), 230 RPC functions (a 610 client sends 20), 41 events, 222 classes and no `NV_ABI_STABLE` tag [docs/vfio/gsp-surface.md "After the reconciliation"; docs/vfio/gsp-abi-615.md §4].

### The categories

| Cat. | Meaning | How the device answers | Evidence |
|---|---|---|---|
| A | pure chip data: static getters, caps, tables, anything `RMCTRL_FLAGS_CACHEABLE` | per-chip table (`chips/<chip>/controls.toml`) from source constants, chip files or one capture | [docs/vfio/gsp-surface.md §3.A, "After E and A"] |
| B | host-forwardable: public getters of live state with a flat struct equal on host and guest | forward to the device's own host subdevice or device, filtered by the D9 deny list | [docs/vfio/gsp-surface.md §3.B; docs/vfio/gsp-plan/b-host-forwardable.md] |
| C | guest-state derived: controls on objects the guest allocated (channels, TSGs, context shares) | resolved through the channel registry, forwarded only to the guest's own host mirror | [docs/vfio/gsp-surface.md §3.C; docs/vfio/gsp-plan/c-guest-state.md] |
| D | needs device modelling: engines, links, power, engine-only state | rule table (REFUSE, DENY, ACK, MODEL, MIRROR, TABLE) generated per version | [docs/vfio/gsp-surface.md §3.D; docs/vfio/gsp-plan/d-device-model.md §2] |
| E | never expected from a stock client on TU106: features the chip lacks | refused per feature family the chip file names absent | [docs/vfio/gsp-surface.md §3.E] |

- The categories are name-and-flag heuristics of `gsp-surface.py`, recorded as they are, so a heuristic change shows as a device-side change [docs/vfio/gsp-abi-615.md §9].
- The D plan sorted the 308 D controls of 615 into 200 REFUSE, 37 DENY, 21 ACK, 42 MODEL, 3 MIRROR and 5 TABLE, and moved 102 to E, 17 to C and 7 to A [docs/vfio/gsp-plan/d-device-model.md summary].
- E is organised as 28 feature families holding 302 controls, 12 from category E and 16 from the D rows; see [feature-families.md](feature-families.md) [docs/vfio/gsp-plan/e-families.md].

### What "refused" means

- A family refusal answers `NV_ERR_NOT_SUPPORTED` (0x56) with the request echoed and the label `refused (feature absent on this chip)` [docs/vfio/gsp-surface.md "After E and A"].
- D REFUSE rows answer `NV_ERR_NOT_SUPPORTED` or the status the header names (3 rows `NV_ERR_INVALID_ARGUMENT`), label `refused (feature absent on this board)` [docs/vfio/gsp-plan/d-device-model.md §2; docs/vfio/gsp-surface.md "After D"].
- D DENY rows refuse for isolation, 9 of the 37 with `NV_ERR_INSUFFICIENT_PERMISSIONS`, the bus rows with `NV_ERR_NOT_SUPPORTED` [docs/vfio/gsp-surface.md "After D"; docs/vfio/gsp-plan/d-device-model.md §6.4].
- A class outside the chip's class database is refused with `NV_ERR_NOT_SUPPORTED`, under a channel with `NV_ERR_INVALID_CLASS`, as GSP-RM's resource server does [docs/vfio/gsp-plan/classes.md Status, §5].
- The profiler classes 0x90CC, 0xB0CC, 0xB1CC and 0xB2CC are refused at allocation with `NV_ERR_INSUFFICIENT_PERMISSIONS` [docs/vfio/gsp-surface.md "After D"].
- An unhandled control also gets `NV_ERR_NOT_SUPPORTED`, from the dispatcher's "unanswered" arm, so a refusal differs from a gap by being deliberate and labelled [docs/vfio/gsp-plan/e-families.md §4].
- `NV_ERR_NOT_SUPPORTED` is what a vGPU guest's RM gives, and the kernel skips its error message for a list of such controls [docs/vfio/gsp-plan/d-device-model.md §6.4; 615 tree: src/nvidia/src/kernel/rmapi/rmapi_utils.c:198].
- The guest kernel logs an assertion and continues when `INTERNAL_CONF_COMPUTE_GET_STATIC_INFO` is refused [615 tree: src/nvidia/src/kernel/gpu/conf_compute/conf_compute.c:486, :591; docs/vfio/costume-ladder.md §3.2].
- The watchdog init tolerates `NV_ERR_NOT_SUPPORTED` but any other error fails `RmInitAdapter` [615 tree: src/nvidia/arch/nvalloc/unix/src/osinit.c:2264-2276].
- User space can stop on a refusal: the 615 libcuda failed `cuInit` with 100 when `CE_GET_LCE_SHIM_INFO` was refused [run: caraxes/runs/cutover-615-2026-09-24/v2-615guest/gate-cuda.txt].

## Consequence for Leandro VFIO

- A new control of a later driver arrives unhandled, gets a category from the generators, and on TU106 mostly lands in an absent family (see [abi-baseline.md](abi-baseline.md)).
- Refusals are safe for kernel paths that tolerate `NV_ERR_NOT_SUPPORTED` and a wall for user space that does not, so a refused control seen in a user-space trace is re-examined, not assumed harmless.
- Nothing in D is forwarded to the host as it is: the host sees only B forwards and the guest's own mirror objects (see [isolation-and-xids.md](isolation-and-xids.md)).
- RPC functions, events and classes are detailed in [gsp-rpc-events-classes.md](gsp-rpc-events-classes.md).

## Open

- The 50 A rows need a one-time capture of the card or a source reading of the kernel's use [docs/vfio/gsp-surface.md "After E and A"].
- The 12 display MODEL rows wait for the display branch [docs/vfio/gsp-surface.md "After round 2 of D"].
- `INVALIDATE_TLB` stays unanswered as a vGPU-only path [docs/vfio/gsp-surface.md Summary].
- The timed-semaphore rows stay refused until Q4 of the D plan is decided [docs/vfio/gsp-surface.md "After round 2 of D"].
- The static configuration keys of the real card's `GSP_INIT` response are unmeasured [docs/vfio/gsp-plan/boot-exchange.md §7 Q1].
