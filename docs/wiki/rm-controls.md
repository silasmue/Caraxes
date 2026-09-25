<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# RM controls

A control is a command on an RM object: a 32-bit id whose high half names the object's
class and whose low half names the command, plus a parameter struct of a fixed size. The
open kernel modules export 1397 control ids in 615.71.09; their flags decide who answers
them, which is what a virtual GPU has to know first.

## The id and the flags

- Id layout: `NV2080_CTRL_CMD_GPU_GET_NAME_STRING` is 0x20800110: class 0x2080 (subdevice),
  category 0x01 (GPU), command 0x10. The definitions live under
  `src/common/sdk/nvidia/inc/ctrl/`.
- Export tables: one generated table per class in `src/nvidia/generated/g_*_nvoc.c` gives
  each control its parameter size, its handler and its flags (`src/nvidia/inc/kernel/rmapi/control.h`):

| Flag | Meaning for a virtual GPU |
|---|---|
| `RMCTRL_FLAGS_ROUTE_TO_PHYSICAL` (0x40) | the kernel RM forwards it to GSP-RM as `GSP_RM_CONTROL`; the device in the GSP role must answer it (844 of 1397 in 615) |
| `RMCTRL_FLAGS_INTERNAL` | sent only by the kernel RM itself, never accepted from user space (246 of the routed ones); invisible to a user-space tracer and to a capture |
| `RMCTRL_FLAGS_NON_PRIVILEGED` / `PRIVILEGED` | privilege is decided per ioctl by `osIsAdministrator()` (615: `src/nvidia/arch/nvalloc/unix/src/escape.c:384`, `src/nvidia/src/kernel/rmapi/control.c:688-694`) |
| `RMCTRL_FLAGS_CACHEABLE`, `_CACHEABLE_BY_INPUT` | the kernel RM caches the answer per GPU (`src/nvidia/src/kernel/rmapi/rmapi_cache.c`); a device cannot add the flag |
| `RMCTRL_FLAGS_GSP_PLUGIN_FOR_VGPU_GSP` | handled by the vGPU plugin in SR-IOV mode; not a GSP-client route |

- The size is checked before anything is read (615: `control.c:446-461`), so a control of the
  wrong version is refused, never misread.
- The kernel skips its error message for a list of controls expected to fail on virtual
  GPUs (615: `src/nvidia/src/kernel/rmapi/rmapi_utils.c:198`).

## Who answers what

| Answered by | Examples | In VFIO Leandro |
|---|---|---|
| the kernel RM itself | `GPU_GET_NAME_STRING`, `FB_GET_INFO_V2`, `GET_WORK_SUBMIT_TOKEN`, `GET_PIDS` | never reaches the device; the answers come from the static configuration the device sent at boot ([identity-and-costume](identity-and-costume.md)) |
| GSP-RM (routed) | `GPU_GET_INFO_V2`, `CE_GET_CAPS`, `GR_GET_INFO`, the display NV0073 controls, `GPFIFO_SCHEDULE` | the device answers per category ([gsp-control-categories](gsp-control-categories.md)) |
| GSS legacy, BINAPI | ids outside the export tables that `nvidia-smi` sends (perf, clocks, thermal) | forwarded to the host with rewritten identity; the reply's `rmctrlFlags` may mark it cacheable |

- VirtIO Leandro sees every control at the ioctl layer and blocks 34 of them whose embedded
  pointers or file descriptors it does not translate (`blocked_ctrls()` in
  `crates/nvrm-abi/src/xlate.rs`, [SECURITY.md](../SECURITY.md)).
- VFIO Leandro sees only the routed ones; the guest's user space is served by the guest's own
  kernel RM as on bare metal.

## Depth

- [research/gsp-surface.md](../research/gsp-surface.md): the counts of 615.71.09.
- [caraxes/gsp-transport-classes.md](../caraxes/gsp-transport-classes.md): every routed control
  by its structure and its transport class.
- [caraxes/nvml.md](../caraxes/nvml.md): the controls `nvidia-smi` sends.
