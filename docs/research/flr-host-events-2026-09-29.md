<!-- SPDX-License-Identifier: MIT -->
# After an FLR no host event reaches the guest (2026-09-29, C75-C89)

- **Symptom:** after any KMD reset (FLR), the Windows display stays dead: the head is blanked,
  the window channel is never kicked again, and Sunshine blocks. Every TDR ended the session.
- **The display is not the cause:**
  - After the GSP re-init, the KMD's display bring-up is identical to the first boot up to
    core kick 16 (C88: channel allocation, init kicks, DP training, modeset kicks 8-16).
  - The next step (window ownership, the first flip, the UNBLANK) waits for the first
    D3D/DWM frame, and that frame never completes.
- **The completions never arrive:**
  - After the FLR, no host non-stall event is forwarded to the guest: the
    "host notifier ... -> guest vector" lines stop within the second of the reset in C75,
    C77, C87, C88 and C89.
  - C88: vector 4 (GR0) stays at 13989 raises from 120 s to the end; post-FLR GR channels
    ring once each (12 channels, 32 doorbells forwarded).
- **Cause:** the host subdevice keeps the old mirror's notifier action.
  - The mirror arms each notifier with `NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION(REPEAT)` on
    its host client's subdevice. The RM takes an active action only from DISABLE
    (subdevice_ctrl_event_kernel.c: NV_ERR_INVALID_STATE otherwise).
  - Freeing an event does not disarm it: `notifyActions` is reset only when the subdevice
    is constructed.
  - The FLR makes a fresh mirror on the same host client and subdevice. Its REPEAT is
    refused with 0x40.
  - `os_event` failed silently: the note went into a dropped Vec, the event object leaked
    (the soak still counted 9 events), and its ctl fd closed. The event thread polled an
    empty set for the rest of the run.
- **Fix (flrdisp1):**
  - The mirror's teardown sets each notifier back to DISABLE before freeing its event.
  - `os_event` treats a refused REPEAT with NV_ERR_INVALID_STATE as "still armed": it
    sends DISABLE, then REPEAT. On any failure it frees the event object.
  - Every arm result is logged and traced ("armed", "armed again after a DISABLE",
    "ERROR ... REFUSED"). The mirror summary counts them: "host events: N armed, M armed
    again, K REFUSED, D disarmed".
  - Tests: `a_notifier_still_armed_by_an_earlier_mirror_is_armed_again` and
    `another_refusal_is_an_error_with_its_status`.
- **Also affects:** any second mirror on the same device process: a VM reboot, and Linux
  guests after a reset.
- Memory model: none.
