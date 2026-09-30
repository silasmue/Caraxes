<!-- SPDX-License-Identifier: MIT -->
# Cloud Hypervisor patches

Apply patches 0001-0003, in order (0004 and 0005 are optional and independent: each applies before or after the others; 0006 needs 0005), to the tag in [CH_VERSION](../CH_VERSION),
currently `v53.0`. `tools/build.sh ch` fetches, patches and builds it. The
[manual setup](../docs/virtio/vm-preparation.md#build-and-patch-cloud-hypervisor)
shows the equivalent commands. The patches are `git format-patch` output, so
`git apply` and `git am` both accept them.

The patches extend Cloud Hypervisor's generic vhost-user device
(`--generic-vhost-user`) and the vhost-user backend channel; 0005 extends the
vfio-user client (`--user-device`). None of them refers to NVIDIA or to Leandro;
virtio-nvrm is one user, the Leandro VFIO device another.

| Patch | Change | Files |
|---|---|---|
| [0001](0001-generic-vhost-user-shmem.patch) | VIRTIO Shared Memory Regions: `SHMEM` negotiation, `GET_SHMEM_CONFIG`, `SHMEM_MAP`/`SHMEM_UNMAP`; vhost-user frontend on `vhost` 0.17 | `Cargo.toml`, `Cargo.lock`, `virtio-devices/Cargo.toml`, `virtio-devices/src/vhost_user/*.rs`, `virtio-devices/src/lib.rs`, `virtio-devices/src/transport/pci_device.rs`, `vmm/src/device_manager.rs` |
| [0002](0002-generic-vhost-user-device-features.patch) | Offer the device-specific feature bits | `virtio-devices/src/vhost_user/generic_vhost_user.rs` |
| [0003](0003-generic-vhost-user-refused-request.patch) | Keep serving the backend channel after a refused request | `virtio-devices/src/vhost_user/mod.rs` |
| [0005](0005-vfio-user-region-io-fds.patch) | vfio-user region I/O fds: request `VFIO_USER_DEVICE_GET_REGION_IO_FDS` for each BAR and register the server's ioeventfd sub-regions with KVM (exact length and datamatch), moved with the BAR, removed with the device; a server that refuses keeps every BAR trapped. Builds against [../vendor/vfio-user-rs](../vendor/vfio-user-rs/README.caraxes.md) beside the tree ([Patch 0005](#patch-0005-vfio-user-region-io-fds)); tested in [../caraxes/runs/patch0005-2026-09-24](../caraxes/runs/patch0005-2026-09-24) | `Cargo.toml`, `Cargo.lock`, `hypervisor/src/vm.rs`, `hypervisor/src/kvm/mod.rs`, `pci/src/vfio.rs`, `pci/src/vfio_user.rs` |
| [0006](0006-vfio-user-region-io-fds-summary.patch) | One INFO line per region for its ioeventfds (registered of offered, descriptors received, datamatch range) and a test of a reply with 253 datamatch sub-regions at one offset, one descriptor each; with the vendored crate's client now announcing `max_msg_fds` 253 ([../vendor/vfio-user-rs](../vendor/vfio-user-rs/README.caraxes.md), crate patch 0002) a server can tell the values written to one register apart ([Patch 0006](#patch-0006-datamatch-sub-regions-at-one-offset), [../docs/vfio/doorbell-token-pool.md](../docs/vfio/doorbell-token-pool.md)); tested in [../caraxes/runs/doorbell-tokens-2026-09-24](../caraxes/runs/doorbell-tokens-2026-09-24). To be folded into 0005 before upstreaming | `pci/src/vfio_user.rs` |
| [0004](0004-acpi-dma-host-bridge.patch) | Emit an ACPI `_DMA` object on a PCI segment's host bridge from `--pci-segment pci_segment=<id>,dma_ranges=[<cpu>:<dma>:<size>,...]`; nothing changes without the option ([ACPI-DMA.md](ACPI-DMA.md)); Leandro VFIO only, tested in [../caraxes/runs/acpi-dma-2026-09-23](../caraxes/runs/acpi-dma-2026-09-23) | `vmm/src/api/openapi/cloud-hypervisor.yaml`, `vmm/src/config.rs`, `vmm/src/device_manager.rs`, `vmm/src/pci_segment.rs`, `vmm/src/vm_config.rs` |
| [0007](0007-vfio-user-readonly-mmap-areas.patch) | Read-only vfio-user mmap areas: `--user-device ...,x_readonly_mmap_areas=[<region>:<offset>:<size>,...]` maps those parts of a region's sparse areas as read-only KVM slots, so reads stay direct and writes trap to the region protocol; areas are validated against the sparse areas | `pci/src/vfio.rs`, `pci/src/vfio_user.rs`, `vmm/src/config.rs`, `vmm/src/device_manager.rs`, `vmm/src/vm_config.rs` |
| [0008](0008-vfio-user-readonly-mmap-bar-move.patch) | Keep read-only mmap areas read-only across BAR moves: `move_bar` re-created every slot writable, and EDK2 moves every BAR at boot, so a Windows guest's writes to a read-only area landed in the mapping (C62, 2026-09-29). To be folded into 0007 before upstreaming | `pci/src/vfio_user.rs` |

[REVIEW-vhost-user.md](REVIEW-vhost-user.md) lists what the previous version of
this series got wrong and why each point changed.

## Specifications

| Specification | Version | Sections implemented |
|---|---|---|
| vhost-user | QEMU `docs/interop/vhost-user.rst` at `1276e6c85dcc` (2026-09-11), sha256 `684b11b15330ee23f2922aab9abd116efa1f48eb15b1b02b0233418e1a224257` | "Message Specification" (header), "MMAP request", "VIRTIO Shared Memory Region configuration", "Back-end communication", "Protocol features", `VHOST_USER_GET_SHMEM_CONFIG`, `VHOST_USER_BACKEND_SHMEM_MAP`, `VHOST_USER_BACKEND_SHMEM_UNMAP`, "VHOST_USER_PROTOCOL_F_REPLY_ACK" |
| VIRTIO | 1.4 cs01 (oasis-tcs/virtio-spec `v1.4-cs01`, `917e900e0246`) | 2.2 "Feature Bits", "Shared Memory Regions" and "Addressing within regions", PCI "Shared memory capability" |
| vfio-user (0005) | QEMU `docs/interop/vfio-user.rst` at tag `v11.1.1`, vendored as `vendor/qemu-vfio-user/vfio-user.rst`, sha256 `cd05c1570caddb18bfa6a39406d92f7d0eb124cd99ca2ab4f7ff9ace0ac38ff0` | "Region I/O via file descriptors", "Message sizes", "Commands", "Header", `VFIO_USER_VERSION` (capability `max_msg_fds`), `VFIO_USER_DEVICE_GET_REGION_IO_FDS` ("Request", "Reply", "Sub-Region IO FD info format (ioeventfd)" and "(ioregionfd)") |
| Linux KVM API (0005) | `Documentation/virt/kvm/api.rst`, `KVM_IOEVENTFD` (4.59 in the text the specification links); values from the host's `<linux/kvm.h>` | `kvm_ioeventfd` (`datamatch`, `addr`, `len`, `fd`, `flags`), `KVM_IOEVENTFD_FLAG_DATAMATCH` (bit 0), `_PIO` (bit 1), `_DEASSIGN` (bit 2), `_VIRTIO_CCW_NOTIFY` (bit 3) |

The vhost-user specification is not vendored in this repository. It was
fetched from `https://gitlab.com/qemu-project/qemu/-/raw/1276e6c85dcc/docs/interop/vhost-user.rst`.
`VHOST_USER_PROTOCOL_F_SHMEM` has been bit 22 since `588acb45c29f` added it.

The vfio-user specification was fetched as a single file with the QEMU v11.1.1
client sources into the git-ignored `vendor/qemu-vfio-user`
([docs/vfio/environment.md](../docs/history/caraxes-environment-2026-09-23.md)); the tag's commit
id was not recorded with it, the tag and the sha256 above identify the text. It is
the current home of the specification: `vendor/libvfio-user` (nutanix/libvfio-user
at `803924493a7c`, 2026-09-08) no longer carries `docs/vfio-user.rst`. libvfio-user
(server, `lib/libvfio-user.c`) and the rust-vmm crate are reference implementations,
not specifications; QEMU v11.1.1's client (`vendor/qemu-vfio-user`) defines the
command number (`protocol.h`) but never sends the request.

## Crate versions

The `vhost` crate 0.16.0 that Cloud Hypervisor v53.0 pins defines
`VhostUserProtocolFeatures::SHMEM` as bit 21. The specification assigns bit 21
to `VHOST_USER_PROTOCOL_F_GPA_ADDRESSES` and bit 22 to SHMEM. `vhost` 0.17.0
corrects this (rust-vmm/vhost#367). Therefore:

- Patch 0001 adds a second `vhost` dependency, `vhost-frontend = { package =
  "vhost", version = "0.17.0" }`, and moves the vhost-user frontend
  (`virtio-devices/src/vhost_user/`: the block, fs, net and generic devices)
  to it. Only the generic device's offered features change. vDPA and vhost-kern code stay on
  `vhost` 0.16: they pass `vm-memory` 0.17 types, and `vhost` 0.17 uses
  `vm-memory` 0.18. The standalone `vhost_user_block` and `vhost_user_net`
  backends keep `vhost` 0.16 and `vhost-user-backend` 0.22. `Cargo.lock` gains
  `vhost` 0.17.0 and `vm-memory` 0.18.0; the other lock changes are the
  resulting version qualifiers.
- Leandro's `vhost-user-nvrm` uses `vhost` 0.17, `vhost-user-backend` 0.23,
  `virtio-queue` 0.18 and `vm-memory` 0.18.

Both ends must be updated together: a 0.16 peer and a 0.17 peer never
negotiate SHMEM (see [Compatibility](#compatibility)).

Patch 0005 needs `vfio_user` with `Client::get_region_io_fds()`, which no release
has. The patch adds `[patch.crates-io] vfio_user = { path = "../vfio-user-rs" }`:
[vendor/vfio-user-rs](../vendor/vfio-user-rs/README.caraxes.md) is 0.1.6 plus the
change ([vfio-user-rs/](vfio-user-rs/), an upstream candidate for rust-vmm/vfio),
with its `vfio-bindings` requirement widened to admit the 0.6.2 that v53.0's
`vfio-ioctls` 0.7.0 pins. `Cargo.lock` replaces `vfio_user` 0.1.3 by that path
package, adds `enumn` 0.1.14, and keeps `vfio_user` on `vm-memory` 0.17.1 (the
version v53.0's own `vfio_user` used; 0001 adds 0.18.0 for the vhost frontend).
`tools/build.sh ch` builds `vendor/cloud-hypervisor`, beside which the path
resolves; another checkout needs a `../vfio-user-rs` link to it. The Leandro VFIO
workspace uses the same copy for the device.

## Message formats

All numbers are in the host's native byte order (little-endian on x86-64).

### Header (12 bytes, every message)

| Offset | Size | Field | Value |
|---:|---:|---|---|
| 0 | 4 | request | message id |
| 4 | 4 | flags | bits 0-1 version (`0x1`), bit 2 `REPLY` (`0x4`), bit 3 `NEED_REPLY` (`0x8`) |
| 8 | 4 | size | payload bytes |

### Messages this series uses

| Channel | Message | Id | Request payload | Reply |
|---|---|---:|---|---|
| main (frontend to backend) | `VHOST_USER_GET_PROTOCOL_FEATURES` | 15 | none | u64 feature mask |
| main | `VHOST_USER_SET_PROTOCOL_FEATURES` | 16 | u64 feature mask | u64 status if `NEED_REPLY` |
| main | `VHOST_USER_SET_BACKEND_REQ_FD` | 21 | none, one fd in `SCM_RIGHTS` | u64 status if `NEED_REPLY` |
| main | `VHOST_USER_GET_SHMEM_CONFIG` | 44 | none | `VhostUserShMemConfig` (2056 bytes) |
| backend (backend to frontend) | `VHOST_USER_BACKEND_SHMEM_MAP` | 9 | `VhostUserMMap` (40 bytes), one fd in `SCM_RIGHTS` | u64: 0 success, non-zero failure; only with `REPLY_ACK` negotiated and `NEED_REPLY` set |
| backend | `VHOST_USER_BACKEND_SHMEM_UNMAP` | 10 | `VhostUserMMap` (40 bytes) | as SHMEM_MAP |

### `VhostUserShMemConfig` (GET_SHMEM_CONFIG reply, 2056 bytes)

| Offset | Size | Field | Meaning |
|---:|---:|---|---|
| 0 | 4 | nregions | number of non-zero entries in `memory_sizes`; at most 256 |
| 4 | 4 | padding | |
| 8 + 8 * shmid | 8 | memory_sizes[shmid] | size of region `shmid` in bytes; 0 = shmid unused; otherwise a multiple of the host page size |

The array index is the shmid. `nregions` is a count, not a prefix length: one
region with shmid 1 is `nregions = 1`, `memory_sizes = [0, S, 0, ...]`.
The configuration is valid for the lifetime of the connection.

### `VhostUserMMap` (SHMEM_MAP and SHMEM_UNMAP payload, 40 bytes)

| Offset | Size | Field | Meaning |
|---:|---:|---|---|
| 0 | 1 | shmid | region to map into |
| 1 | 7 | padding | |
| 8 | 8 | fd_offset | offset into the passed fd (SHMEM_MAP only); page aligned |
| 16 | 8 | shm_offset | offset from the start of region `shmid`; page aligned |
| 24 | 8 | len | bytes; non-zero, page aligned |
| 32 | 8 | flags | bit 0: 1 = read-write, 0 = read-only; other bits must be 0 |

### vfio-user header (16 bytes, every message; patch 0005)

vfio-user.rst, "Header". Offsets are from the start of the message; the message
formats below give payload offsets, from the end of this header.

| Offset | Size | Field | Value |
|---:|---:|---|---|
| 0 | 2 | Message ID | chosen by the sender, echoed in the reply |
| 2 | 2 | Command | `VFIO_USER_DEVICE_GET_REGION_IO_FDS` = 6 ("Commands") |
| 4 | 4 | Message size | whole message including the header |
| 8 | 4 | Flags | bits 0-3 Type (0 command, 1 reply), bit 4 No_reply, bit 5 Error |
| 12 | 4 | Error | errno in an error reply, may be 0; reserved in a command |

"If an error occurs, the reply message must only include the reply header."

### `VFIO_USER_DEVICE_GET_REGION_IO_FDS` request (payload 16 bytes)

| Offset | Size | Field | Value sent by the client |
|---:|---:|---|---|
| 0 | 4 | argsz | maximum reply payload size: 656 (16 + 16 * 40) first; the argsz a truncated reply names on the one retry; always below 65536 |
| 4 | 4 | flags | 0 ("The client must set flags to zero") |
| 8 | 4 | index | the region (BAR index) |
| 12 | 4 | count | 0 ("all other fields must be zero") |

### `VFIO_USER_DEVICE_GET_REGION_IO_FDS` reply

| Offset | Size | Field | Meaning |
|---:|---:|---|---|
| 0 | 4 | argsz | size of the full payload: 16 + count * 40. Above the request's argsz: only these 16 bytes are sent, and the client asks again with this argsz ("Message sizes") |
| 4 | 4 | flags | 0 ("flags must be zero") |
| 8 | 4 | index | the region queried |
| 12 | 4 | count | number of entries |
| 16 + 40 * i | 40 | sub-region i | an entry below |

Ancillary data: the descriptors the entries' `fd_index` values name, as
`SCM_RIGHTS`, at most the client's `max_msg_fds` (`VFIO_USER_VERSION`; 1 when the
client does not say, which is Cloud Hypervisor's case). Entries may share a
descriptor.

### Sub-Region IO FD info, ioeventfd (type 0) and ioregionfd (type 1), 40 bytes

| Offset | Size | ioeventfd | ioregionfd |
|---:|---:|---|---|
| 0 | 8 | offset: start within the region | offset |
| 8 | 8 | size: KVM_IOEVENTFD `len`; 0 = any access size | size |
| 16 | 4 | fd_index: index into the ancillary descriptors | fd_index |
| 20 | 4 | type: `VFIO_USER_IO_FD_TYPE_IOEVENTFD` = 0 | `VFIO_USER_IO_FD_TYPE_IOREGIONFD` = 1 |
| 24 | 4 | flags: `KVM_IOEVENTFD_FLAG_DATAMATCH` 0x1, `_PIO` 0x2, `_VIRTIO_CCW_NOTIFY` 0x8 | `KVM_IOREGION_PIO`, `KVM_IOREGION_POSTED_WRITES` |
| 28 | 4 | padding | padding |
| 32 | 8 | datamatch | user_data |

## Patch 0001: VIRTIO Shared Memory Regions

### Protocol features

| Bit | Mask | Name | Offered by the generic device | Why |
|---:|---|---|---|---|
| 3 | `0x8` | `REPLY_ACK` | yes (already in v53.0) | replies to backend requests |
| 5 | `0x20` | `BACKEND_REQ` | yes (already in v53.0) | the backend channel |
| 10 | `0x400` | `BACKEND_SEND_FD` | **added** | "Back-end communication": fds on the backend channel require it; SHMEM_MAP carries one |
| 21 | `0x20_0000` | `GPA_ADDRESSES` | no | not implemented. The previous series offered this bit under the name SHMEM |
| 22 | `0x40_0000` | `SHMEM` | **added** | GET_SHMEM_CONFIG, SHMEM_MAP, SHMEM_UNMAP |

The existing offers (`CONFIG`, `MQ`, `CONFIGURE_MEM_SLOTS`, `INFLIGHT_SHMFD`,
`LOG_SHMFD`, `DEVICE_STATE`) are unchanged.

### Handshake

The order is that of `GenericVhostUser::new` and activation in Cloud
Hypervisor. Numbers are message ids.

1. `SET_OWNER` (3).
2. `GET_FEATURES` (1). The backend must advertise
   `VHOST_USER_F_PROTOCOL_FEATURES` (bit 30) for the next steps.
3. `GET_PROTOCOL_FEATURES` (15). acked = offered & advertised.
4. `SET_PROTOCOL_FEATURES` (16) with the acked set. From here the frontend
   sets `NEED_REPLY` on its requests if `REPLY_ACK` was acked.
5. `GET_QUEUE_NUM` (17), only if `MQ` was acked.
6. `GET_SHMEM_CONFIG` (44), only if `SHMEM` was acked. The frontend validates
   the reply: `nregions <= 256`, `nregions` equal to the number of non-zero
   sizes, every size a multiple of the page size. A violation fails device
   creation with `Invalid shared memory config from the backend: ...`.
7. `device_manager` backs the regions (below), before the PCI BARs are laid out.
8. Guest driver activation: `SET_FEATURES` (2), memory table, vrings, then
   `SET_BACKEND_REQ_FD` (21) with a fresh backend channel if `BACKEND_REQ`
   was acked. Every activation creates a new channel.
9. The backend sends `SHMEM_MAP` (9) and `SHMEM_UNMAP` (10) on that channel at
   any time while the device is active.

### Regions and the PCI BAR

- One host window backs all regions. Regions sit back to back in shmid
  order; unused shmids take no space. The window is rounded up to a power of
  two (a BAR size) and allocated from the segment's 64-bit MMIO space with
  natural alignment.
- The window is an anonymous `PROT_NONE`, `MAP_NORESERVE` mapping registered
  as a KVM userspace mapping. Unmapped window space is inaccessible.
- The device's region list is indexed by shmid; unused shmids are zero-length
  entries. `pci_device` emits one `VIRTIO_PCI_CAP_SHARED_MEMORY_CFG`
  capability per non-empty entry, with `cap.id = shmid`, `bar = 2`, and the
  region's offset and length within the BAR. A guest driver finds region `n`
  with `virtio_get_shm_region(vdev, &region, n)`.

### SHMEM_MAP

The frontend refuses the request (non-zero reply, mapping unchanged) when:

| Condition | errno in the reply |
|---|---|
| `BACKEND_SEND_FD` not negotiated | `EINVAL` |
| the device has no regions | `EINVAL` |
| `shmid` names no region, or an unused shmid | `EINVAL` |
| `len == 0`, or `shm_offset`, `len` or `fd_offset` not page aligned | `EINVAL` |
| `shm_offset + len` beyond the region | `EINVAL` |
| the range overlaps an existing mapping of that region | `EBUSY` |
| `mmap` fails | its errno |

Checks the `vhost` crate makes before the handler runs, and that end the
channel instead of replying: exactly one fd attached, payload size 40, a known
flag bit, `len != 0`, no overflow of `fd_offset + len` or `shm_offset + len`.

Otherwise the frontend maps the fd with `MAP_SHARED | MAP_FIXED` at
window + region offset + `shm_offset`, `PROT_READ | PROT_WRITE` if flags bit 0
is set, else `PROT_READ`, records the mapping, and replies 0. A failed `mmap`
restores the `PROT_NONE` placeholder over the range.

### SHMEM_UNMAP

The range must equal one earlier SHMEM_MAP of the same shmid (same
`shm_offset` and `len`); otherwise the reply is `EINVAL` and nothing changes.
The same region, alignment and bounds checks apply. On success the frontend
lays a `PROT_NONE` placeholder over the range (rather than leaving a hole in
the window), forgets the mapping and replies 0.

### Reset, snapshot, reconnect

- Device reset: after the backend worker has been joined, every mapping is
  replaced by the placeholder and forgotten ("mappings are automatically
  unmapped by the front-end across device reset operation").
- Snapshot and live migration: refused for a device with regions. The
  mappings are backend state that no migration stream carries.
- Backend reconnect: the window layout is kept and existing mappings are not
  dropped. A new backend process that reuses offsets gets `EBUSY`. Leandro's
  backend exits when the VMM hangs up, so this does not arise in its use.

### Replies

The `vhost` crate's `FrontendReqHandler` sends the reply: 0 on success, the
negated errno as u64 on failure, only when `REPLY_ACK` was negotiated and the
request has `NEED_REPLY` set. Without a reply the backend cannot know the
outcome; backends that map memory should negotiate `REPLY_ACK`.

## Patch 0002: device feature bits

`GET_FEATURES` returns a 64-bit mask. The generic device offers
`DEFAULT_VIRTIO_FEATURES` (transport and vhost-user bits: 26, 28, 29, 30, 32,
35, 36, 38) plus:

| Bits | Mask | Reason |
|---|---|---|
| 0 to 23 | `0x0000_0000_00ff_ffff` | device specific (VIRTIO 1.4, 2.2) |
| 50 to 63 | `0xfffc_0000_0000_0000` | device specific (50 to 127), limited to the 64-bit word |

Bits 41 and 42 are not offered: VIRTIO 1.4 lists them as device specific only
because legacy virtio-net features use them, and its reserved-bit list also
defines bit 41 as `VIRTIO_F_ADMIN_VQ`, which this transport lacks.

Negotiation keeps `offered & backend features`, so only bits the backend
advertises reach the guest. `SET_FEATURES` at activation sends the bits the
guest acked, plus `VHOST_USER_F_PROTOCOL_FEATURES`. A backend that advertises
no device-specific bits negotiates exactly what it did before.

## Patch 0003: refused backend requests

`FrontendReqHandler::handle_request` returns:

| Result | Meaning | Worker |
|---|---|---|
| `Ok` | request handled, reply 0 sent if asked for | continues |
| `Err(ReqHandlerError)` | the handler refused the request; the non-zero reply was sent if asked for, before returning | **continues** (was: ended) |
| any other `Err` | socket broken, stream out of step, malformed or unknown request | ends, device marked disconnected (unchanged) |

The specification lets SHMEM_MAP fail ("requests shall fail" on overlap, "can
fail when there are no resources available"), so a refusal is an outcome of
the request, not a channel failure. The change applies to every vhost-user
device's backend channel; only devices whose handler can refuse (the generic
device's SHMEM handlers, and config-change signalling) are affected.

## Patch 0005: vfio-user region I/O fds

The vfio-user specification lets a server name sub-regions of a region whose
accesses the client should deliver through file descriptors instead of
`VFIO_USER_REGION_WRITE` ("Region I/O via file descriptors"). For an ioeventfd
sub-region the client registers the descriptor with `KVM_IOEVENTFD` ("a client
could configure KVM with the requested ioeventfd via a KVM_IOEVENTFD ioctl()"), so
a guest write there signals the server without an exit to the VMM. Cloud
Hypervisor v53.0 never sent the request. Citations below are to vfio-user.rst
(see [Specifications](#specifications)) unless another source is named.

### What the client does

1. `VfioUserPciDevice::map_mmio_regions()` (called by `device_manager` after the
   BARs are allocated, also on restore) maps the mmap-able areas as before, then,
   for each BAR in order, sends the request (message formats above) through
   `vfio_user::Client::get_region_io_fds()`.
2. The outcome:

   | Outcome | Meaning | What happens |
   |---|---|---|
   | reply in the specification's form | the crate's parser accepted it | each entry is checked (next table) and registered |
   | `Error::InvalidReply` | a well-framed reply not in the specification's form: argsz other than 16 + count * 40 or than the payload size, flags not 0, another index, a type other than 0 or 1, more than the fixed part in a truncated reply, truncated twice or argsz above 65535 | logged (`region I/O fds reply not in the vfio-user specification's form`), this BAR stays trapped, the next BAR is asked |
   | `Error::ErrorReply` | the Error bit is set | logged once (`the server provides no region I/O fds`), no further BAR is asked; every BAR stays trapped |
   | any other error | socket failure, or a message that is not the reply (the stream is out of step) | device creation fails (`VfioUserMapRegion`) |

3. Each ioeventfd entry must pass these checks ("it is the client's responsibility
   to verify the requested values"); a failing entry is logged and its range stays
   trapped:

   | Check | Source |
   |---|---|
   | type 1 (ioregionfd) is skipped | Linux has no KVM ioregionfd ("FIXME: update with final API docs") |
   | flags only `DATAMATCH`, `PIO`, `VIRTIO_CCW_NOTIFY`; `VIRTIO_CCW_NOTIFY` skipped | "flags is any of"; the CCW flag is for s390 subchannels |
   | `PIO` set exactly for an I/O port BAR | an MMIO BAR address is not a port and vice versa |
   | size 0, 1, 2, 4 or 8 | `KVM_IOEVENTFD` `len`; "size ... may be zero if the access size is not relevant" |
   | with `DATAMATCH`: size not 0, datamatch fits in size bytes | `KVM_IOEVENTFD` rejects datamatch with len 0 and compares the written value of len bytes |
   | fd_index below the number of descriptors received | "fd_index is the index in the ancillary data" |
   | offset + max(size, 1) within the BAR | "that the requested offset does not exceed the region's bounds" |
   | not overlapping an mmap()ed area or the MSI-X table or PBA | an mmap()ed range never exits, so the ioeventfd would never fire; the VMM emulates MSI-X and must see those writes |

4. Registration: `KVM_IOEVENTFD` with `addr` = BAR guest address + offset, `len` =
   size, `datamatch` and `KVM_IOEVENTFD_FLAG_DATAMATCH` when the entry's flags have
   it, `KVM_IOEVENTFD_FLAG_PIO` for an I/O BAR. Entries naming the same fd_index
   share one `EventFd`. A failed registration (for example `EEXIST`) is logged; the
   range stays trapped.
5. When the guest moves a BAR, its ioeventfds are deassigned and assigned at the new
   address. When the device goes away (hot-unplug or VM shutdown, `Drop`), all are
   deassigned with the arguments of the assign.
6. `hypervisor::Vm` gains `register_ioevent_exact()` and `unregister_ioevent_exact()`
   with `len` and `datamatch` as given. The existing `register_ioevent()` takes the
   length from the datamatch type (0 without, 4 or 8 with) and
   `unregister_ioevent()` only removes an ioeventfd without datamatch; KVM matches a
   deassign on address, length and datamatch. KVM implements the new calls with a raw
   `KVM_IOEVENTFD` ioctl; other hypervisors keep the default, which refuses, so their
   BARs stay trapped.

The server side (the vendored crate): `Server::add_ioeventfd()` declares ioeventfd
sub-regions; the request is refused with `EINVAL` for flags or count not 0, an index
beyond the regions or argsz below 16, answered with count 0 for a region without
sub-regions, with the fixed part alone when the reply exceeds argsz, and otherwise
with 40-byte type-0 entries and one descriptor per distinct eventfd, leaving out
sub-regions whose descriptor would exceed the client's `max_msg_fds`.

### Field by field

"Client" is patch 0005 with the crate's client, "Server" the crate's server,
"libvfio-user" nutanix/libvfio-user at `803924493a7c` (`include/vfio-user.h`,
`lib/libvfio-user.c` `handle_device_get_region_io_fds`, lines 548-652). The one
row where the reference server disagrees with the specification's layout is
marked **differs**.

| Item | vfio-user.rst | Client | Server | libvfio-user |
|---|---|---|---|---|
| command number | "Commands": 6 | 6 | 6 | 6 (`vfio-user.h:63`) |
| reply header | "Header": ID and command echoed, Type 1 | checked; a mismatch is out of step | echoed | echoed |
| error reply | "Header": Error bit, errno may be 0; header only | `ErrorReply`, stop asking | errno `EINVAL` for bad input, 0 otherwise | errno |
| request argsz | "Request": maximum reply payload; "Message sizes" | 656, then the argsz a truncated reply names, below 65536 | below 16 refused | below 16 or above its maximum transfer size refused |
| request flags, count | "Request": zero | 0, 0 | not 0 refused | not 0 refused |
| request index | "Request": region queried | BAR index | beyond the regions refused | beyond the regions refused; a region without flags refused |
| reply argsz | "Reply": fixed part + array | must be 16 + count * 40 and the payload size | 16 + count * 40 | 16 + count * 48 (**differs**) |
| truncated reply | "Message sizes": argsz needed, payload not included | fixed part only, one retry | fixed part, argsz needed, count | same as Server |
| reply flags, index, count | "Reply": flags 0, index queried, count entries | checked | 0, echoed, count | 0, echoed, count |
| entry i | "Reply": `i * ((argsz - 32) / count)`, 40 bytes currently | 16 + 40 * i | 16 + 40 * i | 16 + 48 * i (**differs**) |
| offset, 0 (8) | "(ioeventfd)": within the region | within the BAR, clear of mmap and MSI-X | declared within the region | `gpa_offset`, 0 |
| size, 8 (8) | "(ioeventfd)": length, may be 0 | KVM `len` 0, 1, 2, 4, 8 | as declared | 8 |
| fd_index, 16 (4) | "(ioeventfd)": index in the ancillary data, may be shared | below the descriptors received | per distinct eventfd | same; `UINT32_MAX` for a sub-region without descriptor |
| type, 20 (4) | "(ioeventfd)", "(ioregionfd)": no numbers | 0 or 1, else the reply is refused | 0 | 0, 1, or 2 = `IOEVENTFD_SHADOW` (`vfio-user.h:174-176`) |
| flags, 24 (4) | "(ioeventfd)": DATAMATCH, PIO, VIRTIO_CCW_NOTIFY | see the checks | only those three | as given |
| padding, 28 (4) | "(ioeventfd)": padding | ignored | 0 | `shadow_mem_fd_index` (**differs**) |
| datamatch, 32 (8) | "(ioeventfd)": datamatch | with DATAMATCH | written at 32 | `shadow_offset` at 32, datamatch at 40 (**differs**) |
| ioregionfd entry | "(ioregionfd)": user_data at 32 | parsed, skipped | never sent | same 40-byte struct (`vfio-user.h:189-197`), 48 apart in the reply's union; not implemented (`docs/ioregionfd.md`) |
| descriptors | "Reply": at least one, may be shared | those named by fd_index | one per distinct eventfd, none with count 0 | same |
| descriptors per message | `VFIO_USER_VERSION`: `max_msg_fds`, 1 when absent | the crate left it out (so 1); since crate patch 0002 it announces 253 (`SCM_MAX_FD`) and receives that many | honoured, and never more than 253 | parsed (`tran.c:71-99`), not applied to this reply |

### Ambiguities and the reading chosen

1. **Entry offset.** "each array entry i is at offset i * ((argsz - 32) / count)"
   contradicts the reply table, where the array starts at offset 16: for one
   40-byte entry argsz is 56 and the formula gives 24. libvfio-user places the
   array directly after the 16-byte fixed part (`sizeof(vfio_user_region_io_fds_reply_t)`,
   `lib/libvfio-user.c:620`) and its test client reads entries from there
   (`test/py/test_device_get_region_io_fds.py`). Entries are at 16 + 40 * i.
2. **Entry size.** "currently this is 40 bytes for both IO FD types, but this is not
   to be relied on". The only implementation that sends another size changes the
   meaning of bytes 28 to 40, so a larger entry cannot be read safely by position.
   Decision (coordinator, 2026-09-24): accept exactly 40 bytes and fail closed; a
   reply of another size leaves the region trapped.
3. **Type numbers.** The specification names `VFIO_USER_IO_FD_TYPE_IOEVENTFD` and
   `_IOREGIONFD` without values; 0 and 1 are libvfio-user's (`vfio-user.h:174-175`).
4. **Flag values.** The specification names the `KVM_IOEVENTFD_FLAG_*` constants;
   the values are `<linux/kvm.h>`'s.
5. **size.** "the length of the sub-region" and a pointer to `KVM_IOEVENTFD` for the
   ioeventfd fields. Read as KVM's `len` (an access width), since a KVM ioeventfd
   matches one address and one width; other sizes are skipped.
6. **"at least one file descriptor"** cannot hold with count 0; neither
   implementation sends one then.
7. **count in a truncated reply.** "the number of sub-regions in the array", but the
   array is left out; libvfio-user sends the total, so does the server, and the
   client does not use it.
8. **A region without sub-regions.** Not stated; libvfio-user refuses a region
   without flags. The server answers count 0 for every valid index.
9. **`max_msg_fds` against distinct descriptors.** Not stated for this reply. The
   server leaves out the sub-regions that would need more; they stay trapped.

### Deviations of the reference implementations

- **libvfio-user ioeventfd entry** (`include/vfio-user.h:178-187` at
  `803924493a7c`): `gpa_offset` 0, `size` 8, `fd_index` 16, `type` 20, `flags` 24,
  `shadow_mem_fd_index` 28, `shadow_offset` 32, `datamatch` 40, 48 bytes, and the
  reply's array is a union of it with the 40-byte ioregionfd entry, so entries are 48
  bytes apart. The fields serve libvfio-user's shadow ioeventfd (type 2,
  `vfio-user.h:176`), which `docs/shadow_ioeventfd.md` says needs unmerged KVM and
  QEMU patches. The specification has padding at 28 and datamatch at 32 in a 40-byte
  entry. Read by the specification, a libvfio-user entry's datamatch would be taken
  from `shadow_offset`, which could swallow guest writes of the wrong value. Our
  client refuses such a reply (the region stays trapped) and our server never sends
  that layout or type 2: **on this one message neither end is wire-compatible with
  libvfio-user until the specification adopts that layout.** Everything else in this
  patch works with a libvfio-user server.
- **libvfio-user `fd_index` `UINT32_MAX`** for a sub-region declared with fd -1 (its
  test `test_device_get_region_io_fds_invalid_fd`); not in the specification. The
  client skips the entry.
- **libvfio-user and `max_msg_fds`:** parsed, not applied to this reply.
- **rust-vmm `vfio_user` servers up to 0.1.6** answer `UnsupportedCommand` before
  reading the request's 16-byte payload, then parse that payload as the next header
  (command = argsz >> 16 = 0) and send a second error reply, to a command never sent.
  The crate's client keeps argsz below 65536 so that header is harmless, follows an
  error reply with a `VFIO_USER_DEVICE_GET_INFO` round trip and drops one such stray
  header-only error reply. With a conforming server nothing is dropped.
- **QEMU v11.1.1** (`vendor/qemu-vfio-user`) defines command 6 but never sends it, so
  there is no reference client for the request; the client side follows the
  specification text and libvfio-user's test client.

### Use by the Leandro VFIO device

`CARAXES_DOORBELL_IOEVENTFD=1` makes `vfio-user-nvgpu` declare one ioeventfd
sub-region on BAR0: offset = the chip's `registers.usermode_doorbell` (0xbb0090 on
TU106, `NOTIFY_CHANNEL_PENDING`), size 4, no datamatch. With 0005 alone the crate's
client announced no `max_msg_fds` (so 1), one eventfd per region was all a reply could
carry, and per-token datamatch entries would have shared it without naming the token
(the token pool, `CARAXES_DOORBELL_TOKENS=N`, needs crate patch 0002: [Patch
0006](#patch-0006-datamatch-sub-regions-at-one-offset)). A reader thread reads the
eventfd; KVM drops the written token, so the thread rings every recorded channel
through the BAR0 write path a trapped doorbell takes (a channel without new GPFIFO
entries runs nothing). The flag is refused with `--host-exec`, where a first doorbell
creates a host mirror channel. `crates/vfio-user-nvgpu/src/{device.rs,main.rs}`.

### Compatibility

- A server that does not implement the request: one log line, every BAR trapped, the
  device works as before (vm3 run S with a vfio_user 0.1.6 server).
- A libvfio-user server with ioeventfd sub-regions: the region stays trapped (unit
  tests with its byte layout; not run against libvfio-user).
- A Cloud Hypervisor without 0005 and a server with sub-regions: never asked; the
  sub-regions are written through `VFIO_USER_REGION_WRITE` as before.
- Hypervisors other than KVM: every BAR trapped.
- `tools/build.sh ch` globs `patches/[0-9][0-9][0-9][0-9]-*.patch`, so its next run
  applies 0005 too; it builds against `vendor/vfio-user-rs`.

### Verification (2026-09-24)

Tree: v53.0 + 0001-0005 (`git am` of the five files), `cargo build --offline --locked
--release --bin cloud-hypervisor`: builds, no warnings, `cloud-hypervisor
v53.0-5-gba20497bc`. v53.0 + 0005 alone: applies with `git apply` and builds.

| Command | Result |
|---|---|
| `cargo test -p vmm --features kvm --lib` | 89 passed (as with 0004) |
| `cargo test -p pci --lib` | 28 passed, 4 new: `vfio_user::tests::sub_region_is_used_as_offered`, `sub_region_is_refused`, `reply_in_the_specification_layout_is_used` (a hand-built reply in the specification's bytes through the crate's parser and the checks), `libvfio_user_reply_keeps_the_region_trapped` (a hand-built 48-byte libvfio-user reply is refused, the region stays trapped) |
| `cargo test -p hypervisor --features kvm --lib` | 1 passed |
| `cargo clippy -p pci -p hypervisor -p vmm --features kvm --all-targets` | only the unfulfilled lint expectation at `vmm/src/lib.rs:775` that v53.0 has |
| `cargo test -p vfio_user` (rust-vmm/vfio at `a38ee7a3eeb5` + the crate patch) | 10 passed, 7 new: parser on the specification's bytes, on a libvfio-user reply, truncated and malformed replies, the server's reply bytes, a client/server round trip (descriptor sharing, truncation and retry, `max_msg_fds`, error reply and reuse), a server without the request |
| `cargo test -p vfio-user-nvgpu` (Leandro VFIO, vendored crate) | 97 passed |

Functional check in vm3, pure emulation (`CARAXES_HOST_RM=1 CARAXES_BOOT_MODEL=1`),
[../caraxes/runs/patch0005-2026-09-24](../caraxes/runs/patch0005-2026-09-24)
(`commands.txt`):

| Run | Result |
|---|---|
| S: patched CH, stock server (vfio_user 0.1.6) | the stray reply (`message_id: 656, command: 0`) dropped, one line `the server provides no region I/O fds`, driver loads |
| A: new server, flag off | no sub-regions; driver loads; bench-trap doorbell (0xbb0090) write p50 28.2 us, scratch 26.7 us; 20000 trapped doorbell writes in the device trace |
| B: new server, `CARAXES_DOORBELL_IOEVENTFD=1` | CH: `ioeventfd at 0xe6bb0090, len 4, datamatch None`; driver loads; bench-trap doorbell write p50 3.9 us (p99 10.3), scratch still 26.7 us; no doorbell write arrived as a region write, 20000 counted by the eventfd; UVM GPU registration (`uvm-register`) succeeds with its 7 doorbells through the eventfd |
| B, removal and re-add | hot-remove: no unregister error, the server exits; re-add with a new server registers the same address, length and (no) datamatch again without `EEXIST`, so the first was deassigned; doorbell write p50 3.9 us on the re-added device |

## Patch 0006: datamatch sub-regions at one offset

A server may offer several ioeventfd sub-regions at one offset that differ only in
datamatch, each with its own eventfd. KVM then signals the eventfd of the value the
guest wrote, and the server knows the value without a `VFIO_USER_REGION_WRITE`. The
Leandro VFIO doorbell token pool uses this: one sub-region per device-chosen work-submit
token ([../docs/vfio/doorbell-token-pool.md](../docs/vfio/doorbell-token-pool.md)).

What the specification allows, with vfio-user.rst lines:

| Item | vfio-user.rst | Reading |
|---|---|---|
| descriptors per message | `max_msg_fds`: "Maximum number of file descriptors that can be received by the sender in one message. Optional. If not specified then the receiver must assume a value of 1" (:515-519) | the client announces what it can receive; a server sends no more |
| descriptors in the reply | "at least one file descriptor ... more than one sub-region may share the same file descriptor" (:1025-1027) | sharing is allowed, not required: one per sub-region is in the specification |
| sub-regions at one offset | entries of offset, size, fd_index, type, flags (DATAMATCH, PIO, VIRTIO_CCW_NOTIFY), datamatch (:1036-1073); nothing restricts offsets to be distinct; "it is the client's responsibility to verify the requested values" (:1029) | allowed; KVM decides which pairs collide |
| reply size | one message | Linux carries at most `SCM_MAX_FD` = 253 descriptors in one `SCM_RIGHTS` message (`include/net/scm.h:18`), so 253 sub-regions with distinct eventfds per region |

What 0005 did with such a reply: every entry is checked on its own, the overlap check is
against mmap()ed areas and the MSI-X table and PBA only, `fd_index` must be below the
descriptors received, and a duplicate that KVM refuses (`EEXIST`) is logged and stays
trapped. So Cloud Hypervisor needed no new check. The limit was the crate: its client
left `max_msg_fds` out (every server had to assume 1) and received at most 16
descriptors (a reply with more fails the receive, vmm-sys-util `recv_with_fds`
`MSG_CTRUNC`, and device creation with it).

The change:

- Crate ([vfio-user-rs/0002](vfio-user-rs/0002-vfio-user-one-eventfd-per-ioeventfd-sub-region.patch),
  `vendor/vfio-user-rs`): the client announces `max_msg_fds` 253 (`CLIENT_MAX_MSG_FDS` =
  `SCM_MAX_FD`) and receives that many; the server never sends more than 253 whatever the
  client announced; `Server::add_ioeventfd` requires a width of 1, 2, 4 or 8 with a
  datamatch that fits and refuses a sub-region KVM would refuse as a duplicate
  (`ioeventfds_collide`: same offset and I/O space, and a size of 0 on either side or the
  same size with a wildcard on either side or the same datamatch; measured on the host's
  KVM, [../caraxes/runs/doorbell-tokens-2026-09-24/kvm-ioeventfd-probe.txt](../caraxes/runs/doorbell-tokens-2026-09-24/kvm-ioeventfd-probe.txt) §2).
- Cloud Hypervisor (0006, `pci/src/vfio_user.rs`): after a region's entries are
  registered, one INFO line `<id>: region <i>: <r> of <n> sub-region(s) registered as
  ioeventfds, <d> descriptor(s) received, <m> with datamatch in [<lo>, <hi>]`
  (`region_io_fds_summary`); per-entry lines as in 0005. Test
  `vfio_user::tests::datamatch_entries_at_one_offset_are_used`: a hand-built reply of 253
  entries at one offset in the specification's bytes, each with DATAMATCH and its own
  fd_index, through the crate's parser and `check_sub_region`; an fd_index beyond the
  descriptors is refused; the summary line.

Why 0006 and not an amended 0005: 0005 is verified and recorded as it stands and the
coordinator's runs use its binary; the functional change is in the crate that 0005 builds
against by path. Neither is upstreamed; before upstreaming 0006 folds into 0005, and crate
patches 0001 and 0002 into one rust-vmm/vfio change.

KVM facts the design relies on (probe on the host, Linux 7.2.4): ioeventfds do not count
against `NR_IOBUS_DEVS` (4096 datamatch entries at one address registered); a write is
compared against the entries of its address in registration order, about 25 ns per entry
passed, so a value that matches none pays for all of them before it exits.

### Verification (2026-09-24)

Tree: `/mnt/vmstore/caraxes/ch-0006`, a clone of ch-0005 (v53.0 + 0001-0005, `ba20497bc`)
plus 0006 (`88e92aded`, branch `patch0006`), built against the worktree's
`vendor/vfio-user-rs` with `--config 'patch.crates-io.vfio_user.path="<tree>/vendor/vfio-user-rs"'`
(the tree's own `../vfio-user-rs` is the main checkout's symlink); `cloud-hypervisor
v53.0-6-g88e92aded`, no warnings.

| Command | Result |
|---|---|
| `cargo test -p vmm --features kvm --lib` | 89 passed (as with 0005) |
| `cargo test -p pci --lib` | 29 passed, 1 new (`datamatch_entries_at_one_offset_are_used`) |
| `cargo test -p hypervisor --features kvm --lib` | 1 passed |
| `cargo clippy -p pci -p hypervisor -p vmm --features kvm --all-targets` | only the unfulfilled lint expectation at `vmm/src/lib.rs:775` that v53.0 has |
| `cargo test -p vfio_user` (rust-vmm/vfio at `a38ee7a3eeb5` + crate patches 0001 and 0002, `/mnt/vmstore/caraxes/vfio-rs-0006`) | 14 passed, 4 new: declaration rules at one offset, descriptor limits (`max_msg_fds` 1, 64, 253, 1000), a round trip of 253 datamatch sub-regions with 253 eventfds (each received descriptor checked against its token's eventfd), a hand-built reply with datamatch entries at one offset; clippy `-D warnings` clean, rustfmt clean |

Functional check in vm1, pure emulation: see
[../caraxes/runs/doorbell-tokens-2026-09-24](../caraxes/runs/doorbell-tokens-2026-09-24)
(`commands.txt`) and [../docs/vfio/doorbell-token-pool.md](../docs/vfio/doorbell-token-pool.md), "Verification".

## How Leandro uses the series

| Item | virtio-nvrm |
|---|---|
| Device | `--generic-vhost-user "device_type=60,socket=...,queue_sizes=[256,256]"` |
| Virtio features | `VERSION_1` (32), `RING_INDIRECT_DESC` (28), `VHOST_USER_F_PROTOCOL_FEATURES` (30); no device-specific bits, so 0002 changes nothing for it |
| Protocol features | `REPLY_ACK`, `BACKEND_REQ`, `BACKEND_SEND_FD`, `SHMEM` |
| GET_SHMEM_CONFIG | `nregions = 1`, `memory_sizes[1] = 8 GiB` (`HOST_VISIBLE_SIZE`); shmid 0 unused |
| Guest | `virtio_get_shm_region(vdev, &shm, 1)`; the module allocates page slots in the region and sends their region-relative offset in `MAP_PREPARE` |
| SHMEM_MAP | shmid 1, `fd_offset = 0`, `shm_offset` = guest-chosen offset, `len` = page-rounded RM mapping size, flags `1` (read-write); the fd is the RM mapping file. The backend checks alignment, bounds and overlap itself before sending |
| SHMEM_UNMAP | shmid 1, the same `shm_offset` and `len`, flags 0. The backend frees the slot only after reply 0 |
| Refusal | the backend reports `EIO` to the guest request and keeps the device; `LEA_TEST_SHMEM_MAP_OOB=1` injects one out-of-region SHMEM_MAP to exercise this path |
| Device reset | the backend forgets its window table when the next activation sends `SET_BACKEND_REQ_FD` |

Code: `crates/vhost-user-nvrm/src/nvrm.rs` (`shmem_config`,
`window_request`, `protocol_features`, `set_backend_req_fd`,
`on_map_prepare`, `release_window`) and
`guest-module/virtio_nvrm/virtio_nvrm.c` (`NVRM_SHM_ID_HOST_VISIBLE`).

## Compatibility

There is no transition switch. An old `vhost-user-nvrm` (vhost 0.16)
advertises SHMEM as bit 21, which the patched frontend does not offer, so the
device comes up without a window. The guest module logs
`virtio_nvrm: no host-visible window -- mmap will fail`. A new backend with an
old Cloud Hypervisor build fails the same way. Rebuild both.

A `vendor/cloud-hypervisor` tree that carries the previous series is modified
in a way `tools/build.sh ch` does not recognise as a prefix of the new series;
it stops and asks for a reset. Reset and rebuild:

```sh
git -C vendor/cloud-hypervisor checkout -- .
tools/build.sh ch
```

## Verification

### Build and unit tests

```sh
git clone --depth 1 --branch "$(cat CH_VERSION)" \
  https://github.com/cloud-hypervisor/cloud-hypervisor.git /tmp/ch
for patch in patches/[0-9][0-9][0-9][0-9]-*.patch; do git -C /tmp/ch apply "$patch" || exit 1; done
cargo build --locked --release --manifest-path /tmp/ch/Cargo.toml --bin cloud-hypervisor
cargo test --locked --manifest-path /tmp/ch/Cargo.toml -p virtio-devices
tools/check.sh
```

Recorded 2026-09-23 on the rewritten series: the three patches apply to
v53.0 with `git apply` and `git am`; the release build succeeds;
`cargo test -p virtio-devices` passes 112 tests, 10 of them new (v53.0 has 102):

| Test (virtio-devices) | Checks |
|---|---|
| `generic_vhost_user::tests::shmem_config_is_indexed_by_shmid` | index = shmid, `nregions` as a count, trailing unused ids dropped |
| `generic_vhost_user::tests::shmem_config_is_validated` | prefix-style `nregions`, count mismatch, unaligned size, `nregions > 256` rejected |
| `generic_vhost_user::tests::map_offsets_are_relative_to_the_region` | a mapping lands at region start + `shm_offset`, not window + `shm_offset` |
| `generic_vhost_user::tests::map_refuses_what_the_specification_does_not_allow` | unused and unknown shmid, out of region, unaligned offset, length and fd offset, overlap (`EBUSY`), adjacent mapping allowed |
| `generic_vhost_user::tests::unmap_needs_exactly_one_mapping` | partial and unknown ranges refused, double unmap refused, remap after unmap |
| `generic_vhost_user::tests::reset_drops_every_mapping` | reset clears all mappings |
| `generic_vhost_user::tests::backend_requests_get_a_zero_or_non_zero_reply` | over a real backend channel with `REPLY_ACK`: map 0, overlapping map non-zero, partial unmap non-zero, unmap 0 |
| `generic_vhost_user::tests::map_needs_backend_send_fd` | SHMEM_MAP refused without `BACKEND_SEND_FD` |
| `generic_vhost_user::tests::device_features_are_the_device_specific_bits` | exactly bits 0-23 and 50-63, disjoint from the default set |
| `vhost_user::tests::only_a_refused_backend_request_keeps_the_worker` | `ReqHandlerError` continues; `InvalidMessage` and `SocketBroken` end the worker |

`nix build .#cloud-hypervisor` builds the patched tree (the package applies
the series as `cargoPatches`, since 0001 changes `Cargo.lock`). `cargo clippy
-p virtio-devices --all-targets -D warnings` is clean; clippy on the `vmm`
crate reports one unfulfilled lint expectation at `vmm/src/lib.rs:775`, which
unpatched v53.0 reports with the same toolchain (Rust 1.89).

`vhost-user-nvrm` adds `nvrm::vhost_user_tests`: the protocol feature bits
against the specification's numbers, the byte layout of the
GET_SHMEM_CONFIG reply and of the SHMEM_MAP/UNMAP payloads, and the handshake
of the patched frontend (steps 1 to 8 above, without SET_FEATURES) against the
real `VhostUserDaemon`, followed by a SHMEM_MAP with an fd and a SHMEM_UNMAP
over the handed-over backend channel with `REPLY_ACK`.

### Manual VM check

Run on 2026-09-23, see [Verification (2026-09-23)](#verification-2026-09-23). After building both
ends, follow [quickstart.md](../docs/virtio/quickstart.md) sections 1 and 2 with the
new binaries, then:

1. Hypervisor: start one VM with `-v` added to the `"$CH"` command line. The
   hypervisor log shows `generic vhost-user _generic_vhost_user0: shared
   memory window 0x200000000 at 0x...` (8 GiB) and no `Invalid shared memory
   config` error. Without `-v` only warnings and errors are logged.
2. Guest: `sudo dmesg | grep virtio_nvrm` shows
   `virtio_nvrm: window 8192 MiB @0x...`, not `no host-visible window`.
   `sudo lspci -vv -d 1af4:` lists a vendor-specific capability for the shared
   memory region on BAR 2 of the virtio-nvrm device.
3. Mappings: `LD_LIBRARY_PATH=/opt/nvrm/lib nvidia-smi` on a compute guest, or
   the desktop start of an Ubuntu guest, succeeds. Its first CUDA or display
   mapping is a SHMEM_MAP of the GPU doorbell.
4. Refusal recovery: restart one backend with `LEA_TEST_SHMEM_MAP_OOB=1` and
   the VM. The backend log shows the injected request and
   `vhost-user-nvrm: SHMEM_MAP: ...`; the hypervisor log shows
   `shared memory request 0x200000000+... exceeds shmid 1` and `vhost-user
   backend request refused`; the guest's first mapping fails with `EIO`, and
   `nvidia-smi` run again succeeds.
5. Reset: in a guest with no NVIDIA client running, unload the guest module
   stack (`virtio_nvrm` last), which resets the device, load it again, and
   repeat step 3. With `LEA_DEBUG=1` the backend logs `new backend channel:
   N window mapping(s) dropped by the device reset` when N mappings were live.
6. A second VM on the same host behaves the same way.

## Upstreaming

- The supported build target is the pinned tag. Before submission, rebase each
  patch onto the intended upstream revision and retest; upstream may already
  have moved to `vhost` 0.17, which removes the `vhost-frontend` alias.
- Submit the three changes separately. Describe the generic device behaviour
  and include a reproducer; GPU use is one application.

## Verification (2026-09-23)

The [manual VM check](#manual-vm-check) on a v1 VM, plus the negative pairing
with the previous frontend. Logs, the launcher, the CUDA test program and every
command are in
[caraxes/runs/vhost-user-check-2026-09-23](../caraxes/runs/vhost-user-check-2026-09-23)
(`commands.txt` lists the commands in order).

Setup: Cloud Hypervisor `v53.0-3-g993681a04` built from this series (the
patch-ids of its three commits equal those of 0001-0003), `vhost-user-nvrm`
built from this branch, host driver 610.57.04 on an RTX 2070. Guest: the baked
Ubuntu 24.04 image `guest-baked-20260801-610.57.04-20260820-105733` (kernel
6.8.0-136) with `nvrm_nodes` and `virtio_nvrm` built in the guest from this
branch, 8 vCPUs, 4 GiB shared memory, the QUICKSTART command line with `-v`.
The CUDA program is `vecadd.c` there: driver API, a PTX kernel JIT-compiled by
the driver, 1 Mi elements checked on the host.

| Step | Result | Evidence |
|---|---|---|
| 1 Hypervisor log | Pass. `generic vhost-user _generic_vhost_user3: shared memory window 0x200000000 at 0x3ffc00000000`; no `Invalid shared memory config` in any session. The device id is `_generic_vhost_user3`, not `_generic_vhost_user0`: the number counts all devices on the command line | `hypervisor-A.txt`, `-B`, `-C` |
| 2 Guest | Pass. `virtio_nvrm: window 8192 MiB @0x3ffc00000000`. The capability at 0xa4 is `VIRTIO_PCI_CAP_SHARED_MEMORY_CFG` (cfg_type 8), bar 2, id 1, offset 0, length `0x2_0000_0000`; lspci prints only the low 32 bits of the 64-bit form (`size=00000000`) | `guest-evidence.txt` |
| 3 Mappings | Pass. `nvidia-smi` works (it makes no window mapping). `vecadd` passes; its first mapping is a 64 KiB uncached SHMEM_MAP at region offset 0, right after the allocation of class `0xc461` (`TURING_USERMODE_A`, the doorbell page) in the ledger of session B. Session A: 87 successful SHMEM_MAPs, no hypervisor warning | `backend-A.txt`, `backend-B.txt` |
| 4 Refusal | Pass. Backend: `LEA_TEST_SHMEM_MAP_OOB -- asking the VMM to map at 0x200000000` and `SHMEM_MAP: Frontend internal error` (the `vhost` crate's name for a non-zero reply). Hypervisor: `shared memory request 0x200000000+0x10000 exceeds shmid 1 of 0x200000000 bytes` and `vhost-user backend request refused ...: Invalid argument (os error 22)`. Guest: `MAP_PREPARE failed: -5`, `cuInit` returns 100. `nvidia-smi` and `vecadd` run next succeed; the channel stayed up | `backend-B.txt`, `hypervisor-B.txt`, `guest-evidence.txt` |
| 5 Reset | Pass. Module unload and reload is a device reset (`event = reset`, then `activated`); `nvidia-smi` and `vecadd` pass afterwards, in sessions A, B and C. No mapping was live at the reset (every client had exited and released its slots), so `new backend channel: N window mapping(s) dropped` was not logged | `hypervisor-A.txt`, `-B`, `-C` |
| 6 Second VM | Pass. Two VMs (v1check, v1check2) with their own backends ran three `vecadd` each at the same time, all passing; the window lines of steps 1 and 2 and the reset of step 5 hold on the second VM too. Host available memory stayed above 10 GiB | `backend-C.txt`, `hypervisor-C.txt`, `guest-evidence.txt` |
| Negative pairing | Pass (no crash), one log silent. Stock CH v53.0.0 with the previous series and the new backend: the device comes up without a window; the guest says `virtio_nvrm: no host-visible window -- mmap will fail`, BAR 2 and the capability are absent, `nvidia-smi` works, `vecadd` fails with `cuInit` 100, the VM shuts down cleanly. The hypervisor log has no window line and no message; the backend log says nothing about the missing SHMEM either | `hypervisor-D.txt`, `backend-D.txt`, `guest-evidence.txt` |

### Found beyond the plan

A device reset while a guest process still holds window mappings ends the VM.
Reproduced twice (sessions A and C): `vecadd` held its context
(`VECADD_HOLD=25`), then `0000:00:05.0` was unbound from `virtio-pci`. At the
reset the hypervisor logged `VCPU generated error: VcpuRun(Failed to run vcpu
... VCPU error Error(14))` and exited; the backend logged `VMM hung up` and
exited. The same unbind and bind with no client running works. Error 14 is
`EFAULT` from `KVM_RUN`: the frontend replaced the mappings with the
`PROT_NONE` placeholder as the specification requires, and a guest thread
still touched a window page it had mapped. The previous series kept mappings
across a reset, so this did not happen there (inferred from the code, not run).
The trigger needs guest root. Options: the guest module zaps the user PTEs of
its window slots when the device goes away, or the frontend backs unmapped
window space with memory that does not fault. Module unload cannot trigger it,
because an open client holds the module.

The backend logs nothing when the frontend does not ack SHMEM. The `vhost-user-backend` 0.23
`VhostUserBackendMut` trait has no hook for the acked protocol features.
`get_shmem_config` is only called after SHMEM is acked, so if
`set_backend_req_fd` arrives with no earlier `get_shmem_config`, the backend
could log a warning.

The host kernel logged `NVRM: VM: nv_alloc_system_pages: failed to allocate
memory` with the matching `NV_ERR_NO_MEMORY` assertion about once for each CUDA
context the guests created, and every run passed. It does not involve the
window. No Xid was logged.
