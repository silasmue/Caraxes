<!-- SPDX-License-Identifier: MIT -->
# vendor/vfio-user-rs: vfio_user 0.1.6 with VFIO_USER_DEVICE_GET_REGION_IO_FDS

This directory is the `vfio_user` crate as published on crates.io, version 0.1.6
(`vfio_user-0.1.6.crate`, sha256
`ea835ab081b29468de55d5066532fc19c99a3ed04b57ea66dfea40559b651070`), built from
rust-vmm/vfio at `a38ee7a3eeb542624dd007e6b005e0de4fc59582` ("vfio-user: Prepare the
0.1.6 release", path `vfio-user`; the crate metadata names
`https://github.com/rust-vmm/vfio-user` as the repository, the crate lives in
`https://github.com/rust-vmm/vfio`). The licence files `LICENSE-APACHE` and
`LICENSE-BSD-3-Clause`, `README.md`, `Cargo.toml.orig` and `examples/` are the
package's, unchanged. The crate's own licence is Apache-2.0 (`Cargo.toml`), with the
BSD-3-Clause file shipped by upstream.

Three changes, each available as a patch:

| Change | Files | Patch | Upstream |
|---|---|---|---|
| `VFIO_USER_DEVICE_GET_REGION_IO_FDS`: `Client::get_region_io_fds()`, `parse_region_io_fds_reply()`, `Server::add_ioeventfd()` and the server's reply; tests | `src/lib.rs`, `CHANGELOG.md` | [../../patches/vfio-user-rs/0001-vfio-user-support-VFIO_USER_DEVICE_GET_REGION_IO_FDS.patch](../../patches/vfio-user-rs/0001-vfio-user-support-VFIO_USER_DEVICE_GET_REGION_IO_FDS.patch), `git format-patch` against `a38ee7a3eeb5` (paths `vfio-user/...`) | candidate for rust-vmm/vfio |
| One eventfd per ioeventfd sub-region: the client announces `max_msg_fds` 253 (`SCM_MAX_FD`) and receives that many; the server sends at most 253; `add_ioeventfd()` accepts datamatch sub-regions at one offset and refuses what KVM would refuse as a duplicate; tests | `src/lib.rs`, `CHANGELOG.md` | [../../patches/vfio-user-rs/0002-vfio-user-one-eventfd-per-ioeventfd-sub-region.patch](../../patches/vfio-user-rs/0002-vfio-user-one-eventfd-per-ioeventfd-sub-region.patch), on top of 0001 | with 0001, as one change |
| `vfio-bindings` requirement `=0.6.3` widened to `>=0.6.2, <=0.6.3` | `Cargo.toml` | the diff below | local only |

An additional local diagnostic adds `Server::run_with_idle_poll()` with `IdlePollMode`
(`Off`, bounded `Hybrid`, or `Spin`). The default `run()` retains its blocking wait;
the opt-in mode peeks for header readiness before that receive and may consume a CPU
while idle. The server now completes short headers with `read_exact()` and owns any
received `SCM_RIGHTS` descriptors before that fallible read. These changes are local
to this vendored copy and are not represented by the upstream-candidate patches above.

The local client also fixes an inverted `VFIO_DEVICE_FLAGS_RESET` test in
`get_regions()`. `Client::resettable()` now follows the advertised flag. A socketpair
regression checks real `DEVICE_GET_INFO` replies with and without RESET support.

```diff
 [dependencies.vfio-bindings]
-version = "=0.6.3"
+version = ">=0.6.2, <=0.6.3"
```

Why the second change: Cloud Hypervisor v53.0 pins `vfio-ioctls` 0.7.0, which pins
`vfio-bindings` `=0.6.2`, and passes `vfio_bindings` types between its own code and
`vfio_user`. With `=0.6.3` the two would not unify. 0.6.3 differs from 0.6.2 only by
a `pasid` field in `vfio_device_attach_iommufd_pt` and `vfio_device_detach_iommufd_pt`,
which this crate does not use. The Leandro workspace resolves 0.6.3, Cloud Hypervisor
0.6.2; both build.

## Who uses it

- The Leandro workspace: `[patch.crates-io] vfio_user = { path = "vendor/vfio-user-rs" }`
  in `../../Cargo.toml`. `crates/vfio-user-nvgpu` declares the doorbell sub-region with
  `Server::add_ioeventfd()` when `CARAXES_DOORBELL_IOEVENTFD=1`.
- Cloud Hypervisor with `patches/0005-vfio-user-region-io-fds.patch`, which adds
  `[patch.crates-io] vfio_user = { path = "../vfio-user-rs" }`: the tree
  `vendor/cloud-hypervisor` finds this directory beside it. A checkout elsewhere needs
  `../vfio-user-rs` to point here (a symlink does).

## What the change implements

The protocol, the byte layouts, the spec citations and the deviations of libvfio-user
are in [../../patches/README.md](../../patches/README.md), "Patch 0005". In short:

- Client: the request (argsz, flags 0, index, count 0), the reply with its `SCM_RIGHTS`
  descriptors, one retry with the argsz a truncated reply names, and a parser that
  accepts only the specification's layout: argsz = payload size = 16 + count * 40,
  flags 0, the requested index, 40-byte entries of type 0 (ioeventfd, datamatch at 32)
  or 1 (ioregionfd, user_data at 32). Anything else is `Error::InvalidReply` with the
  stream still in step. A libvfio-user reply (48-byte ioeventfd entries, datamatch at
  40) is refused, not misread.
- An error reply is `Error::ErrorReply`. Servers of this crate up to 0.1.6 answer the
  request before reading its 16-byte payload and then answer that payload as a
  message of command 0; the client drops that one stray header-only error reply
  behind a `VFIO_USER_DEVICE_GET_INFO` round trip, so the connection stays usable.
- Server: `add_ioeventfd(region, IoEventFdSubRegion { offset, size, flags, datamatch,
  fd })` declares a sub-region; the request is answered with 40-byte ioeventfd entries,
  at most as many distinct descriptors as the client's `max_msg_fds` (1 when the client
  does not say), and the fixed part alone when the reply exceeds the request's argsz.

## Tests

`cargo test -p vfio_user` in a checkout of rust-vmm/vfio at `a38ee7a3eeb5` with the
patch applied: 10 passed (7 new), `cargo clippy -p vfio_user --all-targets -D warnings`
clean. With patch 0002 as well (`/mnt/vmstore/caraxes/vfio-rs-0006`): 14 passed (4 new),
clippy and rustfmt clean. This directory is not a workspace member, so its tests
require an isolated manifest. The local diagnostic and short-header tests are separate
from the upstream-candidate patch test counts above.
