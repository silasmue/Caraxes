<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# NVENC and NVDEC per frame (2026-09-28)

Scope: where a frame's time goes when a 615 guest encodes and decodes (`caraxes/tools/nvenc-probe.c`, 300 frames of 1080p H.264, P1 ULL, CBR), what the device does per frame, the 610 delta, and the fixes ranked by gain per effort. Run: `caraxes/runs/night-2026-09-28/venc-cur2/` (lean vm2-615, device cb4974fc); trace `/mnt/vmstore/caraxes/night-2026-09-28/venc-cur2-gpu-trace.log` (`T:`, µs since device start); encode runs at T:139.09-142.71 s, 147.73-151.00 s and 155.74-159.50 s, each followed by its decode. `D` = `crates/vfio-user-nvgpu/src/`, `K`/`G`/`SDK` as in the other notes (615 tree).

## Method

- Frames are anchored on the first video-channel doorbell of each frame. Guest interrupts are the `msi re-arm` lines: the end of the guest ISR, 45-51 µs after its one trapped read.
- The `event: host notifier ... fired` lines cannot be used for timing. They are queued notes, written at the next trapped BAR0 write, so their timestamps are late [D/intr.rs:186-191, D/mirror.rs:5015, D/trace.rs:79]. Only their order within a group is valid.

## Facts

### Encode, one frame (run 2, p50)

- **Submit burst:**
  - The CUDA compute channel chid 8 (GR TSG, host runlist 0) rings 3.3 times. The CUDA copy channel chid 16 (TSG on COPY2, host runlist 8) rings once. Both go through the token eventfds.
  - The four host doorbells are serialised about 35-40 µs apart; ring to host doorbell is p50 33.9 µs, of which the `plan` step is 21.6 µs [venc-cur2/launch-timing.txt; T:149000168-149000286].
- **Video doorbells:** NVENC0 chid 28 (own host TSG, host runlist 2) rings twice, at +181 µs and 65 µs later. Both trap:
  - the library rings its own token 0x7001c;
  - it never reads the pool token the device writes into its work-submit-token entry [T:147726858; T:149000369, 149000484].
- **Waits:** two NVENC non-stall interrupts per frame (vector 9), both interrupt-driven. There is no guest polling in the loop: the only trapped reads are one per ISR, plus 15 usermode TIME reads in 3 s.
  - interrupt 1 at +2.0 ms;
  - interrupt 2 at +4.86 ms (later modes at 6.2-6.5 and 7.8-8.8 ms in about 40 % of frames);
  - every ISR does one trapped read of the replayable fault-buffer PUT (0xB8302C). nvidia-uvm is loaded [uvm-faults-2026-09-28.md; T:149002160].
- **Gaps with no device activity:**
  - +0.27 to +2.0 ms and +2.0 to +4.86 ms: host engine time;
  - then 3.2-3.9 ms of guest CPU, the fill of the next frame.
- **Nothing else per frame:** no software-executed ring, no shadow sync, no BAR1-pool move and no device-CPU copy. There is one MMU invalidate in 300 frames [T:147945000-150920000, every non-doorbell line].
- **Input upload:** the CPU fills the surface from `nvEncLockInputBuffer`; no copy engine is involved. The mapping sits in the BAR1 pool, moved once per session (707 pages cold) [device-summary.txt, pool lines]. This location is inferred from the pool counters.
- **Session setup, not per frame:** the library's bare COPY2 channel chid 29 (no TSG) runs on the software executor. It does 30 memsets and memcopies (23.9 MB) through the device's CPU in 172 ms (T:147.749-147.921 s). The UVM channels chid 4/6 likewise run only at setup and teardown.

### Decode, one frame (run 2, p50)

- **NVDEC:** chid 28 (host runlist 1) rings once. The ring traps, for the same reason as NVENC (token 0x4001c).
- **NVDEC interrupt:** vector 8 at +4.05 ms (p10 3.93, p90 4.28).
- **Post-process and copy to the host:** then 2 GR rings (chid 8) and 1.2 CE rings (chid 20, runlist 9). Their GR interrupt comes 1.31 ms later (p10 1.13, p90 1.58).
- **CPU phase:** 5.9 ms of guest CPU (PSNR over 2 MP, parse, next submit) until the next NVDEC ring [T:153007860-153019074].

### Budget per frame

| µs | native 615 | guest 615, run 2 | source |
|---|---|---|---|
| encode frame period | 5560-6010 | 9170 p50, 9940 mean | guest.txt, native/venc.txt, trace |
| fill (CPU) | 3210 | 3510 | fill_ms_total / 300 |
| first submit → NVENC doorbell | not measured | 181 | trace |
| NVENC doorbell → last NVENC interrupt | ≤ lat | 4860 (later modes 6200-6500, 7800-8800 in about 40 % of frames) | trace |
| probe latency (EncodePicture → LockBitstream) | 2320-2760 | 5150 (runs 1, 3: 5280, 8150) | probe |
| decode frame period | 6990-7110 | 11720 p50, 12580 mean | probe, trace |
| NVDEC doorbell → interrupt | not measured | 4050 | trace |
| post-process + copy to host → interrupt | not measured | 1310 | trace |
| decode CPU phase | ≈5000-5500 (period − GPU, estimate) | 5930 | trace |

- **Where the encode gap goes:** about 4 ms per frame on average.
  - 2.4 ms (p50) to 3.5 ms (mean) is host engine time, with the device idle;
  - 0.3 ms is the CPU fill;
  - about 0.15 ms is the submit path;
  - about 0.1 ms is the ISR's trapped read.
- **Where the decode gap goes:** about 2.5 ms NVDEC (against the 1.5 ms of the fast 610 run), about 0.9 ms copy round and about 0.5-0.9 ms CPU.

