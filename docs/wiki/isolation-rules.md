<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Isolation rules

The host desktop and the other guests share the card with every VM. These are the rules a
VFIO Leandro device enforces so that a guest reaches only its own memory and its faults
stay its own, what they rest on, and where VirtIO Leandro's boundary differs. Malicious
guests are untested in both; every isolation run used the stock driver.

## The rules (VFIO Leandro)

| Rule | Statement | Enforced by | Evidence |
|---|---|---|---|
| D11 VRAM slice | the device reserves a fixed, contiguous range of the card's VRAM sized to the profile, and refuses when the host would be left without enough free VRAM | one `NV01_MEMORY_LOCAL_USER` FIXED_ADDRESS allocation per VM at its slot; no floating fallback; 1024 MiB host floor | a third 2 GiB VM refused with a 455 MiB shortfall while two ran; every FB access outside `[B, E)` counted, 0 in every run |
| D16 comptags | without compression (the default) no guest comptag line reaches the host; with `CARAXES_COMPRESSION=slice` (the Linux gates) compression stays inside the VM's slice | off: every FB, memsys and caps answer says no compression, a PTE filter forces uncompressed kinds with COMPTAGLINE 0, an audit sweep after each gate; slice: Turing's 1:1 comptag mode derives the line from the physical address | the audit read only kinds 0x00 and 0x06 with comptagline 0 over 924737 PTEs of two VMs; Windows' kind-0x09 surfaces refused, 3744 to 4741 leaves per session, never touched |
| D17 shadow tables | the GPU walks only the device's validated copy of the guest's tables; the guest stays bare metal | `resolve_leaf`; GR and CE on the filtered shadow; no host channel on a guest root | 400 of 400 channels on shadow VA spaces, 0 `DMA_SET_PAGE_DIRECTORY` on a guest root |
| D18 reachable set | the GPU may reach only this VM's slice and guest-RAM pages the device registered | the registered-descriptor table, `GuestMemory::translate`, the swiotlb alias check | the cross-VM pattern test found 0 records and 0 non-zero words in both directions |
| private pages | the top 512 KiB of the slice (the invalidator's and the device CE's tables, rings and USERD) is no guest memory | the software CE's range, the shadow's leaves, trapped BAR1 and BAR2 accesses, the BAR2 memfd and PRAMIN stop below it (`devce.rs` `PRIVATE_AT`, `fb.rs` `bar_access`) | fixed in 8dbce31a: before, a guest could have pointed an ADMIN host channel's PTEs at any host VRAM |
| D12 privilege | the device runs unprivileged and raises `CAP_SYS_ADMIN` on one thread only around named controls | `setcap cap_sys_admin+p` on its copy; windows around `GET_SURFACE_PHYS_ATTR`, `DMA_SET_PAGE_DIRECTORY`, `GPU_PROMOTE_CTX`, the invalidator's `MEM_OP` | the RM refuses a PRIVILEGED control below `USER_ROOT` (`osIsAdministrator`, `escape.c`, `control.c`) |
| channel privilege | a guest channel's privilege never becomes host privilege | every host mirror is allocated with `NVOS04_FLAGS_CHANNEL_DENY_PHYSICAL_MODE_CE`; the guest kernel's privileged copy channels run in the software executor | a Windows copy channel mirrored unprivileged took host Xid 32 (`HCE_PRIV`) on its own channel at its first physical-mode `LAUNCH_DMA` ([research: windows-2026-09-27](../research/windows-2026-09-27.md)); the rule per channel: [software-executor](software-executor.md) |
| refusals | registers, bus, power, clocks, ECC, NVLink, profilers and other clients' state are refused in the GSP role | the D DENY rows, the public-control filter of B forwards, `GPU_EXEC_REG_OPS` answering `NOACCESS` | [gsp-control-categories](gsp-control-categories.md) |
| scrub | the slice is scrubbed by the host before the next owner | the host's scrub on free (`PMA` without `SKIP_SCRUB`, `phys_mem_allocator.c`; on by default unless `RmDisableScrubOnFree`) | the 2 GiB free returned in 1 ms, the next allocation waited 23 to 38 ms, a sampled scan read 0 pattern records |
| teardown | fail closed: mirrors, descriptors, reservation, client, stop at the first failure | `FAIL-CLOSED teardown`, exit status 3, the reservation kept | 4096 MiB stayed pinned, the host RM freed the client with 0 Xid |
| budgets | host resources without quota get per-device budgets | BAR1 windows 2 x 16 MiB and the BAR1 pool (`CARAXES_BAR1_POOL_MIB`, 32 by default); 32768 live mappings; a host BAR1 floor (`CARAXES_HOST_BAR1_FLOOR_MIB`, Linux 64, Windows 96) | a window leak produced 732088 refused host BAR1 mappings without a Xid |
| containment | every host Xid names the device pid and lands on the VM's mirrors | the run checker compares Xid lines with the device pid and stops at the first outside it | the catalogue in [research/isolation-and-xids.md](../research/isolation-and-xids.md) |

