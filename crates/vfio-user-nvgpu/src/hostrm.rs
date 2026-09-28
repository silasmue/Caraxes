// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Host RM access from the device process (decision D10): an unprivileged client on
//! `/dev/nvidiactl` with its own device and subdevice, through `nvrm-client`. Only
//! public controls are forwarded; the object is chosen by the guest object's class.

use nvrm_abi::{sys, NvDevice};
use nvrm_client::RmClient;

use crate::ctrl::HostRm;
use crate::ver::active::{NV01_DEVICE_0, NV2081_BINAPI, NV20_SUBDEVICE_0, NV_ERR_NOT_SUPPORTED};

pub struct HostRmClient {
    rm: RmClient,
    _gpu: NvDevice,
    device: u32,
    subdevice: u32,
    /// NV2081_BINAPI under the subdevice, allocated on first use (nvml.rs).
    binapi: Option<u32>,
    /// GF100_ZBC_CLEAR under the subdevice, allocated on first use (zbc.rs, real compression
    /// per slice only).
    zbc: Option<u32>,
}

impl HostRmClient {
    /// Open the host RM and allocate the device's own root, device 0 and subdevice 0.
    ///
    /// Refused unless start-up selected the host ABI (hostabi.rs `select`): that
    /// check replaces `RmClient::new`'s assertion that the running driver is the
    /// build's default (610.57.04), which would panic on a 615.71.09 host.
    pub fn open() -> anyhow::Result<Self> {
        crate::hostabi::selected().map_err(|e| anyhow::anyhow!(e))?;
        let mut rm = RmClient::open_without_version_check()
            .map_err(|e| anyhow::anyhow!("host RM client: {e}"))?;
        let gpu = NvDevice::open_gpu(0).map_err(|e| anyhow::anyhow!("open /dev/nvidia0: {e}"))?;
        let root = rm.root();
        let device = rm.next_handle();
        let mut dp = sys::NV0080_ALLOC_PARAMETERS::default();
        dp.deviceId = 0;
        // SAFETY: NV0080_ALLOC_PARAMETERS matches NV01_DEVICE_0; no embedded buffers.
        unsafe { rm.alloc(root, device, sys::NV01_DEVICE_0, Some(&mut dp)) }
            .map_err(|e| anyhow::anyhow!("host NV01_DEVICE_0: {e}"))?;
        let subdevice = rm.next_handle();
        let mut sp = sys::NV2080_ALLOC_PARAMETERS::default();
        sp.subDeviceId = 0;
        // SAFETY: NV2080_ALLOC_PARAMETERS matches NV20_SUBDEVICE_0; no embedded buffers.
        unsafe { rm.alloc(device, subdevice, sys::NV20_SUBDEVICE_0, Some(&mut sp)) }
            .map_err(|e| anyhow::anyhow!("host NV20_SUBDEVICE_0: {e}"))?;
        Ok(HostRmClient {
            rm,
            _gpu: gpu,
            device,
            subdevice,
            binapi: None,
            zbc: None,
        })
    }

    /// The host BINAPI object (NON_PRIVILEGED class, resource_list.h:492-503).
    fn binapi(&mut self) -> Option<u32> {
        if self.binapi.is_none() {
            let h = self.rm.next_handle();
            // SAFETY: NV2081_BINAPI takes optional params; none are passed.
            let r = unsafe {
                self.rm.alloc::<sys::NV2081_ALLOC_PARAMETERS>(
                    self.subdevice,
                    h,
                    NV2081_BINAPI,
                    None,
                )
            };
            self.binapi = r.ok().map(|_| h);
        }
        self.binapi
    }

    /// The host ZBC table object (NON_PRIVILEGED class under the subdevice,
    /// rmapi/resource_list.h:874-884 of 615.71.09; no alloc params).
    fn zbc(&mut self) -> Option<u32> {
        if self.zbc.is_none() {
            let h = self.rm.next_handle();
            // SAFETY: GF100_ZBC_CLEAR takes no alloc params (RS_NONE); none are passed.
            let r = unsafe {
                self.rm
                    .alloc::<u32>(self.subdevice, h, GF100_ZBC_CLEAR, None)
            };
            self.zbc = r.ok().map(|_| h);
        }
        self.zbc
    }
}

/// GF100_ZBC_CLEAR (class/cl9096.h): the host's ZBC clear table, reached only through
/// [`HostRm::control`] with this class and one of [`ZBC_CONTROLS`] (zbc.rs, with
/// `CARAXES_COMPRESSION=slice`). It is no guest object's host object: [`host_object`] and
/// [`class_of_control`] never name it, so no generic forward reaches it.
pub const GF100_ZBC_CLEAR: u32 = 0x9096;

/// ctrl9096.h:113, :163, :339, :379, :427 (the same ids in 610.57.04 and 615.71.09).
pub const CMD_SET_ZBC_COLOR_CLEAR: u32 = 0x9096_0101;
pub const CMD_SET_ZBC_DEPTH_CLEAR: u32 = 0x9096_0102;
pub const CMD_SET_ZBC_STENCIL_CLEAR: u32 = 0x9096_0105;
pub const CMD_GET_ZBC_CLEAR_TABLE_SIZE: u32 = 0x9096_0106;
pub const CMD_GET_ZBC_CLEAR_TABLE_ENTRY: u32 = 0x9096_0107;

