# Windows' view of the driver start failure (ETW, 2026-09-25 21:24 to 21:45)

Setup: `vfio-user-nvgpu-v615-extcfg-ifr-d4d37c2f` in pure emulation, `rom-legacy-only.bin` ROM with the ROM BAR on,
the full 1 MiB PROM window (`prom-window-2026-09-25.bin`), QEMU with 4 GiB of RAM, GeForce driver 617.14 (32.0.16.1714), RTX 2070 identity.
The boot's driver start failed the same way as runs P to R. After that, one driver restart ran under ETW.

## What was traced

- Device state after boot: `PCI\VEN_10DE&DEV_1F02&SUBSYS_40111458&REV_A1\3&11583659&0&30`, software key
  `{4d36e968-...}\0007`, **Code 43** (`CM_PROB_FAILED_POST_START`), **ProblemStatus 0x0**. The other `VEN_10DE`
  instances are phantoms left by earlier runs (other slots, the T4 costume).
- ETW session `nvstart`: Microsoft-Windows-DxgKrnl and Microsoft-Windows-Kernel-PnP, all keywords, level 0xff.
  Then `pnputil /restart-device` on that instance (19:29:11Z, returned "restarted successfully" after 65 s), 60 s wait,
  then stop. 76643 events, 0 lost, 156 s.
- Files (in `etw-driver-start/`):
  - `etw-summary.txt`: tracerpt summary.
  - `etw-events-filtered.txt.gz`: every DxgKrnl event, plus the PnP events that name `VEN_10DE` or carry a nonzero status.
  - `etw-events-no-sched.txt`: the same without the scheduler events 105, 106 and 262.
  - `eventlog-*.txt`: the event-log survey and the raw XML of the boot's failure events.
  - `dxgkrnl-549-history.txt`: every adapter start failure since the install.
  - `setupapi-ven10de.txt`, `system-events-filtered.txt`, `registry-before.txt`, `pnputil-restart.txt`.
  - `restart-trace-nonprom-pollcollapsed.log.gz`: the device trace of the restart, which is lines 1682746 to 3212165
    of `gpu-trace.log`. PROM reads are dropped. The 1420137 identical poll reads of `0x10a0a4` are collapsed to the
    first line, a count line and the last line.
