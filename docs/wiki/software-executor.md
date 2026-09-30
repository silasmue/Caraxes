<!-- SPDX-License-Identifier: MIT -->
# The software executor

The software executor is the device's own interpreter of a guest channel: it reads the
GPFIFO entries up to USERD GPPut and executes their methods on the CPU (copies, fills,
remaps, semaphores, software methods, TLB invalidates). It serves the guest channels that
must not run on a host channel, above all the guest kernel's privileged copy channels. Every
other channel runs natively on a host mirror channel ([execution-mirror](execution-mirror.md)).
Code: `crates/vfio-user-nvgpu/src/channel.rs`.

## Where a channel runs

The rule is `Mirror::eligible` and `Mirror::is_gr_shadow` in `mirror.rs`. A guest
channel's privilege never becomes host privilege: every host mirror is allocated with
`NVOS04_FLAGS_CHANNEL_DENY_PHYSICAL_MODE_CE` (`HOST_MIRROR_CHANNEL_FLAGS`), so a
physical-mode CE method there faults that channel only.

| Guest channel | Runs on | Rule |
|---|---|---|
| GR channel in a TSG, or carrying `TURING_COMPUTE_A` | host mirror on a GR shadow VA space | `is_gr_shadow` |
| bare GR channel (no TSG: the kernel's golden-context and WAR channels, nouveau's fence channel) | software | "bare channel (no TSG)" |
| copy channel of a process, bare or in a TSG | host mirror | default with `--host-exec` |
| privileged copy channel of the guest kernel (KERNEL level: the Linux RM's scrubs and page-table writes, the Windows KMD's paging and scrub channels in the system VA space) | software | `privileged_stays_in_software` |
| privileged copy channel of an application: Windows (its root is also a skip-context GR channel's), or an admin client (a root process, `internalFlags` PRIVILEGE ADMIN) | host mirror, unprivileged | `application_copy_channel`, `admin_application_channel` |
| NVENC or NVDEC channel, privileged or not | host mirror in a host TSG of its own | the chip's video classes only ([video-engines](video-engines.md)) |
| UVM channel (`GP100_UVM_SW` on subchannel 5; Windows UVMLite chids 34 to 51) | software | the host refuses the class to an unprivileged client (Xid 32 when tried) |
| Windows management channel (deferred API) | software | [channel-migration-and-deferred-api](channel-migration-and-deferred-api.md) |

- The Windows KMD sets PRIVILEGED on every channel (`kernel_channel.c`), so without the
  application rule CUDA's copies ran in software: HtoD 0.20 GB/s, DtoH 0.07 GB/s, and a
  64 MiB read-back held the device 6.7 s (P29). Mirrored: 12.9 GB/s both ways (P30)
  ([research: windows-findings](../research/windows-findings.md)).
- A root CUDA process on Linux is an admin client: its twelve copy channels ran in
  software at about 1.1 ms per push. Mirrored: root `cuCtxCreate` 373 to 271 ms, d2h 16 MiB
  52.3 to 1.34 ms against 1.31 native
  ([research: ctxcreate](../research/ctxcreate-2026-09-29.md)).

## What it executes

- GPFIFO entries in order, pushbuffer methods, `SET_OBJECT` bindings, semaphore acquire and
  release, reports, `SET_REFERENCE`, the 0x5080 software method, `MEM_OP` TLB invalidates
  (handed to the invalidator).
- The copy engine's `LAUNCH_DMA` in VA and physical mode (`channel.rs` `launch_dma`). Every
  piece must lie in the VM's reservation below the device's private 512 KiB or in
  registered guest RAM (`ce_pieces`); anything else is refused.
- Remaps with `SET_REMAP_COMPONENTS`, modelled in full (d02dc84d). Before, a remap with a
  NO_WRITE component was refused, the guest got RC Xid 45 and the KMD reset the GPU in
  Heaven and Superposition; with the model C91 ran 210 remaps with no RC and no FLR
  ([research: windows-findings](../research/windows-findings.md), step 21).
- A blocked semaphore acquire is re-read by the 1 ms slice clock (4f925b23): the KMD's paging
  channel waited 2.5 s at boot instead of 8.3 s
  ([research: windows-findings](../research/windows-findings.md), step 19, C82).
- Bounded runs: a run stops between entries after 2 ms and the slice clock continues it after
  50 us; a CPU copy slice is 4 ms ([caraxes/execution.md](../vfio/execution.md) §9.7).

## The device CE

- Large transfers go to a device-owned host copy channel (`devce.rs` `DeviceCe`, user level
  on COPY2): fills of at least 64 KiB of plain VRAM, and copies of 1 KiB or more that read
  VRAM. Memsets and copies from guest RAM stay on the CPU
  ([research: device-ce](../research/device-ce-2026-09-29.md), execution.md §9.7).
- Its VA space maps exactly the guest's usable FB at 4 GiB with 2 MiB PTEs; its tables sit
  in the private pages `[E - 512 KiB, E - 256 KiB)`. No guest method reaches the host.
- One push at a time; the guest's semaphore follows the device CE's; the FB store drops its
  caches for the written ranges. A push not done in 2 s turns the engine off for good and
  the transfer is redone on the CPU. Default on, no switch.

| Windows workload | CPU executor (P31, P32) | device CE (P34) |
|---|---|---|
| the KMD's paging fills | 19.7 GiB in 75 s of CPU, 1804 slices | 29251 MiB in 1.6 s of GPU time, 981 pushes |
| transformer first step | 17.7 s | 12.4 s |
| ResNet-18 first step | 98 to 121 s | 61.8 s |

## Where each side costs

| Cost | Value | Record |
|---|---|---|
| a software push of a Linux root process | about 1.1 ms of device time | [ctxcreate](../research/ctxcreate-2026-09-29.md) |
| CPU reads of VRAM through the BAR1 windows | about 10 MB/s | [windows-findings](../research/windows-findings.md) P29 |
| Windows software doorbells under the device lock | 7.9 s in all, 1.7 s of it translating (C70) | [lockfree](../research/lockfree-2026-09-29.md) |
| host RM calls of one GR mirror build | about 10 ms | [gr-build](../research/gr-build-2026-09-29.md) |
| Windows GR and CE shadow builds | mean 82 ms, max 198 ms (C86) | [windows-findings](../research/windows-findings.md) step 20 |
| Linux CUDA context builds | GR 3.3 to 3.6 ms, CE 1.2 to 1.4 ms each | [ctxcreate](../research/ctxcreate-2026-09-29.md) |

- The executor's cost was translation: 4 to 7 uncached table words per translation, through
  two BAR1 windows that evicted each other. `FakeFb::translate_pages` (one walk per 2 MiB
  unit) and the executor TLB (`fb.rs` `ExecTlb`, flushed by table writes and every
  invalidate) removed most of it (execution.md §9.6, §9.7; lockfree, C67 against C70).
- A host mirror's cost is its build, once per channel; its work then runs at native speed.

## Depth

- [research/device-ce-2026-09-29.md](../research/device-ce-2026-09-29.md): why the paging
  channel is not split, the device CE's design and numbers.
- [research/windows-findings.md](../research/windows-findings.md): P29 to P34, steps 12, 19, 21.
- [caraxes/execution.md](../vfio/execution.md) §9.6, §9.7: translations, the TLB, bounded runs.
