<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# GSP boot exchange

The boot exchange is the ordered sequence of register writes, firmware loads and the first
messages by which the kernel RM brings GSP-RM up. A synthetic device answers each step with
the value the CPU checks next; no firmware runs. This page gives the Turing order of
615.71.09 and the differences on Blackwell and Hopper.

## Turing (`kgspBootstrap_TU102`)

| # | Side | Step | Where |
|---|---|---|---|
| 1 | CPU | parses the VBIOS from the PROM window (BAR0 0x300000), finds the FWSEC image and builds the FRTS command in a guest-RAM buffer | `kgspExtractVbiosFromRom_HAL`, `src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c` |
| 2 | CPU | creates the command queue in guest RAM (`msgqTxCreate`) and writes `NV_PGSP_QUEUE_HEAD(0) = 0` while the GSP falcon is halted | `src/nvidia/src/libraries/msgq/msgq.c` |
| 3 | CPU | loads FWSEC into the GSP falcon, starts it, waits for the halt and the WPR2 registers; the device derives the WPR2 values from the FRTS command the CPU wrote | `src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_tu102.c`, the falcon model |
| 4 | CPU | resets the falcon into RISC-V, writes the libos-argument page address into `MAILBOX0/1` | `kernel_gsp_tu102.c` |
| 5 | CPU to SEC2 | Booter Load with the WPR meta address; the device links its status queue here, because the next step arrives without a doorbell | `kernel_gsp_tu102.c` |
| 6 | CPU | `GspStatusQueueInit` polls `msgqRxLink` for the device's queue header (4 s), then writes `NV_PGSP_MSGQ_TAIL(0) = 0` | 615: `kernel_gsp_tu102.c:469`, `msgq.c:270-272` |
| 7 | CPU to GSP | `GMCAPI_CMD_GSP_INIT` with the system information and every registry key as NVKV words, `QUEUE_HEAD(0) = 1` | 615: `kernel_gsp_tu102.c:473`, `kernel_gsp.c:4893-5217` |
| 8 | GSP to CPU | the static configuration as NVKV: GPU names, GID, VBIOS fields, FB length and regions, BAR PDE bases, engine mask, feature words; the response is the init completion | 615: `kernel_gsp.c:5328-5344` |
| 9 | CPU to GSP | `INIT_GSP_TRACE_CRASH_BUFFER`, the internal client, device and subdevice, the first `GSP_RM_CONTROL`s, `UPDATE_BAR_PDE`, the event import's `ALLOC_MEMORY` | `RmInitAdapter` in `src/nvidia/arch/nvalloc/unix/src/osinit.c` |

- A 615 boot carries 272 RPCs (189 controls, 50 allocations, 30 frees). In 610 step 7 and 8
  were four RPCs (`GSP_SET_SYSTEM_INFO`, `SET_REGISTRY`, `GET_GSP_STATIC_INFO`) and a
  `GSP_INIT_DONE` event.
- The static-configuration decoder is strict: a known key of the wrong type, index or length
  fails the whole init with `NV_ERR_INVALID_ARGUMENT`; an unknown key is ignored.
- Nothing in the VBIOS executes; the captured ROM holds two PCI images, and the device
  appends a constructed third one that passes every parser check (decision D8,
  [caraxes/vbios-synthesis.md](../caraxes/vbios-synthesis.md)).

## Teardown, suspend, reload

- Unload sends `GMCAPI_CMD_GSP_SUSPEND` and waits for the processor-suspended bit in
  `MAILBOX0`; then the falcon reset with FWSEC-SB, Booter Unload on SEC2, and a
  function-level reset by the OS before the next load.
- On resume the kernel sets `IN_PM_TRANSITION`, skips the queue link and waits for
  `GSP_RESUME_DONE` instead of sending `GSP_INIT` (615: `kernel_gsp_tu102.c:463-478`).
- A `QUEUE_HEAD` write of 0 while the device's read pointer is not 0 means the queue was
  created anew (reload without reset); the device drops the link.

## Blackwell and Hopper (FSP Chain of Trust)

- From GH100 on `kgspExtractVbiosFromRom` is a stub: no ROM parsing, no FWSEC, no Booter.
  FRTS is set up by the FSP.
- The CPU polls `NV_THERM_I2CS_SCRATCH` for FSP boot complete (GB20x at 0xAD00BC, GH100 at
  0x200BC), then sends MCTP-framed NVDM packets over the FSP EMEM channel: `CAPS_QUERY`,
  `CLOCK_BOOST`, then the COT payload naming the GSP-FMC image, FRTS and
  `GSP_FMC_BOOT_PARAMS` (`src/nvidia/src/kernel/gpu/fsp/arch/blackwell/kern_fsp_gb202.c`,
  `kern_fsp_cot_payload.h`).
- After the COT the CPU waits for target-mask and lockdown release in `HWCFG2` and checks
  `RISCV_CPUCTL.ACTIVE_STAT`; only then does 615 link the status queue and send `GSP_INIT`
  (`src/nvidia/src/kernel/gpu/gsp/arch/hopper/kernel_gsp_gh100.c`).
- The queue and element layer is the same HAL for TU10x through GB20x
  ([gsp-message-queue](gsp-message-queue.md)).

## Depth

- [research/gsp-transport.md](../research/gsp-transport.md): the cited 615 lines.
- [caraxes/gsp-plan/boot-exchange.md](../caraxes/gsp-plan/boot-exchange.md): every step, the
  device's status per step, GC6 and RTD3.
- [caraxes/gsp-boot-turing.md](../caraxes/gsp-boot-turing.md), [caraxes/gsp-boot-blackwell.md](../caraxes/gsp-boot-blackwell.md):
  every register in order.
