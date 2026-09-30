#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""The Windows guest under Cloud Hypervisor (branch win-ch-2026-09-29), and its QEMU maintenance
boot. Same disk (windows-plain/win11.qcow2) and swtpm state as P1-P43. The device's flags are
P43's (the production candidate, P43-prod/env.json); run-ch-gpu.sh starts Cloud Hypervisor.

  run44.py preflight [nogpu]    checks only (binary hash and cap, slots, host floors, files)
  run44.py start <name> [nogpu] device (gpu-up-host.sh) + Cloud Hypervisor (run-ch-gpu.sh)
  run44.py stop                 guest shutdown over SSH, device down, logs archived, checks
  run44.py maint start <name>   QEMU, no GPU device, QGA + user NAT + a virtio-net on the tap
  run44.py maint stop           guest shutdown through QGA
  run44.py qga ping|ps|cmd|get|put ...   qga.py pass-through (maintenance boot only)
  run44.py ssh [powershell...]  SSH to the guest on the tap (192.168.107.2, key id_win)
  run44.py scp <src> <dst>      scp with the kit key (guest paths as win:<path>)
  run44.py pack <script.ps1> [args]         run a tools/windows script as SYSTEM (scheduled task)
  run44.py pack <script.ps1> status [args]  its state and output
  run44.py probe start|status   the D3D11 gate (tools/windows/d3d11-render.ps1)
  run44.py memory               host FB/BAR1 free (read-only)
