// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! `vfio-user-nvgpu`: serve the synthetic GPU (by default the RTX 2070: board data
//! `rtx2070`, chip `tu106`, driver 610.57.04) on a vfio-user socket, or check a
//! running server from the client side.
//!
//! One process serves one VM. The client is Cloud Hypervisor's `--user-device`
//! (docs/caraxes/vfio-user.md); the run recipe is in docs/caraxes/kickoff.md.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use anyhow::{bail, Context, Result};
use nvrm_abi::naming::{self, NameFormat};
use vfio_user::{IdlePollMode, Server};
use vfio_user_nvgpu::bar0::Bar0;
use vfio_user_nvgpu::board::{Board, BOARDS};
use vfio_user_nvgpu::boot::BootModel;
use vfio_user_nvgpu::chip::{self, Chip};
use vfio_user_nvgpu::compslice;
use vfio_user_nvgpu::config::{self as pci_config, ConfigSpace};
use vfio_bindings::bindings::vfio::VFIO_PCI_BAR0_REGION_INDEX;
use vfio_user_nvgpu::device::{
    doorbell_token_sub_regions,
    doorbell_value_sub_regions, irqs,
    msi_rearm_ioeventfd_requested, msi_rearm_sub_region, region_size, regions_with_pages,
    rpc_head_sub_region, rpc_ioeventfd_requested, RPC_IOEVENTFD_ENV,
    spawn_token_reader_hooked, Bar0Areas, Bar1Area, Bar2Region,
    Device, GuestRam, Routes, SharedDevice, MSI_REARM_IOEVENTFD_ENV,
    NV_XVE_CYA_2,
};
use vfio_user_nvgpu::display::fe_page::{self, FePage};
use vfio_user_nvgpu::fb::FbLayout;
use vfio_user_nvgpu::hostmem::HostMemory;
use vfio_user_nvgpu::hostrm::HostRmClient;
use vfio_user_nvgpu::intr::IntrShared;
use vfio_user_nvgpu::intr_page::{self, IntrPage, PageHook};
use vfio_user_nvgpu::privilege::Privilege;
use vfio_user_nvgpu::profile;
use vfio_user_nvgpu::rpc::StaticIdentity;
use vfio_user_nvgpu::selftest;
use vfio_user_nvgpu::tokens;
use vfio_user_nvgpu::trace::Trace;
use vfio_user_nvgpu::ver;

const USAGE: &str = "\
vfio-user-nvgpu --socket <path> [--trace <file>] [--board-data <name|file>] [--chip <chip>]
                [--config <dump>] [--rom <rom.bin>] [--vbios <image>] [--vbios-version <v>]
                [--boot-model] [--fb-mib <n>] [--board <name>] [--host-rm] [--sm-version <m.n>]
                [--reserve-vram [--reserve-slot <n>]] [--register-guest-ram | --identity-dma-table] [--dma-ranges <file>]
                [--host-exec] [--profile <size>] [--name-format <legacy|transport>] [--display]
                [--guest-abi <595|610|615>] [--bar0-registers <file|none>]
                [--persist <file>]
vfio-user-nvgpu --print-name [--board-data <name|file>] [--board <name>] [--fb-mib <n> | --profile <size>]
                [--name-format <legacy|transport>]
