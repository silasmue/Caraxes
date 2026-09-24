## Upcoming release

## Changed

- The client announces `max_msg_fds` 253 (`SCM_MAX_FD`, the most one Unix
  socket message carries) instead of leaving the capability out, and receives
  that many descriptors with a reply, so that a server can give every ioeventfd
  sub-region its own eventfd.

## Added

- `Server::add_ioeventfd()` accepts several sub-regions at one offset told
  apart by datamatch, checks that a datamatch fits the access size, and refuses
  a sub-region that `KVM_IOEVENTFD` would refuse as a duplicate. The server
  never sends more than `SCM_MAX_FD` descriptors in one reply.

- Support `VFIO_USER_DEVICE_GET_REGION_IO_FDS` with ioeventfd sub-regions:
  `Client::get_region_io_fds()` requests and parses the reply in the
  specification's layout, `Server::add_ioeventfd()` declares sub-regions and
  the server answers the request, sending no more file descriptors per reply
  than the client's `max_msg_fds`.

## Fixed

# [v0.1.6]

## Changed

- Bump `vfio-bindings` to 0.6.3.

# [v0.1.5]

## Added

- [[160]](https://github.com/rust-vmm/vfio/pull/160) construct vfio-user server from owned fd

# [v0.1.4]

## Changed

- [[163]](https://github.com/rust-vmm/vfio/pull/163) Support vm-memory 0.18

# [v0.1.3]

## Changed

- [[128]](https://github.com/rust-vmm/vfio/pull/128) Support vm-memory 0.17

## Added

- [[132]](https://github.com/rust-vmm/vfio/pull/132) Implement sparse mmap capability for Server

## Fixed

- [[134]](https://github.com/rust-vmm/vfio/pull/134) Fix incorrect spec implementation for sparse mmap areas

# [v0.1.2]

### Changed
- [[114]](https://github.com/rust-vmm/vfio/pull/114) Cargo.toml: Update deps to latest version

# [v0.1.1]

## Changed
- Bumped vfio-bindings to 0.6.0

# [v0.1.0]

This is the first `vfio-user` crate release.

This crate provides the client and server support for implementing vfio-user devices.

`vfio-user` is now merged into a single monorepo `vfio`.
