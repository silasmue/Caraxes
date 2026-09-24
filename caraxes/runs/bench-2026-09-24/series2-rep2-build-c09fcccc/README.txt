# SPDX-License-Identifier: MIT
Rep 2 of series2, v2 only, moved here from series2/v2 on 2026-09-24 04:20. The device restart
before rep 2 (01:55) copied target/release/vfio-user-nvgpu, which another agent had rebuilt at
01:41 from the main tree with uncommitted changes (the NV_UFLUSH register work in progress):
sha256 c09fcccc2f412bd7407b83d17bbee71ed9e18244dc71049300821426a8dbf93d. Rep 1 (series2/v2)
and the startup runs ran on d140e7cb... (223075d). The interleaved native runs of rep 2 stay
in series2/native. bench-run.sh records the device binary per run since this.