vfio-user-nvgpu --selftest <socket> [--board-data <name|file>] [--chip <chip>] [--config <dump>]

  --socket   vfio-user socket to create (a stale socket file is removed). When the client closes the
             connection, a client connecting within 2 s gets the device reset (a VMM reboot);
             otherwise the device frees its host state and exits
  --trace    access log file (default: none; the summary always goes to stderr)
  --board-data  board data: a built-in board (rtx2070, rtx5060ti, rtx5060ti-on-tu106: the 5060 Ti's PCI identity
             and name on the TU106, docs/caraxes/costume-5060.md) or a boards/*.toml-shaped file (default: rtx2070)
  --sm-version  report SM version <major>.<minor> in every GR info answer instead of the chip's (the
             SM-version rungs of docs/caraxes/costume-ladder.md; the silicon does not change)
  --chip     the chip to present, tu106 or gb206 (or a costume-ladder chip: tu104, ga106, ad106, gh100) (default: the board's, tu106 for rtx2070, gb206 for rtx5060ti)
  --config   lspci -xxxx dump of the card (default: the board's, caraxes/reference/rtx2070/config-space.txt)
  --rom      PCI-visible ROM image for the expansion ROM region (default: the board's, caraxes/reference/rtx2070/rom.bin)
  --vbios    image served through the BAR0 PROM window at its own offsets (default: the
             --rom image); it starts with 55 AA or, like the card's window, with an IFR
             header that leads to the PCI image chain
  --vbios-version  VBIOS version NN.NN.NN.NN.NN of the static info (default: the board's, 90.06.3D.00.76)
  --boot-model  attach the boot model (falcons, GFW/fuse/FB-size/WPR2 registers, GSP message queues)
  --fb-mib   framebuffer size the guest is told, in MiB (default: LEA_VRAM_PROFILE_MIB or 4096)
  --profile  the same as --fb-mib, as a size: 4G, 4GiB, 1333M, 1333MiB or MiB (a vGPU type such as
             4Q is refused: resolve it with nvrm-client --bin vgpuprofile first)
  --board    the personality in the guest name (default: the board's, RTX 2070, giving Leandro VFIO RTX 2070-4G)
  --name-format  transport (Leandro VFIO RTX 2070-4G) or legacy (Leandro RTX 2070-4G); default:
             LEA_GPU_NAME_FORMAT or transport. The name comes from nvrm_abi::naming
  --print-name  print the guest-visible name these flags give and exit
  --host-rm  open the host RM (/dev/nvidiactl, unprivileged client) and forward the forwardable RPCs
  --reserve-vram  hold the profile's size of host VRAM at a fixed address at the top of the host FB
             heap for the device's life and give the guest FB addresses inside it (E5b,
             docs/caraxes/memory-model.md); refused when the host would keep less than 1 GiB free
  --reserve-slot <n>  place the reservation n profile sizes below the top (P20: one slot per
             VM, 2 MiB aligned; default 0 = the top); refused when the slot leaves the heap
  --register-guest-ram  register guest RAM with the host RM in 1 GiB chunks at DMA_MAP and translate
             every guest DMA address through the resulting IOVA table (needs CAP_SYS_ADMIN
             permitted: file capability cap_sys_admin+p, or start with sudo -n)
  --identity-dma-table  publish an identity table (IOVA = GPA) instead, without host registration
  --dma-ranges  file the table is written to, in caraxes_dma_range's ranges= format
  --host-exec  execute on the physical GPU (E5b part 2, docs/caraxes/execution.md): the FB is the
             reserved VRAM through BAR1 windows, the guest's channels get host mirror channels bound
             to the guest's page tables at their first doorbell, GR0/CE non-stall events become guest
             interrupts; implies --boot-model, --host-rm, --reserve-vram and --register-guest-ram
  --display  present the display engine (docs/caraxes/display-ab.md): the display fuse reads 0,
             the display controls are answered from chips/<chip>/display.toml and the board's
             [display] connectors, each connector backed by a DisplayPort sink model; without
             it the device is headless (fuse 1), as in E0-E5
  --selftest connect to a running server and verify the E0 device model
  --persist  file for the settings a real card keeps across resets and power cycles (the ECC
             and MIG modes of a chip that models them, src/families): read at start, where the
             pending settings take effect, and written at every change
  --guest-abi  pin the guest driver's GSP ABI (default: none): 570
             (nova-core), 595 (experiment), 610 or 615 (also the 617 Windows guest); anything
             else is refused (fail closed). Without a pin the guest chooses: every build
             serves all of them and reads the guest's from its GSP boot (RMARGS magic 'GSP '
             for 615, the first queue element's frame for 610 or 570;
             docs/research/one-abi-binary-2026-09-28.md)
  --bar0-registers  the physical card's captured BAR0 registers (bar0.rs CapturedRegisters),
             answering reads of offsets no model owns and nothing latched, traced `captured`
             (default: the newest bar0-registers-*.txt beside the board's config capture;
             none: no table). A comma-separated list merges several captures, e.g. that file
             and falcon-pmu-sec2-2026-09-25.txt. A missing or malformed file, or one whose NV_PMC_BOOT_0 is not
             the presented chip's, leaves no table (those reads stay 0)
Configuration: one set for every guest OS. With --host-exec every host channel runs on a
  shadow VA space, guest DMA addresses are GPAs, and the interrupt page, the hybrid
  doorbell reactor, the guest-token ioeventfds, a 32 MiB BAR1 pool (2 x 16 MiB windows
  beside it) and the host clock boost are on; pure emulation keeps the trapped paths.
  The knobs, besides --profile/--fb-mib and --display:
  CARAXES_COMPRESSION=off|slice  real compression per VM slice (default off, D16b)
  CARAXES_DOORBELL_POLL=off|hybrid|spin  how the doorbell reactor waits (default hybrid
             with --host-exec, off without; off is refused with the interrupt page)
  CARAXES_BAR1_POOL_MIB=<n>  the BAR1 pool, an even 2-128 MiB, 0 for none (default 32
             with --host-exec; docs/caraxes/bar1-pool.md)
  The open switches (kept until an A/B decides them), the diagnostics and the variables
  removed on 2026-09-30 are listed in src/settings.rs. At start the device logs its
  effective configuration in one block (\"config:\" lines) and one warning for every
  removed variable that is still set.
Paths default relative to LEA_ROOT or the current directory.";

struct Args {
    socket: Option<PathBuf>,
    trace: Option<PathBuf>,
    board_data: Board,
    chip: &'static dyn Chip,
    config: PathBuf,
    rom: PathBuf,
    vbios: Option<PathBuf>,
    boot_model: bool,
    fb_mib: u64,
    board: String,
    host_rm: bool,
    display: bool,
    reserve_vram: bool,
    reserve_slot: u32,
    register_guest_ram: bool,
    identity_dma_table: bool,
    dma_ranges: Option<PathBuf>,
    host_exec: bool,
    selftest: Option<PathBuf>,
    name_format: NameFormat,
    print_name: bool,
    /// The pinned guest ABI (`None`: the guest chooses).
    guest_abi: Option<vfio_user_nvgpu::ver::GuestAbi>,
    /// The captured BAR0 table (`None`: no table).
    bar0_registers: Option<PathBuf>,
}

/// The guest ABI as the start-up log names it: the pin, or that the guest chooses.
fn describe_guest_abi(pin: Option<ver::GuestAbi>) -> String {
    match pin {
        Some(g) => format!("{g}, pinned (--guest-abi)"),
        None => "chosen by the guest (570, 610 or 615; 610 until its GSP boot shows it)".to_string(),
    }
}

const VFIO_IDLE_POLL_ENV: &str = "CARAXES_VFIO_IDLE_POLL";
const GUEST_TOKEN_EXTRAS_ENV: &str = "CARAXES_GUEST_TOKEN_EXTRAS";
const DISPLAY_WINDOW_RO_ENV: &str = "CARAXES_DISPLAY_WINDOW_RO";

fn parse_display_window_ro(value: Option<&str>) -> std::result::Result<Option<u64>, String> {
    match value {
        None | Some("0") => Ok(None),
        Some(v) => {
            let instance = v.strip_prefix("window:").filter(|n| {
                !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())
            }).and_then(|n| n.parse::<u64>().ok()).filter(|n| *n < 32);
            instance.map(|n| Some(vfio_user_nvgpu::display::WIN_USERD + n * intr_page::PAGE))
                .ok_or_else(|| format!("{DISPLAY_WINDOW_RO_ENV}={v:?}: expected 0 or window:0..31"))
        }
    }
}

fn requested_display_window_ro() -> Result<Option<u64>> {
    let value = match std::env::var(DISPLAY_WINDOW_RO_ENV) {
        Ok(v) => Some(v),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => bail!("{DISPLAY_WINDOW_RO_ENV} is not Unicode"),
    };
    parse_display_window_ro(value.as_deref()).map_err(anyhow::Error::msg)
}

fn parse_guest_token_extras(value: Option<&str>) -> std::result::Result<Vec<(u32, u32)>, String> {
    let Some(value) = value.filter(|v| *v != "0") else {
        return Ok(Vec::new());
    };
    let invalid = || format!("{GUEST_TOKEN_EXTRAS_ENV}={value:?}: expected 0 or decimal runlist:chid pairs, separated by commas (runlist 0..127, chid 0..4095)");
    let mut pairs = Vec::new();
    for pair in value.split(',') {
        if pairs.len() >= tokens::MAX_TOKENS {
            return Err(format!("{GUEST_TOKEN_EXTRAS_ENV}: at most {} pairs", tokens::MAX_TOKENS));
        }
        let (runlist, chid) = pair.split_once(':').ok_or_else(&invalid)?;
        let decimal = |v: &str| -> Option<u32> {
            (!v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
                .then(|| v.parse().ok())
                .flatten()
        };
        let runlist = decimal(runlist).filter(|v| *v <= tokens::TOKEN_RUNLIST_MAX).ok_or_else(&invalid)?;
        let chid = decimal(chid).filter(|v| *v < 1 << tokens::TOKEN_VECTOR_BITS).ok_or_else(&invalid)?;
        pairs.push((runlist, chid));
    }
    Ok(pairs)
}

fn requested_guest_token_extras() -> Result<Vec<(u32, u32)>> {
    let value = match std::env::var(GUEST_TOKEN_EXTRAS_ENV) {
        Ok(v) => Some(v),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            bail!("{GUEST_TOKEN_EXTRAS_ENV} is not Unicode");
        }
    };
    parse_guest_token_extras(value.as_deref()).map_err(anyhow::Error::msg)
}

fn parse_vfio_idle_poll(value: Option<&str>) -> std::result::Result<IdlePollMode, String> {
    match value {
        None | Some("0") => Ok(IdlePollMode::Off),
        Some("spin") => Ok(IdlePollMode::Spin),
        Some(v) if v.starts_with("hybrid:") => {
            let us = &v["hybrid:".len()..];
            if us.is_empty() || !us.bytes().all(|b| b.is_ascii_digit()) {
                return Err(format!("{VFIO_IDLE_POLL_ENV}={v:?}: expected hybrid:1..1000"));
            }
            match us.parse::<u64>() {
                Ok(n @ 1..=1000) => Ok(IdlePollMode::Hybrid(std::time::Duration::from_micros(n))),
                _ => Err(format!("{VFIO_IDLE_POLL_ENV}={v:?}: expected hybrid:1..1000")),
            }
        }
        Some(v) => Err(format!(
            "{VFIO_IDLE_POLL_ENV}={v:?}: expected 0, hybrid:1..1000, or spin"
        )),
    }
}

fn requested_vfio_idle_poll() -> Result<IdlePollMode> {
    let value = match std::env::var(VFIO_IDLE_POLL_ENV) {
        Ok(v) => Some(v),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(anyhow::anyhow!("{VFIO_IDLE_POLL_ENV} is not Unicode"));
        }
    };
    parse_vfio_idle_poll(value.as_deref()).map_err(anyhow::Error::msg)
}

fn parse_bar_shmem(
    value: Option<&str>, host_exec: bool, boot_model: bool, chip: &str, local_irq: bool,
) -> std::result::Result<bool, String> {
    let enabled = match value {
        None | Some("0") => false,
        Some("1") => true,
        Some(other) => return Err(format!("CARAXES_BAR_SHMEM={other:?}: expected 0 or 1")),
    };
    if enabled && (!host_exec || !boot_model || !chip.eq_ignore_ascii_case("tu106") || !local_irq) {
        return Err("CARAXES_BAR_SHMEM=1 requires fresh TU106 host execution, a boot model and CARAXES_LOCAL_IRQ=1".into());
    }
    Ok(enabled)
}

fn requested_bar_shmem(a: &Args, local_irq: bool) -> Result<bool> {
    let value = match std::env::var("CARAXES_BAR_SHMEM") {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => bail!("CARAXES_BAR_SHMEM is not Unicode"),
    };
    parse_bar_shmem(value.as_deref(), a.host_exec, a.boot_model, &a.chip.data().short_name, local_irq)
        .map_err(anyhow::Error::msg)
}

#[cfg(test)]
mod vfio_idle_poll_tests {
    use super::*;

    #[test]
    fn signal_host_control_drain_finishes_actual_call_before_memory_teardown() {
        use std::sync::mpsc;
        use std::time::Duration;
        struct BlockingHost(mpsc::Sender<()>, mpsc::Receiver<()>);
        impl vfio_user_nvgpu::ctrl::HostRm for BlockingHost {
            fn control(&mut self, _: u32, _: u32, _: &mut [u8]) -> u32 {
                self.0.send(()).unwrap();
                self.1.recv_timeout(Duration::from_secs(5)).unwrap();
                0
            }
        }
        let (entered, running) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let worker =
            vfio_user_nvgpu::host_control::Worker::start(Box::new(BlockingHost(entered, blocked)))
                .unwrap();
        let service = worker.service();
        service.submit(0, 0x2080_a612, vec![0; 54352]).unwrap();
        running.recv_timeout(Duration::from_secs(5)).unwrap();
        let state = Mutex::new(Some(service.clone()));
        let (teardown, observed) = mpsc::channel();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                drain_signal_host_control(&state).unwrap();
                teardown.send(()).unwrap(); // The HostMemory shutdown hook is after this barrier.
            });
            assert!(observed.recv_timeout(Duration::from_millis(30)).is_err());
            release.send(()).unwrap();
            observed.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        assert!(service.submit(0, 0x2080_a612, vec![0; 54352]).is_err());
        worker.stop().unwrap();
    }

    #[test]
    fn bar_shmem_is_explicit_and_limited_to_host_tu106_with_local_irq() {
        assert!(!parse_bar_shmem(None, false, false, "ga102", false).unwrap());
        assert!(!parse_bar_shmem(Some("0"), false, false, "ga102", false).unwrap());
        assert!(parse_bar_shmem(Some("1"), true, true, "TU106", true).unwrap());
        for bad in ["", "on", "true", "2", " 1", "1 "] {
            assert!(parse_bar_shmem(Some(bad), true, true, "tu106", true).is_err());
        }
        for (host, boot, chip, irq) in [
            (false, true, "tu106", true), (true, false, "tu106", true),
            (true, true, "ga102", true), (true, true, "tu106", false),
        ] {
            assert!(parse_bar_shmem(Some("1"), host, boot, chip, irq).is_err());
        }
    }

    #[test]
    fn read_only_window_selection_excludes_other_display_pages() {
        assert_eq!(parse_display_window_ro(None).unwrap(), None);
        assert_eq!(parse_display_window_ro(Some("0")).unwrap(), None);
        assert_eq!(parse_display_window_ro(Some("window:0")).unwrap(), Some(0x690000));
        assert_eq!(parse_display_window_ro(Some("window:4")).unwrap(), Some(0x694000));
        assert_eq!(parse_display_window_ro(Some("window:31")).unwrap(), Some(0x6af000));
        for bad in ["", "4", "on", "window:", "window:-1", "window:32", "window:0x4", "window:4 ", " window:4", "window:18446744073709551616"] {
            assert!(parse_display_window_ro(Some(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn sparse_guest_token_values_are_strict_and_bounded() {
        assert_eq!(parse_guest_token_extras(None).unwrap(), vec![]);
        assert_eq!(parse_guest_token_extras(Some("0")).unwrap(), vec![]);
        assert_eq!(parse_guest_token_extras(Some("0:55,2:56")).unwrap(), vec![(0, 55), (2, 56)]);
        assert_eq!(parse_guest_token_extras(Some("127:4095")).unwrap(), vec![(127, 4095)]);
        for bad in ["", "off", "0x0:55", "0:4096", "128:0", "0:-1", "0:55,", "0:55:1", " 0:55", "0:55 ", "4294967296:0"] {
            assert!(parse_guest_token_extras(Some(bad)).is_err(), "{bad:?}");
        }
        let too_many = std::iter::repeat_n("0:55", tokens::MAX_TOKENS + 1).collect::<Vec<_>>().join(",");
        assert!(parse_guest_token_extras(Some(&too_many)).is_err());
    }

    #[test]
    fn opt_in_modes_are_strict_and_hybrid_is_bounded() {
        assert_eq!(parse_vfio_idle_poll(None).unwrap(), IdlePollMode::Off);
        assert_eq!(parse_vfio_idle_poll(Some("0")).unwrap(), IdlePollMode::Off);
        assert_eq!(parse_vfio_idle_poll(Some("spin")).unwrap(), IdlePollMode::Spin);
        assert_eq!(
            parse_vfio_idle_poll(Some("hybrid:100")).unwrap(),
            IdlePollMode::Hybrid(std::time::Duration::from_micros(100))
        );
        for bad in [
            "",
            "1",
            "hybrid:",
            "hybrid:0",
            "hybrid:1001",
            "hybrid:-1",
            "hybrid: 100",
            "spin ",
        ] {
            assert!(parse_vfio_idle_poll(Some(bad)).is_err(), "{bad:?}");
        }
    }
}

fn parse_args() -> Result<Args> {
    let root = std::env::var_os("LEA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The board data first: it supplies the defaults of the other flags.
    let board_name = args
        .iter()
        .position(|f| f == "--board-data")
        .and_then(|i| args.get(i + 1))
        .map_or("rtx2070", String::as_str);
    let mut board_data = Board::load(board_name)
        .map_err(|e| anyhow::anyhow!("{e} (built in: {})", BOARDS.join(", ")))?;
    let mut chip_name = board_data.chip.clone();
    let mut a = Args {
        socket: None,
        trace: None,
        config: root.join(&board_data.config),
        rom: root.join(&board_data.rom),
        // Resolved after the flags (the board's chip unless --chip names another).
        chip: chip::by_name(&chip_name).map_err(anyhow::Error::msg)?,
        vbios: None,
        boot_model: false,
        fb_mib: std::env::var("LEA_VRAM_PROFILE_MIB")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(4096),
        board: board_data.name.clone(),
        host_rm: false,
        display: false,
        reserve_vram: false,
        reserve_slot: 0,
        register_guest_ram: false,
        identity_dma_table: false,
        dma_ranges: None,
        host_exec: false,
        selftest: None,
        name_format: NameFormat::from_env()?,
        print_name: false,
        board_data: board_data.clone(),
        guest_abi: None,
        // The board's reference directory: the one its config capture lives in.
        bar0_registers: root
            .join(&board_data.config)
            .parent()
            .and_then(vfio_user_nvgpu::bar0::CapturedRegisters::newest_in),
    };
    let mut guest_abi_flag = None;
    let mut it = args.into_iter();
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .with_context(|| format!("{flag} needs a value\n{USAGE}"))
        };
        match flag.as_str() {
            "--socket" => a.socket = Some(PathBuf::from(value()?)),
            "--trace" => a.trace = Some(PathBuf::from(value()?)),
            "--board-data" => {
                value()?;
            }
            "--chip" => chip_name = value()?,
            "--config" => a.config = PathBuf::from(value()?),
            "--rom" => a.rom = PathBuf::from(value()?),
            "--vbios" => a.vbios = Some(PathBuf::from(value()?)),
            "--vbios-version" => board_data
                .set_vbios_version(&value()?)
                .map_err(anyhow::Error::msg)?,
            "--boot-model" => a.boot_model = true,
            "--board" => a.board = value()?,
            "--host-rm" => a.host_rm = true,
            "--display" => a.display = true,
            "--reserve-vram" => a.reserve_vram = true,
            "--reserve-slot" => {
                a.reserve_slot = value()?.parse().context("--reserve-slot needs a number")?
            }
            "--register-guest-ram" => a.register_guest_ram = true,
            "--identity-dma-table" => a.identity_dma_table = true,
            "--dma-ranges" => a.dma_ranges = Some(PathBuf::from(value()?)),
            "--persist" => vfio_user_nvgpu::families::set_persist_path(PathBuf::from(value()?))
                .map_err(anyhow::Error::msg)?,
            "--host-exec" => a.host_exec = true,
            "--fb-mib" => a.fb_mib = value()?.parse().context("--fb-mib needs a number")?,
            "--selftest" => a.selftest = Some(PathBuf::from(value()?)),
            "--profile" => {
                let p: naming::Profile = value()?
                    .parse()
                    .map_err(|e| anyhow::anyhow!("--profile: {e}"))?;
                a.fb_mib = p.fb_mib();
            }
            "--name-format" => {
                a.name_format = value()?
                    .parse()
                    .map_err(|e| anyhow::anyhow!("--name-format: {e}"))?
            }
            "--print-name" => a.print_name = true,
            "--sm-version" => {
                let v = value()?;
                let sm = vfio_user_nvgpu::kgr::parse_sm_version(&v)
                    .with_context(|| format!("--sm-version {v:?}: expected <major>.<minor>"))?;
                vfio_user_nvgpu::kgr::set_sm_version_override(sm).map_err(anyhow::Error::msg)?;
                eprintln!("vfio-user-nvgpu: SM version reported as {v} ({sm:#06x}) in every GR info answer (costume ladder, docs/caraxes/costume-ladder.md)");
            }
            "--bar0-registers" => {
                let v = value()?;
                a.bar0_registers = (v != "none").then(|| PathBuf::from(v));
            }
            "--guest-abi" => {
                guest_abi_flag = Some(value()?.parse().context("--guest-abi needs 570, 595, 610 or 615")?)
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => bail!("unknown argument {other:?}\n{USAGE}"),
        }
    }
    a.chip = chip::by_name(&chip_name).map_err(|e| anyhow::anyhow!("--chip: {e}"))?;
    a.guest_abi =
        ver::check_guest_abi(guest_abi_flag).map_err(|e| anyhow::anyhow!("--guest-abi: {e}"))?;
    ver::pin_guest(a.guest_abi).map_err(|e| anyhow::anyhow!("--guest-abi: {e}"))?;
    // The costume ladder's class translation for the mirror (board `class_map`).
    vfio_user_nvgpu::board::install_class_map(board_data.class_map.clone());
    a.board_data = board_data;
    if a.socket.is_none() && a.selftest.is_none() && !a.print_name {
        bail!("{USAGE}");
    }
    // Host execution needs the boot model, the host RM, the VRAM reservation and registered
    // guest RAM, so --host-exec implies them (naming them as well changes nothing). An
    // explicit --identity-dma-table is not overridden: it is refused below.
    if a.host_exec {
        a.boot_model = true;
        a.host_rm = true;
        a.reserve_vram = true;
        a.register_guest_ram |= !a.identity_dma_table;
    }
    // The host paths talk to the house card, a TU106 (docs/caraxes/blackwell-gb206.md
    // section 4): any other chip identity runs in pure emulation, unless the board is a
    // costume-ladder board that presents it over the TU106 (`host_chip = "tu106"` and the
    // board's own chip, docs/caraxes/costume-ladder.md).
    let chip_name = &a.chip.data().name;
    let on_tu106 = chip_name == "tu106"
        || (a.board_data.host_chip.as_deref() == Some("tu106") && *chip_name == a.board_data.chip);
    if !on_tu106 && (a.host_rm || a.reserve_vram || a.register_guest_ram || a.host_exec) {
        bail!("--chip {chip_name} runs in pure emulation: --host-rm, --reserve-vram, --register-guest-ram and --host-exec need a host card of the same chip (or a costume-ladder board with host_chip = \"tu106\")");
    }
    if a.register_guest_ram && a.identity_dma_table {
        bail!("--register-guest-ram and --identity-dma-table exclude each other");
    }
    if a.reserve_slot != 0 && !a.reserve_vram {
        bail!("--reserve-slot needs --reserve-vram");
    }
    if a.reserve_vram && !a.boot_model {
        bail!("--reserve-vram needs --boot-model (the FB layout lives in the boot model)");
    }
    if a.host_exec && !a.register_guest_ram {
        bail!("--host-exec needs registered guest RAM, not --identity-dma-table (the guest's addresses must be the host's before anything runs on the physical GPU)");
    }
    Ok(a)
}

/// What the start-up resolved, for the configuration block.
struct ConfigView<'a> {
    a: &'a Args,
    env: &'a [(String, String)],
    poll: &'static str,
    compression: bool,
    pool_mib: Option<u64>,
    doorbell_tokens: Option<usize>,
    guest_tokens: Option<usize>,
    msi_rearm: bool,
    rpc_ioeventfd: bool,
    intr_page: bool,
    display_page: bool,
    kick_page: bool,
    usermode_page: bool,
    invalidate_page: bool,
    bar2_memfd: bool,
    zero_fb: bool,
}

/// The effective configuration in one block (settings.rs): the mode, the five knobs with
/// where each value came from, the fast paths in force, the open switches and diagnostics
/// that are set, and the `CARAXES_` variables the device does not read.
fn config_block(c: &ConfigView) -> Vec<String> {
    use vfio_user_nvgpu::settings::{self, Class};
    let a = c.a;
    let on = |b: bool| if b { "on" } else { "off" };
    let from = |name: &str| {
        if c.env.iter().any(|(k, _)| k == name) {
            "set"
        } else {
            "default"
        }
    };
    let mode = if a.host_exec {
        format!(
            "host execution (boot model, host RM, VRAM reservation slot {}, registered guest RAM)",
            a.reserve_slot
        )
    } else {
        let mut parts = vec![if a.boot_model { "boot model" } else { "no boot model" }];
        parts.extend(a.host_rm.then_some("host RM"));
        parts.extend(a.reserve_vram.then_some("VRAM reservation"));
        parts.extend(a.register_guest_ram.then_some("registered guest RAM"));
        format!("emulation without host execution ({})", parts.join(", "))
    };
    let mut lines = vec![
        format!(
            "mode {mode}; chip {}, board {}; guest ABI {}",
            a.chip.data().short_name,
            a.board,
            describe_guest_abi(a.guest_abi)
        ),
        format!(
            "knobs: VRAM {} MiB (--fb-mib/--profile, LEA_VRAM_PROFILE_MIB); display {} (--display); {}={} ({}); {}={} ({}); {}={} ({})",
            a.fb_mib,
            on(a.display),
            compslice::ENV,
            if c.compression { "slice" } else { "off" },
            from(compslice::ENV),
            vfio_user_nvgpu::poll::POLL_ENV,
            c.poll,
            from(vfio_user_nvgpu::poll::POLL_ENV),
            vfio_user_nvgpu::bar1pool::POOL_MIB_ENV,
            c.pool_mib.unwrap_or(0),
            from(vfio_user_nvgpu::bar1pool::POOL_MIB_ENV),
        ),
    ];
    let mut paths = Vec::new();
    if a.host_exec {
        paths.push("every host channel on a shadow VA space, GPA DMA, host boost".to_string());
    }
    if a.reserve_vram {
        let (w, n) = vfio_user_nvgpu::fb::bar1_budget();
        paths.push(format!("BAR1 windows {n} x {} MiB", w >> 20));
    }
    paths.push(format!("interrupt page {}", on(c.intr_page)));
    paths.push(format!(
        "doorbell tokens {}, guest tokens {}",
        c.doorbell_tokens.map_or("off".into(), |t| t.to_string()),
        c.guest_tokens.map_or("off".into(), |t| t.to_string())
    ));
    paths.push(format!(
        "MSI re-arm ioeventfd {}, GSP queue head ioeventfd {}",
        on(c.msi_rearm),
        on(c.rpc_ioeventfd)
    ));
    paths.push(format!(
        "display page {}, kick page {}, usermode page {}, invalidate page {}",
        on(c.display_page),
        on(c.kick_page),
        on(c.usermode_page),
        on(c.invalidate_page)
    ));
    paths.push(format!("BAR2 memfd {}", on(c.bar2_memfd)));
    if c.zero_fb {
        paths.push("zero-based guest FB".to_string());
    }
    lines.push(format!("paths: {}", paths.join("; ")));
    let or_none = |v: Vec<String>| if v.is_empty() { "none".to_string() } else { v.join(" ") };
    lines.push(format!("open switches: {}", or_none(settings::set_of(c.env, Class::Open))));
    lines.push(format!("diagnostics: {}", or_none(settings::set_of(c.env, Class::Diag))));
    let inputs = settings::set_of(c.env, Class::Input);
    if !inputs.is_empty() {
        lines.push(format!("inputs: {}", inputs.join(" ")));
    }
    let unread = settings::unread(c.env);
    if !unread.is_empty() {
        lines.push(format!(
            "not read by the device (launcher settings or misspelled): {}",
            unread.join(" ")
        ));
    }
    lines
}

fn config_space(a: &Args) -> Result<ConfigSpace> {
    let dump = std::fs::read_to_string(&a.config)
        .with_context(|| format!("read {}", a.config.display()))?;
    // The board's capability filter (MSI-X) and its identity overlay (`ids_from`, the
    // costume board) apply to any dump, also a --config one.
    let ids = match &a.board_data.ids_from {
        Some(p) => {
            let root = std::env::var_os("LEA_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            let path = root.join(p);
            Some(
                std::fs::read_to_string(&path)
                    .with_context(|| format!("read {} (the board's ids_from)", path.display()))?,
            )
        }
        None => None,
    };
    let mut cfg = pci_config::for_board_with_ids(&dump, ids.as_deref(), &a.board_data)?;
    if pci_config::ext_config_requested() {
        cfg.set_ext_visible(true);
        eprintln!(
            "vfio-user-nvgpu: {}=1: the config region shows the extended configuration space",
            pci_config::EXT_CONFIG_ENV
        );
    }
    Ok(cfg)
}

/// One soak line (soak.rs) from the host memory client and, when given, the device's counts.
fn host_soak_line(
    label: &str,
    h: &HostMemory,
    dev: Option<&vfio_user_nvgpu::soak::DeviceCounts>,
) -> String {
    vfio_user_nvgpu::soak::line(
        label,
        vfio_user_nvgpu::soak::rss_kib(),
        h.host_figures(),
        Some(&h.soak_held()),
        dev,
    )
}

/// The soak line from the device's counts (taken first, the device lock released) and the
/// host memory client, if the device has one.
fn soak_line(
    label: &str,
    counts: vfio_user_nvgpu::soak::DeviceCounts,
    hm: Option<&Arc<Mutex<HostMemory>>>,
) -> String {
    match hm {
        Some(hm) => {
            let h = hm.lock().unwrap_or_else(PoisonError::into_inner);
            host_soak_line(label, &h, Some(&counts))
        }
        None => vfio_user_nvgpu::soak::line(
            label,
            vfio_user_nvgpu::soak::rss_kib(),
            None,
            None,
            Some(&counts),
        ),
    }
}

/// The periodic soak line while the guest runs (`CARAXES_SOAK_SECS`, default 60 s; the
/// leak watch, docs/caraxes/host-resources.md).
fn spawn_soak_thread(
    device: Arc<Mutex<Device>>,
    hm: Option<Arc<Mutex<HostMemory>>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
) -> Option<std::thread::JoinHandle<()>> {
    let every = vfio_user_nvgpu::soak::interval()?;
    eprintln!(
        "vfio-user-nvgpu: soak line every {} s ({}; 0 turns it off), once at exit; {}",
        every.as_secs(),
        vfio_user_nvgpu::soak::ENV,
        hm.as_ref().map_or("no host memory client".to_string(), |h| h
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .guard()
            .line())
    );
    std::thread::Builder::new()
        .name("soak".into())
        .spawn(move || {
            let mut next = std::time::Instant::now() + every;
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(250));
                if std::time::Instant::now() < next {
                    continue;
                }
                next += every;
                let (counts, vectors) = {
                    let d = device.lock().unwrap_or_else(PoisonError::into_inner);
                    (d.soak_counts(), d.vector_summary())
                };
                let label = format!("{}s", every.as_secs());
                eprintln!("vfio-user-nvgpu: {}", soak_line(&label, counts, hm.as_ref()));
                // Which interrupt vectors the guest services (the NVENC async timeout,
                // windows-findings.md): live, not only at exit.
                if let Some(v) = vectors {
                    eprintln!("vfio-user-nvgpu: {v}");
                }
            }
        })
        .ok()
}

/// Free the host memory on SIGTERM, SIGINT or SIGHUP, then exit. The signals are
/// blocked in every other thread (the mask is set before any thread starts), so this
/// thread alone receives them.
fn drain_signal_host_control(
    service: &Mutex<Option<vfio_user_nvgpu::host_control::Service>>,
) -> Result<(), String> {
    let service = service
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    match service {
        Some(service) => service.close(),
        None => Ok(()),
    }
}

fn spawn_signal_thread(
    hm: Arc<Mutex<HostMemory>>,
    host_control: Arc<Mutex<Option<vfio_user_nvgpu::host_control::Service>>>,
) -> Result<()> {
    // SAFETY: sigset manipulation on a local set; pthread_sigmask on this (the only)
    // thread before the helper exists, so both threads share the blocked mask.
    let set = unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for s in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            libc::sigaddset(&mut set, s);
        }
        if libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) != 0 {
            bail!("pthread_sigmask");
        }
        set
    };
    std::thread::Builder::new()
        .name("signals".into())
        .spawn(move || {
            let mut sig = 0;
            // SAFETY: sigwait on the set blocked above.
            unsafe { libc::sigwait(&set, &mut sig) };
            eprintln!("vfio-user-nvgpu: signal {sig}: freeing host memory and exiting");
            if let Err(error) = drain_signal_host_control(&host_control) {
                eprintln!("vfio-user-nvgpu: signal: host-control drain failed: {error}");
            }
            // A DMA_MAP registration in progress holds the lock; wait for it.
            let (log, closed) = match hm.lock() {
                Ok(mut h) => {
                    eprintln!("vfio-user-nvgpu: {}", host_soak_line("exit (signal)", &h, None));
                    (h.shutdown(), h.fail_closed().map(str::to_owned))
                }
                Err(p) => {
                    let mut h = p.into_inner();
                    (h.shutdown(), h.fail_closed().map(str::to_owned))
                }
            };
            for l in log {
                eprintln!("vfio-user-nvgpu: hostmem: {l}");
            }
            if let Some(line) = vfio_user_nvgpu::hostboost::summary() {
                eprint!("{line}");
            }
            profile::dump_at_exit();
            if let Some(why) = closed {
                eprintln!("vfio-user-nvgpu: exit 3: teardown failed closed ({why})");
                std::process::exit(FAIL_CLOSED_EXIT);
            }
            std::process::exit(128 + sig);
        })?;
    Ok(())
}

fn serve(a: &Args, socket: &Path, privilege: Privilege) -> Result<()> {
    // Strict policy validation precedes threads and every host RM/VRAM allocation.
    let vfio_idle_poll = requested_vfio_idle_poll()?;
    let guest_token_extras = requested_guest_token_extras()?;
    let ro_window_base = requested_display_window_ro()?;
    if ro_window_base.is_some() {
        if !a.host_exec || !a.boot_model || !a.display {
            bail!("{DISPLAY_WINDOW_RO_ENV} needs --host-exec, --boot-model and --display");
        }
        if fe_page::requested_mode(true, true).map_err(anyhow::Error::msg)?
            .is_some_and(|mode| mode.automatic_kicks())
        {
            bail!("{DISPLAY_WINDOW_RO_ENV} overlaps the legacy writable display kick page; use {}=exact-ro or 0", fe_page::DISPLAY_PAGE_ENV);
        }
    }
    if !guest_token_extras.is_empty() && !a.boot_model {
        bail!("{GUEST_TOKEN_EXTRAS_ENV} needs --boot-model");
    }
    if !guest_token_extras.is_empty()
        && vfio_user_nvgpu::um_page::requested(true, true).map_err(anyhow::Error::msg)?
    {
        bail!("{GUEST_TOKEN_EXTRAS_ENV} needs a trapped doorbell, incompatible with {}", vfio_user_nvgpu::um_page::USERMODE_PAGE_ENV);
    }
    let boost_policy = vfio_user_nvgpu::hostboost::requested_policy().map_err(anyhow::Error::msg)?;
    if !a.host_exec {
        vfio_user_nvgpu::hostboost::set_off(boost_policy, "no host execution");
        eprintln!("vfio-user-nvgpu: host boost policy={}: inactive without --host-exec", boost_policy.name());
    }
    // The environment against the settings table (settings.rs): a removed variable that is
    // still set is named at once, so an old wrapper is noticed; the whole configuration
    // follows in one block before the server starts (`config_block`).
    let env = vfio_user_nvgpu::settings::environment();
    for w in vfio_user_nvgpu::settings::removed_warnings(&env) {
        eprintln!("vfio-user-nvgpu: config: {w}");
    }
    let local_irq = match std::env::var("CARAXES_LOCAL_IRQ").as_deref() {
        Ok("1") => true,
        Ok("0") | Err(_) => false,
        Ok(other) => bail!("CARAXES_LOCAL_IRQ={other:?}: expected 0 or 1"),
    };
    if local_irq && (!a.host_exec || !a.boot_model || !a.chip.data().short_name.eq_ignore_ascii_case("tu106")) {
        bail!("CARAXES_LOCAL_IRQ=1 requires fresh TU106 host execution and a boot model");
    }
    let bar_shmem = requested_bar_shmem(a, local_irq)?;
    if bar_shmem && vfio_idle_poll != IdlePollMode::Off {
        anyhow::bail!("CARAXES_BAR_SHMEM=1 requires CARAXES_VFIO_IDLE_POLL=0");
    }
    // The datamatch token pool: one eventfd per pool token, so the eventfd names the
    // channel (docs/caraxes/doorbell-token-pool.md).
    let doorbell_tokens = tokens::requested().map_err(anyhow::Error::msg)?;
    // The MSI re-arm write as an ioeventfd, read by the token pool's reactor
    // (docs/caraxes/intr-page.md §4).
    let msi_rearm = msi_rearm_ioeventfd_requested();
    if local_irq && msi_rearm {
        bail!("CARAXES_LOCAL_IRQ=1 keeps MSI re-arm trapped in the VMM; disable CARAXES_MSI_REARM_IOEVENTFD");
    }
    // The command-queue head write as an ioeventfd, read by the token pool's reactor
    // (docs/caraxes/gsp-transport-615.md §11).
    let rpc_ioeventfd = rpc_ioeventfd_requested();
    let host_control_offlock =
        std::env::var(vfio_user_nvgpu::host_control::ENV).is_ok_and(|v| v == "1");
    if host_control_offlock {
        if !a.host_rm || !a.boot_model || !a.host_exec {
            bail!("CARAXES_HOST_CONTROL_OFFLOCK=1 requires --host-exec/GPA, --host-rm and --boot-model");
        }
        vfio_user_nvgpu::host_control::validate_options(
            std::env::var("CARAXES_ACK_UNANSWERED").is_ok_and(|v| v == "1"),
            vfio_user_nvgpu::recorded::excepted,
            |cmd| {
                std::env::var("CARAXES_WIN_ACK_EXTRA").is_ok_and(|values| {
                    values
                        .split(',')
                        .filter_map(|s| {
                            u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()
                        })
                        .any(|v| v == cmd)
                })
            },
            vfio_user_nvgpu::recorded::overrides,
            vfio_user_nvgpu::nvml::legacy_cacheable_enabled(),
            rpc_ioeventfd,
        )
        .map_err(anyhow::Error::msg)?;
        if vfio_user_nvgpu::ver::pinned().is_some_and(|g| g != vfio_user_nvgpu::ver::GuestAbi::V615)
        {
            bail!("CARAXES_HOST_CONTROL_OFFLOCK=1 requires the MsgqV2 guest transport");
        }
    }
    eprintln!("vfio-user-nvgpu: host control offlock: {} (A612/A613 exact host results, trapped MsgqV2 head)", if host_control_offlock { "on" } else { "off" });

    // The guest's own tokens as datamatch ioeventfds (docs/caraxes/doorbell-token-pool.md
    // (g)): for guests that never read the notifier entry the pool hands tokens out through.
    // By default the set comes from the engines this device offers, sized to the eventfds
    // one reply can carry beside the pool's and the re-arm and head eventfds. They need the
    // boot model (the channels live there).
    let guest_tokens = if a.boot_model {
        let budget = tokens::MAX_TOKENS
            .saturating_sub(doorbell_tokens.unwrap_or(0))
            .saturating_sub(msi_rearm as usize + rpc_ioeventfd as usize);
        let d = a.chip.data();
        let engines: Vec<(&str, u32, bool)> = d
            .device_info
            .engines
            .iter()
            .map(|e| (e.name.as_str(), e.runlist, e.host_driven == 1))
            .collect();
        tokens::GuestTokens::auto(&engines, d.fifo.num_channels.value, budget)
            .map(|base| {
                tokens::GuestTokenPlan::with_extra_values(
                    base,
                    d.fifo.num_channels.value,
                    budget,
                    &guest_token_extras,
                )
            })
            .transpose()
            .map_err(anyhow::Error::msg)?
    } else {
        None
    };
    if !guest_token_extras.is_empty() && guest_tokens.is_none() {
        bail!("{GUEST_TOKEN_EXTRAS_ENV}: no automatic guest-token set fits the FD budget");
    }
    // The pool's reactor reads every datamatch doorbell eventfd, the pool's and the guest
    // tokens'; the MSI re-arm and command-queue head eventfds ride on it.
    let token_reactor_on = doorbell_tokens.is_some() || guest_tokens.is_some();
    if msi_rearm && !token_reactor_on {
        bail!(
            "{MSI_REARM_IOEVENTFD_ENV}=1 needs the token pool's reactor to read its eventfd: {}=N or the guest tokens (--boot-model)",
            tokens::TOKENS_ENV,
        );
    }
    // Fail closed: without the pool's reactor or the boot model the register stays trapped.
    if rpc_ioeventfd && (!token_reactor_on || !a.boot_model) {
        bail!(
            "{RPC_IOEVENTFD_ENV}=1 needs the token pool's reactor to read its eventfd ({}=N or the guest tokens) and the boot model (--boot-model)",
            tokens::TOKENS_ENV
        );
    }
    // Before any thread: SIGUSR2 must be blocked everywhere except the dump thread.
    if profile::requested() {
        let prefix = profile::prefix(a.trace.as_deref());
        profile::start(prefix.clone()).context("profile")?;
        profile::spawn_dump_thread().context("profile dump thread")?;
        profile::register_source(Box::new(profile::doorbell_token_rows));
        profile::register_source(Box::new(profile::routed_rows));
        eprintln!(
            "vfio-user-nvgpu: profile on ({}=1): dumps to {}.<n>.tsv on SIGUSR2 and {}.exit.tsv at exit (src/profile.rs)",
            profile::PROFILE_ENV,
            prefix.display(),
            prefix.display()
        );
    }
    let mut rpc_head = None;
    // How the doorbell eventfds are read (docs/caraxes/doorbell-poll.md): the knob, hybrid
    // with host execution unless set.
    let poll = vfio_user_nvgpu::poll::requested(a.host_exec).map_err(anyhow::Error::msg)?;
    if let Some(hold) = vfio_user_nvgpu::launch::configure_gate().map_err(anyhow::Error::msg)? {
        eprintln!(
            "vfio-user-nvgpu: intr gate: host non-stall events held up to {} ms while this device wrote no host doorbell for {} ms ({}, backlog item 5, docs/caraxes/launch-path.md)",
            hold.as_millis(),
            (hold * vfio_user_nvgpu::launch::HOT_FACTOR).as_millis(),
            vfio_user_nvgpu::launch::GATE_ENV
        );
    }
    vfio_user_nvgpu::launch::configure_fast();
    if vfio_user_nvgpu::launch::configure().map_err(anyhow::Error::msg)? {
        eprintln!(
            "vfio-user-nvgpu: launch timing on: per-step histograms of the doorbell and completion path in the exit summary ({}=1, docs/caraxes/launch-path.md)",
            vfio_user_nvgpu::launch::TIMING_ENV
        );
    }
    if poll.mode != vfio_user_nvgpu::poll::Mode::Off && !token_reactor_on {
        bail!(
            "{}={} needs a doorbell eventfd to poll: {}=N or the guest tokens (--boot-model)",
            vfio_user_nvgpu::poll::POLL_ENV,
            poll.mode.name(),
            tokens::TOKENS_ENV
        );
    }
    // The interrupt page (docs/caraxes/intr-page.md), on with host execution: the guest's
    // acks and enable writes land in memory and are seen only by a polling reactor.
    let intr_page_config = intr_page::requested(a.host_exec);
    let intr_direct = intr_page::direct_requested(
        std::env::var(intr_page::DIRECT_ENV).ok().as_deref(),
        intr_page_config.is_some(),
    )
    .map_err(anyhow::Error::msg)?;
    // The display front-end page (display/fe_page.rs): a third area of the same memfd.
    let display_page_mode =
        fe_page::requested_mode(intr_page_config.is_some(), a.display).map_err(anyhow::Error::msg)?;
    // The legacy writable FE reactor can finish an old scan after reset and
    // raise through the ticker in the new epoch. Exact FE callbacks finish
    // under the current display slot guard; the clock captures its source.
    if local_irq && display_page_mode.is_some_and(|mode| mode.automatic_kicks()) {
        bail!("CARAXES_LOCAL_IRQ=1 requires CARAXES_DISPLAY_PAGE=exact-ro or 0");
    }
    if ["CARAXES_SHADOW_USERD", "CARAXES_DEFERRED_API_CAPTURE", "CARAXES_DEFERRED_CONTEXT"].iter()
        .any(|key| std::env::var(key).as_deref() == Ok("1"))
        && !a.host_exec
    {
        bail!("private USERD/deferred API capture needs --host-exec");
    }
    if std::env::var("CARAXES_DEFERRED_CONTEXT").as_deref() == Ok("1")
        && (std::env::var("CARAXES_SHADOW_USERD").as_deref() != Ok("1")
            || std::env::var("CARAXES_DEFERRED_API_CAPTURE").as_deref() == Ok("1")) {
        bail!("deferred context execution needs private USERD and capture mode off");
    }
    let zero_fb = std::env::var("CARAXES_FB_ZERO_BASED").as_deref() == Ok("1");
    if zero_fb && !a.host_exec {
        bail!("CARAXES_FB_ZERO_BASED=1 needs --host-exec");
    }
    // D16b (compslice.rs): real compression per VM slice. The reservation is allocated
    // compressed, and only shadow-only host execution may run on it (invariant (b)).
    let compression = compslice::requested().map_err(anyhow::Error::msg)?;
    if compression == compslice::Mode::Slice {
        if !a.host_exec {
            bail!("{}=slice needs --host-exec", compslice::ENV);
        }
        eprintln!(
            "vfio-user-nvgpu: compression: {}=slice: the reservation is allocated compressed and the shadow maps compressible guest kinds on its pages (D16b)",
            compslice::ENV
        );
    }
    if intr_page_config.is_some() && poll.mode == vfio_user_nvgpu::poll::Mode::Off {
        bail!(
            "{}=off: host execution runs the interrupt page, whose acks and enable writes only a polling reactor sees (docs/caraxes/intr-page.md §5); use hybrid or spin",
            vfio_user_nvgpu::poll::POLL_ENV
        );
    }
    // Refuse at start when a poll mode cannot pin its reader (no core to choose).
    let reactor = |name| {
        vfio_user_nvgpu::poll::Reactor::new(name, poll, Some(socket.to_path_buf()))
            .map_err(anyhow::Error::msg)
    };
    let token_reactor = reactor("tokens")?;
    if token_reactor_on && !a.boot_model {
        bail!(
            "{}=N needs --boot-model (the channels live in the boot model)",
            tokens::TOKENS_ENV,
        );
    }
    // The BAR1 pool (docs/caraxes/bar1-pool.md), on with host execution: every GPU access
    // goes through a translation the device controls, since every host channel runs on a
    // shadow VA space. CARAXES_BAR1_POOL_MIB sizes it (0: none).
    let bar1_pool = vfio_user_nvgpu::bar1pool::config(a.host_exec).map_err(anyhow::Error::msg)?;
    let cfg = config_space(a)?;
    let rom = std::fs::read(&a.rom).with_context(|| {
        format!(
            "read {} (the PCI-visible ROM of the card; capture it with `cat /sys/bus/pci/devices/<bdf>/rom` after `echo 1 > rom`)",
            a.rom.display()
        )
    })?;
    if rom.len() < 2 || rom[0] != 0x55 || rom[1] != 0xAA {
        bail!(
            "{} does not start with the 55 AA ROM signature",
            a.rom.display()
        );
    }
    let vbios = match &a.vbios {
        Some(p) => {
            let img = std::fs::read(p).with_context(|| format!("read {}", p.display()))?;
            // Served byte for byte at its PROM offsets. An image that starts with 55 AA
            // is taken as before; otherwise the PCI image chain must be found the way the
            // RM finds it (the card's window: an IFR header at 0, the chain at 0x28600;
            // vbios.rs).
            let starts = img.len() >= 2 && img[0] == 0x55 && img[1] == 0xAA;
            match vfio_user_nvgpu::vbios::locate(&img) {
                Some(i) => eprintln!(
                    "vfio-user-nvgpu: PROM image {}: PCI image {:04x}:{:04x} at {:#x} ({:?}), {:#x} bytes, BIT {}",
                    p.display(),
                    i.vendor,
                    i.device,
                    i.offset,
                    i.how,
                    i.len,
                    i.bit.map_or_else(|| "none".into(), |b| format!("at image + {b:#x}"))
                ),
                None if starts => {}
                None => bail!(
                    "{} neither starts with the 55 AA ROM signature nor holds a PCI image the IFR header or a 512-byte scan finds",
                    p.display()
                ),
            }
            Some(img)
        }
        None => None,
    };
    let chip = a.chip;
    let identity = chip.data().identity;
    let trace = Trace::new(a.trace.as_deref())?;
    let mut bar0 = Bar0::new(chip, rom, vbios);
    // The physical card's captured registers for unmodelled offsets; fail closed (no table)
    // on anything wrong with the file.
    match &a.bar0_registers {
        None => eprintln!("vfio-user-nvgpu: no captured BAR0 table; unmodelled registers read 0"),
        Some(p) => match vfio_user_nvgpu::bar0::CapturedRegisters::load(p)
            .and_then(|t| t.check_identity(identity.boot_0).map(|()| t))
        {
            Ok(t) => {
                eprintln!(
                    "vfio-user-nvgpu: captured BAR0 table {}: {} registers answer unmodelled reads (traced `captured`)",
                    p.display(),
                    t.len()
                );
                bar0 = bar0.with_captured(t);
            }
            Err(e) => eprintln!(
                "vfio-user-nvgpu: WARNING: captured BAR0 table refused ({e}); unmodelled registers read 0"
            ),
        },
    }
    // The host driver's ABI, before the first host RM client opens (hostabi.rs,
    // docs/caraxes/cutover-615.md): CARAXES_HOST_ABI, else the running driver's
    // version; any version without measured layouts ends start-up here.
    if a.reserve_vram || a.register_guest_ram || a.host_rm {
        let sel = vfio_user_nvgpu::hostabi::select().map_err(anyhow::Error::msg)?;
        if host_control_offlock && sel.layouts.version != nvrm_sys::DriverVersion::V615 {
            bail!("CARAXES_HOST_CONTROL_OFFLOCK=1 requires the qualified 615 host ABI");
        }
        eprintln!(
            "vfio-user-nvgpu: {} (chosen by {}); guest ABI {}",
            sel.layouts.describe(),
            sel.how,
            describe_guest_abi(a.guest_abi)
        );
    }
    // Host memory for E5b: one client for the reservation and the guest RAM, shared with
    // the signal thread so that SIGTERM frees it too.
    let signal_host_control = Arc::new(Mutex::new(None));
    let host_mem = if a.reserve_vram || a.register_guest_ram {
        let hm = HostMemory::open(privilege).context("open the host RM for host memory")?;
        eprintln!(
            "vfio-user-nvgpu: host memory client open; privilege: {}",
            hm.privilege().describe()
        );
        let hm = Arc::new(Mutex::new(hm));
        spawn_signal_thread(hm.clone(), signal_host_control.clone())?;
        Some(hm)
    } else {
        None
    };
    let intr_shared = IntrShared::new();
    let mut layout = FbLayout::plain(a.fb_mib << 20);
    // P21: the dma-buf behind the guest's BAR1 passthrough area, held for the process's
    // life (docs/caraxes/bar1-passthrough.md §6).
    let mut bar1_export = None;
    // CARAXES_BAR2_MEMFD=1 (docs/caraxes/bar2-memfd.md): region 3 from a memfd. A malformed
    // trap list ends start-up; a memfd that cannot be set up leaves BAR2 trapped.
    let bar2_config = vfio_user_nvgpu::bar2area::config().map_err(anyhow::Error::msg)?;
    let mut bar2_region = None;
    if bar2_config.is_some() && !a.boot_model {
        eprintln!("vfio-user-nvgpu: ERROR: bar2 memfd: needs --boot-model (the FB lives there); starting without the area, BAR2 stays trapped");
    }
    // Whether the host granted the reservation as compressible backing (compslice.rs `Grant`).
    let mut compression_granted = false;
    if a.reserve_vram {
        let hm = host_mem.as_ref().expect("opened above");
        let mut h = hm.lock().unwrap();
        match h.reserve(a.fb_mib << 20, a.reserve_slot, compression) {
            Ok(log) => log
                .iter()
                .for_each(|l| eprintln!("vfio-user-nvgpu: hostmem: {l}")),
            Err(e) => {
                eprintln!("vfio-user-nvgpu: hostmem: {e:#}");
                let log = h.shutdown();
                log.iter()
                    .for_each(|l| eprintln!("vfio-user-nvgpu: hostmem: {l}"));
                bail!("VRAM reservation refused: {e:#}");
            }
        }
        let r = h.reservation().expect("reserved").clone();
        compression_granted = r.compression.is_some();
        layout = FbLayout::reserved(a.fb_mib << 20, r.host_fb_total, r.base, r.end())
            .map_err(anyhow::Error::msg)?;
        if zero_fb {
            layout = layout.zero_based().map_err(anyhow::Error::msg)?;
            eprintln!("vfio-user-nvgpu: experimental zero-based guest FB; host reservation [{:#x}, {:#x}), guest reservation {:?}; shadow execution only", r.base, r.end(), layout.reservation);
        }
        if let Some(len) = bar1_pool {
            // P21 design H: the pool comes out of the usable FB before the static info is
            // built, so the guest's region table already ends its heap below it.
            let (wlen, wn) = vfio_user_nvgpu::fb::bar1_budget();
            vfio_user_nvgpu::bar1pool::check_size(
                &layout,
                len,
                region_size(&a.board_data, 1),
                (wlen * wn) as u64,
            )
            .map_err(anyhow::Error::msg)?;
            // The host keeps a floor of its shared BAR1 (bar1pool.rs `check_host_bar1`).
            let floor = vfio_user_nvgpu::bar1pool::host_bar1_floor(
                std::env::var(vfio_user_nvgpu::bar1pool::HOST_BAR1_FLOOR_ENV)
                    .ok()
                    .as_deref(),
            )
            .map_err(anyhow::Error::msg)?;
            let need = len + (wlen * wn) as u64;
            let free = if floor == 0 {
                Err("not read".into())
            } else {
                vfio_user_nvgpu::bar1pool::host_bar1_free()
            };
            if let Ok(f) = &free {
                eprintln!(
                    "vfio-user-nvgpu: host BAR1: {} MiB free (nvidia-smi -q -d MEMORY), the pool and its windows need {} MiB, floor {} MiB",
                    f >> 20,
                    need >> 20,
                    floor >> 20
                );
            }
            vfio_user_nvgpu::bar1pool::check_host_bar1(need, free, floor)
                .map_err(anyhow::Error::msg)?;
            layout = layout.with_pool(len).map_err(anyhow::Error::msg)?;
        }
        // Owned GFXP in the VM's slice (gfxpslot.rs): slots for the host GR contexts' preemption
        // buffers, carved from the usable FB before the static info is built. Without them
        // every host GR context stays in graphics WFI (one WARN). Off (`gfxpslot::ENABLED`)
        // until the buffers have a VA design: no carve-out, the usable FB as before.
        if a.host_exec && vfio_user_nvgpu::gfxpslot::ENABLED {
            use vfio_user_nvgpu::gfxpslot;
            let slots = gfxpslot::host_geometry(&h).and_then(|g| {
                let n = gfxpslot::count_for(layout.profile);
                let bytes = gfxpslot::carve_bytes(&g, n);
                layout.with_gfxp(bytes).map(|l| (l, g, n, bytes))
            });
            match slots {
                Ok((l, g, n, bytes)) => {
                    layout = l;
                    let (lo, hi) = layout.gfxp_range().expect("just carved");
                    eprintln!(
                        "vfio-user-nvgpu: gfxp slots: {n} x {:#x} bytes at [{lo:#x}, {hi:#x}) of the reservation: {} MiB of the slice ({:.1} % of the profile), reported reserved, device-private; buffers mapped at VAs from {:#x} (docs/research/gr-preemption-2026-09-29.md)",
                        g.slot,
                        bytes >> 20,
                        bytes as f64 * 100.0 / layout.profile as f64,
                        gfxpslot::SLOT_VA_BASE
                    );
                }
                Err(e) => eprintln!(
                    "vfio-user-nvgpu: WARNING gfxp slots: none ({e}); every host GR context stays in graphics WFI"
                ),
            }
        }
        eprintln!(
            "vfio-user-nvgpu: fb layout: address space {:#x} (host card), reservation [{:#x}, {:#x}), usable [{:#x}, {:#x}), BAR2/BAR1 roots {:#x}/{:#x}, fb_length {:#x}",
            layout.addr_space,
            r.base,
            r.end(),
            layout.usable().unwrap().0,
            layout.usable().unwrap().1,
            layout.bar_roots().unwrap().0,
            layout.bar_roots().unwrap().1,
            layout.profile
        );
    }
    // Real compression per VM slice (compression.rs): the guest is told about it only when
    // the host holds the reservation as compressible backing (the grant above).
    if compression == compslice::Mode::Slice && !compression_granted {
        eprintln!(
            "vfio-user-nvgpu: WARNING {}=slice, but the host did not grant a compressible reservation: the guest is told no compression (D16)",
            compslice::ENV
        );
    }
    if compression_granted {
        layout = layout.with_compression(true).map_err(anyhow::Error::msg)?;
        match vfio_user_nvgpu::compression::GuestView::of(&layout, chip.data()) {
            Some(view) => eprintln!("vfio-user-nvgpu: {}", view.describe()),
            None => eprintln!(
                "vfio-user-nvgpu: WARNING {}=slice, but chip {} has no compression facts of its own ([compression] of chip.toml): the guest is told no compression (D16)",
                vfio_user_nvgpu::compression::ENV,
                chip.data().name
            ),
        }
    }
    // The display engine of every device model from here on (display/mod.rs).
    vfio_user_nvgpu::display::configure(vfio_user_nvgpu::display::Setup {
        enabled: a.display,
        chip: chip.data().name.to_ascii_lowercase(),
        board: a.board_data.display.clone(),
    });
    if a.display {
        vfio_user_nvgpu::display::set_intr_shared(intr_shared.clone());
        // Connector commands for the display (hotplug with a new EDID, unplug, plug):
        // `echo edid-alt > <state dir>/display-ctl` (docs/caraxes/display-cd.md).
        if let Some(dir) = socket.parent() {
            vfio_user_nvgpu::display::set_control_file(dir.join("display-ctl"));
        }
        eprintln!("vfio-user-nvgpu: display engine present (fuse 0; board connectors from the board data)");
    }
    let mut host_control_worker = None;
    if a.boot_model {
        let fb_bytes = a.fb_mib << 20;
        let static_identity = StaticIdentity::from_spec(
            chip.data(),
            &a.board_data,
            cfg.ids(),
            &StaticIdentity::spec(&a.board, fb_bytes),
            a.name_format,
            fb_bytes,
        );
        eprintln!("vfio-user-nvgpu: guest name {:?}", static_identity.gpu_name);
        let mut model = BootModel::new(chip, layout, static_identity);
        if a.host_rm {
            let host = HostRmClient::open().context("open the host RM (decision D10)")?;
            eprintln!("vfio-user-nvgpu: host RM open (unprivileged client, device 0, subdevice 0)");
            // Static per-card getters answered from their first host answer (fwdcache.rs).
            let host = vfio_user_nvgpu::fwdcache::CachingHost::on(Box::new(host));
            eprintln!(
                "vfio-user-nvgpu: forward cache on ({} static getters)",
                vfio_user_nvgpu::fwdcache::STATIC.len(),
            );
            if profile::on() && host.enabled() {
                profile::register_source(Box::new(vfio_user_nvgpu::fwdcache::counters));
            }
            if host_control_offlock {
                let worker = vfio_user_nvgpu::host_control::Worker::start(Box::new(host))
                    .context("spawn ordered host-control worker")?;
                let service = worker.service();
                *signal_host_control.lock().unwrap_or_else(PoisonError::into_inner) = Some(service.clone());
                model = model.with_host(service.proxy()).with_host_control(service);
                host_control_worker = Some(worker);
            } else {
                model = model.with_host(Box::new(host));
            }
        }
        if a.host_exec {
            let hm = host_mem.as_ref().expect("checked in parse_args").clone();
            model = model
                .with_exec(hm, intr_shared.clone())
                .map_err(anyhow::Error::msg)?;
            let (wlen, wn) = vfio_user_nvgpu::fb::bar1_budget();
            eprintln!(
                "vfio-user-nvgpu: host execution on: FB backed by the reserved VRAM through BAR1 windows (budget {wn} x {} MiB = {} MiB of the host's shared BAR1; CARAXES_BAR1_WINDOWS, CARAXES_BAR1_WINDOW_MIB); guest channels mirrored on the host at their first doorbell; GR0/CE2-CE4 non-stall events forwarded as guest vectors (docs/caraxes/execution.md)",
                wlen >> 20,
                (wlen >> 20) * wn
            );
            // Highest-P-state hints only (hostboost.rs), not a fixed MHz request.
            // The default lease is renewed throughout backend lifetime.
            use vfio_user_nvgpu::hostboost;
            let flags = vfio_user_nvgpu::hostabi::current().perf_boost_flags;
            match hostboost::start(flags, boost_policy) {
                Ok(()) => eprintln!(
                    "vfio-user-nvgpu: host boost on: policy={} ({}) finite PERF_BOOST hints ({} s) on its own unprivileged RM client, renewed at most every {} ms with refusal backoff; P-state hints only, no MHz guarantee; no CUDA/shared hint or guest duration; finite lease after backend termination (docs/research/host-boost-vm-policy.md)",
                    boost_policy.name(),
                    match boost_policy {
                        hostboost::Policy::Vm => "continuous backend lifetime; guest command/flags cannot lower the request",
                        hostboost::Policy::Activity => "activity-driven; guest command/permitted MULTIMEDIA preserved",
                    },
                    hostboost::DURATION_S,
                    hostboost::RENEW.as_millis(),
                ),
                Err(e) => {
                    eprintln!("vfio-user-nvgpu: WARNING host boost policy={}: {e}; running without it", boost_policy.name());
                    hostboost::set_off(boost_policy, &format!("failed to start: {e}"));
                }
            }
        }
        if let Some((pool0, pool_end)) = layout.pool_range() {
            let len = pool_end - pool0;
            let host_pool0 = layout.guest_range_to_host(pool0, len)
                .ok_or_else(|| anyhow::anyhow!("BAR1 pool lies outside the guest reservation"))?;
            if host_pool0 != pool0 {
                eprintln!("vfio-user-nvgpu: BAR1 pool guest [{pool0:#x}, {pool_end:#x}) maps host [{host_pool0:#x}, {:#x})", host_pool0 + len);
            }
            let hm = host_mem.as_ref().expect("checked above");
            let ex = hm
                .lock()
                .unwrap()
                .export_bar1_passthrough(host_pool0, len)
                .context("BAR1 pool")?;
            let (_, root) = layout.bar_roots().expect("a reserved layout");
            let usable = layout.usable().expect("a reserved layout");
            model.fb.set_bar1_pool(root, pool0, len, usable);
            // The device's own mapping of the pool (the dma-buf's BAR1 range, no extra host
            // BAR1): pool copies and replica merges stop mapping and evicting the FB
            // store's two LRU windows (docs/caraxes/bar1-pool.md §12). Without the mapping
            // the pool works through the windows.
            match ex.map_self(layout.host_reservation.expect("a reserved layout").0) {
                Ok(w) => {
                    if model.fb.pin_store_window(Box::new(w)) {
                        eprintln!("vfio-user-nvgpu: bar1 pool: the device maps the pool through its own dma-buf mapping, outside the FB store's windows (docs/caraxes/bar1-pool.md §12)");
                    }
                }
                Err(e) => eprintln!(
                    "vfio-user-nvgpu: bar1 pool: no mapping of the pool's dma-buf ({e:#}); the pool is reached through the FB store's windows"
                ),
            }
            eprintln!(
                "vfio-user-nvgpu: bar1 pool: guest BAR1 [0, {len:#x}) offered as one sparse mmap area of region 1, backed by a dma-buf of the pool FB [{pool0:#x}, {pool_end:#x}) below the carve-out; the guest's usable FB ends at {:#x} (fbRegionInfo region 0), the pool is reported reserved; every page the guest maps into the area is moved to its pool page; {len:#x} of host BAR1 held while the client maps it ({}={}, docs/caraxes/bar1-pool.md)",
                usable.1,
                vfio_user_nvgpu::bar1pool::POOL_MIB_ENV,
                len >> 20
            );
            bar1_export = Some(ex);
        }
        if let Some(cfg) = &bar2_config {
            let size = region_size(&a.board_data, 3);
            let made = vfio_user_nvgpu::bar2area::Bar2Area::create(size, cfg).and_then(|area| {
                let r = Bar2Region {
                    fd: area.raw_fd(),
                    areas: area.sparse_areas(),
                };
                model.fb.set_bar2_area(area).map(|_| r)
            });
            match made {
                Ok(r) => {
                    eprintln!(
                        "vfio-user-nvgpu: bar2 memfd: guest BAR2 [0, {size:#x}) offered as {} sparse mmap area(s) of region 3 from a sealed memfd; trapped page(s) {:?}; synced at {}, NV_UFLUSH_FB_FLUSH writes (as a flush read), MMU invalidates, BAR2 binds and GSP doorbells; a channel doorbell syncs the live pages its channel could observe (its VAS leaves less set B, page tables, USERD, instance block) (CARAXES_BAR2_MEMFD=1, docs/caraxes/bar2-memfd.md §3)",
                        r.areas.len(),
                        cfg.traps.iter().map(|t| format!("{t:#x}")).collect::<Vec<_>>(),
                        if cfg.flush_page_memory {
                            "every hand-over (MMU invalidate, GSP or channel doorbell) and the flush clock 1 ms after the last sync: the flush page is memory, no BAR2 page traps (CARAXES_BAR2_FLUSH_PAGE=memory, docs/caraxes/intr-page.md §12.1)"
                        } else {
                            "the first hand-over after a flush read (the next MMU invalidate, GSP or channel doorbell, else the flush clock after 1 ms, docs/caraxes/display-pacing.md)"
                        },
                    );
                    bar2_region = Some(r);
                }
                Err(e) => eprintln!(
                    "vfio-user-nvgpu: ERROR: bar2 memfd: {e}; starting without the area, BAR2 stays trapped"
                ),
            }
        }
        if let Some(n) = doorbell_tokens {
            model = model.with_doorbell_tokens(n);
        }
        if rpc_ioeventfd {
            let Some(offset) = model.cmd_head_register() else {
                bail!("{RPC_IOEVENTFD_ENV}=1: this chip's boot model names no command-queue head register");
            };
            let (sub_region, reader) =
                rpc_head_sub_region(offset).context("command-queue head eventfd")?;
            model = model
                .with_rpc_eventfd(reader.try_clone().context("command-queue head eventfd")?)
                .context("command-queue head eventfd")?;
            rpc_head = Some((offset, sub_region, reader));
        }
        bar0 = bar0.with_boot_model(model);
    }
    let intr_page = match intr_page_config {
        Some(cfg) => {
            let d = chip.data();
            let page = IntrPage::create(
                region_size(&a.board_data, VFIO_PCI_BAR0_REGION_INDEX),
                d.registers.intr_tree,
                identity,
                intr_shared.clone(),
                cfg,
            )
            .map_err(|e| anyhow::anyhow!("intr page: {e}"))?;
            let vectors: Vec<u32> = d
                .interrupts
                .entries
                .iter()
                .flat_map(|e| [e.vector_stall, e.vector_nonstall])
                .collect();
            page.check(
                &|off| bar0.model_owns(off),
                d.registers.intr_tree_size,
                &vectors,
            )
            .map_err(|e| anyhow::anyhow!("intr page: {e}"))?;
            page.enable_vectors(&vectors);
            page.set_hint_vectors(&vfio_user_nvgpu::mirror::hint_vectors(d));
            if profile::on() {
                let p = page.clone();
                profile::register_source(Box::new(move || {
                    use std::sync::atomic::Ordering::Relaxed;
                    let s = &p.stats;
                    vec![
                        ("intr page: scans with work", s.scans.load(Relaxed)),
                        ("intr page: acks applied", s.acks.load(Relaxed)),
                        ("intr page: LEAF_EN_SET applied", s.en_set.load(Relaxed)),
                        ("intr page: LEAF_EN_CLEAR applied", s.en_clear.load(Relaxed)),
                        ("intr page: TOP_EN_SET applied", s.top_en_set.load(Relaxed)),
                        ("intr page: TOP_EN_CLEAR applied", s.top_en_clear.load(Relaxed)),
                        ("intr page: triggers applied", s.triggers.load(Relaxed)),
                        ("intr page: MSIs for raises", s.msi_raise.load(Relaxed)),
                        ("intr page: MSI re-arms (set store while pending)", s.msi_edge.load(Relaxed)),
                        ("intr page: SET re-arms suppressed by top mask", s.msi_edge_top_masked.load(Relaxed)),
                        ("intr page: MSIs by the watchdog", s.msi_watchdog.load(Relaxed)),
                    ]
                }));
            }
            bar0 = bar0.with_intr_page(page.clone());
            eprintln!(
                "vfio-user-nvgpu: intr page: BAR0 [0x0, 0x1000) and the interrupt tree [{:#x}, {:#x}) offered as sparse mmap areas of region 0 from a memfd; the reactor applies the guest's stores (sleep bound {} ms, retrigger watchdog {}) (host execution, docs/caraxes/intr-page.md)",
                d.registers.intr_tree,
                d.registers.intr_tree + intr_page::PAGE,
                cfg.sleep_ms,
                cfg.retrigger
                    .map_or("off".into(), |p| format!("{} ms", p.as_millis())),
            );
            Some(page)
        }
        None => None,
    };
    let fe = match (&intr_page, display_page_mode) {
        (Some(page), Some(mode)) => {
            let fe = FePage::create_mode(page, mode)
                .map_err(|e| anyhow::anyhow!("{}={}: {e}", fe_page::DISPLAY_PAGE_ENV, mode.name()))?;
            FePage::check(&vfio_user_nvgpu::display::engine::Engine::handles, &|off| {
                bar0.boot().is_some_and(|b| b.owns_register(off))
            })
            .map_err(|e| anyhow::anyhow!("{}={}: {e}", fe_page::DISPLAY_PAGE_ENV, mode.name()))?;
            // Every display engine from now on keeps its status in the page; the one the
            // boot model already made takes it now.
            vfio_user_nvgpu::display::set_fe_page(fe.clone());
            if profile::on() {
                let f = fe.clone();
                profile::register_source(Box::new(move || {
                    use std::sync::atomic::Ordering::Relaxed;
                    let s = &f.stats;
                    vec![
                        ("display page: scans with work", s.scans.load(Relaxed)),
                        ("display page: event acks applied", s.evt_acks.load(Relaxed)),
                        ("display page: enable stores applied", s.en_stores.load(Relaxed)),
                        ("display page: CORE markers", s.markers.load(Relaxed)),
                        ("display page: raises", s.raises.load(Relaxed)),
                        ("display page: watchdog raises", s.watchdog.load(Relaxed)),
                    ]
                }));
            }
            match bar0.boot_mut() {
                Some(b) if b.ctrl.display.enabled => b.ctrl.display.eng().attach_page(fe.clone()),
                _ => bail!(
                    "{}={}: the display engine is not present",
                    fe_page::DISPLAY_PAGE_ENV, mode.name()
                ),
            }
            bar0 = bar0.with_fe_page(fe.clone());
            if fe.exact_readonly() {
                eprintln!("vfio-user-nvgpu: display page mode=exact-ro: BAR0 [{:#x}, {:#x}) ordinary FE values; synchronous trapped W1C/enables; requires read-only VMM mapping; no markers/watchdog/kick page", fe_page::FE_PAGE, fe_page::FE_PAGE + intr_page::PAGE);
            } else {
            eprintln!(
                "vfio-user-nvgpu: display page: BAR0 [{:#x}, {:#x}) (the display front-end interrupt status) offered as a third sparse mmap area; the display clock advances the engine at every tick, the reactor applies the guest's acks and enables, window AWAKENs are acknowledged by the CORE marker ({}=1, docs/caraxes/intr-page.md §10)",
                fe_page::FE_PAGE,
                fe_page::FE_PAGE + intr_page::PAGE,
                fe_page::DISPLAY_PAGE_ENV
            );
            }
            Some(fe)
        }
        _ => None,
    };
    // The MMU invalidate page (inv_page.rs, docs/caraxes/intr-page.md §11.3): a fourth area.
    let inv = if vfio_user_nvgpu::inv_page::requested(intr_page.is_some(), bar0.boot().is_some())
        .map_err(anyhow::Error::msg)?
    {
        let page = intr_page.as_ref().expect("checked by requested");
        let d = chip.data();
        let layout = vfio_user_nvgpu::inv_page::Layout::of(&d.registers, d.gmmu.fault_buffer_entries)
            .map_err(|e| anyhow::anyhow!("{}=1: {e}", vfio_user_nvgpu::inv_page::INVALIDATE_PAGE_ENV))?;
        let inv = vfio_user_nvgpu::inv_page::InvPage::create(page, layout)
            .map_err(|e| anyhow::anyhow!("{}=1: {e}", vfio_user_nvgpu::inv_page::INVALIDATE_PAGE_ENV))?;
        inv.check(&|off| bar0.boot().is_some_and(|b| b.owns_register(off)))
            .map_err(|e| anyhow::anyhow!("{}=1: {e}", vfio_user_nvgpu::inv_page::INVALIDATE_PAGE_ENV))?;
        bar0 = bar0.with_inv_page(inv.clone());
        let (base, len) = inv.area();
        eprintln!(
            "vfio-user-nvgpu: invalidate page: BAR0 [{base:#x}, {:#x}) (the VF MMU invalidate and fault-buffer pointers) offered as a sparse mmap area; the reactor runs each TRIGGER and clears it when done ({}=1, docs/caraxes/intr-page.md §11.3)",
            base + len,
            vfio_user_nvgpu::inv_page::INVALIDATE_PAGE_ENV
        );
        Some(inv)
    } else {
        None
    };
    // The display kick page (display/chan_page.rs, docs/caraxes/intr-page.md §12.3), on with
    // the display page: the window channels' USERD pages.
    let kicks = if fe.as_ref().is_some_and(|p| p.mode().automatic_kicks()) {
        let page = intr_page.as_ref().expect("the display page needs the interrupt page");
        let k = vfio_user_nvgpu::display::chan_page::ChanPage::create(page)
            .map_err(|e| anyhow::anyhow!("display kick page: {e}"))?;
        bar0 = bar0.with_chan_page(k.clone())
            .map_err(|e| anyhow::anyhow!("display kick page: {e}"))?;
        let (base, len) = k.area();
        eprintln!(
            "vfio-user-nvgpu: display kick page: BAR0 [{base:#x}, {:#x}) (the window channels' USERD pages, PUT and GET) offered as a sparse mmap area; the reactor runs the kick of each PUT change and stores GET (with the display page, docs/caraxes/intr-page.md §12.3)",
            base + len
        );
        Some(k)
    } else {
        None
    };
    let ro_window = if let Some(base) = ro_window_base {
        let intr = intr_page.as_ref().context("read-only window page needs the interrupt memfd")?;
        let page = Arc::new(vfio_user_nvgpu::display::ro_chan_page::RoChanPage::create(intr, base)
            .map_err(anyhow::Error::msg)?);
        bar0 = bar0.with_ro_chan_page(page.clone()).map_err(anyhow::Error::msg)?;
        eprintln!("vfio-user-nvgpu: read-only window page: BAR0 [{base:#x}, {:#x}) exact emulated values; incremental publication, every store remains trapped; requires matching read-only VMM area; reconnect requires backend restart ({DISPLAY_WINDOW_RO_ENV})", base + intr_page::PAGE);
        Some(page)
    } else {
        None
    };
    // The usermode page (um_page.rs, docs/caraxes/intr-page.md §12.2): a fifth area, the
    // clock and the doorbell as memory the reactor owns.
    let um = if vfio_user_nvgpu::um_page::requested(intr_page.is_some(), bar0.boot().is_some())
        .map_err(anyhow::Error::msg)?
    {
        let env = vfio_user_nvgpu::um_page::USERMODE_PAGE_ENV;
        let page = intr_page.as_ref().expect("checked by requested");
        let layout = vfio_user_nvgpu::um_page::Layout::of(&chip.data().registers)
            .map_err(|e| anyhow::anyhow!("{env}=1: {e}"))?;
        let clock = bar0.boot().expect("checked by requested").ctrl.channels.gpu_clock();
        let um = vfio_user_nvgpu::um_page::UmPage::create_with_clock(page, layout, clock)
            .map_err(|e| anyhow::anyhow!("{env}=1: {e}"))?;
        um.check(&|off| bar0.boot().is_some_and(|b| b.owns_register(off)))
            .map_err(|e| anyhow::anyhow!("{env}=1: {e}"))?;
        bar0 = bar0.with_um_page(um.clone());
        let (base, len) = um.area();
        eprintln!(
            "vfio-user-nvgpu: usermode page: BAR0 [{base:#x}, {:#x}) (TIME_0/1 and the doorbell) offered as a sparse mmap area; the reactor publishes the clock every iteration and scans every channel's GPPut after each doorbell written to the page, with a watchdog scan every {} ms; the token pool's ioeventfds are not offered ({env}=1, docs/caraxes/intr-page.md §12.2)",
            base + len,
            vfio_user_nvgpu::um_page::WATCHDOG.as_millis()
        );
        Some(um)
    } else {
        None
    };
    // The reactor's side of the page; from here on raises go through it.
    let page_hook: Option<PageHook> = match &intr_page {
        Some(p) => {
            let (hook, wake) = PageHook::new(p.clone()).context("intr page wake eventfd")?;
            intr_shared.attach_page(wake);
            if intr_direct && intr_shared.attach_direct(p) {
                eprintln!(
                    "vfio-user-nvgpu: intr page: direct completion on ({}=1): a host event's raise writes the page and then the MSI on the event thread",
                    intr_page::DIRECT_ENV
                );
            }
            let hook = match &fe {
                Some(f) => hook.with_display(f.clone()),
                None => hook,
            };
            Some(hook)
        }
        None => None,
    };
    let reactor_wake = page_hook.as_ref().map(|h| h.raise_wake.clone());
    let guest_ram = match (&host_mem, a.register_guest_ram, a.identity_dma_table) {
        (Some(hm), true, _) => GuestRam::Register(hm.clone()),
        (_, _, true) => GuestRam::Identity,
        _ => GuestRam::Plain,
    };
    // With host execution the guest's DMA addresses are its GPAs: its shadow maps translate
    // registered RAM, and the guest loads no helper (every production launcher set
    // CARAXES_GUEST_DMA_GPA=1 with host execution until 2026-09-30).
    let mut device = Device::new(cfg, bar0, trace)
        .with_guest_dma_gpa(a.host_exec)
        .with_guest_ram(guest_ram, a.dma_ranges.clone())
        .with_intr_shared(intr_shared.clone());
    if local_irq {
        let page = intr_page.as_ref().expect("host execution has an interrupt page").clone();
        let b = &chip.data().bar0;
        if b.pcfg_size < NV_XVE_CYA_2 + 4 {
            bail!("CARAXES_LOCAL_IRQ=1: chip has no MSI re-arm register");
        }
        device = device.with_local_irq(page, b.pcfg + NV_XVE_CYA_2).map_err(anyhow::Error::msg)?;
    }
    if bar_shmem {
        device = device.with_bar_shmem().map_err(anyhow::Error::msg)?;
    }
    if let Some(r) = host_mem
        .as_ref()
        .and_then(|h| h.lock().unwrap().reservation().cloned())
    {
        device.log_event(&format!(
            "hostmem: reservation [{:#x}, {:#x}) held (handle {:#x}); FB layout {layout:?}",
            r.base,
            r.end(),
            r.handle
        ));
    }

    if let Ok(meta) = std::fs::symlink_metadata(socket) {
        use std::os::unix::fs::FileTypeExt;
        if meta.file_type().is_socket() {
            std::fs::remove_file(socket)
                .with_context(|| format!("remove stale {}", socket.display()))?;
        }
    }
    let bar1_area = bar1_export.as_ref().map(|e| Bar1Area {
        fd: e.raw_fd(),
        len: e.len,
    });
    let bar0_areas = intr_page.as_ref().map(|p| Bar0Areas {
        fd: p.raw_fd(),
        areas: p
            .areas()
            .into_iter()
            .chain(fe.as_ref().map(|f| f.area()))
            .chain(inv.as_ref().map(|i| i.area()))
            .chain(um.as_ref().map(|u| u.area()))
            .chain(kicks.as_ref().map(|k| k.area()))
            .chain(ro_window.as_ref().map(|p| p.area()))
            .collect(),
    });
    for l in config_block(&ConfigView {
        a,
        env: &env,
        poll: poll.mode.name(),
        compression: compression == compslice::Mode::Slice,
        pool_mib: bar1_pool.map(|b| b >> 20),
        doorbell_tokens,
        guest_tokens: guest_tokens.as_ref().map(|g| g.len()),
        msi_rearm,
        rpc_ioeventfd,
        intr_page: intr_page.is_some(),
        display_page: fe.is_some(),
        kick_page: kicks.is_some(),
        usermode_page: um.is_some(),
        invalidate_page: inv.is_some(),
        bar2_memfd: bar2_region.is_some(),
        zero_fb,
    }) {
        eprintln!("vfio-user-nvgpu: config: {l}");
    }
    // The listening socket stays open across clients (a VMM reboot connects a new client,
    // `client_waiting`); a duplicate of it tells whether one is waiting.
    let listener = std::os::unix::net::UnixListener::bind(socket)
        .with_context(|| format!("vfio-user server: bind {}", socket.display()))?;
    let _socket_file = SocketFile(socket.to_path_buf());
    let waiting = listener.try_clone().context("vfio-user server: duplicate the listener")?;
    let mut server = Server::from_owned_fd(
        std::os::fd::OwnedFd::from(listener),
        true,
        irqs(),
        regions_with_pages(&a.board_data, bar1_area, bar2_region.as_ref(), bar0_areas),
    );
    // The datamatch doorbell eventfds: the pool's first (registered first, so KVM passes
    // them first: Linux's libcuda rings pool tokens), then the guest tokens'. Both only
    // when the reactor reads them; with the usermode page the doorbell is memory, so no
    // ioeventfd could fire and none is offered (the reactor still runs, with the page's
    // scan).
    let token_readers = if token_reactor_on {
        let offset = chip.data().registers.usermode_doorbell;
        let mut readers = Vec::new();
        if let Some(n) = doorbell_tokens {
            let subs = if um.is_some() {
                Vec::new()
            } else {
                doorbell_token_sub_regions(offset, n).context("doorbell token eventfds")?
            };
            for (sub_region, reader) in subs {
                server
                    .add_ioeventfd(VFIO_PCI_BAR0_REGION_INDEX, sub_region)
                    .map_err(|e| anyhow::anyhow!("doorbell token sub-region: {e}"))?;
                readers.push(reader);
            }
            eprintln!(
                "vfio-user-nvgpu: doorbell tokens: BAR0 {offset:#x}, 4 bytes, {n} sub-region(s) with datamatch {:#x}..={:#x} and one eventfd each, offered through VFIO_USER_DEVICE_GET_REGION_IO_FDS; a channel gets a pool token at its first (trapped) doorbell, written to its work-submit-token notifier entry ({}={n}, docs/caraxes/doorbell-token-pool.md)",
                tokens::TOKEN_BASE,
                tokens::TOKEN_BASE + n as u32 - 1,
                tokens::TOKENS_ENV
            );
        }
        let mut guest = Vec::new();
        if let Some(g) = guest_tokens.as_ref() {
            let base = g.base();
            let values = g.values();
            let subs = if um.is_some() {
                Vec::new()
            } else {
                doorbell_value_sub_regions(offset, &values).context("guest token eventfds")?
            };
            for ((sub_region, reader), v) in subs.into_iter().zip(values) {
                server
                    .add_ioeventfd(VFIO_PCI_BAR0_REGION_INDEX, sub_region)
                    .map_err(|e| anyhow::anyhow!("guest token sub-region: {e}"))?;
                guest.push((v, reader));
            }
            eprintln!(
                "vfio-user-nvgpu: doorbell guest tokens: BAR0 {offset:#x}, 4 bytes, {} sub-region(s) with datamatch runlist << 16 | chid, base runlists {:?} interleaved per ChID then {:?} one by one, base ChIDs 0..{} plus {} sparse values, one eventfd each, offered through VFIO_USER_DEVICE_GET_REGION_IO_FDS; a write of one of them is rung as its trapped write (docs/caraxes/doorbell-token-pool.md (g))",
                g.len(),
                base.hot,
                base.rest,
                base.chids,
                g.extra_values().len(),
            );
            if !guest_token_extras.is_empty() {
                let extras: Vec<_> = g.extra_values().iter().map(|v| {
                    (v >> tokens::TOKEN_RUNLIST_SHIFT, v & ((1 << tokens::TOKEN_VECTOR_BITS) - 1))
                }).collect();
                eprintln!(
                    "vfio-user-nvgpu: sparse guest tokens: requested {guest_token_extras:?}, extra datamatch values {extras:?}, after {} unchanged base entries; already covered values are deduplicated, excess values remain trapped ({GUEST_TOKEN_EXTRAS_ENV})",
                    base.len(),
                );
            }
        }
        Some((offset, readers, guest))
    } else {
        None
    };
    // Trapped accesses served without the device mutex (device.rs `Routes`,
    // docs/caraxes/display-own-clock.md §2.4): the interrupt page's two pages and, with the
    // page, the MSI re-arm; with the display, its status registers (display/core_path.rs).
    let display_route = a.display.then(|| vfio_user_nvgpu::device::DisplayRoute {
        slot: device.display_slot(),
        mem: device.mem_view(),
    });
    let routes = Routes {
        terminal_fault: Some(device.terminal_fault_handle()),
        intr_page: intr_page.clone(),
        rearm: intr_page.as_ref().and_then(|_| {
            let b = &chip.data().bar0;
            (b.pcfg_size >= NV_XVE_CYA_2 + 4).then_some(b.pcfg + NV_XVE_CYA_2)
        }),
        display: display_route.clone(),
        fe_page: fe.clone(),
    };
    let rearm = if msi_rearm {
        let b = &chip.data().bar0;
        if b.pcfg_size < NV_XVE_CYA_2 + 4 {
            bail!(
                "{MSI_REARM_IOEVENTFD_ENV}=1: this chip's NV_PCFG mirror ({:#x} bytes at {:#x}) has no NV_XVE_CYA_2",
                b.pcfg_size,
                b.pcfg
            );
        }
        let offset = b.pcfg + NV_XVE_CYA_2;
        let (sub_region, reader) = msi_rearm_sub_region(offset).context("MSI re-arm eventfd")?;
        server
            .add_ioeventfd(VFIO_PCI_BAR0_REGION_INDEX, sub_region)
            .map_err(|e| anyhow::anyhow!("MSI re-arm sub-region: {e}"))?;
        eprintln!(
            "vfio-user-nvgpu: msi re-arm ioeventfd: BAR0 {offset:#x} (NV_XVE_CYA_2), 4 bytes, no datamatch, offered through VFIO_USER_DEVICE_GET_REGION_IO_FDS and read by the token pool's reactor; a client that does not register it keeps trapping the write ({MSI_REARM_IOEVENTFD_ENV}=1, docs/caraxes/intr-page.md §4)"
        );
        Some((offset, reader))
    } else {
        None
    };
    let rpc = match rpc_head {
        Some((offset, sub_region, reader)) => {
            server
                .add_ioeventfd(VFIO_PCI_BAR0_REGION_INDEX, sub_region)
                .map_err(|e| anyhow::anyhow!("command-queue head sub-region: {e}"))?;
            device.set_rpc_ioeventfd(offset);
            eprintln!(
                "vfio-user-nvgpu: rpc ioeventfd: BAR0 {offset:#x} (NV_PGSP_QUEUE_HEAD(0)), 4 bytes, no datamatch, offered through VFIO_USER_DEVICE_GET_REGION_IO_FDS and read by the token pool's reactor and before every trapped access of the GSP page; the head value is taken from the command ring; a client that does not register it keeps trapping the write ({RPC_IOEVENTFD_ENV}=1, docs/caraxes/gsp-transport-615.md §11)"
            );
            Some((offset, reader))
        }
        None => None,
    };
    eprintln!(
        "vfio-user-nvgpu: guest ABI {} ({})",
        describe_guest_abi(a.guest_abi),
        a.guest_abi.map_or("transport read from the guest".to_string(), |g| format!("{:?}", g.transport()))
    );
    eprintln!(
        "vfio-user-nvgpu: {} (BOOT_0 {:#010x}, driver {}) on {}, boot model {}, FB {} MiB, ROM {} bytes, PROM image {} bytes, trace {}",
        chip.data().short_name,
        identity.boot_0,
        describe_guest_abi(a.guest_abi),
        socket.display(),
        if a.boot_model { "on" } else { "off" },
        a.fb_mib,
        device_rom_len(&a.rom),
        a.vbios.as_ref().map_or_else(|| device_rom_len(&a.rom), |p| device_rom_len(p)),
        a.trace
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "off".into())
    );
    let device = Arc::new(Mutex::new(device));
    let host_control_pump = if let Some(worker) = &host_control_worker {
        let service = worker.service();
        let device = device.clone();
        Some(
            std::thread::Builder::new()
                .name("host-ctrl-pump".into())
                .spawn(move || {
                    let mut seen = 0;
                    while service.wait(&mut seen) {
                        let mut device =
                            vfio_user_nvgpu::dlock::lock(&device, "host control completion");
                        if let Err(error) = device.complete_host_controls() {
                            eprintln!("vfio-user-nvgpu: host control completion failed: {error}");
                            break;
                        }
                    }
                })
                .context("spawn host-control completion pump")?,
        )
    } else {
        None
    };

    let mut page_hook: Option<Box<dyn vfio_user_nvgpu::poll::Hook>> = page_hook.map(|h| {
        let h = match &inv {
            Some(i) => {
                let d = device.clone();
                h.with_invalidate(
                    i.clone(),
                    Arc::new(move |t| {
                        vfio_user_nvgpu::dlock::lock(&d, "invalidate page").invalidate_from_page(t)
                    }),
                )
            }
            None => h,
        };
        let h = match &kicks {
            Some(k) => {
                let d = device.clone();
                h.with_kicks(
                    k.clone(),
                    Arc::new(move || {
                        d.lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .poll_display_kicks()
                    }),
                )
            }
            None => h,
        };
        let h = match &um {
            Some(u) => {
                let d = device.clone();
                h.with_usermode(
                    u.clone(),
                    Arc::new(move |hint| {
                        // A doorbell the page's reactor took is a ring of the launch chain
                        // (launch.rs), from the take to the host doorbell.
                        if hint.is_some() {
                            vfio_user_nvgpu::launch::begin(std::time::Instant::now(), false);
                        }
                        let mut dev = vfio_user_nvgpu::dlock::lock(&d, "usermode page reactor");
                        vfio_user_nvgpu::launch::mark(vfio_user_nvgpu::launch::Step::Lock);
                        let rung = dev.doorbell_from_page(hint);
                        drop(dev);
                        vfio_user_nvgpu::launch::end();
                        rung
                    }),
                )
            }
            None => h,
        };
        Box::new(h) as Box<dyn vfio_user_nvgpu::poll::Hook>
    });
    if a.display {
        // The display clock advances the engine at its ticks, as the card's display runs on
        // its own clock: latches, a flip's FINISHED one frame later and LAST_DATA happen at
        // the tick whether or not the guest reads a display register then (with the display
        // page its status reads no longer trap at all; without it a KMD waiting for FINISHED
        // in memory reads none either).
        // While the device mutex is held (a mirror build, a shadow walk) the tick runs
        // without it, through guest memory and the pinned FB record windows
        // (display/core_path.rs, docs/caraxes/display-own-clock.md §2.5); what those cannot
        // reach waits for the next tick that gets the mutex.
        let d = device.clone();
        let route = display_route.clone();
        if fe.as_ref().is_some_and(|page| page.exact_readonly()) {
            use vfio_user_nvgpu::display::engine::{ticker, DriverOutcome};
            ticker().set_scoped_driver(Arc::new(move || {
                match vfio_user_nvgpu::dlock::try_lock(&d, "display clock tick") {
                    Some(mut g) => {
                        // catch_up already signals changed causes; keep the ordinary
                        // level re-raise, but complete it while Device owns the engine.
                        if g.display_tick() { ticker().raise(); }
                    }
                    None => { if let Some(route) = route.as_ref() { route.tick_exact_and_raise(); } }
                }
                DriverOutcome::Delivered
            }));
        } else {
            vfio_user_nvgpu::display::engine::ticker().set_driver(Arc::new(move || {
                match vfio_user_nvgpu::dlock::try_lock(&d, "display clock tick") {
                    Some(mut g) => g.display_tick(),
                    None => route.as_ref().is_some_and(|r| r.tick()),
                }
            }));
        }
    }
    // The held-ring completer (ringgate.rs, docs/caraxes/execution.md §9.1): while doorbells
    // are held for their root's shadow jobs, it waits for the worker's progress without the
    // device lock and takes the lock briefly to forward the rings that are ready. It also
    // rings the doorbell writes the vfio-user loop posted without the lock (§9.9).
    let completer = {
        let d = device.clone();
        let gate = vfio_user_nvgpu::ringgate::global();
        std::thread::Builder::new()
            .name("ring-completer".into())
            .spawn(move || {
                let mut seen = u64::MAX;
                while let Some(g) = gate.wait_ready(seen, std::time::Duration::from_millis(50)) {
                    seen = g;
                    if gate.has_posted() {
                        vfio_user_nvgpu::dlock::lock(&d, "posted doorbells").ring_posted(&gate, false);
                    }
                    if gate.has_userd() {
                        vfio_user_nvgpu::dlock::lock(&d, "shadow USERD rings").ring_polled_userds(&gate);
                    }
                    if gate.waiting() == 0 {
                        continue;
                    }
                    vfio_user_nvgpu::dlock::lock(&d, "held ring completer").complete_held_rings();
                }
            })
            .context("spawn the ring completer")?
    };
    // The BAR2 memfd's flush clock: a flush read's owed sync that no hand-over settled
    // within the bound (bar2area.rs `DeferredFlush`, docs/caraxes/display-pacing.md).
    let flush_clock = device
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .bar2_flush_clock();
    if let Some(c) = flush_clock {
        let d = device.clone();
        c.spawn_clock(Box::new(move || {
            vfio_user_nvgpu::dlock::lock(&d, "bar2 flush clock").settle_bar2_flush()
        }))
        .context("bar2 flush clock")?;
    }
    let mut readers = Vec::new();
    if let Some((offset, token_eventfds, guest_eventfds)) = token_readers {
        readers.push((
            "tokens",
            spawn_token_reader_hooked(
                device.clone(),
                offset,
                token_eventfds,
                guest_eventfds,
                token_reactor,
                page_hook.take(),
                rearm,
                rpc,
                routes.clone(),
            )
            .context("doorbell token reader")?,
        ));
    }
    if page_hook.is_some() {
        bail!("intr page: no doorbell reactor to run the page in");
    }
    if poll.mode != vfio_user_nvgpu::poll::Mode::Off {
        eprintln!(
            "vfio-user-nvgpu: doorbell poll: {} reader ({}={}, pin {:?})",
            poll.mode.name(),
            vfio_user_nvgpu::poll::POLL_ENV,
            poll.mode.name(),
            poll.pin
        );
    }
    let soak_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let soak = spawn_soak_thread(device.clone(), host_mem.clone(), soak_stop.clone());
    // Private USERD needs progress even before display initialization or in a headless
    // guest. The host sees guest submissions only after the shadow mapping barrier.
    let userd_poll = if std::env::var("CARAXES_SHADOW_USERD").as_deref() == Ok("1") {
        let d = device.clone();
        let stop = soak_stop.clone();
        // Without the device lock (docs/caraxes/execution.md §9.11): USERD GP_GET and GP_PUT
        // are plain memory on the card. The poller harvests the GPU-owned words under each
        // channel's own USERD mutex and posts the channels with entries not yet forwarded;
        // the ring completer rings those under the lock. C75: the locked poll held the lock
        // 897k times (19.9 s).
        let view = d.lock().unwrap_or_else(PoisonError::into_inner).mem_view();
        let gate = vfio_user_nvgpu::ringgate::global();
        Some(std::thread::Builder::new().name("shadow-userd".into()).spawn(move || {
            let mut cursor = 0usize;
            while !stop.load(std::sync::atomic::Ordering::Acquire) {
                std::thread::sleep(std::time::Duration::from_millis(1));
                if stop.load(std::sync::atomic::Ordering::Acquire) { break }
                let watch = vfio_user_nvgpu::mirror::userd_watch();
                if watch.is_empty() {
                    continue;
                }
                let mem = view.read().unwrap_or_else(PoisonError::into_inner).clone();
                // At most 32 channels a tick, round-robin, as the locked poll did.
                let n = watch.len().min(32);
                for k in 0..n {
                    let (chid, slot) = &watch[(cursor + k) % watch.len()];
                    if slot.poll(&mem) {
                        gate.post_userd(*chid);
                    }
                }
                cursor = (cursor + n) % watch.len();
            }
        }).context("spawn shadow USERD poller")?)
    } else {
        None
    };
    // A long software CE transfer runs in slices (channel.rs `CE_SLICE`) so that the
    // vfio-user loop answers between them; the next slice runs on this clock, with or
    // without a display tick or another doorbell. Registered guest RAM is applied here too.
    let ce_slices = {
        let d = device.clone();
        let stop = soak_stop.clone();
        std::thread::Builder::new()
            .name("ce-slices".into())
            .spawn(move || {
                // Whether the last pass continued a run: the next one comes after 50 us.
                let mut busy = false;
                while !stop.load(std::sync::atomic::Ordering::Acquire) {
                    // A run stopped at its budget continues after a short pause, in which the
                    // accesses waiting for the device lock take it (Windows C81: the KMD's
                    // paging channel is the critical path of a context creation; a 1 ms pause
                    // after every 2 ms run idled a third of it). The flag is read once per
                    // pass: reading it takes it.
                    std::thread::sleep(if busy {
                        std::time::Duration::from_micros(50)
                    } else {
                        std::time::Duration::from_millis(1)
                    });
                    busy = vfio_user_nvgpu::channel::ce_slices_pending();
                    if busy {
                        vfio_user_nvgpu::dlock::lock(&d, "software run continuation")
                            .continue_ce_slices();
                    }
                    // A software channel blocked on a semaphore acquire continues once the
                    // release is there, as the host re-evaluates an acquire (channel.rs
                    // `AcquireWait`), not at the next doorbell of any channel.
                    if vfio_user_nvgpu::channel::acquire_waits_pending() {
                        d.lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .recheck_acquires();
                    }
                    // Guest RAM registered off the loop joins the table without waiting
                    // for the next access (registrar.rs).
                    if vfio_user_nvgpu::registrar::registrations_done() {
                        d.lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .apply_registrations();
                    }
                }
            })
            .context("spawn the software CE slice clock")?
    };
    // A live state dump on request: `echo state > <state dir>/device-ctl` prints every
    // channel, host object and the interrupt state into this log (device.rs `state_dump`).
    // `gr <chid>` prints retained opt-in graphics capture without causing an RC.
    let device_ctl = {
        let d = device.clone();
        let stop = soak_stop.clone();
        let page = intr_page.clone();
        let path = socket.parent().map(|p| p.join("device-ctl"));
        std::thread::Builder::new()
            .name("device-ctl".into())
            .spawn(move || {
                let Some(path) = path else { return };
                let _ = std::fs::remove_file(&path);
                let mut n = 0u32;
                while !stop.load(std::sync::atomic::Ordering::Acquire) {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    let Ok(cmd) = std::fs::read_to_string(&path) else { continue };
                    let _ = std::fs::remove_file(&path);
                    let gr = cmd.trim().strip_prefix("gr ").and_then(|s| s.trim().parse::<u32>().ok());
                    if cmd.trim() != "state" && gr.is_none() {
                        eprintln!("vfio-user-nvgpu: device-ctl: unknown command {:?} (known: state, gr <chid>)", cmd.trim());
                        continue;
                    }
                    n += 1;
                    let t0 = std::time::Instant::now();
                    let mut lines = {
                        let mut device = d.lock().unwrap_or_else(PoisonError::into_inner);
                        match gr {
                            Some(chid) => device.gr_capture_dump(chid),
                            None => device.state_dump(),
                        }
                    };
                    if gr.is_none() {
                        if let Some(p) = &page {
                            lines.push(p.state_line());
                        }
                    }
                    let label = if gr.is_some() { "gr snapshot" } else { "state" };
                    for l in &lines {
                        eprintln!("vfio-user-nvgpu: {label} {n}: {l}");
                    }
                    eprintln!(
                        "vfio-user-nvgpu: {label} {n}: {} line(s) in {} us",
                        lines.len(),
                        t0.elapsed().as_micros()
                    );
                }
            })
            .context("spawn the device-ctl reader")?
    };
    let mut backend = SharedDevice::new(device.clone(), routes);
    let mut clients = 1u32;
    if vfio_idle_poll != IdlePollMode::Off {
        eprintln!(
            "vfio-user-nvgpu: vfio-user server idle poll {vfio_idle_poll:?} ({VFIO_IDLE_POLL_ENV}); CPU-for-latency diagnostic"
        );
    }
    let result = loop {
        let r = if vfio_idle_poll == IdlePollMode::Off {
            server.run(&mut backend)
        } else {
            server.run_with_idle_poll(&mut backend, vfio_idle_poll)
        }
        .map_err(|e| anyhow::anyhow!("vfio-user server: {e}"));
        if local_irq && intr_page.as_ref().is_some_and(|page| page.remote_owned()) {
            // A VMM-owned tree and queue are scoped to one socket session. A
            // new process creates fresh descriptors, nonce and producer state.
            break r;
        }
        if r.is_err() || !client_waiting(&waiting, RECONNECT_WINDOW) {
            break r;
        }
        let mut d = device.lock().unwrap_or_else(PoisonError::into_inner);
        if let Err(e) = d.client_gone() {
            eprintln!(
                "vfio-user-nvgpu: a new client connected, but the previous client's state was not released ({e}); ending the session"
            );
            break r;
        }
        clients += 1;
        eprintln!(
            "vfio-user-nvgpu: client {clients}: the previous client closed the connection and a new one connected (a VMM reboot): device reset, the previous client's memory regions and IRQ eventfds released; {}",
            soak_line("new client", d.soak_counts(), host_mem.as_ref())
        );
    };
    soak_stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = ce_slices.join();
    let _ = device_ctl.join();
    if let Some(t) = userd_poll {
        let _ = t.join();
    }
    if let Some(t) = soak {
        let _ = t.join();
    }
    vfio_user_nvgpu::ringgate::global().stop();
    let _ = completer.join();
    for l in vfio_user_nvgpu::dlock::summary() {
        eprintln!("vfio-user-nvgpu: {l}");
    }
    if let Some(worker) = &host_control_worker {
        if let Err(error) = device
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain_host_controls()
        {
            eprintln!("vfio-user-nvgpu: host control final drain failed: {error}");
        }
        worker.service().stop_pump();
    }
    if let Some(pump) = host_control_pump {
        pump.join()
            .map_err(|_| anyhow::anyhow!("host-control completion pump panicked"))?;
    }
    let mut device = device.lock().unwrap_or_else(PoisonError::into_inner);
    // The soak line at exit, before the teardown (what the guest's session left held), and
    // after it (what survived the teardown: every host object count should be 0).
    eprintln!(
        "vfio-user-nvgpu: {}",
        soak_line("exit", device.soak_counts(), host_mem.as_ref())
    );
    // The client is gone: free the descriptors and the reservation before exiting.
    device.shutdown();
    let mut closed = None;
    if let Some(hm) = &host_mem {
        let (log, c) = match hm.lock() {
            Ok(mut h) => (h.shutdown(), h.fail_closed().map(str::to_owned)),
            Err(p) => {
                let mut h = p.into_inner();
                (h.shutdown(), h.fail_closed().map(str::to_owned))
            }
        };
        log.iter()
            .for_each(|l| eprintln!("vfio-user-nvgpu: hostmem: {l}"));
        closed = c;
        let h = hm.lock().unwrap_or_else(PoisonError::into_inner);
        let held = h.soak_held();
        eprintln!(
            "vfio-user-nvgpu: {}; {}",
            vfio_user_nvgpu::soak::line("after teardown", vfio_user_nvgpu::soak::rss_kib(), None, Some(&held), None),
            h.guard().line()
        );
    }
    eprint!("{}", device.summary());
    if let Some(line) = vfio_user_nvgpu::hostboost::summary() {
        eprint!("{line}");
    }
    for (name, st) in &readers {
        eprint!("{}", st.summary(name));
    }
    if let Some(p) = &intr_page {
        eprint!("{}", p.summary());
    }
    if vfio_user_nvgpu::launch::fast() {
        eprint!("{}", vfio_user_nvgpu::launch::fast_summary());
    }
    if vfio_user_nvgpu::launch::on() {
        if let Some(c) = vfio_user_nvgpu::launch::chain() {
            eprint!("{}", c.summary());
        }
    }
    if let Some(f) = &fe {
        eprint!("{}", f.summary());
    }
    if let Some(i) = &inv {
        eprint!("{}", i.summary());
    }
    if let Some(u) = &um {
        eprint!("{}", u.summary());
    }
    if let Some(k) = &kicks {
        eprint!("{}", k.summary());
    }
    if let (Some(w), Some(_)) = (&reactor_wake, &fe) {
        eprintln!(
            "reactor wake-up after a display raise (blocked reactor): {}",
            w.lock().unwrap_or_else(PoisonError::into_inner).describe()
        );
    }
    drop(device);
    let worker_result = host_control_worker.map_or(Ok(()), |worker| worker.stop());
    profile::dump_at_exit();
    worker_result.map_err(anyhow::Error::msg)?;
    if let Some(why) = closed {
        // Fail closed (P20 item 4a): never report a clean exit when host state the GPU
        // could still reach was kept; the fd close at exit hands it to the host RM.
        eprintln!("vfio-user-nvgpu: exit {FAIL_CLOSED_EXIT}: teardown failed closed ({why})");
        std::process::exit(FAIL_CLOSED_EXIT);
    }
    result
}

/// Exit status of a teardown that failed closed (hostmem.rs `run_teardown`).
const FAIL_CLOSED_EXIT: i32 = 3;

/// How long the device waits for a new client after one closed the connection. Cloud
/// Hypervisor's reboot connects its new client within about 0.7 s (C100: the reset event
/// at 439.62 s, the new client's connection at 440.46 s); a VMM that exited sends none,
/// and the device's exit waits this long.
const RECONNECT_WINDOW: std::time::Duration = std::time::Duration::from_secs(2);

/// Whether a client connects to `listener` within `window` (one already waiting in the
/// backlog counts): poll(2) for POLLIN on the listening socket.
fn client_waiting(listener: &impl std::os::fd::AsRawFd, window: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + window;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let mut p = libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd for the duration of the call.
        let n = unsafe { libc::poll(&mut p, 1, left.as_millis().min(i32::MAX as u128) as i32) };
        if n > 0 {
            return p.revents & libc::POLLIN != 0;
        }
        if n == 0 || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return false;
        }
    }
}

/// The socket file goes when `serve` returns (as `vfio_user::Server` removes a socket it
/// bound itself; this one is bound here so that it outlives one client).
struct SocketFile(PathBuf);

impl Drop for SocketFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn device_rom_len(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

/// `vfio-user-nvgpu --dma-table-alias <table file> <gpa> <size>`: print the table with
/// the guest's swiotlb pool aliased below 2^40 (dmamap.rs `with_alias`), for the launcher.
fn dma_table_alias(args: &[String]) -> Result<()> {
    let [file, gpa, size] = args else {
        bail!("--dma-table-alias <table file> <gpa hex> <size hex>");
    };
    let hex = |s: &str| u64::from_str_radix(s.trim_start_matches("0x"), 16);
    let t = vfio_user_nvgpu::dmamap::DmaTable::parse(&std::fs::read_to_string(file)?)
        .map_err(anyhow::Error::msg)?;
    let t = t
        .with_alias(
            hex(gpa)?,
            hex(size)?,
            vfio_user_nvgpu::dmamap::LOW_ALIAS_IOVA,
        )
        .map_err(anyhow::Error::msg)?;
    println!("{}", t.module_param());
    Ok(())
}

fn main() -> Result<()> {
    let argv: Vec<String> = std::env::args().collect();
    if argv.get(1).map(String::as_str) == Some("--dma-table-alias") {
        return dma_table_alias(&argv[2..]);
    }
    let a = parse_args()?;
    if a.print_name {
        let name = StaticIdentity::guest_name_as(&a.board, a.fb_mib << 20, a.name_format);
        println!("{name}");
        return Ok(());
    }
    // First, before any thread: reduce the privilege to what D12 allows.
    let privilege = if a.register_guest_ram {
        Privilege::init(true)?
    } else if a.selftest.is_none() {
        Privilege::init(false)?
    } else {
        Privilege::none()
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if let Some(socket) = &a.selftest {
        let cfg = config_space(&a)?;
        let expect = selftest::Expect {
            chip: a.chip,
            board: &a.board_data,
            pci: cfg.ids(),
            class_revision: cfg.dword(0x08),
            cap_ptr: cfg.byte(0x34),
        };
        let passed = selftest::run(socket, &expect)?;
        println!("selftest passed: {}", passed.join(", "));
        return Ok(());
    }
    serve(&a, a.socket.as_deref().expect("socket is set"), privilege)
}
