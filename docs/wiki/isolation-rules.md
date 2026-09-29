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
| D11 VRAM slice | the device reserves a fixed, contiguous range of the card's VRAM sized to the profile, and refuses when the host would be left without enough free VRAM | one `NV01_MEMORY_LOCAL_USER` FIXED_ADDRESS allocation per VM at its slot; no floating fallback; 1024 MiB host headroom | a third 2 GiB VM refused with a 455 MiB shortfall while two ran; every FB access outside `[B, E)` counted, 0 in every run |
| D16 no comptags | no guest comptag line ever reaches the host | every FB, memsys and caps answer says no compression; a PTE filter forces uncompressed kinds with COMPTAGLINE 0; an audit sweep after each gate; the slice is never allocated with `NVOS32_ATTR_COMPR` | the audit read only kinds 0x00 and 0x06 with comptagline 0 over 924737 PTEs of two VMs |
| D17 shadow tables | the GPU walks only the device's validated copy of the guest's tables; the guest stays bare metal | `resolve_leaf`; GR and CE on the filtered shadow; no host channel on a guest root | 400 of 400 channels on shadow VA spaces, 0 `DMA_SET_PAGE_DIRECTORY` on a guest root |
| D18 reachable set | the GPU may reach only this VM's slice and guest-RAM pages the device registered | the registered-descriptor table, `GuestMemory::translate`, the swiotlb alias check | the cross-VM pattern test found 0 records and 0 non-zero words in both directions |
| D12 privilege | the device runs unprivileged and raises `CAP_SYS_ADMIN` on one thread only around named controls | `setcap cap_sys_admin+p` on its copy; windows around `GET_SURFACE_PHYS_ATTR`, `DMA_SET_PAGE_DIRECTORY`, `GPU_PROMOTE_CTX`, the invalidator's `MEM_OP` | the RM refuses a PRIVILEGED control below `USER_ROOT` (`osIsAdministrator`, `escape.c`, `control.c`) |
| refusals | registers, bus, power, clocks, ECC, NVLink, profilers and other clients' state are refused in the GSP role | the D DENY rows, the public-control filter of B forwards, `GPU_EXEC_REG_OPS` answering `NOACCESS` | [gsp-control-categories](gsp-control-categories.md) |
| scrub | the slice is scrubbed by the host before the next owner | the host's scrub on free (`PMA` without `SKIP_SCRUB`, `phys_mem_allocator.c`; on by default unless `RmDisableScrubOnFree`) | the 2 GiB free returned in 1 ms, the next allocation waited 23 to 38 ms, a sampled scan read 0 pattern records |
| teardown | fail closed: mirrors, descriptors, reservation, client, stop at the first failure | `FAIL-CLOSED teardown`, exit status 3, the reservation kept | 4096 MiB stayed pinned, the host RM freed the client with 0 Xid |
| budgets | host resources without quota get per-device budgets | BAR1 windows 2 x 32 MiB, the BAR1 pool, `CARAXES_MAP_BUDGET` 32768 live mappings | a window leak produced 732088 refused host BAR1 mappings without a Xid |
| containment | every host Xid names the device pid and lands on the VM's mirrors | the run checker compares Xid lines with the device pid and stops at the first outside it | the catalogue in [research/isolation-and-xids.md](../research/isolation-and-xids.md) |

## What a guest can never do

- Map VRAM outside its slice, or RAM it did not get registered.
- Choose a compressible page kind or write a comptag line.
- Ring a channel that is not its own; the token pool names only its mirrors.
- Read or write the card's registers (`GPU_EXEC_REG_OPS` returns the three boot ids from
  the model and `NOACCESS` for everything else) or change the boot identity.
- Reach GSP-RM's bus, power, clock, ECC, NVLink, MIG, confidential-computing or profiler
  paths.
- See another VM's memory, or the desktop's, through any control the device forwards.

## What the rules do not give

- No QoS: budgets exist for BAR1 only, not for GPU time, channel count or RM work; a VA
  teardown of half a million mappings once stalled the host's `nvidia-smi` for 20 s without
  any Xid.
- A timing side channel: the host's non-stall notifier is host-wide, so an idle guest sees
  the desktop's completions as interrupts ([interrupts](interrupts.md)).
- The device cannot check that the host has scrub on free enabled, since an unprivileged
  client cannot read the key.
- No adversarial test yet: a fuzzing guest against the GSP dispatcher and the shadow filter,
  a hostile page-table writer and a doorbell storm are the missing tests.

## VirtIO Leandro's boundary

- The VM is the policy boundary; the guest kernel separates guest processes. The backend
  validates request sizes, pointer spans, descriptor metadata and memory ranges, resolves
  tokens to backend-owned FDs, charges a pin budget and blocks 34 controls and the
  capability-FD classes.
- Remaining risks named in [SECURITY.md](../SECURITY.md): foreign RM handles through DUP and
  UVM imports, unannotated pointer fields, backing-page release without a reference ledger,
  no partition of the card. Hostile-guest containment is not established.

## Depth

- [research/isolation-and-xids.md](../research/isolation-and-xids.md): the rules with their
  lines and every Xid with cause and fix.
- [caraxes/parallel.md](../caraxes/parallel.md): the parallelism proof (slots, pattern test,
  scrub, fail-closed teardown, the refused third VM).
- [caraxes/memory-model.md](../caraxes/memory-model.md) §3, §7.4: privilege and the fail-closed argument.
