<!-- SPDX-License-Identifier: MIT -->
<!-- Source survey written 2026-09-23 for the two debugging sessions of E5b part 2 (the PBENTRY
     wall on mirrored channels and the Xid 38 on host-built GR contexts). No VM, no host GPU
     access, no code change. Every claim cites a file and a function or line; "inference" marks
     what the sources do not say directly. -->
# Execution formats: PBDMA errors, GMMU v2, write-combining, server-reserved PDEs

## Sources and short names

| Short name | Path |
|---|---|
| `pbdma.ref` | NVIDIA open-gpu-doc `manuals/turing/tu104/dev_pbdma.ref.txt` (public hardware manual, fetched 2026-09-23 into the session scratchpad; TU104 and TU106 share the Turing host) |
| `ram.ref` | open-gpu-doc `manuals/turing/tu104/dev_ram.ref.txt` (RAMIN, RAMFC, `FIFO_DMA` pushbuffer format) |
| `mmu.ref` | open-gpu-doc `manuals/turing/tu104/dev_mmu.ref.txt` and `manuals/volta/gv100/dev_mmu.ref.txt` |
| `clc46f.h` | `vendor/open-gpu-kernel-modules/src/common/sdk/nvidia/inc/class/clc46f.h` |
| `nouveau/` | `drivers/gpu/drm/nouveau/nvkm/` of the Ubuntu noble (6.8) tree in the session scratchpad (`linux/ubuntu-noble`) |
| `RM/` | `vendor/open-gpu-kernel-modules/src/nvidia/src/kernel/` (610.57.04) |
| `D/` | `crates/vfio-user-nvgpu/src/` |

The open RM does not contain the PBDMA interrupt handler: on Turing it runs inside GSP-RM, and the
kernel RM only receives the result (`_kgspRpcRCTriggered`, `_kgspRpcOsErrorLog`,
`RM/gpu/gsp/kernel_gsp.c:585-760`). `PBENTRY` does not appear anywhere in the open RM tree. The
hardware semantics below therefore come from the public manuals, with nouveau as the second source.

---

## 1. PBDMA errors, and the checklist for a mirrored channel

### 1.0 Check the evidence first: the recorded PBDMA error is DEVICE, not PBENTRY

The only PBDMA Xids recorded so far (`caraxes/runs/e5b2-2026-09-23/a2-host-xid.txt`, six UVM CE
channels, host chids 0x32..0x37) read `Xid 32 ... intr 00800000`. Bit 23 is
`NV_PPBDMA_INTR_0_DEVICE`, not `PBENTRY` (bit 18 = 0x00040000) (`pbdma.ref` "INTR_0",
`#define NV_PPBDMA_INTR_0_DEVICE 23:23`; the same table in `nouveau/engine/fifo/gk104.c`
`gk104_runq_intr_0_names`, used for Turing through `gv100_runq`, `nouveau/engine/fifo/tu102.c:275`).
No run directory contains an `intr 00040000` line.

