<!-- SPDX-License-Identifier: MIT -->
# vm4-615 with Sunshine on NVENC: two causes found, both fixed in the next binary

- Session: `sottr-2026-09-24/session-615-20260925-163955` (stage worktree), binary
  `vfio-user-nvgpu-v615-nvenc-c6abfa6`, `SOTTR_SUN_ENCODER=nvenc`, 8 GiB, 3 GiB profile,
  BAR1 pool 64 MiB, `CARAXES_MIRROR_BARE_CE=1`.
- Sunshine (and the probe inside the VM): the session opens, the encoder object is placed
  (`host video object class 0xc4b7 ...: its context at 0x11c000000, in the host-only area`),
  then `nvEncInitializeEncoder` returns NV_ENC_ERR_OUT_OF_MEMORY (10) after about 10 s.
  Sunshine fell back to libx264.
- Cause 1, the OUT_OF_MEMORY (`probe-rmlog.txt`, `ce-channel-not-scheduled.txt`):
  - No RM call failed.
  - After the NVC4B7 object the library opens a bare copy channel (engineType COPY2) and
    rings it before its copy object exists.
  - With `CARAXES_MIRROR_BARE_CE=1` the device built it on the host, found no engine
    object, and abandoned the build (`shadow: no engine object; not scheduling`).
  - The push never ran, and the library timed out.
  - On vm2 (no bare-CE mirroring) the channel ran in software, which is why the probes
    passed.
  - Fix: a copy channel without its object is bound to its engine (mirror.rs `copy_bind`),
    as a video channel is.
- Cause 2, the Xid 31 on CE0 at 16:49:27 (`host-kernel-164927.txt`, FAULT_PDE,
  VA 0x121400000):
  - 0x121400000 was the attribute CB promoted by a GR channel of the client
    0xc1d00066 root 0x13471a000 at 547 s (`attrib-cb-promote-547s.txt`).
  - That root has the default layout, not the video layout (the video-layout roots of this
    session are 0x1e5259000 and 0x1e5250000, NVENC's own).
  - The GL contexts were recreated about every 24 s (the next attribute CB at 0x121000000).
    The shadow kept every exclusion it had ever seen, so the reused VA 0x121400000 stayed
    unmapped, and a copy read there faulted.
  - Fix: the exclusions are recomputed from the root's live channels (mirror.rs
    `exclusions_of`) at every doorbell, build and re-sync; a freed channel's ranges go back
    to set A.
- Display kicks with vkcube-wayland for 12 s (`vkcube-kicks-per-second.txt`, `W bar0
  0x696000` per trace second): 1 to 9 per second, not about 60. This was measured after the
  Xid, with the capture broken. Retake it with the new binary.
