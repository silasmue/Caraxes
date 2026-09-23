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
}

impl HostRmClient {
    /// Open the host RM and allocate the device's own root, device 0 and subdevice 0.
    pub fn open() -> anyhow::Result<Self> {
        let mut rm = RmClient::new().map_err(|e| anyhow::anyhow!("host RM client: {e}"))?;
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
}

impl HostRm for HostRmClient {
    fn control(&mut self, class: u32, cmd: u32, params: &mut [u8]) -> u32 {
        let object = match class {
            NV20_SUBDEVICE_0 => self.subdevice,
            NV01_DEVICE_0 => self.device,
            NV2081_BINAPI => match self.binapi() {
                Some(h) => h,
                None => return NV_ERR_NOT_SUPPORTED,
            },
            _ => self.rm.root(),
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
