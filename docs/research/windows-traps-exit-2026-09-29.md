<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Windows without the doorbell trap: QEMU io fds, the usermode page, Cloud Hypervisor (2026-09-29)

Scope: why a Windows CUDA short kernel costs 54-60 us (Linux 11 us, native 6 us), and three ways
out: (b) ioeventfds from QEMU, (d) the usermode page as memory, (e) Windows under Cloud Hypervisor.

## Facts

- Every Windows doorbell traps: P33 counted 0 reactor rings, host doorbell to host event p50
  23.6 us, event to raise 1.2 us [windows-findings.md P33].
- Short kernel medians (launch + synchronize): Windows 54.0-59.4 us (P34, P35), Linux 11 us on
  d13f8b72 with the token pool's ioeventfds, native 6 us [windows-findings.md P34/P35; the Linux
  compute series of 2026-09-29].
- Trap census of one CUDA probe process (P36, `CARAXES_PROFILE=1`, SIGUSR2 dumps around it, 21 s:
  init, JIT, 2100 launches, 1100 synchronizes, bandwidth) [P36-gate/gpu-profile.1.tsv to .2.tsv,
  `profile-rank.py --base`]:

  | Class | Traps | Guest-seen ms | Per |
  |---|---|---|---|
  | BAR2 writes (about 4096 per 4 KiB page: byte stores) | 119508 | 1376 | context setup |
  | interrupt tree (`NV_VF_CPU_INTR_*`) | 17223 | 387 | 16.8 per interrupt |
  | `NV_PMC_INTR_EN` set/clear (0x160/0x180) | 10284 | 370 | 10.0 per interrupt |
  | usermode doorbell (0xbb0090) | 2839 | 110 | 1.3 per launch |
  | MSI re-arm (`NV_PCFG` 0x88704) | 1026 | 46 | 1 per interrupt |
  | MMU invalidate, GSP queue and falcon | 1666 | 120 | RPCs, maps |
  | usermode `TIME_0/1` (0xbb0080/84) | 0 | 0 | - |

- About 1026 interrupts in 21 s, 28 traps each (0.78 ms guest-seen per interrupt); PTIMER was read
  3 times, the usermode TIME never.
- QEMU 11.1.1's vfio-user client maps the sparse areas a region offers (the BAR1 pool runs that
  way under QEMU [docs/vfio/zero-based-fb.md]), but has no `VFIO_USER_DEVICE_GET_REGION_IO_FDS`:
  no ioeventfd for a vfio-user region [its 14 vfio-user trace events (`-trace help`) and its
  strings have no io-fd message].
- Cloud Hypervisor with patches 0005/0006 asks for the io fds and registers the token pool's
  datamatch ioeventfds with KVM: a ring is one KVM exit ending in an eventfd [patches/README.md
  "Patch 0005", "Patch 0006"; docs/vfio/doorbell-token-pool.md].
- The zero-based FB (Windows) refuses the token-less wildcard doorbell ioeventfd; the token pool
  (one datamatch per token) is allowed [main.rs `CARAXES_FB_ZERO_BASED` checks].

## (b) ioeventfds from QEMU

- Needs a QEMU patch: at realize, send `GET_REGION_IO_FDS` per region and add each sub-region as
  `memory_region_add_eventfd` (datamatch when the entry has one). The device side exists (the
  vendored crate's patch 0002, used under Cloud Hypervisor).
- Cost: a patched QEMU on the host, a new component to carry; the guest stays stock.
- Gain: the doorbell becomes one KVM exit (1.2-1.4 us) instead of a vfio-user message; the TIME
  reads on the same page still trap.

## (d) The usermode page as memory under QEMU

- The page 0xbb0000 (TIME_0/1, NOTIFY_CHANNEL_PENDING) as a sparse area of the interrupt page's
  memfd, owned by the reactor (um_page.rs, `CARAXES_USERMODE_PAGE=1`; needs `CARAXES_INTR_PAGE=1`,
  never run with Windows). QEMU maps it without a patch.
- Doorbell: a write lands in memory; the reactor swaps the word to its sentinel and scans every
  known channel's GPPut (ctrl.rs `doorbell_pending`). Detection latency is the reactor's poll:
  a spinning reactor about 1 us, the hybrid reactor up to its 1 ms sleep bound.
- O(changed) scan: `doorbell_pending` reads every recorded channel's GPPut per doorbell (Windows
  records over 50 channels: guest chids up to 51, UVMLite alone 34-51 [device-ce-2026-09-29.md]).
  On the critical path only the hinted channel:
  the word's value is the channel's token, rung at once if its GPPut advanced; the full scan runs
  later, at most every 100 us, and the watchdog (10 ms) stays as the last net for a coalesced
  write. A wrong hint costs a wasted read, never a wrong ring (GPPut decides).