/// The controls the device may issue on its host ZBC object: the three setters that add an
/// entry or reference an equal one (the host places it), and the two getters. Never the
/// indexed `SET_ZBC_CLEAR_TABLE` (0x90960104), which writes a caller-chosen slot of the
/// table the host desktop and every VM share, nor the deprecated combined getter.
pub const ZBC_CONTROLS: &[(u32, &str)] = &[
    (CMD_SET_ZBC_COLOR_CLEAR, "SET_ZBC_COLOR_CLEAR"),
    (CMD_SET_ZBC_DEPTH_CLEAR, "SET_ZBC_DEPTH_CLEAR"),
    (CMD_SET_ZBC_STENCIL_CLEAR, "SET_ZBC_STENCIL_CLEAR"),
    (CMD_GET_ZBC_CLEAR_TABLE_SIZE, "GET_ZBC_CLEAR_TABLE_SIZE"),
    (CMD_GET_ZBC_CLEAR_TABLE_ENTRY, "GET_ZBC_CLEAR_TABLE_ENTRY"),
];

/// Whether `cmd` may go to the host ZBC object.
pub fn zbc_control_allowed(cmd: u32) -> bool {
    ZBC_CONTROLS.iter().any(|&(c, _)| c == cmd)
}

/// The host objects the client holds. A control is issued only on one of them; there is
/// no route to the host root client (a control of another class used to land there, on
/// the wrong object; gsp-plan/b-host-forwardable.md §1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostObject {
    Subdevice,
    Device,
    Binapi,
}

/// The host object for a guest object's class, `None` for every other class.
pub fn host_object(class: u32) -> Option<HostObject> {
    match class {
        NV20_SUBDEVICE_0 => Some(HostObject::Subdevice),
        NV01_DEVICE_0 => Some(HostObject::Device),
        NV2081_BINAPI => Some(HostObject::Binapi),
        _ => None,
    }
}

/// The class of the host object a control is addressed to by its id: the interface class
/// in bits 31:16 (NV0080 device, NV2080 subdevice, NV2081 BINAPI). `None` for a control of
/// any other class, which the device never forwards.
pub fn class_of_control(cmd: u32) -> Option<u32> {
    let class = match cmd >> 16 {
        0x0080 => NV01_DEVICE_0,
        0x2080 => NV20_SUBDEVICE_0,
        0x2081 => NV2081_BINAPI,
        _ => return None,
    };
    host_object(class).map(|_| class)
}

impl HostRm for HostRmClient {
    fn control(&mut self, class: u32, cmd: u32, params: &mut [u8]) -> u32 {
        let object = match host_object(class) {
            Some(HostObject::Subdevice) => self.subdevice,
            Some(HostObject::Device) => self.device,
            Some(HostObject::Binapi) => match self.binapi() {
                Some(h) => h,
                None => return NV_ERR_NOT_SUPPORTED,
            },
            None if class == GF100_ZBC_CLEAR && zbc_control_allowed(cmd) => match self.zbc() {
                Some(h) => h,
                None => return NV_ERR_NOT_SUPPORTED,
            },
            None => return NV_ERR_NOT_SUPPORTED,
        };
        let mut p = sys::NVOS54_PARAMETERS::default();
        p.hClient = self.rm.root();
        p.hObject = object;
        p.cmd = cmd;
        // A zero-sized control must carry a NULL pointer, or RM answers
        // NV_ERR_INVALID_ARGUMENT (e.g. GPU_QUERY_INFOROM_ECC_SUPPORT has no params).
        p.params = if params.is_empty() {
            std::ptr::null_mut()
        } else {
            params.as_mut_ptr() as usize as sys::NvP64
        };
        p.paramsSize = params.len() as u32;
        // SAFETY: NVOS54 is the escape ABI; `params` is the guest's own copy of the
        // control payload, sized by the guest, and only public controls are forwarded.
        match unsafe { self.rm.ctl().ioctl_raw(sys::NV_ESC_RM_CONTROL, &mut p) } {
            Ok(()) => p.status as u32,
            Err(_) => 0xFFFF_FFFF,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only the three objects the client holds are reachable; the root client is not.
    #[test]
    fn only_subdevice_device_and_binapi() {
        assert_eq!(host_object(NV20_SUBDEVICE_0), Some(HostObject::Subdevice));
        assert_eq!(host_object(NV01_DEVICE_0), Some(HostObject::Device));
        assert_eq!(host_object(NV2081_BINAPI), Some(HostObject::Binapi));
        for class in [0u32, 0x41, 0x9096, 0x90E6, 0x208F, 0x402C, 0xB0CC, 0x906F] {
            assert_eq!(host_object(class), None, "{class:#x}");
        }
        assert_eq!(class_of_control(0x0080_0294), Some(NV01_DEVICE_0));
        assert_eq!(class_of_control(0x2080_0153), Some(NV20_SUBDEVICE_0));
        assert_eq!(class_of_control(0x2081_0108), Some(NV2081_BINAPI));
        assert_eq!(class_of_control(0x0000_0B02), None);
        assert_eq!(class_of_control(0x90E6_0101), None);
    }

    /// The host ZBC object takes the three adding setters and the two getters, never the
    /// indexed setter or the combined getter, and no generic forward names its class.
    #[test]
    fn zbc_object_takes_only_its_five_controls() {
        for cmd in [
            0x9096_0101,
            0x9096_0102,
            0x9096_0105,
            0x9096_0106,
            0x9096_0107,
        ] {
            assert!(zbc_control_allowed(cmd), "{cmd:#x}");
        }
        for cmd in [
            0x9096_0104,
            0x9096_0103,
            0x9096_0000,
            0x2080_0153,
            0x0080_200A,
        ] {
            assert!(!zbc_control_allowed(cmd), "{cmd:#x}");
        }
        assert_eq!(host_object(GF100_ZBC_CLEAR), None);
        for &(cmd, _) in ZBC_CONTROLS {
            assert_eq!(class_of_control(cmd), None);
        }
    }
}