- Sources: [research: isolation-and-xids](../research/isolation-and-xids.md) (D11 to D18,
  scrub, teardown, containment); [research: windows-findings](../research/windows-findings.md)
  (the private pages, the security fix of 2026-09-29; Windows D16, step 23);
  [research: compression-isolation](../research/compression-isolation-2026-09-28.md) (the
  slice); [flags](../../caraxes/runs/flags-2026-09-30/NOTE.md) (windows, pool, floors).

## What a guest can never do

- Map VRAM outside its slice, the device's private pages, or RAM it did not get registered.
- Choose a compressible page kind or write a comptag line outside its slice.
- Ring a channel that is not its own; a pool token or a guest token names only its channels.
- Run physical-mode copy work on the host GPU.
- Read or write the card's registers (`GPU_EXEC_REG_OPS` returns the three boot ids from
  the model and `NOACCESS` for everything else) or change the boot identity.
- Reach GSP-RM's bus, power, clock, ECC, NVLink, MIG, confidential-computing or profiler
  paths.
- See another VM's memory, or the desktop's, through any control the device forwards.

## What the rules do not give

- No QoS: budgets exist for BAR1 and mappings only, not for GPU time, channel count or RM
  work; a VA teardown of half a million mappings once stalled the host's `nvidia-smi` for
  20 s without any Xid. With graphics WFI a guest context can hold GR up to the host's
  4.29 s switch timeout, freezing the host desktop until Xid 109
  ([research: gr-preemption](../research/gr-preemption-2026-09-29.md)).
- Host VRAM outside the slice: the shadow VA spaces' page tables and the host GR contexts
  (about 450 MiB beside a 4 GiB Windows slice, P34). Under host VRAM pressure a shadow map
  got `NO_MEMORY` and the guest's own context took Xid 31, a failure a real card never has
  ([codec matrix](../../caraxes/runs/codec-2026-09-30/matrix.md), l2).
- Timing side channels: the host's non-stall notifier is host-wide, so an idle guest sees
  the desktop's completions as interrupts ([interrupts](interrupts.md)); the host API has no
  per-channel non-stall event. With compression on, the host's ZBC table is shared: a guest
  can learn whether a slot is free and whether a value is present, a covert channel of one
  bit per setter over 15 slots; each VM may add 4 entries
  ([research: zbc-host-table](../research/zbc-host-table-2026-09-28.md)).
- The device cannot check that the host has scrub on free enabled, since an unprivileged
  client cannot read the key.
- No adversarial test yet: a fuzzing guest against the GSP dispatcher and the shadow filter,
  a hostile page-table writer and a doorbell storm are the missing tests.

## VirtIO Leandro's boundary

- The VM is the policy boundary; the guest kernel separates guest processes. The backend
  validates request sizes, pointer spans, descriptor metadata and memory ranges, resolves
  tokens to backend-owned FDs, charges a pin budget and blocks 34 controls and the
  capability-FD classes.
- Remaining risks named in [security.md](../virtio/security.md): foreign RM handles through DUP and
  UVM imports, unannotated pointer fields, backing-page release without a reference ledger,
  no partition of the card. Hostile-guest containment is not established.

## Depth

- [research/isolation-and-xids.md](../research/isolation-and-xids.md): the rules with their
  lines and every Xid with cause and fix.
- [caraxes/parallel.md](../vfio/parallel.md): the parallelism proof (slots, pattern test,
  scrub, fail-closed teardown, the refused third VM).
- [caraxes/memory-model.md](../vfio/memory-model.md) §3, §7.4: privilege and the fail-closed argument.