- Bounded TIME rate: the reactor stores TIME at every iteration; spinning, that is one 64-bit
  store per loop and a cache line bouncing with every guest reader. A bound (one store per
  microsecond at most) keeps readers within 1 us of the device clock and the time monotonic.
- TIME: the CUDA census has no usermode TIME read, so the page gains Windows compute nothing
  there; the bound matters only for the reactor's CPU and cache traffic. Desktop and D3D are
  not censused yet (Linux GL/Vulkan read TIME about 925 times per second under Mutter).
- Gain: the doorbell only (1.3 traps per launch). The two larger trap classes of the census
  have their own memory windows, also mappable by QEMU without a patch: the interrupt page
  (`CARAXES_INTR_PAGE=1`, the 17 tree traps per interrupt) and the BAR2 memfd
  (`CARAXES_BAR2_MEMFD=1`, the byte stores of context setup: 1.4 s per CUDA process here).
  Neither has run with Windows.
- Risks: a missed doorbell costs up to the watchdog (10 ms); the reactor's CPU (one core
  spinning, or up to 1 ms latency hybrid); the page cannot be split, so any other register the
  guest touches there must be modelled in memory too (`UmPage::check` refuses otherwise).

## (e) Windows under Cloud Hypervisor: the blockers

- Firmware: done. `CLOUDHV.fd` (edk2-stable202508, CloudHvX64, Secure Boot and TPM2 on) booted
  the installed image headless on 2026-09-25; QEMU's OVMF does not run under Cloud Hypervisor
  [caraxes/runs/win11-2026-09-25/README.md "Firmware build"].
- TPM: done. `--tpm socket=` to the same swtpm state as QEMU (`up-ch.sh`); TPM-bound state
  survives the move, Windows may ask for re-activation [same README, "Setup notes"].
- Display: Cloud Hypervisor has no VGA, no GOP and no keyboard. Nothing is visible before the
  NVIDIA driver loads; then the device's display engine is the only display (Sunshine/Moonlight
  as on Linux). The installer and WinRE repairs (the P27 DISM repair over QMP send-key) stay on
  QEMU with the same disk.
- Tooling: the run kits talk to the guest through the QEMU guest agent (virtio-serial named
  port) and QEMU's user network (10.0.2.2 file server, RDP hostfwd). Under Cloud Hypervisor: tap
  plus NAT (`up-ch.sh` has it; NAT rules removed at the end) and SSH or WinRM in the guest
  instead of QGA. This is the largest piece of work.
- Hyper-V enlightenments: QEMU passes `hv_relaxed,hv_vapic,hv_spinlocks,hv_time`; Cloud
  Hypervisor needs `--cpus kvm_hyperv=on` (not set in `up-ch.sh`).
- Device: Windows needs GPA mode (`CARAXES_GUEST_DMA_GPA=1`, no guest module); under Cloud
  Hypervisor without a vIOMMU DMA addresses are guest physical as under QEMU, and the registrar
  takes whole memory zones as pieces. Untested with the GPU.
- Gain: the token pool's ioeventfds (Linux short kernel 11 us), the interrupt page and the
  display kick page as on Linux; one hypervisor for both guests.

## Consequence for Caraxes

- Order under QEMU, each a measured A/B on Windows (short kernel, PyTorch first step, smoke):
  1. BAR2 memfd: the largest trap class, one switch, known on Linux;
  2. interrupt page: 27 of the 28 traps per interrupt (the tree and BAR0 page 0 with
     `PMC_INTR_EN`);
  3. the usermode page with a spinning reactor and the O(changed) scan: the doorbell.
- (b) is the cheaper doorbell if a patched QEMU is acceptable: one KVM exit, no spinning core.
- (e) brings all Linux windows at once (token pool, re-arm ioeventfd, interrupt and kick pages)
  but needs the guest tooling moved to SSH; worth it once the QEMU-only windows are measured.

## Open

- A census of the Windows desktop and a D3D workload (TIME reads, present path).
- (Corrected 2026-09-29: `PMC_INTR_EN` lies in BAR0 page 0, which the interrupt page maps too.)
- (Fixed in c6e651b1: the host-event histogram now times from the oldest unanswered host
  doorbell, and trapped doorbells are rings of the chain.)

## P37: the A/B under QEMU (2026-09-29, windows-findings.md "P37")

- Arm 1 (BAR2 memfd): context setup twice as fast, steady-state rates 40 % lower.
- Arm 2 (plus the interrupt page): TDRs from the first D3D11 device; not usable on Windows yet.
- The usermode page needs the interrupt page's memfd and reactor: blocked with it.