### 610 delta

- The device's per-frame handling is the same for both drivers:
  - rings per channel, trapped video doorbells, interrupts per frame;
  - video layout, own host TSG and runlist, shadow leaves and gathers of the video root.
- The measured gap between 610 and 615 is not a driver effect:
  - the 2026-09-25 610 guest encoded at 104.8, 123.8 and 156.8 fps;
  - the 615 guest at 127.9 and 95.1 fps, and at 88.8-100.6 fps tonight;
  - "610 near native" was the best of three 610 runs.
- In that 610 run the first two interrupts (mostly NVENC's) came at +0.86 and +2.27 ms, and NVDEC finished at about 1.5 ms (mode). Tonight they came at +2.0 and +4.86 ms, and NVDEC at 4.05 ms. So the host engine phases differ by a factor of 2.1-2.7 between runs [caraxes/runs/nvenc-2026-09-25/probe-*/probe.txt and probe-trace.txt.xz].

### Host power state (the likely cause)

- **Native path:** `cuCtxCreate` sends `PERF_BOOST` {BOOST_TO_MAX \| CUDA_YES, duration INFINITE}. The host kernel passes it to GSP as `INTERNAL_PERF_BOOST_SET_2X` [docs/vfio/cuda-lookahead.md C2; K/gpu/perf/kern_perf_boost.c:100-122].
- **Guest path:** the same request reaches the device as 0x20800a9a, at context create and destroy. The device acknowledges it and boosts nothing [T:147089192, 154961065; D/cuda.rs:29-33, 76-84].
- **Who may send it:** `PERF_BOOST` is NON_PRIVILEGED and ROUTE_TO_VGPU_HOST (flags 0x10318) [G/g_subdevice_nvoc.c:11198-11204]. So an unprivileged host client may send it, and NVIDIA vGPU routes a guest's boost to its host. There is also a MULTIMEDIA flag [SDK/ctrl/ctrl2080/ctrl2080perf.h:80-99].
- **Observed card state:**
  - during guest copy benchmarks the card sat at P8 with PCIe gen 1 in 270 of 302 samples (P0 in 25) [caraxes/runs/opt-memfd-pool-2026-09-25/link-on-3.csv; docs/vfio/bar2-memfd.md, copies];
  - before tonight's native block it read 315 MHz [native/gpu-before.txt].
- **Inferred, not measured during a video run:**
  - the engine slowdown is uniform (NVENC, NVDEC and the GR copy round);
  - the peaks are tight within a run but differ between runs;
  - there is a second mode 1.5-3.5 ms later.
  - This fits the card's clock state rather than a fixed cost per job.

## Consequence for Caraxes

| # | fix | effort | gain per frame (encode / decode) | proof | confidence |
|---|---|---|---|---|---|
| 1 | Host clocks for the guest's work: send the guest's `SET_2X` {flags, duration} as `NV2080_CTRL_CMD_PERF_BOOST` on the device's host subdevice; clear it when the guest client is freed and at device exit | small (one control and its bookkeeping) plus a decision (C2 says "possible, not recommended") | −2.3 to −2.6 ms p50 (−3 to −4 ms mean, the later modes gone) / −2.5 ms NVDEC, −0.6 to −0.9 ms copy. Expected: encode about 150-160 fps (≈0.9×), decode about 115-120 fps | first, read-only: host `nvidia-smi --query-gpu=timestamp,pstate,clocks.gr,clocks.video,pcie.link.gen.current,utilization.encoder,utilization.decoder -lms 100` during one native and one guest probe; then an A/B with one host process holding a CUDA context while the guest runs; after the fix, P0 and gen 3 in the samples and engine phases within 10 % | medium |
| 2 | Ring path: finish the `plan` fix (branch planfast-2026-09-28); give the video channels' own tokens an ioeventfd each (datamatch on the guest's token for the video runlists), so their 2+1 doorbells per frame stop trapping | the fix in progress; the tokens are moderate work | −0.10 to −0.15 ms / −0.06 to −0.10 ms | `ring to host doorbell` p50 ≤ 15 µs; first submit → NVENC doorbell ≤ 80 µs; no `W bar0 0x00bb0090` in the loop | high (mechanism), medium (size) |
| 3 | Interrupt path: serve the fault-buffer register page (0xB83000; PUT and GET stay 0, since faults never reach the guest) from a memfd like the intr page | small to moderate | −35 to −45 µs per interrupt: −0.08 to −0.10 ms / −0.08 to −0.10 ms | no `R bar0 0x00b8302c` in the loop; probe latency p50 −0.08 ms against the same build without it | medium-high |

- With all three, the expected encode is about 0.9-0.95× native. Decode stays at about 0.85× because of the CPU phase (Open).
- A host boost from a VM touches host clocks, power and fairness towards the desktop and other VMs, so it is a decision for the user. A policy could hold one device-level boost only while mirrored video or GR channels are busy.

## Open

- Guest CPU phases are about 10 % slower than native: the fill takes 3.51 against 3.21 ms, and the decode CPU phase 5.9 against an estimated 5.0-5.5 ms. vCPU placement and the host governor are not measured.
- The video channels' pushbuffers are not decoded. Whether NVENC's second job waits on a GR semaphore stays open: the vector 9/4 order varies from run to run and does not separate 610 from 615. A trace option could decode the host-class methods of mirrored video channels at each doorbell (semaphore acquire/release, NON_STALL_INTERRUPT).
- Session setup (23.9 MB of software memsets and memcopies on chid 29) affects `nvEncInitializeEncoder` and short sessions (Sunshine start), not fps. One route is to run bare CE channels on a host CE.
- Contention on the GR runlist with the host desktop is not measured.
- The note timestamps (Method) should be taken when the note is created, not when it is written.
