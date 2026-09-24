// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
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
use vfio_user::Server;
use vfio_user_nvgpu::bar0::Bar0;
use vfio_user_nvgpu::board::{Board, BOARDS};
use vfio_user_nvgpu::boot::BootModel;
use vfio_user_nvgpu::chip::{self, Chip};
use vfio_user_nvgpu::config::{self as pci_config, ConfigSpace};
use vfio_bindings::bindings::vfio::VFIO_PCI_BAR0_REGION_INDEX;
use vfio_user_nvgpu::device::{
    doorbell_ioeventfd_requested, doorbell_sub_region, doorbell_token_sub_regions, irqs,
    region_size, regions_with, spawn_doorbell_reader, spawn_token_reader, Bar1Area, Device,
    GuestRam, SharedDevice, DOORBELL_IOEVENTFD_ENV,
};
use vfio_user_nvgpu::fb::FbLayout;
use vfio_user_nvgpu::hostmem::HostMemory;
use vfio_user_nvgpu::hostrm::HostRmClient;
use vfio_user_nvgpu::intr::IntrShared;
use vfio_user_nvgpu::privilege::Privilege;
use vfio_user_nvgpu::rpc::StaticIdentity;
use vfio_user_nvgpu::selftest;
use vfio_user_nvgpu::tokens;
use vfio_user_nvgpu::trace::Trace;
use vfio_user_nvgpu::ver;

const USAGE: &str = "\
vfio-user-nvgpu --socket <path> [--trace <file>] [--board-data <name|file>] [--chip <chip>]
                [--config <dump>] [--rom <rom.bin>] [--vbios <image>] [--vbios-version <v>]
                [--boot-model] [--fb-mib <n>] [--board <name>] [--host-rm]
                [--reserve-vram [--reserve-slot <n>]] [--register-guest-ram | --identity-dma-table] [--dma-ranges <file>]
                [--host-exec] [--profile <size>] [--name-format <legacy|transport>] [--display]
                [--guest-abi <595|610|615>]
vfio-user-nvgpu --print-name [--board-data <name|file>] [--board <name>] [--fb-mib <n> | --profile <size>]
                [--name-format <legacy|transport>]
