<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# ZBC with real compression (2026-09-28)

Scope: the host's ZBC clear table under real compression per VM slice ([compression-isolation-2026-09-28.md](compression-isolation-2026-09-28.md)): who may add, how entries are placed and released, what the device forwards, what a guest sees and can learn. `K` = `src/nvidia/src/kernel/`, `G` = `src/nvidia/generated/`, `SDK` = `src/common/sdk/nvidia/inc/` of the 615 tree; runs under `caraxes/runs/comp-zbc-2026-09-28/`.

## Facts

- One table per GPU, 15 slots each for colour, depth and stencil, shared by the host desktop and every VM; GSP-RM seeds colour 1-3, depth 1-2, stencil 1-3 [zbc-host-probe.txt; chips/tu106/zbc.toml].
- On Turing the ZBC kinds are exactly the compressible kinds [615 tree: K/gpu/mem_mgr/arch/turing/mem_mgr_tu102_base.c:52-53]: a fast clear on a compressed slice stores a table index in the compbits, which the hardware resolves through the host's table.
- Any client may add: `GF100_ZBC_CLEAR` is NON_PRIVILEGED under a subdevice [615 tree: K/rmapi/resource_list.h:874-884]; the three adding setters and GET_ZBC_CLEAR_TABLE_ENTRY are NON_PRIVILEGED and ROUTE_TO_PHYSICAL, flags 0x10248 [615 tree: G/g_zbc_api_nvoc.c:181, 196, 241, 271], GET_ZBC_CLEAR_TABLE_SIZE 0x50048 [:256]. GSP-RM keeps the table; no setter returns an index [615 tree: SDK/ctrl/ctrl9096.h:118-123, 167-171, 343-347].
- On this host an equal entry is referenced again (NV_OK, table unchanged, index 6), and a new colour on the full table gets `NV_ERR_INSUFFICIENT_ZBC_ENTRY` 0x80, not the header's INSUFFICIENT_RESOURCES; values read back bit for bit [zbc-host-probe.txt, zbc-host-probe-full.txt; 615 tree: SDK/nvstatuscodes.h:157].
- Nothing is released per entry or per client: the kernel's ZbcApi constructor and destructor are empty [615 tree: G/g_zbc_api_nvoc.h:416-422], and an entry outlives the client that set it [zbc-host-probe.txt, second client].
- The only count is of ZBC-kind surfaces (`zbcSurfaces`, per GR) at allocation and free [615 tree: K/gpu/mem_mgr/arch/maxwell/mem_mgr_gm107.c:539-563, 611-634]; on 0↔1 the kernel sends `INTERNAL_MEMSYS_SET_ZBC_REFERENCED` [mem_mgr_gm107.c:265-320], and GSP-RM may flush the table only when the PF and every VF report none [615 tree: SDK/ctrl/ctrl0080/ctrl0080internal.h:117-126].
- `SET_ZBC_REFERENCED` needs kernel privilege or a VF [615 tree: K/gpu/mem_sys/kern_mem_sys_ctrl.c:1316]; a compressed `NV01_MEMORY_LOCAL_PHYSICAL` skips the count (`ZBC_SKIP_ZBCREFCOUNT_YES`) [615 tree: K/mem_mgr/phys_mem.c:119-120].
- vGPU forwards the adding setters, the getters and `SET_ZBC_REFERENCED` to the host [615 tree: K/vgpu/rpc.c:4627-4637, 4697-4698, 4812-4816]; the indexed `SET_ZBC_CLEAR_TABLE` (flags 0x48, no ROUTE_TO_VGPU_HOST) is NOT_SUPPORTED on a vGPU guest [G/g_zbc_api_nvoc.c:226; K/rmapi/control.c:779-789]; an SR-IOV VF takes its index range from static info [K/gpu/mem_sys/zbc_api.c:27-43].
- The host desktop filled all 12 free colour slots within 5 h of the boot (booted 15:15, probe 20:11; the order differs from the 2026-09-24 sweep); depth and stencil hold only the defaults [zbc-host-probe.txt; caraxes/runs/gsp-round2-d-2026-09-24/q5-host-sweep/sweep.txt].
- The stock UMD reads the table (TABLE_SIZE, GET_ENTRY; vk-offscreen: 33 and 495 calls) and sets nothing in the 10 native traces; the display gate sends `SET_ZBC_COLOR_CLEAR` three times [matrix/traces/610.57.04/control/*.jsonl; caraxes/runs/gsp-d-2026-09-24/after/gate-dscan.txt].

## Consequence for Caraxes

`CARAXES_COMPRESSION=slice` (zbc.rs `SliceZbc`); without it the local table answers as before, byte for byte.

- Setters: colour, depth and stencil go to the host's table on the device's own client, through a `GF100_ZBC_CLEAR` object that takes only those three and the two getters (hostrm.rs `ZBC_CONTROLS`). `bSkipL2Table` is cleared; format INVALID is refused. The device reads the table before (15 GET_ENTRY) and, for a new entry, after the set to learn the index.
- Cap: `CARAXES_ZBC_MAX` (default 4, 0-15) new slots per table and VM; at the cap the answer is 0x80, not forwarded. An entry the host already holds costs nothing.
- Full host table: the host's 0x80 reaches the guest unchanged; its clears of that value stay non-ZBC, as on a full card.
- Indexed setter: NOT_SUPPORTED and never forwarded, as on a vGPU guest.
- Guest view: GET_ZBC_CLEAR_TABLE_ENTRY and GET_ZBC_CLEAR_TABLE answer the chip defaults and this VM's entries at their host indices, without a host call; the desktop's and other VMs' entries read as empty; TABLE_SIZE stays the chip row (1-15).
- `SET_ZBC_REFERENCED`: recorded per VM, NV_OK, not forwarded.
- VM exit: the device log lists the host slots the VM used and filled; no entry can be released.

## What a guest can learn

- No other tenant's value is read out.
- Left: whether the table has a free slot (0x80, one bit per setter); where its own new entries land, and so which lower slots are taken; whether a guessed value is present (placement without a new slot). A guess of a present value costs no cap slot.
- Between two VMs this is a low-rate covert channel (one setter per bit, 15 slots).

## Open

- Hook for workstream A: the slice must count as a ZBC-kind surface on the host (`NV01_MEMORY_LOCAL_USER` with a compressible kind and `ATTR2_ZBC_SKIP_ZBCREFCOUNT_NO`), or the device holds a small compressible anchor while the guest reports ZBC surfaces. `LOCAL_PHYSICAL` is not counted, and a host without its own ZBC surfaces could then flush the VM's entries.
- The desktop owns the colour table: guest colour ZBC hits only values already present (black, white, the defaults, the desktop's greys).
- A VM's filled slots stay until the host flushes (usually the next boot); VM restarts with new values accumulate. A host-wide ledger could make the cap span VM lifetimes.
- The local table (switch off) answers a full table with 0x1A; the card answers 0x80.
- First-free placement is inferred from fill order, not from source; "for the client" in the 0x80 text may mean a per-client limit in GSP-RM, which a full table cannot show.
