<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Finite host boost policy

With `--host-exec`, `CARAXES_HOST_BOOST_POLICY=vm` is the default. It requests `PERF_BOOST {flags=BOOST_TO_MAX (2), duration=3 seconds}` immediately when the separate host-boost worker starts, then renews at most every second throughout the **backend process lifetime**. The backend may start before a VM connects, and desktop idle does not suspend these requests. This policy does not detect whether the VM is currently rendering.

`CARAXES_HOST_BOOST_POLICY=activity` selects the prior activity-based behavior: guest doorbells, owned mirrored-channel non-stall events and guest boost requests cause renewals; a quiet interval stops them. This mode preserves the guest command, including BOOST_1LEVEL, and host-supported MULTIMEDIA flags. VM policy always uses flags exactly 2, regardless of guest downgrade, CLEAR or optional flags. Both policies discard guest duration, CUDA, CUDA_PRIORITY and ASYNC; neither uses an infinite or shared CUDA hint.

The policy accepts only the canonical strings `vm` and `activity`; unset means `vm`. Empty, whitespace, non-UTF-8 and other values fail at the start of `serve`, before profile threads or host RM/VRAM allocation. Argument parsing, privilege reduction and logger setup precede `serve`. Without host execution the selected policy is reported inactive and no boost worker starts. The environment registry recognizes this supported policy. Startup, normal exit and handled signal exit report the chosen policy, including startup failure.

Refusals back off 5, 10, 20…300 seconds. The retry pause now starts at the actual post-control completion instant for **both** policies, fixing an inherited slow-ioctl edge; successful renewal cadence stays unchanged. Worker calls remain outside the Device mutex. The optional counters count accepted requests and included guest requests, not achieved clocks.

These are P-state hints, not a global MHz lock. NVIDIA 615 `ctrl2080perf.h:53–106,835–846` permits other P-state limits; dynamic frequency, temperature and power policy remain driver governed. No frequency, memory clock, voltage, power limit, fan or GPU command rewrite is added. The whole physical GPU is affected, including host work or other guests. Keeping a higher P-state through idle can cost power and heat.

Termination stops further renewals. Every accepted request asks to expire within 3 seconds of that request; driver-side expiry remains a runtime qualification requirement, and other clients may keep the GPU boosted afterward. There is no permanent/infinite request left by this policy.

Source-only implementation and deterministic tests do not prove hardware behavior or a score improvement. Root separately observed actual 1830–1860 MHz physical boost during old C164 rendering; the new policy addresses lease and guest-command consistency rather than establishing that old rendering never boosted. Root owns build/tests, new-policy clock sampling, short image-correctness gate, health checks and ordinary benchmark comparison. No new-policy hardware qualification is claimed here.

Primary local NVIDIA 615 checkout: `/mnt/vmstore/caraxes/ogkm-615.71.09`, commit `61dcc93722ecb418bb5f2e00923f05b4b8051dd1`. Historical activity design: [host-boost-2026-09-28.md](host-boost-2026-09-28.md). Implementation: `crates/vfio-user-nvgpu/src/hostboost.rs`, early policy/startup wiring: `src/main.rs`, registry: `src/settings.rs`.
