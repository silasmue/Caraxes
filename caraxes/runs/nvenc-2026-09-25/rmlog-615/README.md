<!-- SPDX-License-Identifier: MIT -->
# rmlog-615: the 615 NVENC refusal, the guest's RM calls against the host's

- `rmlog.txt`: `caraxes/tools/rmioctl-log.c` in vm2-615 (1 GiB), the probe with `-n 1`.
  Reference: `../native-rmlog/host-rm.log`, the same probe on the host (615.71.09, PASS).
- The encoder client, step by step:
  - Identical in both up to `NV0080_CTRL_CMD_GR_GET_CAPS_V2` (0x801109) and
    `MC_GET_ARCH_INFO` (0x20801701).
  - The host then queries `NVDEC_GET_CAPS_V2` (0x801c02), `NVENC_GET_CAPS` (0x801b01) and
    `GET_VIRTUALIZATION_MODE` (0x800289: 0), `GPU_GET_INFO_V2` (0x11: 0,
    0x31 NVENC_STATS_REPORTING: 1), then opens the session (0x20808165, 0x20808163).
  - The guest skips both caps queries. It gets `GET_VIRTUALIZATION_MODE` = 1 (NMOS) and the
    same `GPU_GET_INFO_V2` answer. Then it fails, with a `0x215` NV0000 control answered
    0x1f on the error path.
- Answers that differ, all served by the guest kernel itself (none reaches the device):
  - **`GPU_GET_ENGINES_V2` (0x20800170): the guest lists 7 engines: GR0, CE0-4, SW (0x22).
    The host lists 9: GR0, CE0-4, NVDEC0 (0x13), and two more.** The guest's engine list has
    no NVDEC and no NVENC. The kernel builds it from the class DB (`gpuUpdateEngineTable`,
    gpu.c:5109-5154). A class stays in the DB when its engine has an engstate, or when
    GSP's `engineCaps` has the engine's bit (`gpuCheckEngineWithOrderList_KERNEL`,
    gpu.c:6694-6820). The device sends `engineCaps` 0x081bfe02 0xc4 (GR0, CE0-8, NVDEC0/1,
    NVENC0, SW, SEC2), so the drop comes from elsewhere. Open.
  - `GET_VIRTUALIZATION_MODE`: guest 1 (NMOS, the guest kernel sees a passthrough GPU), host 0.
  - `GR_GET_CAPS_V2`: the capsTbl differs. The host's two calls also differ from each other,
    so the logged bytes are not a clean comparison.
- Experiments, `rmioctl-log.so` rewriting the guest's answer (`exp-*.txt`): virtualization
  mode 0, the host's GR caps, and both together. None of them changes the result (NVENCSTATUS 2).
- Next: `RMLOG_ENGINES=1,9,10,11,12,13,19,27,38` (the logger has it), to see whether the
  engine list is what the library checks. Then find why the 615 guest kernel drops the
  video classes (its class DB from the device, the engine order list).
- Host note: during two of these vm2-615 runs another `vfio-user-nvgpu` (win11, binary
  `vfio-user-nvgpu-v615-nvenc-1c4c848`) ran on the host, started 15:56 and again about 16:05.
  There was no NVRM line and no Xid.
