// SPDX-License-Identifier: MIT
//! One GPU timestamp domain per virtual device. Physical execution reads the
//! physical usermode timer; GSP CPU/interval time remains the logical clock.
//! No offset extrapolation or maximum-deviation guarantee is made here.
use anyhow::{bail, Context, Result};
use nvrm_abi::{sys, NvDevice};
use nvrm_client::{mem::ReadOnlyTimer, RmClient};
use std::sync::Arc;
use std::time::Instant;

const TIME_LOW: usize = 0x80;
const TIME_HIGH: usize = 0x84;
const HIGH_MASK: u32 = 0x1fff_ffff;
const READ_ATTEMPTS: usize = 8;

/// Bounded high-low-high protocol; a changing high word retries. Low bits 4:0
/// and high bits 31:29 are reserved in NV_VIRTUAL_FUNCTION_TIME_0/1.
fn coherent_read(mut read: impl FnMut(usize) -> u32) -> Option<u64> {
    for _ in 0..READ_ATTEMPTS {
        let hi = read(TIME_HIGH) & HIGH_MASK;
        let lo = read(TIME_LOW) & !0x1f;
        let after = read(TIME_HIGH) & HIGH_MASK;
        if hi == after {
            return Some((u64::from(hi) << 32) | u64::from(lo));
        }
    }
    None
}

/// Fields drop in declaration order: mapping unmaps before own RM client frees
/// its usermode/subdevice/device. No HostMemory shutdown can free this lease.
struct PhysicalClock {
    map: ReadOnlyTimer,
    _rm: RmClient,
    _gpu: NvDevice,
}

impl PhysicalClock {
    fn open(gpu_path: &str) -> Result<Self> {
        // HostMemory has already verified the runtime ABI; don't assert the
        // diagnostic library's build-default driver version here.
        if gpu_path != "/dev/nvidia0" {
            bail!("physical GPU timer requires the same deviceId=0 selection as HostMemory");
        }
        let mut rm = RmClient::open_without_version_check().context("GPU timer RM client")?;
        let gpu = NvDevice::open(gpu_path).context("GPU timer device")?;
        let gpu = gpu
            .open_for_mapping(rm.ctl())
            .context("GPU timer device registration")?;
        let root = rm.root();
        let device = rm.next_handle();
        let mut dp = sys::NV0080_ALLOC_PARAMETERS::default();
        dp.deviceId = 0; // same selected device as HostMemory::open
                         // SAFETY: matching fixed class allocation parameters, no embedded pointers.
        unsafe { rm.alloc(root, device, sys::NV01_DEVICE_0, Some(&mut dp)) }
            .context("GPU timer device object")?;
        let subdevice = rm.next_handle();
        let mut sp = sys::NV2080_ALLOC_PARAMETERS::default();
        sp.subDeviceId = 0;
        // SAFETY: matching fixed subdevice parameters.
        unsafe { rm.alloc(device, subdevice, sys::NV20_SUBDEVICE_0, Some(&mut sp)) }
            .context("GPU timer subdevice")?;
        let usermode = rm.next_handle();
        // SAFETY: VOLTA_USERMODE_A has no allocation parameters.
        unsafe { rm.alloc::<u8>(subdevice, usermode, sys::VOLTA_USERMODE_A, None) }
            .context("GPU timer usermode")?;
        // SAFETY: private lease owns the client/objects, no explicit FREE or
        // handle exposure; map drops before _rm closes its root.
        let map = unsafe { nvrm_client::mem::map_timer_read_only(&rm, &gpu, subdevice, usermode) }
            .context("GPU timer read-only mapping")?;
        let clock = Self {
            map,
            _rm: rm,
            _gpu: gpu,
        };
        let first = clock
            .try_now()
            .ok_or_else(|| anyhow::anyhow!("GPU timer unstable at startup"))?;
        if first == 0 {
            bail!("GPU timer returned zero at startup");
        }
        // Equal immediate reads are permitted by 32 ns granularity. Require
        // observed advancement within bounded setup reads; never accept a dead
        // nonzero mapping. Hardware execution is not attached until this passes.
        for _ in 0..16 {
            let next = clock
                .try_now()
                .ok_or_else(|| anyhow::anyhow!("GPU timer unstable at startup"))?;
            if next != first {
                return Ok(clock);
            }
            std::thread::yield_now();
        }
        bail!("GPU timer did not advance during bounded startup reads")
    }
    fn try_now(&self) -> Option<u64> {
        coherent_read(|offset| self.map.read_u32(offset))
    }
}

