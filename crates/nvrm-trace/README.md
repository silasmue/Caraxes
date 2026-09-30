<!-- SPDX-License-Identifier: MIT -->
# nvrm-trace

Shared: an `LD_PRELOAD` tracer, a measuring instrument, not a data path.

- Interposes open, close, dup, ioctl, mmap, read and poll on NVIDIA device nodes, event FDs
  and selected DRM calls, in any process on the host or in a guest, and logs each call.
- Files: `src/lib.rs` (the hooks), `src/fdtable.rs` (FD tracking), `src/log.rs` (records and
  formats).
- Use: `cargo build --release -p nvrm-trace`, then
  `LEA_TRACE_FILE=out.tsv LD_PRELOAD=target/release/libnvrm_trace.so <program>`.
- Docs: [reference](reference.md) (formats, variables, limits),
  [tracing in development](../../DEVELOPMENT.md#trace-calls).
