<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Host clock boost (2026-09-28)

Scope: why the host card ran a VM's work at P8; what `NV2080_CTRL_CMD_PERF_BOOST` does in the RM; the device's activity-driven boost on branch `hostboost-2026-09-28` (`crates/vfio-user-nvgpu/src/hostboost.rs`). The user authorised this one host write on 2026-09-28 (about 22:45); every other clock, power or register control stays off limits. Background: video-path-2026-09-28.md, fix 1 (main checkout, not yet committed); [cuda-lookahead.md](../vfio/cuda-lookahead.md) C2, which had called this "possible, not recommended".

## Facts

### The gap

- The same NVENC/NVDEC probe was run twice, with host `nvidia-smi` sampling [caraxes/runs/night-2026-09-28/clocks-*.csv]:
  - native: P2, video 1305 MHz, memory 6801 MHz, PCIe gen 3; ENC 165 fps;
  - guest: P8 in 308 of 322 samples, video 540 MHz, memory 405 MHz, PCIe gen 1; ENC 90-98 fps.
- Natively, `cuCtxCreate` sends `PERF_BOOST` {flags 0x12 = BOOST_TO_MAX | CUDA, duration 0xffffffff}, and the teardown sends a clear [cuda-lookahead.md C2, X5].
- In a guest, the kernel passes that request on as `INTERNAL_PERF_BOOST_SET_2X` 0x20800a9a {NvBool flags; NvU32 duration}. `flags` keeps the low byte of the public flags [615 tree: src/nvidia/src/kernel/gpu/perf/kern_perf_boost.c:100-122; src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080internal.h:1697-1705].
- Before this branch the device acknowledged 0x20800a9a and boosted nothing (cuda.rs `ACKNOWLEDGED`).

### What the RM does with PERF_BOOST

The verified sources are the 615.71.09 tree, with 610.57.04 (the Caraxes vendor copy) where it differs.

- **Params:** {NvU32 flags; NvU32 duration}, 8 bytes, the same in both versions [615 tree: src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080perf.h:103-106; 610: :97-102]. `nvrm-sys` does not bind ctrl2080perf.h, so the device lays the struct out by hand and pins it in a test.
- **Flags:** CMD 1:0 (CLEAR 0, BOOST_1LEVEL 1, BOOST_TO_MAX 2), CUDA 4:4, ASYNC 5:5, CUDA_PRIORITY 6:6, MULTIMEDIA 7:7. MULTIMEDIA exists in 615 only [615 ctrl2080perf.h:75-94; 610 :75-90]. BOOST_1LEVEL is deprecated [615 tree: src/nvidia/inc/kernel/gpu/perf/kern_perf_boost.h:65].
- **Duration:** in seconds, below DURATION_MAX 3600; 0xffffffff means "until cleared" [ctrl2080perf.h:66-67, :96-97]. The RM has no sub-second boost.
- **Privilege:** the export flags are 0x10318 = NON_PRIVILEGED \| GPU_LOCK_DEVICE_ONLY \| API_LOCK_READONLY \| ROUTE_TO_VGPU_HOST \| GSP_PLUGIN_FOR_VGPU_GSP [615 tree: src/nvidia/generated/g_subdevice_nvoc.c:11192-11204, 610 :11032; bit values src/nvidia/inc/kernel/rmapi/control.h:205-287].
  - An unprivileged client passes the privilege check [615 tree: src/nvidia/src/kernel/rmapi/control.c:686-712].
  - CUDA is a field, not a privilege: nothing in the kernel path checks it [kern_perf_boost.c:42-94].
- **Validation:** on a dGPU the kernel checks nothing and forwards {flags, duration} to GSP [kern_perf_boost.c:90-122]. Only the Tegra iGPU path shows rules:
  - a finite duration above 3600 is INVALID_ARGUMENT;
  - an infinite one is cut to 1 s, because CUDA never clears (Bug 5397272) [kern_perf_boost.c:53-87].
- **Several clients:** "Boosts from different clients are being tracked independently" [ctrl2080perf.h:56-58]. The hint model has up to 512 hints [kern_perf_boost.h:35-40, :43-79]:
  - one per hClient, each with its own duration and flags;
  - index 0 is shared by all CUDA clients, with a refCount and `bIsInfinite`.
  - No open kernel code uses the struct. The arbitration runs in GSP-RM, which is closed.
- **A new boost while one is active:** the slot holds one duration and one flags word per client, so a client's new request replaces its old one. This is inferred from the struct, not from executable code.
  - The vGPU 3X variant says an infinite boost is overridden by "a next maximum duration registered" [ctrl2080internal.h:1907-1944]. So within one kernel RM the longest boost wins.
