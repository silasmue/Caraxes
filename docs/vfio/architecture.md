<!-- SPDX-License-Identifier: MIT -->
# Architecture

Leandro VFIO serves an unmodified NVIDIA guest driver a synthetic Turing GPU over vfio-user.
One Rust process, `vfio-user-nvgpu` (`crates/vfio-user-nvgpu`), is the whole device: it
answers PCI configuration space, the BAR0 registers, the GSP boot exchange and the GSP RPC
stream, keeps the guest's VRAM slice and registered RAM on the host, shadows the guest's
page tables and runs the guest's channels on host channels it owns as an ordinary client
of the host driver. The concepts are explained in the [wiki](../wiki/README.md); this page
is the source map.

## Data path

![The data path: what traps and what is memory](figures/v2-data-path.svg)

- The guest's kernel RM runs in GSP-client mode and believes it talks to firmware through
  BAR0 registers and two message queues in guest RAM.
- Cloud Hypervisor is an ordinary vfio-user client: `REGION_READ/WRITE` for trapped
  accesses, `DMA_MAP` with file descriptors for guest RAM, `SET_IRQS` for the one MSI, sparse
  mmap areas for the pages the device offers as memory, ioeventfds for doorbells.
- The device answers or forwards each RPC, keeps a shadow of every page table the guest
  writes, and mirrors each guest channel on a host channel whose USERD lies in the guest's
  memory.
- The host RM and the GPU stay shared with the host desktop; the GPU walks only the
  device's shadow tables.

## Request trace

For one control the guest's kernel RM sends to what it believes is GSP-RM:

1. The kernel RM writes an MCTP-framed element into the command queue in guest RAM and
   writes `NV_PGSP_QUEUE_HEAD(0)` through BAR0: the one trapped access of the RPC.
2. `msgq::v2` reads the element; `gsprpc.rs` and `rpc.rs` decode the RPC header and the
   `GSP_RM_CONTROL` payload by the guest's ABI version (`ver/`).
3. `ctrl.rs` dispatches by control id: a table-driven answer (`chipctl.rs`, `dmodel.rs`,
   `public.rs`, `families/`), a forward to the host RM (`hostrm.rs`, filtered and cached by
   `fwdcache.rs`), a guest-state answer, or a refusal with GSP-RM's own error code.
4. The reply element lands in the status queue, `NV_PGSP_MSGQ_HEAD(0)` advances, and the
   device raises the MSI; every answer carries its provenance label (decision D9).

A submission never takes this path: GPFIFO entries and GPPut go through the BAR1 pool as
memory, the work-submit token rings an ioeventfd, and the reactor (`poll.rs`) applies the
pending shadow plans and rings the host doorbell.

## Addresses

| Address | Meaning |
|---|---|
| Guest FB address | an offset into the guest's VRAM; equal to the host VRAM address inside the VM's fixed slice (decision D11) |
| Guest physical address (GPA) | guest RAM as the VMM maps it; what the guest writes into its page tables for sysmem |
| Host IOVA | the address the GPU uses for a registered guest-RAM chunk; the device translates GPA to IOVA in the shadow (decision D18) |
| Guest GPU virtual address | an address in a VA space the guest's RM or UVM built in its own GMMU tables |
| Shadow GPU virtual address | the same address in the device's shadow VA space on the host, mapped only for admitted leaves |
| Host VA | the device's mapping of guest RAM, the memfd areas and its own host RM allocations |

A guest table entry becomes a host mapping only after `resolve_leaf` admits it: inside the
slice or one registered descriptor, a permitted kind, outside every device-reserved range.

## The device process

One process per VM. The modules by role (`crates/vfio-user-nvgpu/src`):