vfio-user-nvgpu --selftest <socket> [--board-data <name|file>] [--chip <chip>] [--config <dump>]

  --socket   vfio-user socket to create (a stale socket file is removed)
  --trace    access log file (default: none; the summary always goes to stderr)
  --board-data  board data: a built-in board (rtx2070, rtx5060ti) or a boards/*.toml-shaped file (default: rtx2070)
  --chip     the chip to present, tu106 or gb206 (default: the board's, tu106 for rtx2070, gb206 for rtx5060ti)
  --config   lspci -xxxx dump of the card (default: the board's, caraxes/reference/rtx2070/config-space.txt)
  --rom      PCI-visible ROM image for the expansion ROM region (default: the board's, caraxes/reference/rtx2070/rom.bin)
  --vbios    image served through the BAR0 PROM window (default: the --rom image)
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
             interrupts (needs --reserve-vram, --register-guest-ram and --host-rm)
  --display  present the display engine (docs/caraxes/display-ab.md): the display fuse reads 0,
             the display controls are answered from chips/<chip>/display.toml and the board's
             [display] connectors, each connector backed by a DisplayPort sink model; without
             it the device is headless (fuse 1), as in E0-E5
  --selftest connect to a running server and verify the E0 device model
  --guest-abi  the guest driver's GSP ABI this device must serve (default: CARAXES_GUEST_ABI, else
             the build's): 610 needs a default build, 615 a `--features v615` build (message
             queue v2, GMCAPI init; docs/caraxes/gsp-transport-615.md), 595 a 610 build with
             CARAXES_GUEST_ABI=595; anything else is refused (fail closed)
Environment: CARAXES_DOORBELL_IOEVENTFD=1 offers the usermode doorbell register as an
  ioeventfd sub-region (VFIO_USER_DEVICE_GET_REGION_IO_FDS; needs patch 0005 in Cloud
  Hypervisor, a client without it keeps trapping); the eventfd carries no token, so a
  wakeup rings every channel whose GPPut moved since its last doorbell. Refused with
  CARAXES_BAR1_PASSTHROUGH=1.
  CARAXES_DOORBELL_TOKENS=N (1-253, or on = 64) offers the doorbell register as N
  ioeventfd sub-regions with KVM datamatch, one eventfd per device-chosen token; a
  channel's first doorbell traps and hands it a pool token through its work-submit-token
  notifier entry, every later ring ends in the token's eventfd (needs patch 0005 and a
  client announcing max_msg_fds; docs/caraxes/doorbell-token-pool.md). Excludes
  CARAXES_DOORBELL_IOEVENTFD=1; combines with CARAXES_BAR1_PASSTHROUGH=1.
  CARAXES_DOORBELL_POLL=off|spin|hybrid chooses how the thread that reads those eventfds
  waits: off blocks in epoll_wait (default), spin polls with epoll_wait timeout 0 and
  never sleeps, hybrid spins CARAXES_DOORBELL_POLL_IDLE_US (default 200) after the last
  event and then blocks. spin and hybrid pin the reader to CARAXES_DOORBELL_POLL_CPU=<n>,
  or to a core chosen at start (auto, the default: not CPU 0, not a vCPU thread's, the
  highest-numbered idle one), or not at all (none). Needs one of the two above
  (docs/caraxes/doorbell-poll.md)
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
    guest_abi: u32,
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
        guest_abi: 0,
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
            "--guest-abi" => {
                guest_abi_flag = Some(value()?.parse().context("--guest-abi needs 595, 610 or 615")?)
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => bail!("unknown argument {other:?}\n{USAGE}"),
        }
    }
    a.chip = chip::by_name(&chip_name).map_err(|e| anyhow::anyhow!("--chip: {e}"))?;
    a.guest_abi = ver::check_guest_abi(
        guest_abi_flag,
        std::env::var("CARAXES_GUEST_ABI").ok().as_deref(),
    )
    .map_err(|e| anyhow::anyhow!("--guest-abi: {e}"))?;
    a.board_data = board_data;
    if a.socket.is_none() && a.selftest.is_none() && !a.print_name {
        bail!("{USAGE}");
    }
    // The host paths talk to the house card, a TU106 (docs/caraxes/blackwell-gb206.md
    // section 4): any other chip identity runs in pure emulation.
    let chip_name = &a.chip.data().name;
    if chip_name != "tu106" && (a.host_rm || a.reserve_vram || a.register_guest_ram || a.host_exec)
    {
        bail!("--chip {chip_name} runs in pure emulation: --host-rm, --reserve-vram, --register-guest-ram and --host-exec need a host card of the same chip");
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
    if a.host_exec && !(a.reserve_vram && a.register_guest_ram && a.host_rm) {
        bail!("--host-exec needs --reserve-vram, --register-guest-ram and --host-rm (the guest's addresses must be the host's before anything runs on the physical GPU)");
    }
    Ok(a)
}

fn config_space(a: &Args) -> Result<ConfigSpace> {
    let dump = std::fs::read_to_string(&a.config)
        .with_context(|| format!("read {}", a.config.display()))?;
    // The board's capability filter (MSI-X) applies to any dump, also a --config one.
    pci_config::for_board(&dump, &a.board_data)
}

/// Free the host memory on SIGTERM, SIGINT or SIGHUP, then exit. The signals are
/// blocked in every other thread (the mask is set before any thread starts), so this
/// thread alone receives them.
fn spawn_signal_thread(hm: Arc<Mutex<HostMemory>>) -> Result<()> {
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
            // A DMA_MAP registration in progress holds the lock; wait for it.
            let (log, closed) = match hm.lock() {
                Ok(mut h) => (h.shutdown(), h.fail_closed().map(str::to_owned)),
                Err(p) => {
                    let mut h = p.into_inner();
                    (h.shutdown(), h.fail_closed().map(str::to_owned))
                }
            };
            for l in log {
                eprintln!("vfio-user-nvgpu: hostmem: {l}");
            }
            if let Some(why) = closed {
                eprintln!("vfio-user-nvgpu: exit 3: teardown failed closed ({why})");
                std::process::exit(FAIL_CLOSED_EXIT);
            }
            std::process::exit(128 + sig);
        })?;
    Ok(())
}

fn serve(a: &Args, socket: &Path, privilege: Privilege) -> Result<()> {
    // The eventfd does not carry the token: the reader rings the channels whose GPPut
    // moved since their last doorbell (`Dispatcher::doorbell_pending`), on every path.
    let doorbell_ioeventfd = doorbell_ioeventfd_requested();
    // The datamatch token pool: one eventfd per pool token, so the eventfd names the
    // channel (docs/caraxes/doorbell-token-pool.md).
    let doorbell_tokens = tokens::requested().map_err(anyhow::Error::msg)?;
    if doorbell_tokens.is_some() && doorbell_ioeventfd {
        // Both would be ioeventfds of the one doorbell register: KVM refuses a wildcard
        // next to datamatch entries at the same address and length (EEXIST;
        // caraxes/runs/doorbell-tokens-2026-09-24/kvm-ioeventfd-probe.txt, cases b, c).
        bail!("{DOORBELL_IOEVENTFD_ENV}=1 and {}=N exclude each other: choose the token-less ioeventfd or the token pool", tokens::TOKENS_ENV);
    }
    // How the doorbell eventfds are read (docs/caraxes/doorbell-poll.md).
    let poll = vfio_user_nvgpu::poll::requested().map_err(anyhow::Error::msg)?;
    if poll.mode != vfio_user_nvgpu::poll::Mode::Off && doorbell_tokens.is_none() && !doorbell_ioeventfd {
        bail!(
            "{}={} needs a doorbell eventfd to poll: {}=N or {DOORBELL_IOEVENTFD_ENV}=1",
            vfio_user_nvgpu::poll::POLL_ENV,
            poll.mode.name(),
            tokens::TOKENS_ENV
        );
    }
    // Refuse at start when a poll mode cannot pin its reader (no core to choose).
    let reactor = |name| {
        vfio_user_nvgpu::poll::Reactor::new(name, poll, Some(socket.to_path_buf()))
            .map_err(anyhow::Error::msg)
    };
    let token_reactor = reactor("tokens")?;
    let ioeventfd_reactor = reactor("ioeventfd")?;
    if doorbell_tokens.is_some() && !a.boot_model {
        bail!(
            "{}=N needs --boot-model (the channels live in the boot model)",
            tokens::TOKENS_ENV
        );
    }
    if doorbell_ioeventfd && vfio_user_nvgpu::fb::bar1_passthrough_config().is_some() {
        // The token-less ioeventfd path rings the channels whose GPPut moved, read from the
        // FB store's watched copy that the trapped BAR1 writes keep current; with the
        // passthrough area those writes bypass the device, the copy stays stale, no channel
        // is ever rung and the guest waits forever (caraxes/runs/p21-bar1-2026-09-24/
        // B-on-with-ioeventfd-hung). The token pool (CARAXES_DOORBELL_TOKENS=N) needs no
        // GPPut: the eventfd names the channel, so it combines with the area.
        bail!("{DOORBELL_IOEVENTFD_ENV}=1 is refused together with CARAXES_BAR1_PASSTHROUGH=1: the ioeventfd carries no token and the passthrough area hides the GPPut writes the token-less path relies on; use the token pool ({}=N) instead", tokens::TOKENS_ENV);
    }
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
            if img.len() < 2 || img[0] != 0x55 || img[1] != 0xAA {
                bail!(
                    "{} does not start with the 55 AA ROM signature",
                    p.display()
                );
            }
            Some(img)
        }
        None => None,
    };
    let chip = a.chip;
    let identity = chip.data().identity;
    let trace = Trace::new(a.trace.as_deref())?;
    let mut bar0 = Bar0::new(chip, rom, vbios);
    // The host driver's ABI, before the first host RM client opens (hostabi.rs,
    // docs/caraxes/cutover-615.md): CARAXES_HOST_ABI, else the running driver's
    // version; any version without measured layouts ends start-up here.
    if a.reserve_vram || a.register_guest_ram || a.host_rm {
        let sel = vfio_user_nvgpu::hostabi::select().map_err(anyhow::Error::msg)?;
        eprintln!(
            "vfio-user-nvgpu: {} (chosen by {}); guest ABI {}",
            sel.layouts.describe(),
            sel.how,
            vfio_user_nvgpu::ver::active::DRIVER_VERSION
        );
    }
    // Host memory for E5b: one client for the reservation and the guest RAM, shared with
    // the signal thread so that SIGTERM frees it too.
    let host_mem = if a.reserve_vram || a.register_guest_ram {
        let hm = HostMemory::open(privilege).context("open the host RM for host memory")?;
        eprintln!(
            "vfio-user-nvgpu: host memory client open; privilege: {}",
            hm.privilege().describe()
        );
        let hm = Arc::new(Mutex::new(hm));
        spawn_signal_thread(hm.clone())?;
        Some(hm)
    } else {
        None
    };
    let intr_shared = IntrShared::new();
    let mut layout = FbLayout::plain(a.fb_mib << 20);
    // P21: the dma-buf behind the guest's BAR1 passthrough area, held for the process's
    // life (docs/caraxes/bar1-passthrough.md §6).
    let mut bar1_export = None;
    if a.reserve_vram {
        let hm = host_mem.as_ref().expect("opened above");
        let mut h = hm.lock().unwrap();
        match h.reserve(a.fb_mib << 20, a.reserve_slot) {
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
        layout = FbLayout::reserved(a.fb_mib << 20, r.host_fb_total, r.base, r.end())
            .map_err(anyhow::Error::msg)?;
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
            model = model.with_host(Box::new(host));
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
        }
        if let Some((off, len)) = vfio_user_nvgpu::fb::bar1_passthrough_config() {
            if !a.host_exec {
                bail!(
                    "CARAXES_BAR1_PASSTHROUGH=1 needs host execution (the FB on the reserved VRAM)"
                );
            }
            let bar1 = region_size(&a.board_data, 1);
            let (fb0, len) = vfio_user_nvgpu::fb::bar1_passthrough_area(&layout, off, len, bar1)
                .map_err(anyhow::Error::msg)?;
            let hm = host_mem.as_ref().expect("checked in parse_args");
            let ex = hm
                .lock()
                .unwrap()
                .export_bar1_passthrough(fb0, len)
                .context("BAR1 passthrough (CARAXES_BAR1_PASSTHROUGH=1)")?;
            let (_, root) = layout.bar_roots().expect("a reserved layout");
            model.fb.set_bar1_passthrough(root, fb0, len);
            eprintln!(
                "vfio-user-nvgpu: bar1 passthrough: guest BAR1 [0, {len:#x}) offered as one sparse mmap area of region 1, backed by a dma-buf of FB [{fb0:#x}, {:#x}) (reservation offset {off:#x}); {len:#x} of host BAR1 held while the client maps it (CARAXES_BAR1_PASSTHROUGH=1, docs/caraxes/bar1-passthrough.md)",
                fb0 + len
            );
            bar1_export = Some(ex);
        }
        if let Some(n) = doorbell_tokens {
            model = model.with_doorbell_tokens(n);
        }
        bar0 = bar0.with_boot_model(model);
    }
    let guest_ram = match (&host_mem, a.register_guest_ram, a.identity_dma_table) {
        (Some(hm), true, _) => GuestRam::Register(hm.clone()),
        (_, _, true) => GuestRam::Identity,
        _ => GuestRam::Plain,
    };
    let mut device = Device::new(cfg, bar0, trace)
        .with_guest_ram(guest_ram, a.dma_ranges.clone())
        .with_intr_shared(intr_shared);
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
    let mut server = Server::new(socket, true, irqs(), regions_with(&a.board_data, bar1_area))
        .map_err(|e| anyhow::anyhow!("vfio-user server: {e}"))?;
    let doorbell = if doorbell_ioeventfd {
        let offset = chip.data().registers.usermode_doorbell;
        let (sub_region, reader) = doorbell_sub_region(offset).context("doorbell eventfd")?;
        server
            .add_ioeventfd(VFIO_PCI_BAR0_REGION_INDEX, sub_region)
            .map_err(|e| anyhow::anyhow!("doorbell sub-region: {e}"))?;
        eprintln!(
            "vfio-user-nvgpu: doorbell ioeventfd: BAR0 {offset:#x}, 4 bytes, no datamatch, offered through VFIO_USER_DEVICE_GET_REGION_IO_FDS ({DOORBELL_IOEVENTFD_ENV}=1)"
        );
        Some((offset, reader))
    } else {
        None
    };
    let token_readers = match doorbell_tokens {
        Some(n) => {
            let offset = chip.data().registers.usermode_doorbell;
            let mut readers = Vec::with_capacity(n);
            for (sub_region, reader) in
                doorbell_token_sub_regions(offset, n).context("doorbell token eventfds")?
            {
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
            Some((offset, readers))
        }
        None => None,
    };
    eprintln!(
        "vfio-user-nvgpu: guest ABI {} ({:?})",
        a.guest_abi,
        ver::active::GSP_TRANSPORT
    );
    eprintln!(
        "vfio-user-nvgpu: {} (BOOT_0 {:#010x}, driver {}) on {}, boot model {}, FB {} MiB, ROM {} bytes, PROM image {} bytes, trace {}",
        chip.data().short_name,
        identity.boot_0,
        ver::active::DRIVER_VERSION,
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
    let mut readers = Vec::new();
    if let Some((offset, reader)) = doorbell {
        readers.push((
            "ioeventfd",
            spawn_doorbell_reader(device.clone(), offset, reader, ioeventfd_reactor)
                .context("doorbell reader")?,
        ));
    }
    if let Some((offset, token_eventfds)) = token_readers {
        readers.push((
            "tokens",
            spawn_token_reader(device.clone(), offset, token_eventfds, token_reactor)
                .context("doorbell token reader")?,
        ));
    }
    if poll.mode != vfio_user_nvgpu::poll::Mode::Off {
        eprintln!(
            "vfio-user-nvgpu: doorbell poll: {} reader ({}={}, {}={:?})",
            poll.mode.name(),
            vfio_user_nvgpu::poll::POLL_ENV,
            poll.mode.name(),
            vfio_user_nvgpu::poll::POLL_CPU_ENV,
            poll.pin
        );
    }
    let result = server
        .run(&mut SharedDevice(device.clone()))
        .map_err(|e| anyhow::anyhow!("vfio-user server: {e}"));
    let mut device = device.lock().unwrap_or_else(PoisonError::into_inner);
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
    }
    eprint!("{}", device.summary());
    for (name, st) in &readers {
        eprint!("{}", st.summary(name));
    }
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