#[derive(Clone)]
pub struct GpuClock(Source);
#[derive(Clone)]
enum Source {
    Emulated(Instant),
    Physical(Arc<PhysicalClock>),
    #[cfg(test)]
    Test(Arc<std::sync::atomic::AtomicU64>),
}
impl GpuClock {
    pub fn emulated(epoch: Instant) -> Self {
        Self(Source::Emulated(epoch))
    }
    pub(crate) fn physical(gpu_path: &str) -> Result<Self> {
        Ok(Self(Source::Physical(Arc::new(PhysicalClock::open(
            gpu_path,
        )?))))
    }
    pub fn is_physical(&self) -> bool {
        !matches!(self.0, Source::Emulated(_))
    }
    pub fn now_ns(&self) -> u64 {
        match &self.0 {
            Source::Emulated(epoch) => epoch.elapsed().as_nanos() as u64,
            // Failure must never silently switch epochs or return frozen time.
            Source::Physical(clock) => match clock.try_now() {
                Some(ns) => ns,
                None => {
                    // A panic could kill only the fast-read/publisher thread and
                    // leave a partly alive device. Terminate the whole backend;
                    // kernel FD teardown releases leases, outer VM owner handles
                    // its exact VMM cleanup. This is not orderly Rust Drop.
                    eprintln!("vfio-user-nvgpu: fatal physical GPU timer coherence failure; no clock fallback");
                    std::process::exit(1);
                }
            },
            #[cfg(test)]
            Source::Test(value) => value.load(std::sync::atomic::Ordering::Relaxed),
        }
    }
    #[cfg(test)]
    pub(crate) fn for_test(value: u64) -> (Self, Arc<std::sync::atomic::AtomicU64>) {
        let value = Arc::new(std::sync::atomic::AtomicU64::new(value));
        (Self(Source::Test(value.clone())), value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coherent_timer_retries_low_word_rollover_and_masks_reserved_bits() {
        let mut words = [1, 0xffff_ffe0, 2, 0xe000_0002, 0x3f, 2].into_iter();
        assert_eq!(
            coherent_read(|_| words.next().unwrap()),
            Some(0x2_0000_0020)
        );
        assert!(words.next().is_none());
    }
    #[test]
    fn coherence_failure_is_bounded_without_frozen_fallback() {
        let mut calls = 0;
        assert_eq!(
            coherent_read(|_| {
                calls += 1;
                calls
            }),
            None
        );
        assert_eq!(calls as usize, READ_ATTEMPTS * 3);
    }
    #[test]
    fn full_timer_wrap_is_returned_in_its_native_domain() {
        assert_eq!(
            coherent_read(|offset| if offset == TIME_HIGH {
                u32::MAX
            } else {
                u32::MAX
            }),
            Some(0x1fff_ffff_ffff_ffe0)
        );
        assert_eq!(coherent_read(|_| 0), Some(0));
    }
    #[test]
    fn logical_cpu_clock_stays_separate_from_selected_gpu_clock() {
        let mut reg = crate::channel::Registry::new(crate::fixture::rtx2070().chip.data().channel);
        let before = reg.now_ns();
        let (clock, value) = GpuClock::for_test(0x1234_5678_90);
        reg.set_gpu_clock(clock.clone());
        assert_eq!(reg.gpu_now_ns(), 0x1234_5678_90);
        assert!(reg.now_ns() >= before && reg.now_ns() < 0x1234_5678_90);
        value.store(0x1234_5678_a0, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(reg.gpu_clock().now_ns(), 0x1234_5678_a0);
    }
    #[test]
    fn gpu_timer_routes_through_locked_fast_vf_and_function_reset() {
        let f = crate::fixture::rtx2070();
        let mut b = crate::boot::BootModel::new(
            f.chip,
            crate::fb::FbLayout::plain(1 << 30),
            f.identity(1 << 30),
        );
        let (clock, value) = GpuClock::for_test(0x3_ffff_ff00);
        b.set_gpu_clock(clock);
        let fast = b.fast();
        assert_eq!(b.read32(0x9410), Some(3));
        value.store(0x4_0000_0040, std::sync::atomic::Ordering::Relaxed);
        let mut word = [0; 4];
        assert!(fast.try_read(0, 0x9400, &mut word));
        assert_eq!(
            u32::from_le_bytes(word),
            0xffff_ffe0,
            "shared torn-read guard"
        );
        let regs = f.chip.data().registers;
        assert_eq!(b.read32(regs.usermode_time_0), Some(0x40));
        assert_eq!(b.read32(regs.usermode_time_1), Some(4));
        let notes = b
            .write32(0x9400, 99, &crate::guestmem::GuestMemory::new())
            .unwrap();
        assert!(notes.iter().any(|n| n.contains("PTIMER")));
        assert_eq!(b.ctrl.channels.gpu_now_ns(), 0x4_0000_0040);
        b.reset();
        value.store(0x5_0000_0080, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(b.read32(0x9410), Some(5));
        assert!(fast.try_read(0, regs.usermode_time_0, &mut word));
        assert_eq!(
            u32::from_le_bytes(word),
            0x80,
            "old fast-reader Arc keeps selected provider"
        );
        assert_eq!(b.ctrl.channels.gpu_now_ns(), 0x5_0000_0080);
    }

    #[test]
    fn public_correlation_keeps_logical_cpu_time_and_selected_gpu_time() {
        let f = crate::fixture::rtx2070();
        let mut reg = crate::channel::Registry::new(f.chip.data().channel);
        let (clock, _) = GpuClock::for_test(1_791_558_986_000_000_000);
        reg.set_gpu_clock(clock);
        let mut attrs = crate::public::GuestAttrs::default();
        let mut rc = crate::rc::RcState::default();
        let mut ctx = crate::public::Ctx {
            host: None,
            reservation: None,
            chip: f.chip.data(),
            now_ns: 12_345,
            object: (0, 0),
            key: (0, 0),
            mirror: None,
            attrs: &mut attrs,
            rc: &mut rc,
            reg: Some(&reg),
            link_control: 0,
        };
        let mut params = vec![0; crate::ver::v610::public::TIME_CORR_SIZE];
        params[0] = 0x14;
        params[1] = 2;
        let answer = crate::public::answer(&mut ctx, 0x2080_0406, &params).unwrap();
        assert_eq!(answer.status, crate::rpc::NV_OK);
        for i in 0..2 {
            let o = crate::ver::v610::public::TIME_CORR_SAMPLES + 16 * i;
            assert_eq!(
                u64::from_le_bytes(answer.params[o..o + 8].try_into().unwrap()),
                12_345
            );
            assert_eq!(
                u64::from_le_bytes(answer.params[o + 8..o + 16].try_into().unwrap()),
                1_791_558_986_000_000_000
            );
        }
    }

    #[test]
    fn pure_emulation_actual_registry_correlation_preserves_exact_cpu_gpu_pair() {
        let f = crate::fixture::rtx2070();
        let reg = crate::channel::Registry::new(f.chip.data().channel);
        let mut attrs = crate::public::GuestAttrs::default();
        let mut rc = crate::rc::RcState::default();
        let mut ctx = crate::public::Ctx {
            host: None,
            reservation: None,
            chip: f.chip.data(),
            now_ns: 12_345,
            object: (0, 0),
            key: (0, 0),
            mirror: None,
            attrs: &mut attrs,
            rc: &mut rc,
            reg: Some(&reg),
            link_control: 0,
        };
        let mut params = vec![0; crate::ver::v610::public::TIME_CORR_SIZE];
        params[0] = 0x14;
        params[1] = 1;
        let answer = crate::public::answer(&mut ctx, 0x2080_0406, &params).unwrap();
        let o = crate::ver::v610::public::TIME_CORR_SAMPLES;
        assert_eq!(
            u64::from_le_bytes(answer.params[o..o + 8].try_into().unwrap()),
            12_345
        );
        assert_eq!(
            u64::from_le_bytes(answer.params[o + 8..o + 16].try_into().unwrap()),
            12_345
        );
    }
}