| Role | Modules |
|---|---|
| vfio-user server and glue | `device.rs` (regions, interrupts, the `ServerBackend`), `main.rs`, `lib.rs`, `settings.rs` (every variable the device reads, the `config:` block), `trace.rs`, `profile.rs`, `soak.rs`, `selftest.rs`, `fixture.rs` |
| PCI and BAR0 | `config.rs`, `bar0.rs`, `boot.rs` (the boot model behind `--boot-model`), `falcon.rs` (GSP and SEC2 blocks), `vbios.rs`, `intr.rs` (the interrupt tree and the MSI) |
| Chip, board, version | `chip/` (the `Chip` trait; `tu106.rs`, `turing.rs`, `gb206.rs`, `boot_fsp.rs`, `derived.rs`), `board.rs`, `costume.rs`, `ver/` (per-version RPC, control and static-info layouts; the guest ABI detection), `hostabi.rs` (the host driver's layouts) |
| GSP transport and RPCs | `msgq.rs`, `msgq/v2.rs`, `gmcapi.rs` (GMCAPI and NVKV), `gspboot.rs`, `gsprpc.rs`, `rpc.rs`, `events.rs`, `gspmem.rs` (`ALLOC_MEMORY`) |
| Controls | `ctrl.rs` (the dispatcher), `chipctl.rs` and `families/` (chip data and refused families), `dmodel.rs` (the category D rule table), `public.rs` (host-forwardable public controls), `recorded.rs` (pure-emulation answers), `fwdcache.rs`, `kgr.rs`, `nvml.rs`, `cuda.rs`, `uvm.rs`, `rc.rs` (error and teardown paths), `zbc.rs`, `rusd.rs` (the RM user shared data page), `deferred.rs` and `ctrl/deferred_context.rs` (Windows' deferred API objects and contexts), `classes.rs` |
| Memory model | `fb.rs` (the VRAM store and BAR1 windows), `hostmem.rs` (the reservation, guest-RAM registration), `registrar.rs`, `guestmem.rs`, `dmamap.rs`, `hostfloor.rs`, `compression.rs` and `compslice.rs` (D16, D16b) |
| Page tables and the shadow | `ptcache.rs` (the incremental walk of a guest GMMU tree), `shadow.rs` (the shadow VA space, plans, gathers, regions), `lazy.rs`, `mbuild.rs`, `invalidate.rs`, `invgate.rs`, `inv_page.rs`, `pdbwatch.rs`, `gmmu_v3.rs` (Hopper and Blackwell format) |
| Windows the guest writes as memory | `intr_page.rs`, `um_page.rs`, `bar1pool.rs`, `bar2area.rs`, `fastread.rs`, `untrapped.rs`, `display/fe_page.rs`, `display/chan_page.rs` |
| Execution | `mirror.rs` (host TSGs, channels, engine objects; the software executor), `channel.rs` (the channel registry), `userd.rs`, `tokens.rs` (the doorbell token pool and guest tokens), `poll.rs` (the reactor), `ringgate.rs` (held rings), `launch.rs`, `devce.rs` (the device-owned host CE), `preempt.rs` and `gfxpslot.rs` (GR preemption), `dlock.rs` (the device lock's holders), `privilege.rs` (D12), `hostrm.rs` (the host RM client), `hostboost.rs` |
| Display | `display/` (`mod.rs`, `engine.rs`, `core_path.rs`, `aux.rs`, `edid.rs`, `dump.rs`) with the `dp-sink` crate |

Data: `chips/<chip>/` (boot model, registers, engine and class tables, families,
`controls.toml`), `boards/*.toml` (PCI ids, BAR sizes, name, display connector), the
generated per-version tables under `ver/`, and `caraxes/abi/` (the GSP ABI snapshots the
tables are checked against).

## Ownership

- **Per VM:** the device process, the VRAM slice, the guest-RAM registrations, the host RM
  client and its subdevice, the reactor thread, the shadow worker, the BAR1 pool and the
  memfd areas, the display engine.
- **Per guest root (client):** one shadow VA space and its walk cache; the gather and region
  objects of its mappings.
- **Per guest TSG and channel:** one host TSG and one host channel with the same engine
  objects, or a software-executor channel for paging and copy work; the USERD in the guest's
  memory.
- Guest handles are the guest's; the device keys objects by `(hClient, hObject)` because
  guest clients reuse handle numbers.
- Teardown fails closed: mirror objects, descriptors, reservation, client, stopping at the
  first failure; the reservation stays pinned rather than reused.

## Chip-bound versus version-bound

| Chip-bound (TU106, Turing) | Version-bound (guest driver) | Host-bound |
|---|---|---|
| the boot register sequence, falcon bases `0x110000` and `0x840000`, the FSP model of Blackwell | RPC function numbers and struct layouts, the `GSP_INIT` keys, control parameter layouts | the host RM's control layouts (`hostabi.rs`), selected at start from the running host driver |
| the GMMU format (v2 on Turing, v3 from Hopper), the PRAMIN and BAR2 translation | the transport (pre-610 elements for nova and nouveau, 615's MCTP framing) | host BAR1 and VRAM floors |
| engine and class tables, the FIFO device-info table, feature families | registry key names, the per-version static-info layout | |
| the PCI identity, BAR layout and config space | | |

One binary carries every version table and reads the guest's ABI at the GSP boot
([research: one-abi-binary](../research/one-abi-binary-2026-09-28.md)); a new driver version
is a snapshot, a diff against the 615 baseline and regenerated tables
([gsp-abi-615.md](gsp-abi-615.md)); a new chip is a `Chip` implementation with data files
([versatility.md](versatility.md)).

## Differences from Leandro VirtIO

| | Leandro VirtIO (v1) | Leandro VFIO (v2) |
|---|---|---|
| Guest driver | patched (`virtio_nvrm.ko`) | stock, unmodified |
| Guest-side transport | a virtio device plus vhost-user | none: PCI, BARs and guest RAM |
| Where the frontend lives | a guest module in the VM, `vhost-user-nvrm` on the host | one host process, `vfio-user-nvgpu` |
| Identity | rewritten in RM replies after allocation | native at the device level: the profile feeds `GSP_INIT` and BAR0 before any guest software runs (D6); no reply is rewritten |
| RM vocabulary at the boundary | about 290 ioctl shapes | 14 RPC functions carrying the same alloc and control vocabulary inline |
| Reused from v1 | is v1 | the translation tables and the naming model (`nvrm-abi`), the bindings (`nvrm-sys`), the host RM client (`nvrm-client`), the gates and their verdict rules |

## Repository and validation boundaries

- The device crate, its data files and `crates/dp-sink` are v2; `crates/vhost-user-nvrm` and
  the guest modules are v1; the `nvrm-*` crates are shared and must keep v1 building.
- `tools/check.sh` runs the unit tests (about 850 in the device crate), clippy, the licence
  headers and the GSP ABI drift check; `CARAXES_CHECK_EMU_BOOT=1` boots the synthetic GPU
  without a host RM or a GPU.
- Hardware evidence is a run folder under `caraxes/runs/` with the commit, the settings, the
  guest and device logs, and 0 host Xid on the device's pid; the gates and the four-guest
  gate are described in [testing.md](testing.md) and the [verification methods](../wiki/verification-methods.md).