Every VM start is done by the operator."""
import hashlib, json, os, pathlib, re, shutil, signal, subprocess, sys, tempfile, time

S = pathlib.Path('/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain')
K = S / 'CH-build'
# The repository this script lives in (CH-build/run44.py is a symlink to it): the launchers next
# to it, the guest scripts under tools/windows.
HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[2]
QGA = HERE / 'qga.py'
TAP, TAP_MAC, GUEST_IP = 'caraxes7', '02:00:00:00:00:47', '192.168.107.2'
KEY = K / 'id_win'
# The device binary: CH_SERVER names a *-cap copy in CH-build or P43-build (default P43's).
SERVER = pathlib.Path(os.environ.get('CH_SERVER', str(S / 'P43-build/vfio-user-nvgpu-p43-cap')))
FB_MIB = int(os.environ.get('CH_FB_MIB', '4096'))
MEM_GIB = int(os.environ.get('CH_MEM_GIB', '8'))
VRAM_NEED_MIB = FB_MIB + 1536
env = {k: v for k, v in os.environ.items() if not k.startswith(('CARAXES_', 'WIN_'))}
# CH_BIN: another Cloud Hypervisor binary (run-ch-gpu.sh WIN_CH), e.g. a fork build with a fix.
if os.environ.get('CH_BIN'):
    env['WIN_CH'] = os.environ['CH_BIN']
env.update(WIN_STATE_DIR=str(S), WIN_MEM=f'{MEM_GIB}G', WIN_MEM_GIB=str(MEM_GIB), WIN_GPU_FB_MIB=str(FB_MIB),
           WIN_QEMU_DISPLAY='none', QGA_SOCK=str(S / 'qga.sock'), QMP_SOCK=str(S / 'qmp.sock'),
           QGA_TIMEOUT='900', WIN_TAP=TAP, WIN_TAP_MAC=TAP_MAC)
# P43's device flags (P43-prod/env.json), without CARAXES_SHADOW_CHANNEL_DIAG since C66 (its GPFIFO
# coverage query waited under the device lock for the worker's set-A apply: C65's 347 ms holds).
DEV = dict(CARAXES_DEFERRED_CONTEXT='1', CARAXES_SHADOW_USERD='1', CARAXES_FB_ZERO_BASED='1',
           CARAXES_TRACE_ACCESSES='0', CARAXES_SEMAPHORE_DIAG='1',
           WIN_GPU_CONFIG=str(ROOT / 'caraxes/reference/rtx2070/config-space.txt'),
           WIN_GPU_ROM='/mnt/vmstore/caraxes/win11/rom-legacy-only.bin', WIN_GPU_ROMBAR='1',
           WIN_GPU_VBIOS=str(ROOT / 'caraxes/reference/rtx2070/prom-window-2026-09-25-from-55aa.bin'),
           WIN_GPU_DISPLAY='1', CARAXES_EMU_GR='1', CARAXES_RUSD='1',
           CARAXES_WIN_DISPLAY_QUERIES='1', CARAXES_WIN_ACKS='1', CARAXES_PREFER_RECORDED='1',
           CARAXES_LPWR_2806_OK='1', CARAXES_ACK_UNANSWERED='1',
           CARAXES_WIN_EXCEPT='2080a70a,2080a709,2080a801,2080a02f,20803c01',
           CARAXES_HOST_BAR1_FLOOR_MIB='96', WIN_HOST_MODE='exec',
           CARAXES_COMPRESSION='off', WIN_RESERVE_SLOT=os.environ.get('CH_SLOT', '0'),
           CARAXES_SHADOW_FENCE='1', CARAXES_LAUNCH_TIMING='1')
# CH_DEV_EXTRA="K=V K=V": temporary A/B flags on top (recorded in env.json).
for kv in os.environ.get('CH_DEV_EXTRA', '').split():
    k, v = kv.split('=', 1)
    DEV[k] = v


def die(msg):
    sys.exit(f'ERROR: {msg}')


def current():
    return S / (S / 'current').read_text().strip()


def ssh_base():
    return ['-F', '/dev/null', '-i', str(KEY), '-o', 'StrictHostKeyChecking=no',
            '-o', 'UserKnownHostsFile=/dev/null', '-o', 'LogLevel=ERROR', '-o', 'ConnectTimeout=5',
            '-o', 'BatchMode=yes', '-o', 'ServerAliveInterval=15']


def ssh_user():
    f = K / 'guest-user'
    return f.read_text().strip() if f.exists() else 'caraxes'


def ssh(script, timeout=600, check=False):
    r = subprocess.run(['ssh', *ssh_base(), f'{ssh_user()}@{GUEST_IP}', script], capture_output=True,
                       text=True, timeout=timeout)
    if check and r.returncode:
        die(f'ssh exit {r.returncode}: {r.stderr.strip()[:300]}')
    return r


def scp(src, dst):
    subprocess.run(['scp', '-q', *ssh_base(), str(src), f'{ssh_user()}@{GUEST_IP}:{dst}'], check=True)


def running(pid):
    try:
        st = pathlib.Path(f'/proc/{pid}/stat').read_text()
        return st.split(') ')[1][0] != 'Z'
    except FileNotFoundError:
        return False


def host_memory():
    """Read-only `nvidia-smi -q -d MEMORY` (logged in host-reads.log): text and free MiB."""
    r = subprocess.run(['nvidia-smi', '-q', '-d', 'MEMORY'], capture_output=True, text=True)
    free, section = {}, None
    for line in r.stdout.splitlines():
        t = line.strip()
        if t.startswith('FB Memory Usage'):
            section = 'fb'
        elif t.startswith('BAR1 Memory Usage'):
            section = 'bar1'
        elif t.startswith(('Conf Compute', 'Protected')):
            section = None
        elif section and t.startswith('Free') and ':' in t:
            free[section] = int(t.split(':')[1].split()[0])
    with (K / 'host-reads.log').open('a') as f:
        f.write(f"{time.strftime('%F %T')} nvidia-smi -q -d MEMORY (read-only): free {free}\n")
    return r.stdout + r.stderr, free


def dmesg():
    """The host kernel log (read-only `sudo -n dmesg -T`), as lines."""
    r = subprocess.run(['sudo', '-n', 'dmesg', '-T'], capture_output=True, text=True)
    return r.stdout.splitlines(keepends=True)


def uptime():
    """The host's uptime in seconds: the clock of the kernel log's raw timestamps."""
    return float(pathlib.Path('/proc/uptime').read_text().split()[0])


def dmesg_since(t):
    """The host kernel log lines stamped after uptime `t` (read-only `sudo -n dmesg`, raw
    timestamps). A line count taken at the start does not work: the ring buffer drops its
    oldest lines (C80: 780 lines at the start, 768 at the stop, 54 Xid 31 lines missed)."""
    r = subprocess.run(['sudo', '-n', 'dmesg'], capture_output=True, text=True)
    out = []
    for line in r.stdout.splitlines(keepends=True):
        m = re.match(r'\[\s*(\d+\.\d+)\]', line)
        if m and float(m.group(1)) >= t:
            out.append(line)
    return out


def gpu_processes():
    out = []
    for p in pathlib.Path('/proc').glob('[0-9]*'):
        try:
            if (p / 'comm').read_text().strip() in ('vfio-user-nvgpu', 'vhost-user-nvrm'):
                out.append((int(p.name), (p / 'cmdline').read_bytes().split(b'\0')))
        except OSError:
            pass
    return out


def preflight(gpu=True):
    for f in ('qemu.pid', 'ch.pid'):
        if (S / f).exists():
            die(f'{f} exists: stop the owned run first')
    subprocess.run(['ip', 'link', 'show', TAP], check=True, stdout=subprocess.DEVNULL)
    avail = int(next(l.split()[1] for l in open('/proc/meminfo') if l.startswith('MemAvailable'))) >> 20
    if avail < MEM_GIB + 6:
        die(f'host MemAvailable {avail} GiB < {MEM_GIB} GiB guest + 6 GiB headroom')
    # The guest disk (qcow2), the traces and the state live on this filesystem: a full one
    # stopped C92's trace for 4 min and would fail the guest's own writes (2026-09-29).
    free = shutil.disk_usage(S).free >> 30
    if free < 30:
        die(f'{S}: {free} GiB free < 30 GiB (guest disk growth, traces)')
    if not gpu:
        print(f'preflight ok (no GPU): MemAvailable {avail} GiB')
        return
    if not SERVER.exists():
        die(f'no {SERVER}')
    sums = SERVER.parent / 'SHA256SUMS'
    want = {l.split()[1]: l.split()[0] for l in sums.read_text().splitlines() if l.strip()}
    base = SERVER.name.removesuffix('-cap')
    if hashlib.sha256(SERVER.read_bytes()).hexdigest() != want.get(base):
        die(f'{SERVER.name} does not match {sums} ({base})')
    if 'cap_sys_admin' not in subprocess.run(['getcap', str(SERVER)], capture_output=True, text=True).stdout:
        die(f'{SERVER} lacks cap_sys_admin+p')
    slot = int(DEV['WIN_RESERVE_SLOT'])
    lo, hi = (slot + 1) * FB_MIB, slot * FB_MIB
    for p, a in gpu_processes():
        o_slot = int(a[a.index(b'--reserve-slot') + 1]) if b'--reserve-slot' in a else 0
        o_mib = int(a[a.index(b'--fb-mib') + 1]) if b'--fb-mib' in a else 4096
        o_lo, o_hi = (o_slot + 1) * o_mib, o_slot * o_mib
        if b'--reserve-vram' in a and (o_hi >= lo or o_lo <= hi):
            print(f'tolerated: device pid {p} slot {o_slot} x {o_mib} MiB (no overlap)')
            continue
        die(f'a GPU device with an overlapping or unknown reservation runs: pid {p}')
    for f in ('linux-busy', 'win-busy'):
        if (S.parent.parent / f).exists():
            die(f'/mnt/vmstore/caraxes/{f} exists')
    _, free = host_memory()
    if free.get('fb', 0) < VRAM_NEED_MIB:
        die(f"host FB free {free.get('fb')} MiB < {VRAM_NEED_MIB} MiB")
    pool = int(DEV.get('WIN_BAR1_POOL_MIB', '0') or 0)
    if free.get('bar1', 0) < 96 + pool:
        die(f"host BAR1 free {free.get('bar1')} MiB < {96 + pool} MiB (floor 64 + windows 32 + pool {pool})")
    print(f"preflight ok: {SERVER.name}, MemAvailable {avail} GiB, host free FB {free.get('fb')} MiB "
          f"BAR1 {free.get('bar1')} MiB")


def start(name, gpu=True):
    preflight(gpu)
    out = S / name
    out.mkdir(exist_ok=False)
    e = dict(env, WIN_CH_SERIAL=str(out / 'serial.log'))
    st = {'epoch': int(time.time()), 'hypervisor': 'cloud-hypervisor', 'gpu': gpu, 'mem_gib': MEM_GIB,
          'dmesg_lines': len(dmesg()), 'uptime': uptime()}
    if gpu:
        e.update(DEV, WIN_GPU_SERVER=str(SERVER), CARAXES_RECORD_HOST=str(out / 'host-controls.tsv'))
        st.update(binary=SERVER.name, sha256=hashlib.sha256(SERVER.read_bytes()).hexdigest())
        src = SERVER.parent / 'source.json'
        if src.exists():
            st['source'] = json.loads(src.read_text())
        text, free = host_memory()
        (out / 'host-smi-before.txt').write_text(text)
        st['host_free_before'] = free
    else:
        e['WIN_GPU'] = '0'
    (out / 'env.json').write_text(json.dumps({k: v for k, v in e.items() if k.startswith(('WIN_', 'CARAXES_'))}, indent=2))
    (S / 'current').write_text(out.name)
    if gpu:
        with (out / 'launch.log').open('w') as f:
            subprocess.run([str(HERE / 'gpu-up-host.sh')], env=e, stdout=f, stderr=subprocess.STDOUT, check=True,
                           start_new_session=True)
        st['win_device_pid'] = int((S / 'gpu.pid').read_text())
        # The device removes a stale display-ctl when it starts; arm the automatic scanout
        # dump now (window 0's new frames, at most one per 3 s, the last 10 kept).
        (S / 'display-ctl').write_text('dump auto\n')
    with (out / 'ch.log').open('w') as f:
        p = subprocess.Popen([str(HERE / 'run-ch-gpu.sh')], env=e, stdout=f, stderr=subprocess.STDOUT,
                             stdin=subprocess.DEVNULL, start_new_session=True)
    (S / 'ch.pid').write_text(str(p.pid))
    st['ch_pid'] = p.pid
    (out / 'start.json').write_text(json.dumps(st, indent=2))
    print('started', out.name, 'device', st.get('win_device_pid'), 'cloud-hypervisor', p.pid,
          '; wait for `run44.py ssh hostname`')


def stop():
    out = current()
    if (S / 'ch.pid').exists():
        pid = int((S / 'ch.pid').read_text())
        if running(pid):
            try:
                ssh('Stop-Computer -Force', timeout=20)
            except subprocess.TimeoutExpired:
                pass
            for _ in range(120):
                if not running(pid):
                    break
                time.sleep(1)
            if running(pid):
                # The ACPI power button through the API socket, then SIGTERM.
                subprocess.run(['curl', '-s', '--unix-socket', str(S / 'ch.sock'), '-X', 'PUT',
                                'http://localhost/api/v1/vm.power-button'], capture_output=True)
                for _ in range(90):
                    if not running(pid):
                        break
                    time.sleep(1)
            if running(pid):
                if b'cloud-hypervisor' not in pathlib.Path(f'/proc/{pid}/cmdline').read_bytes():
                    die('PID ownership mismatch')
                os.kill(pid, signal.SIGTERM)
                for _ in range(15):
                    if not running(pid):
                        break
                    time.sleep(1)
            if running(pid):
                die('Cloud Hypervisor still active, refusing GPU cleanup')
        (S / 'ch.pid').unlink()
    st = json.loads((out / 'start.json').read_text())
    if st.get('gpu'):
        try:
            dpid = int((S / 'gpu.pid').read_text())
            for _ in range(60):
                if not pathlib.Path(f'/proc/{dpid}').exists():
                    break
                time.sleep(1)
        except (OSError, ValueError):
            pass
        subprocess.run([str(HERE / 'gpu-down.sh')], env=env, check=True)
        for f in ['gpu.log', 'gpu-trace.log']:
            if (S / f).exists():
                shutil.move(str(S / f), str(out / f))
        for f in S.glob('scanout-*.png'):
            shutil.move(str(f), str(out / f.name))
        text, _ = host_memory()
        (out / 'host-smi-after.txt').write_text(text)
    # The host kernel log since the start (read-only `sudo -n dmesg`, allowed by the user;
    # `journalctl -k` shows nothing to this user).
    # Lines by their timestamp since the start's uptime (the line count of runs before C81 is
    # only a fallback: the ring buffer drops its oldest lines).
    if 'uptime' in st:
        (out / 'host-kernel.log').write_text(''.join(dmesg_since(st['uptime'])))
    else:
        (out / 'host-kernel.log').write_text(''.join(dmesg()[st.get('dmesg_lines', 0):]))
    print('stopped; results', out)
    if st.get('gpu'):
        subprocess.run(['bash', str(HERE / 'check-ch.sh'), str(out)])


# The signed-in user's packs (upack) live where that user can write.
USER_DIR = r'C:\Users\Public\caraxes'


def pack(script, args, tag, wait_s, user=None):
    """Upload every pack script, then run `script` as SYSTEM through a one-shot scheduled task
    (the guest agent's SYSTEM context under QEMU; an SSH session's children die with it). With
    `user`, the task runs in that signed-in user's interactive session instead (a desktop to
    present to); no password is stored: the task runs only while the user is signed in."""
    d = USER_DIR if user else r'C:\Windows\Temp'
    if user:
        ssh(f"New-Item -ItemType Directory -Force '{d}' | Out-Null", check=True)
    for f in sorted((ROOT / 'tools/windows').glob('*.ps1')):
        scp(f, f"{d.replace(chr(92), '/')}/caraxes-{f.name}")
    scp(script, f"{d.replace(chr(92), '/')}/caraxes-{tag}.ps1")
    base = rf'{d}\caraxes-{tag}'
    arglist = ','.join('"' + a.replace('"', '`"') + '"' for a in args)
    wrapper = rf'''$ErrorActionPreference = "Stop"
Remove-Item -Force -ErrorAction SilentlyContinue {base}-exit.txt
$a = @("-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-File","{base}.ps1"{("," + arglist) if args else ""})
$p = Start-Process powershell.exe -ArgumentList $a -PassThru -RedirectStandardOutput {base}-out.txt -RedirectStandardError {base}-err.txt
$null = $p.Handle
if (-not $p.WaitForExit({wait_s * 1000})) {{ $p.Kill(); $r = "PROBE_TIMEOUT {wait_s} s" }} else {{ $r = "PROBE_EXIT=" + $p.ExitCode }}
Set-Content -Encoding ascii {base}-exit.txt $r
'''
    with tempfile.NamedTemporaryFile('w', suffix='.ps1', delete=False) as t:
        t.write(wrapper)
    scp(t.name, f"{d.replace(chr(92), '/')}/caraxes-{tag}-run.ps1")
    os.unlink(t.name)
    prin = (f"New-ScheduledTaskPrincipal -UserId '{user}' -LogonType Interactive -RunLevel Limited" if user
            else "New-ScheduledTaskPrincipal -UserId SYSTEM -LogonType ServiceAccount -RunLevel Highest")
    r = ssh(rf'''$act = New-ScheduledTaskAction -Execute powershell.exe -Argument '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File {base}-run.ps1'
$prin = {prin}
$set = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Seconds {wait_s + 120}) -AllowStartIfOnBatteries
Register-ScheduledTask -Force -TaskName caraxes-{tag} -Action $act -Principal $prin -Settings $set | Out-Null
Remove-Item -Force -ErrorAction SilentlyContinue {base}-exit.txt,{base}-out.txt,{base}-err.txt
Start-ScheduledTask -TaskName caraxes-{tag}
"started caraxes-{tag}"''', check=True)
    print(r.stdout.strip())


def pack_status(tag, status_file, user=False):
    base = rf'{USER_DIR if user else r"C:\Windows\Temp"}\caraxes-{tag}'
    r = ssh(rf'''if (Test-Path {base}-exit.txt) {{ Get-Content {base}-exit.txt }} else {{ "RUNNING: " + (Get-ScheduledTask -TaskName caraxes-{tag}).State }}
foreach ($f in "{base}-out.txt","{base}-err.txt") {{ if (Test-Path $f) {{ Get-Content $f }} }}''')
    if status_file.suffix == '.json':
        # check-ch.sh reads the guest agent's exec-status shape (out-data in base64).
        import base64
        status_file.write_text(json.dumps({'out-data': base64.b64encode(r.stdout.encode()).decode(),
                                           'err-data': base64.b64encode(r.stderr.encode()).decode()}))
    else:
        status_file.write_text(r.stdout + r.stderr)
    print(r.stdout + r.stderr)


cmd = sys.argv[1] if len(sys.argv) > 1 else ''
if cmd == 'preflight':
    preflight(gpu='nogpu' not in sys.argv[2:])
elif cmd == 'start':
    start(sys.argv[2], gpu='nogpu' not in sys.argv[3:])
elif cmd == 'stop':
    stop()
elif cmd == 'memory':
    _, free = host_memory()
    print(f"host free FB {free.get('fb')} MiB BAR1 {free.get('bar1')} MiB",
          'OK' if free.get('bar1', 0) >= 64 else 'BELOW 64 MiB BAR1: stop Windows')
elif cmd == 'maint':
    if sys.argv[2] == 'start':
        for f in ('qemu.pid', 'ch.pid'):
            if (S / f).exists():
                die(f'{f} exists: stop the owned run first')
        subprocess.run(['ip', 'link', 'show', TAP], check=True, stdout=subprocess.DEVNULL)
        out = S / sys.argv[3]
        out.mkdir(exist_ok=False)
        (S / 'current').write_text(out.name)
        e = dict(env, WIN_GPU='0')
        (out / 'start.json').write_text(json.dumps({'epoch': int(time.time()), 'maintenance': True,
                                                    'win_gpu': 0, 'tap': TAP, 'mem_gib': MEM_GIB}, indent=2))
        with (out / 'qemu.log').open('w') as f:
            p = subprocess.Popen([str(HERE / 'run-qemu-gpu.sh')], env=e, stdout=f, stderr=subprocess.STDOUT,
                                 stdin=subprocess.DEVNULL, start_new_session=True)
        (S / 'qemu.pid').write_text(str(p.pid))
        print('maintenance boot (no GPU device):', out.name, 'qemu', p.pid)
    else:
        out = current()
        if (S / 'qemu.pid').exists():
            pid = int((S / 'qemu.pid').read_text())
            try:
                subprocess.run([str(QGA), 'ps', 'Stop-Computer -Force'], env=env, timeout=20, capture_output=True)
            except subprocess.TimeoutExpired:
                pass
            for _ in range(600):
                if not running(pid):
                    break
                time.sleep(1)
            else:
                die(f'QEMU {pid} still runs after 10 min')
            (S / 'qemu.pid').unlink()
        print('maintenance boot stopped;', out)
elif cmd == 'qga':
    sys.exit(subprocess.run([str(QGA), *sys.argv[2:]], env=env).returncode)
elif cmd == 'ssh':
    sys.exit(subprocess.run(['ssh', *ssh_base(), f'{ssh_user()}@{GUEST_IP}', *sys.argv[2:]]).returncode)
elif cmd == 'scp':
    a = [x.replace('win:', f'{ssh_user()}@{GUEST_IP}:', 1) if x.startswith('win:') else x for x in sys.argv[2:]]
    sys.exit(subprocess.run(['scp', *ssh_base(), *a]).returncode)
elif cmd == 'probe':
    out = current()
    if sys.argv[2] == 'start':
        pack(ROOT / 'tools/windows/d3d11-render.ps1', [], 'd3d-probe', 90)
    else:
        pack_status('d3d-probe', out / 'd3d-probe-status.json')
elif cmd in ('pack', 'upack'):
    user = ssh_user() if cmd == 'upack' else None
    out = current()
    script = ROOT / 'tools/windows' / sys.argv[2]
    args = sys.argv[3:]
    polling = bool(args) and args[0] == 'status'
    args = args[1:] if polling else args
    tag = '-'.join([script.stem] + [''.join(c for c in a if c.isalnum()) for a in args])
    if polling:
        pack_status(tag, out / f'{tag}-status.txt', user=bool(user))
    else:
        pack(script, args, tag, int(os.environ.get('PACK_WAIT_S', '600')), user=user)
        (out / f'{tag}-pid.json').write_text(json.dumps({'args': args, 'epoch': int(time.time())}))
else:
    print(__doc__)