DEVICE means "a SW-class method: the method's subchannel specified a SW engine" (`pbdma.ref`
INTR_0_DEVICE). Subchannels 5-7 are software subchannels; any method on them, SetObject included,
is kicked back to software through DEVICE (`ram.ref` "FIFO_DMA", paragraph "Subchannels 5-7 are
for software methods"). The first push of every UVM CE channel does exactly that:
`uvm_hal_turing_host_init` sends `SET_OBJECT(GP100_UVM_SW)` on `UVM_SUBCHANNEL_C076` = 5
(`kernel-open/nvidia-uvm/uvm_turing_host.c:30-34`, `uvm_push_macros.h:89,98`). The host RM services
DEVICE only for a channel that owns a SW object; the mirror's `0xc076` allocation was refused
(`a2-mirror-trace-excerpt.txt`: `host object class 0xc076 refused: 0x1b
NV_ERR_INSUFFICIENT_PERMISSIONS`). **So the a2 Xids are the unserviced UVM software method, not a
fetch problem** (inference for the "unserviced" part: the GSP-side SW-method dispatcher is not in
the open source; the bit decode and the subchannel-5 SetObject are cited). Such channels can never
run on a host PBDMA while `GP100_UVM_SW` cannot be allocated on the mirror; the software methods
(`FAULT_CANCEL_*`, `CLEAR_FAULTED` equivalents, `uvm_turing_host.c:530-590`) would have to be
stripped or kept on the software executor.

If the debugging agents have a PBENTRY observation outside these files, §1.1 applies to it.

### 1.1 What raises PBENTRY (bit 18)

`pbdma.ref` INTR_0_PBENTRY, verbatim conditions, when Host expects the PB entry to be a PB
instruction:

1. the PB entry does not decode into a PB instruction;
2. the decoded instruction is obsolete or otherwise invalid (see `ram.ref` FIFO_DMA);
3. an incrementing or increment-once header whose COUNT makes the method address wrap past the
   maximum method address (12-bit dword address, `NV_FIFO_DMA_METHOD_ADDRESS 11:0`).

Plus one more stated elsewhere: a `SET_SUBDEVICE_MASK` or `USE_SUBDEVICE_MASK` while
`NV_PPBDMA_SUBDEVICE_CHANNEL_DMA` is DISABLE raises PBENTRY (`pbdma.ref`, "SUBDEVICE" register text).

Valid PB instructions (`ram.ref` FIFO_DMA table, `clc46f.h:285-358`), header dword bits:

| SEC_OP 31:29 | TERT_OP 17:16 | Entry |
|---|---|---|
| 000 | 01 / 10 / 11 | SET / STORE / USE_SUBDEVICE_MASK (bits 31:16 must be exactly 0x0001/2/3; mask in 15:4) |
| 000 | 00 | only the universal NOP `0x00000000`; any other value is the obsolete Fermi-era incrementing format -> PBENTRY |
| 001 | x | incrementing method header (COUNT 28:16, SUBCH 15:13, ADDR 11:0, bit 12 reserved) |
| 010 | x | obsolete (`SEC_OP_GRP2_USE_TERT`, old non-incrementing) -> PBENTRY |
| 011 | x | non-incrementing header |
| 100 | x | immediate-data header (data 28:16) |
| 101 | x | increment-once header |
| 110 | x | `RESERVED6` -> PBENTRY |
| 111 | x | END_PB_SEGMENT |

**What PBENTRY is not.** It is not an address fault. An untranslatable GPFIFO or pushbuffer VA is
an MMU fault (Xid 31, `ROBUST_CHANNEL_FIFO_ERROR_MMU_ERR_FLT`, `sdk/nvidia/inc/nverror.h:49`), with
a fault-buffer entry naming the HOST/PBDMA client. Alignment and length problems of the GP entry are
GPENTRY/GPFIFO/GPPTR (§1.2). PBENTRY means **the PBDMA fetched bytes, and the bytes were not a valid
instruction**: garbage, stale, or zero-filled-but-not-all-zero data, or a correct pushbuffer read
from the wrong offset (a header treated as data or vice versa). The hardware writes the offending
raw dword to `NV_PPBDMA_HDR_SHADOW` (0x040118 + 0x2000·i) or `NV_PPBDMA_PB_DATA0`, the decoded one
to `NV_PPBDMA_PB_HEADER`, the count to `NV_PPBDMA_PB_COUNT` (`pbdma.ref` PBENTRY recovery steps 1-2,
"HDR_SHADOW"). Those registers are what the RC diagnostics carry (§1.3).

**Therefore, for a mirror whose software walker read the same entries fine, the candidates are
(each an inference, ranked):**

- (a) **Non-snooped read of guest RAM (aperture 3).** The guest RM gives uncached and
  write-combined sysmem the PTE aperture `SYS_NONCOH` and only cached sysmem `SYS_COH`
  (`kgmmuGetMemAperture_IMPL`, `RM/gpu/mmu/kern_gmmu.c:587-606`). GPFIFOs and pushbuffers of
  the CUDA and RM paths are commonly write-combined user mappings, so their PTEs carry aperture 3.
  On bare metal the CPU mapping really is WC, and the data is in DRAM when the doorbell rings.
  Inside the VM the guest's WC/UC PAT attribute is honoured only if KVM honours guest PAT for that
  memory; with an emulated (vfio-user) device KVM sees no non-coherent DMA and can map guest RAM
  write-back (inference about the host kernel, to be checked on this host's KVM). The guest's
  stores then sit in the host CPU cache, and a no-snoop PCIe read from the GPU returns stale DRAM.
  The device's walker reads the same bytes through the CPU, coherently, and sees the right data
  (`D/fb.rs` `decode_pte` maps apertures 2 and 3 alike to `Target::Sys`). Test: log the aperture of
  the PTEs behind the GPFIFO and pushbuffer VAs at the first doorbell; if they are 3, rewrite nothing
  yet, but flush the guest pages from the host (`clflush` over the pushbuffer range in the device,
  which maps guest RAM) before ringing the host doorbell, and see whether PBENTRY disappears.
- (b) **Stale write-combined BAR1 data in VRAM** for GPFIFOs/pushbuffers the guest placed in FB:
  the device writes guest FB through host BAR1 windows. `D/hostmem.rs` `Window::flush` does
  `sfence` plus one read of the window's first byte; `D/ctrl.rs:210` calls `gm.fb.flush()` before
  the host doorbell. That is the same sequence the RM uses (§3) and is sufficient only if every
  window that received a write is flushed; `VramStore::flush` flushes all held windows
  (`D/fb.rs:441-445`) but a window **evicted** by the LRU (`window_for`, `D/fb.rs:333-341`) is
  dropped without `flush()` (the `Drop` of `hostmem::Window` only `munmap`s,
  `D/hostmem.rs:113-118`). `munmap` of a WC mapping does not by itself order the pending WC stores
  against the later doorbell (inference). Add a flush before eviction.
- (c) **A stale translation in the physical GMMU TLB**, when the guest changed PTEs after the host
  last walked that page directory and the matching invalidate did not reach the host with the right
  PDB (see §2.6, divergence 9, and §4.4: on a replayed VA space `NV0080_CTRL_CMD_DMA_INVALIDATE_TLB`
  invalidates the host kernel's own root, not the guest root).
- (d) **Different translation rule** for the same VA in the hardware and the walker (§2.6: big/small
  page precedence, sparse and LPT-invalid encodings, reads outside the reservation).
- (e) **Subdevice-mask instructions with SUBDEVICE_CHANNEL_DMA disabled** in the host RAMFC. Not
  mirror-specific: the host builds RAMFC the same way for every channel, so this would also hit
  native CUDA. Low.

### 1.2 GP entries, the GPFIFO ring, and the other INTR_0 bits

GP entry (`pbdma.ref` "GP_ENTRY", `clc46f.h:264-282`), 8 bytes:

| Field | Bits | Rule |
|---|---|---|
| `GP_ENTRY0_FETCH` | 0 | 0 unconditional, 1 conditional (on SUBDEVICE_STATUS) |
| `GP_ENTRY0_GET` | 31:2 | PB segment VA bits 31:2: **dword-aligned** (bits 1:0 not stored) |
| `GP_ENTRY1_GET_HI` | 7:0 | VA bits 39:32: the PBDMA address space is **40 bits** |
| `GP_ENTRY1_LEVEL` | 9 | MAIN / SUBROUTINE (only affects TOP_LEVEL_GET reporting) |
| `GP_ENTRY1_LENGTH` | 30:10 | segment length **in dwords**; 0 = control entry |
| `GP_ENTRY1_SYNC` | 31 | WAIT: finish Host processing of the previous segment first |
| `GP_ENTRY1_OPCODE` | 7:0 (control entries only) | 0 NOP, 1 ILLEGAL, 2 GP_CRC, 3 PB_CRC |

Error bits and what each would have meant (`pbdma.ref` INTR_0 field texts):

| Bit | Name | Cause |
|---|---|---|
| 0 | MEMREQ | a memory request not accepted within the timeout; unrecoverable |
| 1-4 | MEMACK_TIMEOUT/EXTRA, MEMDAT_TIMEOUT/EXTRA | memory acks/data missing or unexpected (memory subsystem, e.g. a hung sysmem read) |
| 5 | MEMFLUSH | flush failure |
| 6 | MEMOP | a MEM_OP method failed |
| 7, 9-12 | LBCONNECT, LBACK_*, LBDAT_* | latency-buffer protocol errors, unrecoverable (bit 8 LBREQ exists only in nouveau's older table) |
| 13 | GPFIFO | GP ring crosses 0xFF_FFFF_FFFF: `GP_BASE + (1<<LIMIT2)*8 - 1` too large |
| 14 | GPPTR | GP_PUT/GP_GET/GP_FETCH not in `[0, 1<<LIMIT2)`: **a GPPut the guest wrote that is ≥ the entry count** |
| 15 | GPENTRY | control entry with opcode ILLEGAL or unknown; or a PB segment that crosses the end of the 40-bit VA space (the last dword of the VA space is unusable) |
| 16 | GPCRC | debug CRC over GP entries (only with OPCODE GP_CRC) |
| 17 | PBPTR | PB GET > PUT after a restore |
| 18 | PBENTRY | §1.1 |
| 19 | PBCRC | debug CRC over a segment (OPCODE PB_CRC) |
| 20 | CLEAR_FAULTED_ERROR | a CLEAR_FAULTED method whose target channel never faulted |
| 21 | METHOD | internal method, invalid Host method, `NV_UDMA_ILLEGAL`, a privileged method on a non-privileged channel (`AUTH_LEVEL`), unknown YIELD op, SYNCPOINT |
| 22 | METHODCRC | debug |
| 23 | DEVICE | SW method: subchannel 5-7 (§1.0) |
| 24 | ENG_RESET | engine reset while the PBDMA served a channel of that runlist |
| 25 | SEMAPHORE | invalid `SEM_EXECUTE` data |
| 26 | ACQUIRE | semaphore acquire timeout |
| 27 | PRI | PRI write to the PBDMA while busy |
| 30 | PBSEG | a compressed method sequence split into a FETCH_CONDITIONAL segment |
| 31 | SIGNATURE | RAMFC signature mismatch (the instance block is not a valid channel) |

Bits 8, 28 and 29 are not defined for Turing (`pbdma.ref`; nouveau's shared table still names 29 NO_CTXSW_SEG).

GPFIFO base: `NV_RAMFC_GP_BASE`/`GP_BASE_HI` hold a 37-bit address in 8-byte units (the ring is
8-byte aligned), the ring size is `1 << GP_BASE_HI_LIMIT2` entries (**a power of two**), and it is a
**virtual** address in the channel's VA space (`pbdma.ref` "GP_BASE"). GP_GET/PUT are relative to it.
Nouveau's second-level decode (`intr_1`) adds CTXNOTVALID (`nouveau/engine/fifo/gv100.c:146-165`).

### 1.3 How the RM logs it, and which lines to look for

- **Kernel log, from GSP**: `NVRM: Xid (PCI:...): 32, pid=..., name=..., channel 0x<chid> intr
  <INTR_0>, method0 <METHOD0>, data0 <DATA0>, ... method3/data3`. The string is built inside GSP-RM and
  printed by the kernel RM verbatim (`_kgspRpcOsErrorLog` -> `nvErrorLog2_va(..., "%s",
  rpc_params->errString)`, `RM/gpu/gsp/kernel_gsp.c:724-760`). `intr` is INTR_0; the four
  method/data pairs are the PBDMA method FIFO (`NV_PPBDMA_METHOD0..3`/`DATA0..3`, 0x0400c0.. in
  `pbdma.ref`). For PBENTRY the useful registers (HDR_SHADOW, PB_HEADER, PB_COUNT, GP_SHADOW) are
  **not** in that line.
- **Xid 31** (MMU fault) would come with the fault-buffer decode (client HOST/PBDMA, fault type,
  VA). Xid 13/69 are GR exceptions.
- **The RC notifier the device already receives** (`HOST RC EVENT ... exceptType`,
  `D/mirror.rs` event thread) carries only the Xid number.
- **RC-diag records.** GSP attaches `RmRcDiag` records (tag/value/attribute register dumps) to the
  RC event; the kernel copies them into its journal (`_kgspRpcRCTriggered`,
  `RM/gpu/gsp/kernel_gsp.c:633-690`, `rcdbAddRcDiagRecFromGsp`) and tags them with the channel
  owner's process id (`rcdbUpdateRcDiagRecContext`). An **unprivileged** client reads them with
  `NV0000_CTRL_CMD_NVD_GET_RCERR_RPT` (0x607, `sdk/.../ctrl0000/ctrl0000nvd.h:468-606`) on its
  root client: `owner` must not be `RCDB_RCDIAG_DEFAULT_OWNER` (use
  `NV0000_CTRL_CMD_NVD_RCERR_RPT_ANY_OWNER_ID` 0xFFFFFFFF), and the process filter is forced to the
  caller's own pid (`cliresCtrlCmdNvdGetRcerrRpt_IMPL`, `RM/rmapi/client_resource.c:5430-5470`;
  `krcCliresCtrlNvdGetRcerrRptCheckPermissions_KERNEL`, `RM/gpu/rc/kernel_rc_ctrl.c:293-319`;
  filter `_rcdbInternalGetRcDiagRec`, `RM/diagnostics/journal.c:844-848`). The device process owns
  the mirror channels, so it can read its own records after each RC. Which tags GSP puts there is
  not in the open source (inference: the PBDMA error path dumps the PBDMA registers, which is where
  HDR_SHADOW would be). This is a read of host GPU state through the RM API: report it to the
  operator (MEMORY rule).
- `NVreg_RmMsg`/`ResmanDebugLevel` prints need a debug-enabled host driver and flood dmesg
  (memory note "Host dmesg flood cause"); not recommended.

### 1.4 Checklist: what the device must guarantee before ringing a mirrored channel's doorbell

VA space and instance block

1. The host channel's PDB is the guest root. For an externally owned VAS this is the
   `DMA_SET_PAGE_DIRECTORY` root (`gvaspaceExternalRootDirCommit_IMPL`,
   `RM/mem_mgr/gpu_vaspace.c:2813`), for a replayed VAS the root handed to GSP by
   `COPY_SERVER_RESERVED_PDES` (§4). The root is 4 KiB aligned (`NV_RAMIN_PAGE_DIR_BASE_LO` 31:12,
   `ram.ref`), in VIDMEM, inside the reservation.
2. The host VAS uses **64 KiB big pages**, like the guest (`NV_RAMIN_BIG_PAGE_SIZE` bit 11, 1 = 64 KiB,
   `ram.ref`; the mirror sets `bigPageSize = 64K`, `D/mirror.rs:386-390`; the replayed VAS passes the
   guest's own parameters, `D/mirror.rs:514-520`, so check the guest's `bigPageSize` is 0 or 64 KiB).
   A 128 KiB host VAS would index every big-page table differently from the guest.
3. `USE_VER2_PT_FORMAT` is set by RM for every Turing VAS (`ram.ref` bit 10; not a device concern).

GPFIFO

4. The GPFIFO VA, `gpFifoOffset`, is the guest's, 8-byte aligned, below 2^40, and
   `gpFifoOffset + entries*8 - 1 < 2^40` (GPFIFO rule).
5. `gpFifoEntries` is the guest's value and a power of two (LIMIT2 is a log2; RM derives it at
   channel alloc). Every GPPut the device writes into USERD, including the "restore" after the host
   cleared USERD (`b2-mirror-trace-excerpt.txt`), is `< entries` (GPPTR).
6. Every page of the ring is mapped by a valid PTE in the guest tables *and* the physical TLB holds
   no older translation for it (invalidate with the correct PDB after any guest PTE change, §2.5).

Entries and pushbuffers

7. Each entry's segment is dword aligned (`GET` bits 1:0 are dropped silently, `clc46f.h:268`), below
   2^40, and does not end on the last dword of the 40-bit space (GPENTRY).
8. Control entries (LENGTH 0) use opcode 0 (or 2/3 with CRC); opcode 1 or others -> GPENTRY.
9. The first dword the PBDMA fetches is a valid PB instruction (§1.1 table) and the header/data
   sequence stays in step across segments.
10. The bytes the PBDMA reads are the bytes the guest wrote: for aperture-3 sysmem, the guest's
    stores must be in DRAM (candidate (a)); for VRAM written through host BAR1, every window that
    received a write, including evicted ones, has been flushed (candidate (b)).
11. No method on subchannels 5-7 unless the host channel owns a SW object that services it (§1.0).
    CE channels: `SET_OBJECT` with the CE class on subchannel 0 or 4 must name the class of the host
    CE object on that channel (`uvm_turing_ce.c:30-37`).
12. USERD: the host channel's USERD is the guest's page (vidmem USERD -> the reservation object at
    `PA - base`, `D/mirror.rs`), so GPGet/GPPut writeback lands where the guest reads it.

---

## 2. GMMU v2 on Turing, and every place the software walker is lenient

### 2.1 Levels (49-bit VA, 64 KiB big pages)

`kgmmuFmtInitLevels_GP10X` (`RM/gpu/mmu/arch/pascal/kern_gmmu_fmt_gp10x.c:31-105`, bound for TU102+),
and the same layout in nouveau `gp100_vmm_desc_16/12` (`nouveau/subdev/mmu/vmmgp100.c`, table
ending `{ PGD, 2, 8, 0x1000, &gp100_vmm_desc_pd1 }`):

| Level | VA bits | Entries | Entry | Table size and alignment |
|---|---|---|---|---|
| PD3 (root, PDB) | 48:47 | 4 | 8 B PDE | 32 B used; the PDB itself must be **4 KiB aligned** (`NV_RAMIN_PAGE_DIR_BASE_LO` 31:12, `ram.ref`; `NV_MMU_VER2_PDE_ADDRESS_SHIFT` 12) |
| PD2 | 46:38 | 512 | 8 B PDE | 4 KiB, 4 KiB aligned |
| PD1 | 37:29 | 512 | 8 B PDE | 4 KiB, 4 KiB aligned |
| PD0 | 28:21 | 256 | 16 B dual PDE, or an 8 B PTE in the low half for a 2 MiB page | 4 KiB, 4 KiB aligned |
| PT big (64 KiB) | 20:16 | 32 | 8 B PTE | **256 B**, 256 B aligned (address stored `>> 8`, `NV_MMU_VER2_DUAL_PDE_ADDRESS_BIG_SHIFT` 8) |
| PT small (4 KiB) | 20:12 | 512 | 8 B PTE | 4 KiB, 4 KiB aligned |

The big-page size is per instance block: `NV_RAMIN_BIG_PAGE_SIZE` bit (128·32+11), 0 = 128 KiB,
1 = 64 KiB, honoured when `NV_PFB_PRI_MMU_CTRL_USE_PDB_BIG_PAGE_SIZE` is set (`ram.ref`). With
128 KiB the big PT has 16 entries over VA 20:17 (`bigPageShift` 17 accepted by
`kgmmuFmtInitLevels_GP10X`).

### 2.2 Entry layouts (Turing, `mmu.ref` tu104 = gv100 for these fields)

PDE (PD3, PD2, PD1), 64 bits:

| Bits | Field |
|---|---|
| 0 | `IS_PTE` (must be 0 at these levels on Turing: RM defines no large page above PD0) |
| 2:1 | `APERTURE`: 0 INVALID, 1 VIDEO_MEMORY, 2 SYSTEM_COHERENT, 3 SYSTEM_NON_COHERENT |
| 3 | `VOL` (with APERTURE INVALID: sparse, `kern_gmmu_gm200.c:52-66`) |
| 5 | `NO_ATS` |
| 32:8 | `ADDRESS_VID` = next-level PA >> 12 (37-bit reach); 35:33 peer id |
| 53:8 | `ADDRESS_SYS` = next-level address >> 12 |

Dual PDE (PD0), 128 bits:

| Bits | Field |
|---|---|
| 0 | `IS_PTE`: 1 = the low 64 bits are a 2 MiB PTE |
| 2:1 / 3 | `APERTURE_BIG` / `VOL_BIG` (big-PT half; `VOL_BIG` with aperture INVALID = the whole PDE is sparse, `nouveau/subdev/mmu/vmmgp100.c:253-258`) |
| 5 | `NO_ATS` |
| 32:4 | `ADDRESS_BIG_VID` = big-PT PA >> 8; 53:4 `ADDRESS_BIG_SYS` |
| 66:65 / 67 | `APERTURE_SMALL` / `VOL_SMALL` |
| 96:72 | `ADDRESS_SMALL_VID` = small-PT PA >> 12; 117:72 `ADDRESS_SMALL_SYS` |

PTE, 64 bits:

| Bits | Field |
|---|---|
| 0 | `VALID` |
| 2:1 | `APERTURE`: 0 VIDEO, 1 PEER, 2 SYSTEM_COHERENT, 3 SYSTEM_NON_COHERENT (note: PDE and PTE encodings differ) |
| 3 | `VOL` |
| 4 | `ENCRYPTED` (read-only field on Turing) |
| 5 | `PRIVILEGE` |
| 6 | `READ_ONLY` |
| 7 | `ATOMIC_DISABLE` |
| 32:8 | `ADDRESS_VID` = page PA >> 12; 35:33 peer id |
| 53:8 | `ADDRESS_SYS` = page address >> 12 (IOVA for sysmem) |
| 55:36 | `COMPTAGLINE` (Turing: 20 bits) |
| 63:56 | `KIND`: 0x00 PITCH, 0x06 GENERIC_MEMORY, 0x07 INVALID, 0x08-0x0E compressible, 0x0F SMSKED (`published/turing/tu102/dev_mmu.h:96-112`, `kind_macros.h:28-31`) |

Invalid-entry encodings the hardware distinguishes (RM and nouveau agree):

| Encoding | Meaning | Source |
|---|---|---|
| PTE all zero | invalid; for a **big** PTE: "unmapped", the MMU goes on to the small PTE of the same 4 KiB | RM fills invalid entries with zero (`gmmu_walk.c:862-864`); nouveau `nvkm_vmm_ref_sptes`/`unref_sptes`, `vmm.c:229-251`, `:328-358` |
| PTE `VALID=0, VOL=1` | sparse: reads return 0, writes dropped, no fault | `kern_gmmu_gm200.c:41-50`; nouveau `gp100_vmm_pgt_sparse` (`vmmgp100.c:162-167`) |
| big PTE `VALID=0, PRIV=1` | "invalid": the MMU does **not** look at the small PTEs, access faults | nouveau `gp100_vmm_lpt_invalid` (`vmmgp100.c:182-187`) |
| big PTE `VALID=0, VOL=1, PRIV=1` | "NV4K", only for ATS-enabled VA spaces | `kgmmuFmtFamiliesInit_GV100` (`kern_gmmu_gv100.c:61-75`), `mmu_walk.c:1196-1200` |
| PDE `APERTURE=0, VOL=1` | sparse directory | `kern_gmmu_gm200.c:52-66` |

### 2.3 Which half of a dual PDE the hardware follows

For a VA under a PD0 entry with both halves valid, the MMU looks at the **big PTE first**: a valid
big PTE maps the whole 64 KiB; a zero big PTE sends it to the small PTE; a sparse or LPT-invalid big
PTE stops it (nouveau's state machine: "Sparse LPTEs prevent SPTEs from being accessed", "MMU
supports blocking SPTEs by marking an LPTE as INVALID", "LPTEs that overlap valid SPTEs are
unmapped as opposed to invalid or sparse, which would prevent the MMU from looking at the SPTEs",
`nouveau/subdev/mmu/vmm.c:247-251`, `:344-357`, `:438-446`). That order is an inference from these
comments; no NVIDIA document in reach states it. A PD0 entry with `IS_PTE` set is a 2 MiB page
(`gmmu_fmt.c:63-85`); its address must be 2 MiB aligned and a 64 KiB PTE's address 64 KiB aligned
(inference: the offset bits are taken from the VA).

### 2.4 Apertures, volatile, sysmem addresses

- VIDEO = FB PA (identity with the reservation here). SYSTEM_* = a DMA address: on this host an
  IOVA of the host IOMMU domain (sysmem-iova.md), which is what the guest writes after D13.
- COHERENT vs NON_COHERENT: a coherent access snoops CPU caches, a non-coherent one does not (PCIe
  no-snoop). The RM chooses NON_COHERENT for every non-cached memdesc
  (`kgmmuGetMemAperture_IMPL`, `kern_gmmu.c:587-606`); the RAMIN/RAMFC equivalents are
  `_SYS_MEM_NONCOHERENT` targets (`ram.ref` ENGINE_WFI_TARGET, PAGE_DIR_BASE_TARGET).
- VOL marks an access that must not be cached in the GPU's L2 for sysmem; nouveau sets it for every
  host-memory mapping by default (`gp100_vmm_valid`, `vmmgp100.c:430`).

### 2.5 After CPU writes to page tables: flush, invalidate, membar

- **Register invalidate** (what the RM kernel does, and what the guest writes to the device):
  `NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE_PDB` 0xB830A0 (PDB >> 12 in 31:4, aperture bit 1),
  `..._UPPER_PDB` 0xB830A4, `..._MMU_INVALIDATE` 0xB830B0 (TRIGGER 31, ALL_VA 0, ALL_PDB 1,
  HUBTLB_ONLY 2 for BAR spaces), then poll TRIGGER to 0 (`kgmmuCommitTlbInvalidate_TU102`,
  `kgmmuCheckPendingInvalidates_TU102`, `RM/gpu/mmu/arch/turing/kern_gmmu_tu102.c:48-117`;
  `kgmmuInvalidateTlb_GM107`, `RM/gpu/mmu/arch/maxwell/kern_gmmu_gm107.c:106-290`; nouveau
  `tu102_vmm_flush`, `vmmtu102.c:26-50`).
- **Before** the invalidate, after the PTE writes: `kbusFlush_HAL(VIDEO | SYSTEM)`
  (`dmaUpdateVASpace` end, `RM/gpu/mem_mgr/arch/maxwell/virt_mem_allocator_gm107.c:2566-2573`; the
  sparse path `gpu_vaspace.c:1423-1425`). On GV100+ `kbusFlushSingle_GV100` does a full CPU fence
  for SYSTEM and **one read of `pReadToFlush` through BAR2** for VIDEO
  (`RM/gpu/bus/arch/volta/kern_bus_gv100.c:331-400`).
- **After** the invalidate, for PTE downgrades (unmap, permission loss) outside BAR2: a
  `SYSMEMBAR` (`kgmmuSetTlbInvalidateMembarWarParameters` + `kbusSendSysmembar`,
  `kern_gmmu_gm107.c:231-234`, `:266-279`).
- **In a pushbuffer** (what UVM and CUDA do on their own channels): `NVC46F_MEM_OP_A..D`
  (`clc46f.h:114-199`): `MEM_OP_C.TLB_INVALIDATE_PDB` ONE with `PDB_APERTURE` and `PDB_ADDR_LO`,
  `MEM_OP_D.TLB_INVALIDATE_PDB_ADDR_HI` and `OPERATION = MMU_TLB_INVALIDATE (9)`, optionally
  `MEM_OP_A.TLB_INVALIDATE_SYSMEMBAR`; a pure barrier is `OPERATION = MEMBAR (5)` with
  `MEM_OP_C.MEMBAR_TYPE = SYS_MEMBAR` (`uvm_hal_turing_host_membar_sys`,
  `uvm_turing_host.c:41-47`). Because these carry the guest's PDB address and the guest PDB is the
  host channel's PDB by identity, they are correct as-is on a mirrored channel.
- **For a host RM client**: `NV0080_CTRL_CMD_DMA_INVALIDATE_TLB` (0x80180c, non-privileged,
  `ctrl0080dma.h:456-495`) invalidates the PDB **of the RM walker's root**
  (`deviceCtrlCmdDmaInvalidateTLB_IMPL`, `RM/gpu/mem_mgr/dma.c:963-1010` -> `gvaspaceInvalidateTlb_IMPL`,
  `gpu_vaspace.c:2086-2131`, root from `mmuWalkGetPageLevelInfo`). It does no `kbusFlush` first.
  For an externally owned VAS the walker root is the `SET_PAGE_DIRECTORY` root (the guest root).
  For a replayed VAS it is **the host kernel's own root**, not the guest's (§4.4).

### 2.6 `D/fb.rs` walker against the hardware: every divergence

"HW" = the physical GMMU as specified above; "walker" = `FakeFb::translate_in` and friends
(`D/fb.rs:557-658`) as used by the software executor (`D/channel.rs:94-129`).

| # | Case | HW would | Walker does |
|---|---|---|---|
| 1 | dual PDE, big PTE **valid** and small PTE valid for the same VA | use the 64 KiB big page (§2.3, inference) | uses the small PTE (`fb.rs:644-650`) |
| 2 | big PTE `VALID=0, VOL=1` (sparse) over a valid small PTE | sparse: read 0, drop writes | uses the small PTE |
| 3 | big PTE `VALID=0, PRIV=1` (LPT invalid) over a valid small PTE | fault | uses the small PTE |
| 4 | PD0 entry with big aperture INVALID + `VOL_BIG` (sparse PDE) | the whole 2 MiB is sparse | ignores `VOL_BIG`, walks the small half (`big_table` returns `None`, `fb.rs:570-576`) |
| 5 | PD3/PD2/PD1 entry with aperture INVALID + VOL (sparse directory) | sparse, no fault | `Unmapped { level }` (`next_table`, `fb.rs:560-566`), the executor errors out |
| 6 | PDE `IS_PTE` bit 0 set at PD3/PD2/PD1 | undefined on Turing | ignored, followed as a directory |
| 7 | PTE `READ_ONLY` on a write | MMU fault | write done (`decode_pte` never tests bit 6, `fb.rs:578-591`; fb-model.md §4b promised a drop) |
| 8 | PTE `PRIVILEGE` | only privileged clients may access; others fault (inference: `mmu.ref` defines the bit without prose) | ignored |
| 9 | **TLB** | caches PDEs and PTEs per PDB until an invalidate for that PDB; a PTE the guest changed is not seen until then | no TLB: every access re-walks, so it never sees a stale entry |
| 10 | aperture 3 (SYS_NON_COHERENT) | no-snoop DMA read: sees DRAM, not dirty CPU cache lines | coherent CPU read via `GuestMemory` (`decode_pte`, `fb.rs:586-589`, same as aperture 2) |
| 11 | aperture 1 (PEER) | peer access (none on this host: fault) | `Unmapped { level: 0 }` |
| 12 | `KIND` compressible (0x08-0x0E) with `COMPTAGLINE` | decompresses through comptags; with no comptag backing (the reservation is COMPR NONE) the result is undefined or an error (host-gr.md's reading of the CE stall) | ignored, raw bytes |
| 13 | big-page size | from `NV_RAMIN_BIG_PAGE_SIZE` of the host instance block | hard-coded 64 KiB: VA[20:16] and a 16-bit offset (`fb.rs:652-655`) |
| 14 | VIDEO address outside the reservation `[B, E)` | reads/writes real host VRAM there (host data, or WPR: faults) | `VramStore::pieces` sends it to a host-RAM side store (`Piece::Outside`, `fb.rs:365-373`, `:412`); counted in `outside`, but the access **succeeds** on fake bytes |
| 15 | VIDEO address inside the reservation when the host refused every BAR1 window | real VRAM | reads zeros, drops writes silently (`Piece::Unmapped`, `fb.rs:413`, `:429`) |
| 16 | VIDEO address ≥ the FB address space | fault | `read` returns false (`FakeFb::read`, `fb.rs:529-536`): the executor errors, `bar_access` returns zeros |
| 17 | SYSTEM address | host IOMMU translates the IOVA; an IOVA the host never registered faults in the IOMMU | `GuestMemory::translate` through the device's DMA table; the guest's swiotlb **alias** at 0xf000000000 is in the guest's table only (memory-model.md §2), so the two can disagree there |
| 18 | PTE address alignment for 64 KiB / 2 MiB pages | offset bits come from the VA (inference) | adds the VA offset to whatever address is stored |
| 19 | VA bits above 48 | out of the 49-bit space (PBDMA: above 39 invalid) | silently masked (`(va >> 47) & 3`, `fb.rs:613`) |
| 20 | root size | `numEntries` of `SET_PAGE_DIRECTORY` bounds the root | reads any of the 4 root entries (`root_entries` is recorded but not enforced) |

What is **not** a divergence: the address field widths and shifts (PDE 32:8/53:8 `<< 12`, big half
32:4/53:4 `<< 8`, small half at bit 72 `<< 12`, PTE 32:8/53:8 `<< 12`) match `mmu.ref` exactly, and
so do the level indices.

---

## 3. Write-combining, BAR1, PRAMIN: what the RM does before the GPU may walk

- **The primitive.** `osFlushCpuWriteCombineBuffer` is a bare `sfence` on x86-64
  (`os_flush_cpu_write_combine_buffer`, `kernel-open/nvidia/os-interface.c:1101-1110`). It drains
  the CPU's WC buffers onto PCIe as posted writes; it does not wait for them to reach FB.
- **The confirmation.** A non-posted read from the same BAR afterwards cannot pass the posted
  writes (PCIe ordering), so when it returns the writes are in the GPU's L2. The RM's standard form
  is `kbusFlush_HAL(BUS_FLUSH_VIDEO_MEMORY)` = one `MEM_RD32` of `pReadToFlush`, an FB page mapped
  through BAR2 (`kbusFlushSingle_GV100`, `kern_bus_gv100.c:380-400`, the page set up by
  `kbusSetupCpuPointerForBusFlush_GV100`, `:50-99`), and for sysmem a full CPU fence
  (`portAtomicMemoryFenceFull`, `:357-360`).
- **BAR2 PTE updates by the kernel** (`kbusUpdateRmAperture_GM107`, `kern_bus_gm107.c:2750-2786`):
  PTE writes, then `osFlushCpuWriteCombineBuffer()`, then `kbusFlush_HAL` (the read) or, before
  `pReadToFlush` exists, `kbusSendSysmembar`, then `kgmmuInvalidateTlb_HAL` for the BAR2 PDB.
  During bootstrap through PRAMIN it reads the last entry back through PRAMIN as the flush
  (`kern_bus_gm107.c:2464-2475`, fb-model.md §1.2).
- **User VA spaces** (`dmaUpdateVASpace`): all PDE/PTE writes go through BAR2 memmgr transfers
  (`gmmu_walk.c` fills and copies; there is no per-write flush in `gmmu_walk.c` or `mem_utils.c`),
  and one `kbusFlush_HAL(VIDEO | SYSTEM)` plus one TLB invalidate close the batch
  (`virt_mem_allocator_gm107.c:2566-2573`, and `:3032-3034`).
- **VerifyBar2** uses the same pair plus an L2 evict RPC (`kern_bus_gm107.c:4189-4196`).
- **L2**: BAR1/BAR2 CPU accesses and GPU engine accesses to VIDEO meet in L2, so no L2 flush is
  needed between a CPU write through BAR and an engine read of VRAM; the RM issues L2
  evicts only in the self-test.

**What the device's VRAM store has to copy:** after any batch of guest writes that the physical GPU
may read (page tables, GPFIFO, pushbuffer, USERD, context buffers), (1) `sfence`, (2) one read
through **each** BAR1 window that received writes since the last flush (a read in another window
does not order them: different mappings, same device, so PCIe ordering does hold, but a window
that was `munmap`ed by eviction has no mapping left to read through; flush it before eviction),
(3) then the host invalidate for the right PDB, (4) then the doorbell. The natural trigger points
are the guest's own ones: its `pReadToFlush` read through the device's BAR3 (the guest's
`kbusFlush`), its MMU_INVALIDATE register write (0xB830B0), and its doorbell. Today the device
flushes only at the doorbell (`D/ctrl.rs:207-210`) and at the forwarded invalidate
(`D/mirror.rs:895-921`, which does not flush first).

---

## 4. Server-reserved PDEs and the GR context in GSP-client mode

Both RMs in this setup are GSP clients: the guest's kernel RM (towards the device's fake GSP) and
the host's kernel RM (towards the real GSP). The same code runs on both sides.

### 4.1 How a client VA space reserves a range for the server

- `FERMI_VASPACE_A` takes `NV_VASPACE_ALLOCATION_PARAMETERS {index, flags, vaSize,
  vaStartInternal, vaLimitInternal, bigPageSize, vaBase, pasid}` (`sdk/.../nvos.h:3100-3157`;
  flags `SHARED_MANAGEMENT` bit 2, `IS_EXTERNALLY_OWNED` bit 3, `VA_INTERNAL_LIMIT` bit 7,
  `nvos.h:3158-3173`). The kernel sends the same parameters to its server
  (`NV_RM_RPC_ALLOC_OBJECT` in `vaspaceapiConstruct_IMPL`,
  `RM/gpu/mem_mgr/vaspace_api.c:227-275`). With `VA_INTERNAL_LIMIT` the kernel ignores the
  caller's `vaStartInternal/vaLimitInternal` and uses `[4 GiB, 4.5 GiB)`
  (`vaspace_api.c:370-375`).
- In `gvaspaceConstruct_IMPL`, for every VAS except BAR/FLA/PMU/HDA/HWPM/PERFMON
  (`gpu_vaspace.c:578-593`), `gvaspaceReserveSplitVaSpace_IMPL` (`gpu_vaspace.c:394-457`) sets
  `vaStartServerRMOwned = min(vaLimitInternal - 512 MiB + 1, 4 GiB)` and
  `vaLimitServerRMOwned = start + 512 MiB - 1` (`SPLIT_VAS_SERVER_RM_MANAGED_VA_START`
  0x1_0000_0000, `_SIZE` 0x2000_0000, `generated/g_gpu_vaspace_nvoc.h:90-91`), 2 MiB aligned, and
  on a client (`IS_GSP_CLIENT`) calls `_gvaspaceReserveVaForClientRm` (`gpu_vaspace.c:305-360`):
  1. the range becomes non-allocatable for the client;
  2. the client's walker **reserves and pins the page levels down to PD0** for the range
     (`mmuWalkReserveEntries` at the PD0 level): root, PD2, PD1 and the PD0 page covering
     `[4 GiB, 4.5 GiB)` exist and are fixed;
  3. `gvaspaceCopyServerRmReservedPdesToServerRm_IMPL` (`gpu_vaspace.c:3825-3935`) fills the
     parameters with `_gvaspacePopulatePDEentries` (`:5003-5040`: `pageSize` 2 MiB,
     `virtAddrLo/Hi` = the server range, one `{physAddress, size, aperture, pageShift}` per level
     from `gvaspaceGetPageLevelInfo`, i.e. root/PD2/PD1/PD0) and sends
     `NV90F1_CTRL_CMD_VASPACE_COPY_SERVER_RESERVED_PDES` (0x90f10106, `ctrl90f1.h:268-332`,
     PRIVILEGED, flags 0x14004) to the server by RPC (`_gvaspaceCopyServerRmReservedPdesToServerRm`,
     `:4944-4975`). The contract, verbatim: "server RM can mirror these PDEs in its mmu walker
     state. Any lower level PDEs/PTEs allocated under these top level PDEs will be modified
     exclusively by server RM. Client RM won't touch those." (`:3906-3912`).
- **Server side** (`gvaspaceCopyServerReservedPdes_IMPL`, `gpu_vaspace.c:4217-4336`, the branch
  after `IS_GSP_CLIENT`): reserves page tables for `virtAddrLo..Hi`, invalidates the TLB
  (`:4272`), and for each level, bottom-up, `mmuWalkModifyLevelInstance(..., vaStartServerRMOwned,
  memdesc(levels[i]))` (`:4296-4333`): the server's walker **adopts the client's pages** as its own
  level instances for that range. From then on the server writes PD0 entries and page tables
  under the client's PD0 page for `[4 GiB, 4.5 GiB)`.
- **On a client** the same control is not executed locally: `gvaspaceCopyServerReservedPdes_IMPL`
  forwards it to its own server and returns (`gpu_vaspace.c:4245-4257`). This matters for the
  mirror (§4.4).

What the guest does with it: the device records it and takes `levels[0]` as the VAS root
(`D/channel.rs` `record_server_reserved_pdes`, `:593-630`). Measured on vm2: root 0xe9000000,
PD2 +0x1000, PD1 +0x2000, PD0 +0x3000, range 0x1_0000_0000..0x1_1fff_ffff
(`caraxes/runs/e5b2-2026-09-23/b2-mirror-trace-excerpt.txt`, first lines). The fake GSP maps
nothing there, so the guest's PD0 page for the range stays empty.

### 4.2 Who maps the GR context buffers in GSP-client mode

- The **client** allocates and maps them: `_kgrAlloc` calls `kgrctxAllocCtxBuffers` and
  `kgrctxMapCtxBuffers` when `kgrctxShouldManageCtxBuffers` (= client-RM-allocated ctx buffers
  enabled, `kernel_graphics_context.c:2623-2631`), then `kgrobjPromoteContext`
  (`RM/gpu/gr/kernel_graphics_object.c:173-227`).
- `kgrctxMapCtxBuffers_IMPL` (`kernel_graphics_context.c:1615-1705`) maps into the **channel's
  VAS**: the main context (`ppEngCtxDesc[]->pMemDesc`), the patch buffer, the PM buffer if any,
  the register access map (priv access map), and, only for 2D/3D objects or a legacy (non-subcontext)
  TSG, the global buffers through `kgrctxMapGlobalCtxBuffers` (bundle CB, pagepool, attribute CB,
  RTV CB, GfxP pool and control block). The FECS event buffer is mapped at every object
  allocation (`kernel_graphics.c:1813-1824`). **For an externally owned VAS it returns at once**
  (`kernel_graphics_context.c:1638-1639`), and `kgraphicsMapCtxBuffer` asserts on one
  (`kernel_graphics.c:1936`).
- The server learns the result through `NV2080_CTRL_CMD_GPU_PROMOTE_CTX` (0x2080012b,
  `ctrl2080gpu.h:919-968`; flags 0x10244 = PRIVILEGED | ROUTE_TO_PHYSICAL | ROUTE_TO_VGPU_HOST |
  GSP_PLUGIN, `generated/g_subdevice_nvoc.c:4141-4150`, `RM/../inc/kernel/rmapi/control.h:199-287`;
  the handler lives in GSP-RM, not in the open tree). Each entry is
  `{gpuPhysAddr, gpuVirtAddr, size, physAttr, bufferId, bInitialize, bNonmapped}`:
  - `kgrctxPrepareInitializeCtxBuffer_IMPL` (`kernel_graphics_context.c:1713-1852`): for buffers
    not yet initialized (MAIN, PATCH, PM, FECS_EVENT, PRIV_ACCESS_MAP, ...): `gpuPhysAddr`, `size`,
    `physAttr` (aperture VIDMEM/COH_SYS/NCOH_SYS, GPU-uncached), `bInitialize = TRUE`,
    `bNonmapped = TRUE`. The server then initializes the buffer at that PA (for MAIN: from its
    golden context image);
  - `kgrctxPreparePromoteCtxBuffer_IMPL` (`:1885-1982`): `gpuVirtAddr` = the VA the client
    mapped the buffer at in this channel's VAS, `bNonmapped = FALSE`. **Skipped for an externally
    owned VAS** ("RM is not responsible for promoting the buffers when UVM is enabled", `:1907-1909`).
  - Which IDs (`kgrobjGetPromoteIds_FWCLIENT`, `kernel_graphics_object.c:580-652`): compute in a
    subcontext TSG: MAIN, PM, PATCH, FECS_EVENT, PRIV_ACCESS_MAP, UNRESTRICTED_PRIV_ACCESS_MAP;
    2D/3D or legacy TSG adds BUNDLE_CB, PAGEPOOL, ATTRIBUTE_CB, RTV_CB_GLOBAL, GFXP_POOL,
    GFXP_CTRL_BLK.
- The guest's own promotion, as recorded (`caraxes/runs/e5c-2026-09-23/c5-core-pass-jit-wall/
  device/cuda-jit.trace:1218`): MAIN pa 0x1e8512000 va 0x1_2001_0000 size 0xee000 init; PATCH
  pa 0x1e8aab000 va 0x1_200f_e000 size 0x5000 init; FECS_EVENT (9) va 0x1_2000_0000 VA-only;
  PRIV_ACCESS_MAP (10) va 0x1_2011_0000 VA-only. All VAs sit just above the server range, in the
  guest's client-internal range; all PAs lie in the reservation.
- The engine context pointer the GR engine loads is a **virtual** address in the channel's VA
  space: `NV_RAMIN_ENGINE_WFI_MODE` VIRTUAL, `ENGINE_WFI_PTR_LO/HI`, `ENGINE_WFI_VEID` (`ram.ref`
  RAMIN words 132-134); with subcontexts GR translates through `NV_RAMIN_SC_PAGE_DIR_BASE(veid)`
  (`ram.ref` `SC_PDB_VALID`, 166·32+i). So FECS resolves MAIN, and through the patch buffer the
  others, **via the channel's PDB**.

### 4.3 Why a bare PROMOTE_CTX replay is refused, and what an externally owned VAS changes

- Externally owned VAS (`IS_EXTERNALLY_OWNED`, then `DMA_SET_PAGE_DIRECTORY`): RM allocates no
  page tables and no VA (`gvaspaceExternalRootDirCommit_IMPL`, `gpu_vaspace.c:2946-2959`, the
  comment there), maps no GR buffers (`:1638`), promotes no VAs (`:1908`), and
  `kchannelIsSchedulable_IMPL` refuses a GR channel on it unless `bIsContextBound`
  (`RM/gpu/fifo/kernel_channel.c:2130-2136`). Only `nvGpuOpsBindChannelResources` sets that flag,
  after sending PROMOTE_CTX with the VAs the external owner mapped (`RM/rmapi/nv_gpu_ops.c:
  11060-11140`); that is nvidia-uvm's kernel interface, not reachable from a user client. The owner
  learns what to map from `NV2080_CTRL_CMD_GR_GET_CTX_BUFFER_INFO` (0x20801219) or the internal
  `KGR_GET_CTX_BUFFER_PTES` (0x20800a28), both kernel-privileged (flags 0x8000 only,
  `g_subdevice_nvoc.c`).
- The measured `NV_ERR_OBJECT_NOT_FOUND` for a user PROMOTE_CTX on an externally owned GR channel
  (host-exec.md stage F) is produced inside GSP-RM (the control is routed to physical); the open
  source cannot say which lookup failed (inference: the server finds no context state it expects to
  promote for that channel).
- A **replayed** (non-external) mirror VAS avoids the schedulability rule, but then the host kernel
  maps and promotes **its own** buffers into **its own** page tables (§4.4).

### 4.4 What the host does with the replayed VA space (the b2 configuration)

1. `FERMI_VASPACE_A` with the guest's parameters plus `VA_INTERNAL_LIMIT` (`D/mirror.rs:514-535`):
   the host kernel builds its own root/PD2/PD1/PD0 (in host memory), reserves `[4 GiB, 4.5 GiB)`
   for the host GSP and sends **its** levels to the host GSP (§4.1).
2. The device's `COPY_SERVER_RESERVED_PDES` with the guest's root/PD2/PD1 (`D/mirror.rs:536-556`)
   is forwarded unchanged to the host GSP (`gpu_vaspace.c:4245-4257`); the GSP walker adopts the
   guest's pages. The **host kernel's walker keeps its own root**.
3. Channel PDB: the evidence points at the guest root (the GPFIFO at guest VA 0x2_0020_0000 was
   fetched; a PDB on the host kernel's root would have raised an MMU fault, Xid 31, not Xid 38)
   (inference: the RAMIN is written by GSP from its VAS state, not visible in the open source).
4. The GR object allocation (`TURING_COMPUTE_A` on the mirror channel) makes the host kernel
   allocate MAIN/PATCH/... and map them with **its own walker**, i.e. under the host kernel root,
   then PROMOTE_CTX those VAs to GSP (§4.2).
5. FECS loads the main context at a VA that the guest root does not map to the host's buffer: it
   is either unmapped there or, if the host and guest RMs picked the same VA (same allocator, same
   internal range, inference), mapped to the **guest's** MAIN buffer, which nobody initialized (the
   fake GSP only records `bInitialize`, `D/channel.rs` `record_promote_ctx`). Either way FECS
   cannot load a valid context: the Xid 38 on every GR mirror channel at its first push
   (`b2-host-xid.txt`). Xid 38 is not defined in the open headers (`nverror.h` skips 37 and 38);
   treat it as a firmware-reported context-switch failure (inference). The logged class 0xc597
   (TURING_A) is what GSP reports for the channel's GR context, not an object the mirror allocated.
6. `NV0080_CTRL_CMD_DMA_INVALIDATE_TLB` on this VAS invalidates the host kernel's root (§2.5), so
   the device's forwarded invalidates (`D/mirror.rs:895-921`) miss the guest root the channel runs
   on. Use a `MEM_OP` TLB invalidate with the guest PDB in a pushbuffer instead, or rely on the
   guest's own MEM_OPs.

### 4.5 The rule: what the device must do for a host-built GR context to run the guest's push

Every address the physical GR engine dereferences for this channel must resolve **through the PDB
in the channel's RAMIN** (the guest root, and the same root as `SC_PDB` for the channel's VEID).
That is three sets:

| Set | VAs | Written by | Source |
|---|---|---|---|
| A | the guest's own work: GPFIFO, pushbuffers, QMDs, constant buffers, shader and local memory, semaphores | the guest (its RM and user driver) | guest tables, correct by identity |
| B | the context buffers FECS and the pipeline use: MAIN (the RAMIN `ENGINE_WFI_PTR`), PATCH, PM, PRIV_ACCESS_MAP, FECS_EVENT, and for 2D/3D the global CBs | the **client RM** of the VAS (§4.2) | must be mapped in the guest chain at exactly the VAs promoted to the host GSP |
| C | the server-reserved range `[4 GiB, 4.5 GiB)` | the **server** (host GSP), under the PD1 entry the guest reserved for it (`gpu_vaspace.c:3906-3912`) | handed over by the replayed `COPY_SERVER_RESERVED_PDES` (root, PD2, PD1; the host GSP brings its own PD0, `D/mirror.rs:506-513`) |

Consequences:

- **Set B cannot come from the host kernel** as long as the host kernel's walker root is not the
  guest root: its mappings land in tables the channel never walks, and a user client can neither
  read their VAs/PAs (`GR_GET_CTX_BUFFER_INFO` is kernel-privileged) nor point guest PTEs at host
  buffers outside the reservation without breaking the isolation rule (`FbLayout::inside`,
  fb-model.md §5). `SHARED_MANAGEMENT` + `SET_PAGE_DIRECTORY` does migrate the RM's root into the
  client's page, but it copies the RM's root entries for `[vaStart, vaLimitInternal]` into it
  (`_gmmuWalkCBCopyEntries_SkipExternal`, `gpu_vaspace.c:2774-2800`; commit at `:3000-3027`): at PD3
  granularity (128 TiB per entry) that is entry 0, the entry that carries every guest mapping, so it
  would cut the guest's own tables off (inference from the entry arithmetic).
- **So set B has to be the guest's own buffers** (the guest's promote entries, §4.2): the guest RM
  already mapped them in the guest chain at VAs above the server range, with PAs in the
  reservation. What is missing is the server's half of the guest's PROMOTE_CTX: (1) initialize the
  guest's MAIN (golden image) and PATCH at their PAs, and (2) bind those VAs to the host channel.
  Only the host GSP can do both, and only through a PROMOTE_CTX for the host channel that carries
  the guest's entries (VAs for all, `bInitialize` with the guest's PAs for MAIN/PATCH). With a
  replayed VAS the host kernel has already promoted its own buffers at object allocation, so this
  would be a second promotion; whether GSP accepts it from a privileged user client is **untested**
  (host-exec stage F was on an externally owned VAS and got OBJECT_NOT_FOUND). This is the (c2)
  experiment, and the only configuration in which the guest's promoted buffers matter: they must
  then be the ones FECS uses, consistent with the guest's view (the guest may read back or patch
  them).
- Under (c1), a context built by the host from its own golden image, the guest's promoted buffers
  are **unused** by the hardware, but their VAs must not collide with the host kernel's choices,
  and the host's buffers must still be reachable through the guest root: not achievable from user
  space with a replayed VAS (above). The practical (c1) is therefore a host channel whose VAS root
  is the host kernel's own (a normal host VAS), with the guest's set-A mappings copied into the
  host's tables: the shadow page table the identity design set out to avoid (fb-model.md §5,
  dma-routes.md).
- Set C needs nothing from the device beyond the replay it does today, provided the guest never
  maps into `[4 GiB, 4.5 GiB)` (it cannot: the guest RM reserved it for its server) and the host GSP
  gets exactly root/PD2/PD1 (with PD0 handed over it rewrites the guest's reserved PD0 page, which
  is harmless, but the mirror's measurement with four levels was the Xid 38 case, `D/mirror.rs:506-509`).

**Checklist for the next GR experiment (c2):** (1) replayed VAS as today; (2) after the host
`TURING_COMPUTE_A` allocation, send `GPU_PROMOTE_CTX` (CAP_SYS_ADMIN window) for the host channel
with the guest's recorded entries, `engineType` GR0, `hChanClient/hObject` = the host channel (not
the TSG); (3) flush the BAR1 windows first so the guest's MAIN/PATCH pages are in VRAM; (4) invalidate
the guest root by a `MEM_OP` in a host push, not by `DMA_INVALIDATE_TLB`; (5) read the RC-diag
records (§1.3) after any RC to see what FECS reported.
