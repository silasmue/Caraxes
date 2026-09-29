// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The one privilege the device may use (decision D12): CAP_SYS_ADMIN, raised into
//! the effective set only around the host RM controls that are PRIVILEGED, today
//! `NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR` for the IOVAs of registered guest RAM.
//!
//! RM takes the privilege of each ioctl from `capable(CAP_SYS_ADMIN)` at the time of
//! the call (`src/nvidia/arch/nvalloc/unix/src/escape.c:381`, `os.c:648`,
//! `kernel-open/nvidia/os-interface.c:389-392`), so a capability in the permitted set
//! that is effective only for those calls is exactly as much as needed. This is the
//! scoping of `caraxes/tools/sysmem-iova.rs`, narrower than Leandro v1's
//! `LEA_ADMIN_PRIV=1`.
//!
//! Two ways to start (docs/caraxes/memory-model.md, "Privilege"):
//! - **File capability** (preferred, the launcher's way): the binary carries
//!   `cap_sys_admin+p`. The process runs as the user from the start with
//!   CAP_SYS_ADMIN permitted and not effective; it never has any other privilege.
//! - **`sudo -n`**: the process starts as root and at once becomes `SUDO_UID`/`SUDO_GID`
//!   with `PR_SET_KEEPCAPS`, keeping only CAP_SYS_ADMIN in the permitted set.
//!
//! Capabilities are per thread: [`Privilege::init`] runs first in `main`, before any
//! thread exists, so every thread inherits the reduced sets; [`Privilege::with_admin`]
//! raises the capability on the calling thread only.

use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{bail, Context, Result};

const CAP_VERSION_3: u32 = 0x2008_0522;
/// CAP_SYS_ADMIN is capability 21 (`include/uapi/linux/capability.h`), in word 0.
const CAP_SYS_ADMIN_BIT: u32 = 1 << 21;

#[repr(C)]
struct CapHeader {
    version: u32,
    pid: i32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct CapData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

fn caps_get() -> Result<[CapData; 2]> {
    let mut hdr = CapHeader {
        version: CAP_VERSION_3,
        pid: 0,
    };
    let mut data = [CapData::default(); 2];
    // SAFETY: a version-3 header and two capability words, as capget(2) requires.
    let r = unsafe {
        libc::syscall(
            libc::SYS_capget,
            &mut hdr as *mut CapHeader,
            data.as_mut_ptr(),
        )
    };
    if r != 0 {
        return Err(std::io::Error::last_os_error()).context("capget");
    }
    Ok(data)
}

/// Set this thread's effective and permitted word 0 (words 1 and inheritable cleared).
fn caps_set(effective: u32, permitted: u32) -> Result<()> {
    let hdr = CapHeader {
        version: CAP_VERSION_3,
        pid: 0,
    };
    let data = [
        CapData {
            effective,
            permitted,
            inheritable: 0,
        },
        CapData::default(),
    ];
    // SAFETY: as in caps_get; capset(2) only reads both.
    let r = unsafe { libc::syscall(libc::SYS_capset, &hdr as *const CapHeader, data.as_ptr()) };
    if r != 0 {
        return Err(std::io::Error::last_os_error()).context("capset");
    }
    Ok(())
}

pub struct Privilege {
    admin: bool,
    windows: AtomicU64,
    how: String,
}

impl Privilege {
    /// Reduce the process to what the device may hold: nothing, or with `want_admin`
    /// CAP_SYS_ADMIN permitted but not effective. Refuses to go on as root, and refuses
    /// `want_admin` when the capability is not available.
    pub fn init(want_admin: bool) -> Result<Self> {
        // SAFETY: plain identity queries.
        let euid = unsafe { libc::geteuid() };
        let mut how;
        if euid == 0 {
            let id = |k: &str| -> Result<u32> {
                std::env::var(k)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .filter(|&v: &u32| v != 0)
                    .with_context(|| format!("started as root without {k}: start the device with `sudo -n` from the user's shell, or give the binary the file capability cap_sys_admin+p and start it as the user"))
            };
            let (uid, gid) = (id("SUDO_UID")?, id("SUDO_GID")?);
            // SAFETY: plain syscalls on this process, checked one by one; no thread
            // exists yet.
            unsafe {
                if libc::prctl(libc::PR_SET_KEEPCAPS, 1, 0, 0, 0) != 0
                    || libc::setgroups(0, std::ptr::null()) != 0
                    || libc::setresgid(gid, gid, gid) != 0
                    || libc::setresuid(uid, uid, uid) != 0
                    || libc::prctl(libc::PR_SET_KEEPCAPS, 0, 0, 0, 0) != 0
                {
                    return Err(std::io::Error::last_os_error())
                        .context("drop from root to SUDO_UID/SUDO_GID");
                }
            }
            how = format!("started by sudo, now uid {uid} gid {gid}");
        } else {
            how = format!("uid {euid}");
        }
        let have = caps_get()?[0].permitted & CAP_SYS_ADMIN_BIT != 0;
        if want_admin && !have {
            bail!(
                "CAP_SYS_ADMIN is not in the permitted set ({how}); the guest-RAM registration needs it for GET_SURFACE_PHYS_ATTR (D12). Give the binary the file capability (`sudo setcap cap_sys_admin+p <binary>`, as tools/caraxes-vm.sh does) or start it with `sudo -n`"
            );
        }
        let keep = if want_admin { CAP_SYS_ADMIN_BIT } else { 0 };
        caps_set(0, keep)?;
        let c = caps_get()?;
        how.push_str(&format!(
            "; capabilities effective {:#x} permitted {:#x} inheritable {:#x}",
            c[0].effective, c[0].permitted, c[0].inheritable
        ));
        if c[0].effective != 0 || c[0].permitted & !keep != 0 || c[1].permitted != 0 {
            bail!("capabilities not reduced: {how}");
        }
        Ok(Privilege {
            admin: want_admin,
            windows: AtomicU64::new(0),
            how,
        })
    }

    /// No privilege at all (tests, and runs without guest-RAM registration).
    pub fn none() -> Self {
        Privilege {
            admin: false,
            windows: AtomicU64::new(0),
            how: "none requested".into(),
        }
    }

    pub fn has_admin(&self) -> bool {
        self.admin
    }

    /// How the process got here, for the log.
    pub fn describe(&self) -> &str {
        &self.how
    }

    /// How many privileged windows were opened.
    pub fn windows(&self) -> u64 {
        self.windows.load(Ordering::Relaxed)
    }

    /// Run `f` with CAP_SYS_ADMIN effective on this thread, then drop it again.
    pub fn with_admin<T>(&self, f: impl FnOnce() -> T) -> Result<T> {
        if !self.admin {
            bail!("privileged call without CAP_SYS_ADMIN (the device was started without it)");
        }
        caps_set(CAP_SYS_ADMIN_BIT, CAP_SYS_ADMIN_BIT)?;
        self.windows.fetch_add(1, Ordering::Relaxed);
        let r = f();
        caps_set(0, CAP_SYS_ADMIN_BIT)?;
        Ok(r)
    }
}