- **Client free:** the open kernel RM clears no PERF_BOOST when a subdevice or client is freed. Its only boost cleanup at subdevice destruct is the dynamic boost limit of nvidia-powerd [615 tree: src/nvidia/src/kernel/gpu/subdevice/subdevice.c:136-170].
  - Whether GSP-RM drops a freed client's hint is not visible. With finite durations it does not matter.

## Consequence for Caraxes

### Design (`src/hostboost.rs`)

| constant | value | why |
|---|---|---|
| duration | 3 s | whole seconds only; outlives two renewal intervals, so one late renewal never drops the clocks |
| renewal | at most every 1 s while active | at most 1 RM call per second |
| idle | 1 s without activity: no more renewals | the boost lapses 2-4 s after the last activity |
| backoff | 5 s, doubling to 300 s | a refusal or ioctl failure is logged once and counted |

- **Mechanism:** the `host-boost` thread sends `PERF_BOOST` {BOOST_TO_MAX, 3 s} on the subdevice of its own unprivileged host RM client (`HostRmClient::open`).
  - With a client of its own, the thread never waits on the device mutex, and its boost has its own hint slot.
  - Nothing is cleared at exit; a crashed device leaves at most 3 s of boost behind.
- **Activity sources:**
  - every guest doorbell: `Dispatcher::doorbell_chid`, the end of every route (trapped, token pool, token-less eventfd, usermode page, shadow USERD poll);
  - every host non-stall event of the VM's channels (mirror.rs `event_loop`);
  - the guest's own `SET_2X`.
- **Hot path:** the ring and event paths never make the RM call and take no lock:
  - off, or while a mark is set already: one relaxed load;
  - the first activity after each look of the worker: one compare-exchange;
  - the first activity after the worker parked: one unpark.
- **The guest's request:**
  - it keeps its command, and MULTIMEDIA on a 615 host (`HostLayouts::perf_boost_flags`);
  - it loses CUDA, CUDA_PRIORITY and ASYNC: renewals must not touch the shared, reference-counted CUDA hint that host CUDA clients hold;
  - the duration is always 3 s, never the guest's infinite one;
  - a clear is not forwarded and does not count as activity; later renewals go back to BOOST_TO_MAX;
  - the guest still gets NV_OK, as before.
  - This deviates from "forward the guest's flags" only in the CUDA bits.
- **Configuration:**
  - on by default with `--host-exec`; `CARAXES_HOST_BOOST=0` turns it off;
  - no other switch: the constants are code.
- **Log:**
  - one start-up line;
  - the first renewal (time and RM call time) and the first refusal;
  - at exit: `host boost: N renewals (M from guest requests), K refused, first at +T s; RM call mean X us, max Y us over C call(s)`.
  - `first at` counts from the device's start.
- **Tests (fake clock):**
  - renewal at most once per interval;
  - idle stops the renewals;
  - the first activity after idle boosts at once and wakes the parked worker;
  - the guest's infinite CUDA request is sent as {0x2, 3};
  - backoff 5/10 s with one log line;
  - the params layout from both bindings.

### Isolation

- A boost raises the clocks of the whole card. Other tenants (the host desktop, other VMs) can only gain; the cost is power and heat.
- The finite duration bounds the boost to the VM's active periods plus 2-4 s.
- A guest can hold the boost only by ringing doorbells or making its engines raise events, at one RM call per second.
- Parallel VMs each hold their own client and slot, and the slots are tracked independently.

### What to measure (A/B on the same probe)

- Run the NVENC/NVDEC probe twice: with the default (boost on), then with `CARAXES_HOST_BOOST=0`.
- Sample on the host throughout: `nvidia-smi --query-gpu=timestamp,pstate,clocks.sm,clocks.video,clocks.mem,pcie.link.gen.current -lms 100`.
- Measure ENC and DEC fps and a short-kernel launch latency (`CARAXES_LAUNCH_TIMING=1`); `HostToNext` and `ToHost` must stay flat.
- Keep the device's `host boost:` lines.
- Expected with the boost on:
  - P0 or P2 and PCIe gen 3 while the probe runs;
  - P8 again 2-4 s after it ends;
  - ENC about 150-160 fps.

## Open

- **GSP-side semantics are inferred:** replace or extend, and the free of a client's hint. The A/B sampling shows the lapse; the exit line's call count shows the RM call rate.
- **The CUDA limit:** without the CUDA bit the boost is not a CUDA boost. The card may go to P0 instead of the P2 CUDA gets natively. Watch power and memory clock.
- **Long kernels:** a busy stretch longer than 2-4 s with no doorbell and no event relies on the card's own utilisation governor after the boost lapses.
- **RM call cost:** if the P-state switch runs synchronously in GSP, the call takes longer. The exit line's max shows it; the call is off the ring path either way.
