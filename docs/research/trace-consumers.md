<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Trace consumers

Without an event sink, `Trace::event/events` skip output formatting and bookkeeping. `Device::after_write` drains boot notes before checking the sink; resets, pending MSI delivery and enable publication continue. Polled USERD rings execute before their trace gate. Access counters remain active.

With a sink, event order, immediate single-event flush, batch flush, access filtering and the free-space guard retain their behavior. Producer note allocation and shared error/RC queues are unchanged.

Validation: 924 unit tests passed, one ignored; both integration tests passed. Focused regressions cover output flushing, absent/stopped sinks, counters, note draining, FLR and MSI enable/disable. Independent source review found no blockers. Existing private USERD synchronization regression passed; this is not a hardware publication test.

Private qualification: `/mnt/vmstore/caraxes/windows-3dmark-2026-10-04/C167-trace-final-qualification.json`. Windows performance requires a separate comparison with VM boost policy fixed. No FPS improvement is established by the source change.