- The 80 MB tracerpt XML and the ETL stay in the guest at `C:\etw\`. Not fetched.

## Failure events and status codes

- **DxgKrnl 139 (DxgkDdiStartDevice stop)** at 19:30:17.105564Z:
  - Status **0xC000009A** (`STATUS_INSUFFICIENT_RESOURCES`), NumVidPnSources 0.
  - The matching start event, 138, is at 19:29:12.411487Z, so **DxgkDdiStartDevice ran for 64.69 s**.
- **DxgKrnl 549** (Admin channel, "Adapter start failed"):
  - Status 0xC000009A, FailureReason 1 = `StartAdapter_DdiStartDeviceFailed`, GraphicsVendorId 0x10DE.
- **DxgKrnl 457** (Admin channel, "Start Device Failed", `StartDevice_Failure`): status 0xC000009A.
- **DxgKrnl 494**: "Adapter StartDevice has completed with status", EventData1 = 0xFFFFFFFFC000009A.
- **Kernel-PnP**:
  - 220 at 12.411 and 222 at 17.154 bracket IRP_MN_START_DEVICE.
  - 902 at 15.41 is the driver watchdog: the start is still running after 3 s.
  - 903 closes it after 64746 ms.
  - 821: the start request completes with Status 0.
  - 830: ProblemCode 43, ProblemStatus 0x0.
  - 1011: "surprise removed because reported as failed".
  - 860: problem 0 changes to 43, status 0x0.
- **Side results, not failures**:
  - DxgKrnl 161 at AddDevice: QueryInterface `{693a2cb1-...}` returns 0xC00000BB (`STATUS_NOT_SUPPORTED`).
  - DxgKrnl 131 on another device object: three times 0xC0000034.
  - Kernel-PnP 213 (DXGKrnl service): 0xC000010E.
- **The driver logs nothing itself**: no `nvlddmkm` event in System and no nvlddmkm provider in the trace.
  The System log only has UserPnp 20003 since boot.
- **The code is generic**: all 35 recorded adapter start failures carry 0xC000009A with reason 1
  (`dxgkrnl-549-history.txt`, since 16:11 local time). That includes the older runs that stopped after identification,
  before any falcon work. The status does not say where RM failed. The README's "no Kernel-PnP start event, silent"
  holds for the System log only. The DxgKrnl-Admin channel had these events all along.

## Correlation with the device trace

- Clock anchor: the device's first BAR0 read (`NV_PMC_BOOT_42`, t = 295.369 s) is taken as the start of
  DxgkDdiStartDevice. That makes guest time about 0.29 s ahead of the device's time base. The restart's config
  reads then land on the PnP query-remove at 12.057.
- Timeline, in seconds from the start of DdiStartDevice:
  - 0 to 2.8: identification, the VBIOS through the PROM window, and the chip setup.
  - At 2.81 (`boot: gsp reset 8..10`, `gsp start 3`): FWSEC is loaded into the GSP falcon (IMEM through 0x110180/184)
    and started. The model reports it halted. The driver reads `GFW_BOOT` = 0xff and goes on.
  - At 2.84, on the PMU falcon: reads 0x10a040, 0x10a3c0 and 0x80, writes 0x10a014 = 0xffff, `NV_PFB_NISO_FLUSH_SYSMEM_ADDR`,
    0x10a048 = 0x100, then **writes 0x2 to 0x10a0a4 and polls it**. It reads back 0x2 on every read, 1.42 million times.
  - **The poll lasts 59.98 s**. This is RM's 60 s timeout.
  - Then about 1.9 s of teardown:
    - A PMU falcon debug dump: 0x10a200/0x10a20c pairs, 0x10a100 CPUCTL = 0x2, 0x10a104 = 0x10000.
    - The BAR1 block and PFIFO.
    - About 1 s of `NV_PMC_BOOT_0` / 0xb81000 reads.
    - A SEC2 falcon dump (0x840140/0x840144/0x84012c).
    - IRQ clears (`IRQMCLR` +0x14 := 0xffffffff) on SEC2, 0x1c8000 and 0x9a4000, and `NV_PMC_ENABLE` bits dropped.
    - A GSP reset (`gsp reset 11..14`) and a second FWSEC run (`gsp start 4`).
    - The PMU is prepared again: 0x10a0a4 = 0x2, the 0x10ae00 block, DMEM through 0x10a1c0/1c4.
    - `NV_PMC_ENABLE` is stepped down and the BAR2 block is cleared.
  - Last driver BAR0 access at device t = 360.0576 s, which is about 17.100Z in guest time.
- **Last driver access to the failure event: under 10 ms.** DxgKrnl 139 is at 17.1056Z. The error is the driver's own
  60 s timeout plus cleanup, not a Windows-side timeout. Windows then disables memory decode (cfg 0x04 := 1) and
  re-reads the IDs.

## What this suggests the device must answer

- The driver's start path stalls at a single register: **PMU falcon (0x10a000) + 0xa4**. The driver writes 0x2 and
  waits for the value to change. The device latches the write and returns it unchanged, so the wait always times out.
  Everything after that is cleanup.
- Next for the model:
  - An answer at 0x10a0a4, most likely the request bit cleared or acknowledged the way the PMU falcon does after the
    0x10a048 setup.
  - Then whatever the driver does after it. Per the coordinator's analysis, that is the ACR load and unload on the PMU
    (DMEM through 0x10a1c0/1c4 and the 0x10ae00 block, seen in the teardown's repeat).
- Windows adds nothing more specific: 0xC000009A is RM's generic start error as it reaches DxgKrnl. ProblemStatus stays 0.

## GSP-RM registry keys

- **Not done.** The Claude Code permission system refused the `reg add` of `EnableGpuFirmware` and
  `RMEnableGpuFirmware` in the guest ("Modify Shared Resources"). The keys were not written in any other way.
- State before the attempt, recorded in `registry-before.txt`:
  - No `*GpuFirmware*` value under `HKLM\SYSTEM\CurrentControlSet\Services\nvlddmkm` (including `Parameters`) or
    under the card's class key.
  - The class key is `Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0007`, found through
    `DEVPKEY_Device_Driver`. Several keys have the DriverDesc "NVIDIA GeForce RTX 2070"; 0007 is the live one.
  - 0007 has no `Parameters` subkey, only `CopyToVmWhenNewer`, `CopyToVmWhenNewerWow64` and `Session`.
    Place (d) does not exist.
- To run the test, the user has to allow it or run the commands themselves. Once the values are set, the success
  criteria are those from the task: SEC2 booter writes (0x840180/0x8401c0, CPUCTL 0x840100 := 2), GSP RISC-V
  registers (0x111000-0x111fff) or the queues (0x110c00-0x110cff), instead of the 0x10a0a4 poll.
