<!-- SPDX-License-Identifier: MIT -->
# vm4-615 fresh boot: vkcube-wayland --c 120 at the stage's steps

Session `sottr-2026-09-24/session-615-20260925-180701`, binary `vfio-user-nvgpu-v615-68838c54`,
booted 18:07:01. The trace second is the device trace's clock (about host time minus 18:07:03).

| Step | Host time | Trace s | Result |
|---|---|---|---|
| Desktop up, Sunshine not started yet | 18:09:40 | 157 | PASS, 120 frames in 7.7 s (15.5 fps) |
| After Sunshine's encoder probes (h264_nvenc found) | 18:11:10 | 249 | PASS, 120 frames in 20.9 s (5.7 fps) |
| After a Moonlight connect | 18:12:13 | 310 | FAIL, timeout after 60 s, no refused c5c0 |

- The failure (`table.txt`, `fail-after-moonlight-connect-rmlog.txt`):
  - vkcube created its device (no refused allocation; `c5c0_refused=0`) and then did not
    finish 120 frames within 60 s. Its GPU work stopped completing.
- The same window, from the guest (`guest-smi-and-sunshine-log.txt`, `moonlight.log`):
  - 18:12:10: Moonlight connects, Sunshine creates h264_nvenc and starts the KMS capture.
  - About 18:12:28: Moonlight reports "Connection terminated: -100" (no video arrived).
  - 18:12:29: Sunshine ends the session.
  - 18:12:39: Sunshine: "Fatal: Hang detected! Session failed to terminate in 10 seconds",
    and it exits. Its video thread never returned.
  - So the GR work of the capture and of vkcube hung together.
- The host kernel burst at 18:12:42.6 (`host-kernel-181242.txt`, 23 lines `VA Space alloc
  failed ... NO_MEMORY`, 0x120000000 to 0x120c3ffff):
  - It matches trace 339.69 s (`device-trace-334-345s.txt`). After the guest freed the dead
    Sunshine's GR channels (339.49-339.65 s, 120 s of life each), the shadow of root
    0x12fc34000 recomputed its exclusions and mapped the 252-253 set-B leaves it had kept
    out ("0 set-B excluded ... 253 mapped in 5 call(s)").
  - That is the exclusion recompute of 4a6ba83. It released the guest's GR context VAs when
    the guest channels were freed, while the host's GR context at the same VAs (the promote
    binds them) was still being torn down. The fixed mappings collided with it.
  - The guest saw no new error at that moment: Sunshine had already exited, and vkcube was
    still hung.
  - The burst followed the hang; it did not start it. Not chased on the device side, as
    asked.
- Not reached: 5 minutes of streaming, and the CUDA NVENC probe (stopped at the first
  failure).
- Guest dmesg (`guest-dmesg.txt`): four `WARNING ... __nv_drm_handle_flip_event` from 328 s
  on. Guest nvidia-smi: steamwebhelper 1175 MiB (697 MiB in the earlier session),
  gnome-shell 180 MiB, no Sunshine.

## Interrupts around the hang (live trace, per trace second)

Columns: "host notifier fired -> no MSI (disabled or no fd)" / "-> guest vector 4, MSI" /
"msi re-arm" writes.

| Trace s | no MSI | MSI | re-arm |
|---|---|---|---|
| 280-284 | 0 | 2-14 | 140-175 |
| 285-295 | 0-22 | 0-682 | 187-428 |
| 296-315 (Moonlight connected, capture and vkcube) | 16-67 | 170-220 | 260-407 |
| 316-320 | 2-24 | 16-170 | 144-275 |

- From 296 s on, 30 to 67 host notifier events per second are traced as "no MSI (disabled or
  no fd)". Correction (main, 18:45): in the interrupt-page mode that label is a *coalesced*
  raise (`IntrShared::raise`: the vector's bit was still pending, so no second MSI), not a
  dropped interrupt; the page's reactor re-pends such a vector at the guest's ack (`again`)
  with its own MSI. The rate shows the reactor lagging behind the events (it waits for the
  device lock, which the pool's poll holds 3 to 60 ms), not a lost completion by itself.
- Per-second counts: `msi-per-second.txt` (second, no-MSI, MSI, re-arm).
- The graphics work stopped while the host GPU stayed fault-free and Xid-free (0 Xid, no RC).
  Open suspects, in order: the BAR1 pool (the device log shows "remap refused (the guest maps
  it twice)" and "cannot move back ... the USERD of mirrored channel" refusals in the same
  window: a page the GPU or the guest writes through one copy and reads through another
  would hang exactly like this), the pool's lock hold (interrupt and doorbell latency), an
  interrupt lost in the trapped path. The no-pool reproduction (`SOTTR_NO_POOL=1`, same
  steps) decides the first.
- Not changed here: the interrupt path is another agent's.
