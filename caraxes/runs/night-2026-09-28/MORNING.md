<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Morning summary, night of 2026-09-28/29

Evidence: [README.md](README.md) (all tables, commands, run folders). dev is not pushed.

## Stable device for all three guest types (your priority)

One binary on dev (b2e0d749 + docs): the guest's ABI is detected at the GSP boot. Gated on the same head 8e621e0a:
- **nova** (Linux 7.2.7): ABI 570 detected, 5 probes, bind/unbind/reload, 0 refused.
- **Linux, NVIDIA open 615:** smoke, compute series and scrub stress; 0 Xid.
- **Windows 11 / 617.14:** D3D11, D3D12, Vulkan, CUDA, managed memory, NVENC/NVDEC, smoke ALL PASS; 0 Xid, 0 TDR.
  - **Caveat:** every Windows run binary (P28-P38) carried the P17 diagnostic build patch, which binds owned GFXP buffers on the host (host vidmem, outside the VM's slice). dev itself has no such bind.
  - **P39 confirmed it (05:00): the plain dev binary does not start the Windows driver at all (Code 43).** dev refuses the KMD's GR_GFX_POOL_QUERY_SIZE, and the KMD unloads.
  - **Fixed in P41 (06:00), with no diagnostic patch:** the pool-query answer, the captured TU106 display caps (for every guest), and compute preemption parity. D3D11 and smoke ALL PASS, CUDA and NVENC work.
  - P43 (P41 without the unused GFXP slots) is the production candidate being gated on Linux, nova and Windows.

## Performance, Linux vs native (defaults, no extra switches)

| Workload | native | Linux guest | ratio |
|---|---|---|---|
| llama 1B pp512 / tg128 | 7639 / 305 | 8037-8565 / 282-300 | 1.05-1.12 / 0.93-0.98 |
| PyTorch transformer | 107485 tok/s | 109579-110248 | 1.02 |
| PyTorch ResNet-18 | 532.5 img/s | 511-553 | 0.96-1.04 |
| NVENC 1080p | 165-180 fps | 146-178 | 0.9-1.0 |
| NVDEC 1080p | 137-143 fps | 129-154 | 0.93-1.0 |
| short kernel | 6 µs | 11-13 µs | the one outlier |
| vkmark / glmark2 (4000+ FPS micro-benchmarks) | 15973 / 12214 | ~5600 / ~4300 | 0.35 (per-frame overhead; not the game metric) |

Windows lags: short kernel 59-76 µs, ResNet 0.66-0.7, NVENC 0.7-0.8. Cause: QEMU traps every doorbell (no ioeventfd).

Windows A/B (P37/P38):
- **BAR2 memfd:** context setup twice as fast (ResNet first step 63 → 30 s, NVENC init 4.8 → 0.6 s), but steady rates −40 %: the guest waits for the deferred flush. P38 tests sync-at-flush.
- **Interrupt page:** breaks Windows (TDRs); probably its tag bit 31, which the KMD reads as a pending vector. Not a Windows default.

Graphics: the vblank handler's display-page writes each ran an unrelated BAR1 pool merge (~110 µs). Fix 95f1af1 (branch vblank-2026-09-29) is built; the vkmark A/B follows after P38.

## What was wrong and is fixed on dev

- **Short kernel 40-48 µs → 11 µs:**
  - set B was recomputed twice per doorbell (20 µs);
  - now cached (8d9ab89c), plus LAUNCH_FAST=2 as the default (ca2559f5).
- **ResNet-18 crashed (host Xid 31):**
  - since 9ddc665c CUDA maps through nvidia-uvm, whose invalidates never held the shadow;
  - fixed by the MEM_OP hold (c8f9ce08).
- **NVENC/NVDEC 0.55×:**
  - the host card stayed in P8 while the VM worked;
  - fixed by the activity-driven host boost (78d4b0fd, your go). ENC 0.55 → 0.9-1.0.
- **UVM:**
  - a Linux guest is told "no UVM" (945431f9, your call), so managed memory fails cleanly instead of with a host Xid;
  - Windows keeps bit 1: managed memory works either way there, and bit 1 gives 4× pinned-copy bandwidth (A/B P31).
- **Windows:**
  - CUDA contexts (8-channel TSG) and TDR at every exit;
  - D3D12 and Vulkan;
  - CE channels mirrored (copies 0.2 → 12.9 GB/s), the video channels;
  - the software CE sliced so QEMU never times out;
  - guest-RAM registration off the loop.
- **Device CE:** allocation clears and scrubs run on a device-owned host CE confined to the VM's FB. 29 GiB in 1.6 s instead of 75 s of CPU; Linux scrub stress 28 → 12 s.
- **Security:**
  - the device's own channel tables and rings (top 512 KiB of the reservation) were reachable as guest memory through the software CE, the shadow, BAR1/BAR2 and PRAMIN;
  - now refused on every path (8e621e0a).
- **Defaults instead of switches:** host boost, LAUNCH_FAST=2, the display kick page (+11-26 % glmark2).

## SotTR at 720p (your instruction: no balloon yet)

- It does not help: the game fills VRAM by its budget, not by the resolution, and the NVKMS refusal came at 3883/4096 MiB again.
- At 720p the benchmark kept rendering afterwards (GPU 88 %), with the stream frozen. ~60 s later the host logged **Xid 109 CTX SWITCH TIMEOUT** on the game's GR channel, then device lost and the game aborted.
- New stability item: the host context probably runs without graphics preemption (GFXP), so long draws cannot be switched out. Analysis: docs/research/gr-preemption-2026-09-29.md, being written.
- Evidence and screenshots: `sottr720-8e621e0a/`.

## Decisions for you

1. **SotTR is blocked on Linux by the stock driver's NVKMS refusal at full VRAM.**
   - Reproduced with screenshots: the picture freezes at 3884/4096 MiB, then the game exits.
   - No device-only fix exists (docs/research/display-balloon-2026-09-29.md).
   - The design: a per-cgroup VRAM cap on the guest's app slice via the 615 driver's own accounting, with R automatic = 52/82/162 MiB for 1080p/1440p/4K.
   - Needs one guest-side unit (no driver patch).
   - Decide:
     - (a) allow the unit?
     - (b) R out of the profile or on top?
     - (c) `display_reserve_mib` as one of the knobs?
2. **UVM for Linux:** a preload shim could keep managed memory in guest RAM (docs/research/uvm-guest-ram-2026-09-28.md). The upstreamable nvidia-uvm patch (~300 lines) needs a CLA and a public PR, which touches the licence rule.
3. **Windows doorbells:**
   - a QEMU patch for vfio-user io fds (one KVM exit per doorbell); or
   - Windows under Cloud Hypervisor (firmware and TPM done; no display/keyboard for the installer; tooling work).
   - See docs/research/windows-traps-exit-2026-09-29.md.
4. **The device trace** costs ~10 % in graphics micro-benchmarks. Off by default in the launcher?
5. **NAS:** the games disk reads at ~62 MiB/s (qcow2 on /mnt/vmstore); you mentioned 25 Gbit/s.
6. **GFXP (graphics preemption, the Linux Xid 109 fix) needs a VA decision.**
   - Built in the slice as you decided, P42 bound it at VAs ≥ 2^47, and the context-switch unit, which keeps only 47 bits, faulted on both VMs (host Xid 31). It is reverted.
   - The buffers must sit below 2^47 in a range no guest can map. Options:
     - ~~(a) the guest's reserved server-RM range~~: ruled out by the RM source (gvaspaceReserveSplitVaSpace always puts the host's own server range there in a base-0 shadow VAS);
     - (c) host first fit per TSG with a VA fence on Linux too: worked on Windows in 20 runs, but not structurally collision-free;
     - (d) a GFXP bind without buffers, so the host RM would have to provide them: structurally safe only if GSP-RM really allocates buffers for a pointer-free GFXP.
       - The open source suggests it does not: the kernel RM never allocates per-context GFXP buffers, and GSP-RM's heap is fixed at 48 KiB per channel.
       - Finding out needs one test with your go (a long draw beside host vkcube; risk of one host Xid and a ~4 s desktop freeze).
       - If it works: host VRAM outside the slice, ~6.6 MiB per context, capped per VM;
     - (e, Windows only) the KMD's own pool VAs: does not help Linux.
   - Until then: **production = P41** (Windows runs without any patch, compute CTA/CILP parity on both OSes, graphics WFI).
7. **Long-draw test:** proving the Xid 109 fix needs a synthetic guest draw of ~6 s. It provokes one host Xid and a ~4 s host-desktop freeze, so it waits for your go.

## Open items (not blocking)

- Log noise marked ERROR but handled: the BAR1 pool "USERD cannot move back" and "maps twice" lines; the untrapped-GPPut "ran ahead with plans pending" detector (pre-existing, no fault).
- libcuda reports "VMM: no" with UVM off on Linux (cuMemCreate family unavailable).
- The usermode and invalidate pages lose or are unsafe as they are (usermode −40 %; invalidate lacks the untrapped hold).
- `cargo doc -D warnings` fails on dev (private doc links), from before tonight.
