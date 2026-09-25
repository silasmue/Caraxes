<!-- SPDX-License-Identifier: MIT -->
# probe-pkg615: the 615 libraries on the 1 GiB profile still refuse NVENC, so it is the userspace

- Setup:
  - `install-615-video.sh` installed `libnvidia-encode` and `libnvidia-decode` 615.71.09 into
    vm2-615's overlay, through a NAT for 192.168.102.0/24. Its 3 rules were removed after.
  - Binary `vfio-user-nvgpu-v615-nvenc-1c4c848`, vm2-615 with the 1 GiB profile.
- Result: `nvEncOpenEncodeSessionEx` returned NVENCSTATUS 2, as on vm4-615 (3 GiB). The
  profile is not the cause.
- Native reference: `../native-rmlog/`, the probe on the host, 615.71.09, RESULT PASS. It
  was logged with `caraxes/tools/rmioctl-log.c`, which records every RM escape after the
  kernel answered it.
  - Before the session open the library sends `GPU_GET_INFO_V2` with 2 entries:
    - index 0x11, which the host answers 0;
    - index 0x31 (`NVENC_STATS_REPORTING_STATE`), which the host answers 1 (ENABLED).
    Then it sends `0x20808165`.
  - In the guest, index 0x31 is answered by the guest kernel itself (not marked for the
    physical side, so the device never sees it): `subdevice_ctrl_gpu_kernel.c:359-380`.
    The value is `pGpu->encSessionStatsReportingState` from `_gpumgrGetEncSessionStatsReportingState`
    (`gpu_mgr.c:1388-1430`). That is NOT_SUPPORTED when the guest's class list has no class
    on the MSENC engine at that point.
- Suspect, not yet verified: the guest answers index 0x31 with NOT_SUPPORTED, and the 615
  library treats that as "unsupported device". The next run: `rmioctl-log.so` in the guest,
  to read the guest's value.
- `../strace-615/strace.txt`: the guest's file accesses (no sysfs or proc read besides
  /proc/driver/nvidia/params and the device nodes).
