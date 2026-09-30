#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""R4 Windows guest helper (docs/caraxes/experiments.md R4): SSH to the Windows 11 guest on
the tap (192.168.107.2, the kit's key id_win, PowerShell login shell), as the Caraxes runs do
(CH-build/run44.py), with outputs in the current R4 run folder ($PT_DIR/runs/<current>).

  pt-guest.py wait [SECONDS]            until SSH answers (first boot with the card: PnP installs
                                        617.14 for the new device instance)
  pt-guest.py ssh [POWERSHELL...]       a command, or an interactive session
  pt-guest.py put LOCAL GUESTPATH | get GUESTPATH LOCAL
  pt-guest.py pnp                       the card's functions in PnP, the display adapter's mode
  pt-guest.py mode W H HZ | mode list   set-mode.ps1 in the signed-in session (primary display)
  pt-guest.py clock                     guest-host clock offset (best of 7 round trips) -> clock.txt
  pt-guest.py etw start | etw stop LABEL        tools/windows/dxgk-etw.ps1 (the C61-C77 capture)
  pt-guest.py etw-boot arm | collect LABEL | disarm   the same provider as an autologger (boot)
  pt-guest.py dwm-restart               restart DWM (the desktop's flip chain starts again)
  pt-guest.py task SCRIPT.ps1 [ARGS..] [--user] [--wait S] [--label L]   scheduled task
  pt-guest.py bench                     smoke, d3d12-probe, cuda-probe, present-probe (native baseline)
Env: PT_DIR (/mnt/vmstore/caraxes/r4), PT_WINTOOLS (this repository's tools/windows).
Every guest action that matters for the traces also appends a host-time line to markers.txt."""
import calendar
import os
import pathlib
import subprocess
import sys
import tempfile
import time
import zipfile

KIT = pathlib.Path('/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build')
KEY = KIT / 'id_win'
IP = '192.168.107.2'
USER = (KIT / 'guest-user').read_text().strip() if (KIT / 'guest-user').exists() else 'caraxes'
HERE = pathlib.Path(__file__).resolve().parent
WINTOOLS = pathlib.Path(os.environ.get('PT_WINTOOLS', str(pathlib.Path(__file__).resolve().parents[3] / 'tools/windows')))
R4 = pathlib.Path(os.environ.get('PT_DIR', '/mnt/vmstore/caraxes/r4'))
USER_DIR = r'C:\Users\Public\caraxes'
SSH = ['-F', '/dev/null', '-i', str(KEY), '-o', 'StrictHostKeyChecking=no', '-o', 'UserKnownHostsFile=/dev/null',
       '-o', 'LogLevel=ERROR', '-o', 'ConnectTimeout=5', '-o', 'BatchMode=yes', '-o', 'ServerAliveInterval=15']


def run_dir():
    cur = R4 / 'current'
    d = R4 / 'runs' / cur.read_text().strip() if cur.exists() else pathlib.Path.cwd()
    d.mkdir(parents=True, exist_ok=True)
    return d


def iso(t):
    return time.strftime('%Y-%m-%dT%H:%M:%S', time.gmtime(t)) + f'.{int(t % 1 * 1e6):06d}Z'


def mark(text):
    with open(run_dir() / 'markers.txt', 'a') as f:
        f.write(f'{iso(time.time())} {text}\n')


def ssh(cmd, timeout=600, check=False, capture=True):
    r = subprocess.run(['ssh', *SSH, f'{USER}@{IP}', cmd], capture_output=capture, text=True, timeout=timeout)
    if check and r.returncode:
        sys.exit(f'ERROR: ssh exit {r.returncode}: {(r.stderr or "").strip()[:300]}')
    return r


def put(src, dst):
    subprocess.run(['scp', '-q', *SSH, str(src), f'{USER}@{IP}:{dst}'], check=True)


def get(src, dst):
    subprocess.run(['scp', '-q', *SSH, f'{USER}@{IP}:{src}', str(dst)], check=True)


def ps_file(guest_path, args='', timeout=600):
    """Run a .ps1 already in the guest in a fresh PowerShell (execution policy bypassed)."""
    r = ssh(f'powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File {guest_path} {args}',
            timeout=timeout)
    print(r.stdout + r.stderr, end='')
    return r


def wait(seconds=600):
    end = time.time() + seconds
    while time.time() < end:
        try:
            r = ssh('hostname', timeout=15)
            if r.returncode == 0:
                print(f'guest {r.stdout.strip()} answers ({iso(time.time())})')
                mark('guest ssh up')
                return 0
        except subprocess.TimeoutExpired:
            pass
        time.sleep(5)
    sys.exit(f'ERROR: no SSH answer from {IP} in {seconds} s')


def pnp():
    r = ssh(r"""Get-PnpDevice -PresentOnly | Where-Object InstanceId -like 'PCI\VEN_10DE*' |
  Format-Table -AutoSize Status,Class,FriendlyName,Problem,InstanceId | Out-String -Width 220
Get-CimInstance Win32_VideoController | Format-List Name,DriverVersion,Status,CurrentHorizontalResolution,CurrentVerticalResolution,CurrentRefreshRate
Get-CimInstance Win32_PnPSignedDriver | Where-Object DeviceID -like 'PCI\VEN_10DE*' | Format-Table -AutoSize DeviceName,DriverVersion | Out-String -Width 200""")
    text = r.stdout + r.stderr
    (run_dir() / 'pnp.txt').write_text(text)
    print(text)


def clock():
    """Guest UTC against the host clock over one open SSH channel: the round with the smallest
    round trip gives the offset (ETW Clock-Time is guest UTC; QEMU's -msg timestamps are host UTC)."""
    code = ("$i = 0; while ($i -lt 7) { $null = [Console]::In.ReadLine(); "
            "[Console]::Out.WriteLine([DateTime]::UtcNow.ToString('o')); [Console]::Out.Flush(); $i++ }")
    p = subprocess.Popen(['ssh', *SSH, f'{USER}@{IP}', code], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         text=True, bufsize=1)
    best = None
    for _ in range(7):
        t0 = time.time()
        p.stdin.write('x\n')
        p.stdin.flush()
        g = p.stdout.readline().strip()
        t1 = time.time()
        day, _, frac = g.rstrip('Z').partition('.')
        guest = calendar.timegm(time.strptime(day, '%Y-%m-%dT%H:%M:%S')) + float('0.' + (frac or '0'))
        if best is None or t1 - t0 < best[0]:
            best = (t1 - t0, guest - (t0 + t1) / 2, g, (t0 + t1) / 2)
    p.stdin.close()
    p.wait(timeout=15)
    line = (f'{iso(best[3])} host, guest {best[2]}: guest-host {best[1] * 1e3:+.3f} ms '
            f'(+-{best[0] * 5e2:.3f} ms, rtt {best[0] * 1e3:.3f} ms)')
    with open(run_dir() / 'clock.txt', 'a') as f:
        f.write(line + '\n')
    print(line)


def fetch_zip(guest_files, label):
    """Zip guest files, copy the zip into the run folder, unpack it there as <label>-<name>."""
    z = r'C:\etw\r4-fetch.zip'
    paths = ','.join(f"'{g}'" for g in guest_files)
    ssh(f"Compress-Archive -Force -Path {paths} -DestinationPath '{z}'", timeout=1800, check=True)
    local = run_dir() / f'{label}.zip'
    get(z.replace('\\', '/'), local)
    with zipfile.ZipFile(local) as zf:
        for n in zf.namelist():
            (run_dir() / f'{label}-{n}').write_bytes(zf.read(n))
    print(f'{label}: {local} ({local.stat().st_size >> 10} KiB) unpacked in {run_dir()}')


def etw(action, label=None):
    g = r'C:\Windows\Temp\caraxes-dxgk-etw.ps1'
    if action == 'start':
        clock()
        put(WINTOOLS / 'dxgk-etw.ps1', g.replace('\\', '/'))
        ps_file(g, '-Action start', timeout=120)
        mark('etw start')
        return
    if not label:
        sys.exit('etw stop LABEL')
    mark(f'etw stop {label}')
    r = ps_file(g, '-Action stop', timeout=3600)
    (run_dir() / f'etw-{label}.txt').write_text(r.stdout + r.stderr)
    clock()
    fetch_zip([r'C:\etw\dxgk.csv', r'C:\etw\dxgk.etl'], f'etw-{label}')


def etw_boot(action, label=None):
    g = r'C:\Windows\Temp\caraxes-etw-boot.ps1'
    put(HERE / 'etw-boot.ps1', g.replace('\\', '/'))
    if action in ('arm', 'disarm'):
        ps_file(g, f'-Action {action}', timeout=120)
        if action == 'arm':
            print(f'now reboot the guest: {sys.argv[0]} ssh Restart-Computer -Force; then wait, the '
                  'desktop plus 60 s, and collect')
        return
    if not label:
        sys.exit('etw-boot collect LABEL')
    mark(f'etw-boot stop {label}')
    r = ps_file(g, '-Action stop', timeout=3600)
    (run_dir() / f'etw-{label}.txt').write_text(r.stdout + r.stderr)
    clock()
    fetch_zip([r'C:\etw\dxgk-boot.csv', r'C:\etw\dxgk-boot.etl'], f'etw-{label}')


def task(script, args, user=False, wait_s=600, label=None):
    """Run a PowerShell script as SYSTEM, or in the signed-in user's session (--user: a desktop
    to present to), through a one-shot scheduled task (an SSH session's children die with it);
    the same wrapper as CH-build/run44.py pack/upack. Prints and keeps its output."""
    script = pathlib.Path(script)
    if not script.exists():
        script = WINTOOLS / script.name if (WINTOOLS / script.name).exists() else HERE / script.name
    d = USER_DIR if user else r'C:\Windows\Temp'
    dd = d.replace('\\', '/')
    tag = label or '-'.join([script.stem] + [''.join(c for c in a if c.isalnum()) for a in args])
    if user:
        ssh(f"New-Item -ItemType Directory -Force '{d}' | Out-Null", check=True)
    for f in sorted(WINTOOLS.glob('*.ps1')):
        put(f, f'{dd}/caraxes-{f.name}')
    put(script, f'{dd}/caraxes-{tag}.ps1')
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
    with tempfile.NamedTemporaryFile('w', suffix='.ps1', delete=False, dir=run_dir()) as t:
        t.write(wrapper)
    put(t.name, f'{dd}/caraxes-{tag}-run.ps1')
    os.unlink(t.name)
    prin = (f"New-ScheduledTaskPrincipal -UserId '{USER}' -LogonType Interactive -RunLevel Limited" if user
            else "New-ScheduledTaskPrincipal -UserId SYSTEM -LogonType ServiceAccount -RunLevel Highest")
    ssh(rf'''$act = New-ScheduledTaskAction -Execute powershell.exe -Argument '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File {base}-run.ps1'
$prin = {prin}
$set = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Seconds {wait_s + 120}) -AllowStartIfOnBatteries
Register-ScheduledTask -Force -TaskName caraxes-{tag} -Action $act -Principal $prin -Settings $set | Out-Null
Remove-Item -Force -ErrorAction SilentlyContinue {base}-exit.txt,{base}-out.txt,{base}-err.txt
Start-ScheduledTask -TaskName caraxes-{tag}''', check=True)
    mark(f'task {tag} start')
    print(f'started caraxes-{tag} ({"user" if user else "SYSTEM"}), waiting up to {wait_s} s')
    end = time.time() + wait_s + 90
    state = ''
    while time.time() < end:
        time.sleep(5)
        state = ssh(rf'if (Test-Path {base}-exit.txt) {{ Get-Content {base}-exit.txt }} else {{ "RUNNING" }}',
                    timeout=60).stdout.strip()
        if state and state != 'RUNNING':
            break
    mark(f'task {tag} end {state}')
    r = ssh(rf'foreach ($f in "{base}-out.txt","{base}-err.txt") {{ if (Test-Path $f) {{ Get-Content $f }} }}')
    text = f'{state}\n{r.stdout}{r.stderr}'
    (run_dir() / f'{tag}.txt').write_text(text)
    print(text)
    return state


def bench():
    for script, args, user, wait_s in (('smoke.ps1', [], False, 900), ('d3d12-probe.ps1', [], False, 300),
                                       ('cuda-probe.ps1', [], False, 600),
                                       ('present-probe.ps1', ['-Frames', '600'], True, 300)):
        task(WINTOOLS / script, args, user=user, wait_s=wait_s, label=f'bench-{pathlib.Path(script).stem}')


def main():
    a = sys.argv[1:]
    cmd = a[0] if a else ''
    if cmd == 'wait':
        return wait(int(a[1]) if len(a) > 1 else 600)
    if cmd == 'ssh':
        return subprocess.run(['ssh', *SSH, f'{USER}@{IP}', *a[1:]]).returncode
    if cmd == 'put':
        return put(a[1], a[2])
    if cmd == 'get':
        return get(a[1], a[2])
    if cmd == 'pnp':
        return pnp()
    if cmd == 'mode':
        if a[1:2] == ['list']:
            return task(HERE / 'set-mode.ps1', ['-List'], user=True, wait_s=120, label='mode-list')
        mark(f'mode {a[1]}x{a[2]}@{a[3]}')
        return task(HERE / 'set-mode.ps1', ['-Width', a[1], '-Height', a[2], '-Hz', a[3]], user=True,
                    wait_s=120, label=f'mode-{a[1]}x{a[2]}-{a[3]}')
    if cmd == 'clock':
        return clock()
    if cmd == 'etw':
        return etw(a[1], a[2] if len(a) > 2 else None)
    if cmd == 'etw-boot':
        return etw_boot(a[1], a[2] if len(a) > 2 else None)
    if cmd == 'dwm-restart':
        mark('dwm restart')
        r = ssh('Stop-Process -Name dwm -Force; Start-Sleep 3; Get-Process dwm | Format-Table Id,StartTime')
        print(r.stdout + r.stderr)
        return 0
    if cmd == 'task':
        user = '--user' in a
        wait_s = int(a[a.index('--wait') + 1]) if '--wait' in a else 600
        label = a[a.index('--label') + 1] if '--label' in a else None
        skip = {i for i, x in enumerate(a) if x in ('--wait', '--label')}
        skip |= {i + 1 for i in skip}
        rest = [x for i, x in enumerate(a[2:], 2) if i not in skip and x != '--user']
        return task(a[1], rest, user=user, wait_s=wait_s, label=label)
    if cmd == 'bench':
        return bench()
    print(__doc__)
    return 1


if __name__ == '__main__':
    sys.exit(main() or 0)
