#!/usr/bin/env bash
# One Windows session of the Windows agent (run inside vmlock.sh windows): start, steps, always
# stop. Usage: session.sh TAG BINARY-cap STEPS...
# steps: etwclean arm restart heaven superposition stages reboot growc sleep=N
# DEV_EXTRA adds device flags (A/B), e.g. "CARAXES_BAR2_MEMFD=1".
set -uo pipefail
TAG=$1 BIN=$2; shift 2
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain
OUT=/mnt/vmstore/caraxes/runs-2026-09-30/win/$TAG; mkdir -p "$OUT"
cd $K || exit 1
s() { timeout ${T:-60} python3 CH-build/run44.py ssh "$@" 2>&1 | tr -d '\r'; }
up() { for i in $(seq 1 90); do kill -0 "$(cat $K/ch.pid 2>/dev/null)" 2>/dev/null || { echo "Cloud Hypervisor exited: $(grep -a 'Fatal error' $K/$TAG/ch.log 2>/dev/null | tail -1 | sed 's/.*Fatal error: //' | cut -c1-160)"; return 1; }; s '(Get-Process explorer -ErrorAction SilentlyContinue | Measure-Object).Count' 2>/dev/null | grep -q '^[1-9]' && return 0; sleep 5; done; return 1; }
arm() { echo "gpu.log: $(grep -c 'events: host OS event on .*: armed$\|: armed \[' gpu.log) armed, $(grep -c 'armed again' gpu.log) armed again, $(grep -ci 'REFUSED' gpu.log) refused-lines; $(grep -cE 'mirror: events: .*(armed|ERROR)' gpu.log) event lines"; grep -E 'mirror: events: .*(armed|ERROR)' gpu.log | tail -12 | cut -c1-160; }
latches() { local w=${1:-30}; local e=$(python3 -c "import json; print(json.load(open('$K/$TAG/start.json'))['epoch'])"); awk -v t=$(( ($(date +%s) - e - w) * 1000000 )) '$1>t && /display channel 0xc57e\/[0-9] kick .*1 UPDATE/' gpu-trace.log | wc -l; }
pack() { local script=$1; shift; PACK_WAIT_S=${PW:-1200} timeout 300 python3 CH-build/run44.py upack $script "$@" 2>&1 | tail -1
  for i in $(seq 1 ${PN:-150}); do st=$(timeout 30 python3 CH-build/run44.py upack $script status "$@" 2>&1); echo "$st" | grep -qE 'PROBE_EXIT|PROBE_TIMEOUT' && break; grep -q 'flr: ' gpu-trace.log && { echo "FLR during $script"; break; }; sleep 10; done
  timeout 30 python3 CH-build/run44.py upack $script status "$@" 2>&1 | grep -v save_cache | tail -${PT:-20}; }
stop() { # Only this session's own run (another agent's run may be `current` after a failed start).
    [ "$(cat $K/current 2>/dev/null)" = "$TAG" ] || { echo "stop skipped: current is $(cat $K/current 2>/dev/null), not $TAG"; return; }
    echo "stop $(date +%T)"; timeout 400 python3 CH-build/run44.py stop > "$OUT/stop.txt" 2>&1; tail -12 "$OUT/stop.txt"; }
trap stop EXIT
echo "start $TAG $(date +%T) $BIN"
CH_BIN=/mnt/vmstore/caraxes/ch-readonly-mmap/target-0008/release/cloud-hypervisor CH_SERVER=$BIN CH_SLOT=0 \
  CH_DEV_EXTRA="CARAXES_INTR_PAGE=1 WIN_BAR1_POOL_MIB=32 WIN_CPUS=8 CARAXES_PROFILE=1 ${DEV_EXTRA:-}" \
  timeout 300 python3 CH-build/run44.py start $TAG > "$OUT/start.txt" 2>&1 || { cat "$OUT/start.txt"; exit 1; }
tail -3 "$OUT/start.txt"
# Host Xid watchdog: any host kernel line of this run's device pid ends the session (the EXIT
# trap stops the VM at once).
dpid=$(python3 -c "import json; print(json.load(open('$K/$TAG/start.json'))['win_device_pid'])")
( while kill -0 $$ 2>/dev/null; do sleep 5; if [ "$(sudo -n dmesg | grep -c "pid=$dpid,")" != 0 ]; then echo "HOST LINE OF pid $dpid at $(date +%T): stopping"; sudo -n dmesg | grep "pid=$dpid," | head -3; kill -TERM $$; break; fi; done ) &
up && echo "desktop up $(date +%T)" || echo "desktop NOT up $(date +%T)"
for st in "$@"; do
    echo "=== $st $(date +%T)"
    case $st in
    etwclean) s 'Remove-Item -Force -ErrorAction SilentlyContinue C:\etw\*.etl, C:\etw\*.csv, C:\etw\*.zip; "{0:N1} GB free on C:" -f ((Get-PSDrive C).Free/1GB)' ;;
    arm) arm ;;
    restart)
        id=$(s '(Get-PnpDevice -Class Display | Where-Object { $_.InstanceId -like "PCI\VEN_10DE*" } | Select-Object -First 1).InstanceId' | tail -1)
        echo "restart device $id"; T=120 s "pnputil /restart-device \"$id\"" | tail -3; sleep 45; up; echo "latches in the last 30 s: $(latches 30)"; arm ;;
    heaven) PT=8 pack heaven.ps1 -Quality HIGH -Tessellation NORMAL | grep -E 'FPS|Frames|Time|PASS|FAIL|FLR|end' ;;
    superposition) PT=14 pack superposition.ps1 -Preset 1080p_medium | grep -E 'Score|FPS|PASS|FAIL|FLR|end' ;;
    stages) PT=16 PW=120 PN=12 pack d3d11-stages.ps1 ;;
    reboot) s 'shutdown /r /t 0' >/dev/null; sleep 60; up && echo "back up $(date +%T)"; sleep 20; echo "latches in the last 30 s: $(latches 30)"; arm ;;
    growc) # After the first boot on the flattened raw image: C: grows to its maximum when the free
           # space follows it directly; otherwise (Windows' recovery partition after C:) the new
           # space becomes an NTFS volume D: "Games", and no partition is moved or deleted.
        T=300 s '$p = Get-Partition -DriveLetter C; $d = $p.DiskNumber; $m = (Get-PartitionSupportedSize -DriveLetter C).SizeMax; "C: {0:N1} GiB, max {1:N1} GiB; disk {2}: {3:N1} GiB, {4:N1} GiB unallocated" -f ($p.Size/1GB), ($m/1GB), $d, ((Get-Disk $d).Size/1GB), (((Get-Disk $d).Size - (Get-Disk $d).AllocatedSize)/1GB); Get-Partition -DiskNumber $d | ForEach-Object { "part {0} {1} {2:N1} GiB at {3:N1} GiB {4}" -f $_.PartitionNumber, $_.Type, ($_.Size/1GB), ($_.Offset/1GB), $_.DriveLetter }; if ($m -gt $p.Size + 1GB) { Resize-Partition -DriveLetter C -Size $m; "resized: C: {0:N1} GiB" -f ((Get-Partition -DriveLetter C).Size/1GB) } elseif (((Get-Disk $d).Size - (Get-Disk $d).AllocatedSize) -gt 10GB -and -not (Get-Volume -FileSystemLabel Games -ErrorAction SilentlyContinue)) { $n = New-Partition -DiskNumber $d -UseMaximumSize -DriveLetter D; Format-Volume -Partition $n -FileSystem NTFS -NewFileSystemLabel Games -Confirm:$false | Out-Null; "new volume D: Games {0:N1} GiB" -f ((Get-Partition -DriveLetter D).Size/1GB) } else { "nothing to do" }' ;;
    repair) # A guest disk that took a failed write (C92, 22:47:49): an online scan, the system
            # files from the local component store, and chkdsk /f at the next boot if the scan
            # found problems. No network.
        T=900 s 'chkdsk C: /scan' | tail -12
        T=2400 s 'sfc /scannow' | tr -d '\000' | tail -6
        if T=120 s 'chkdsk C: /scan' | grep -qiE 'Fehler|error|corrupt|beschädigt|Probleme'; then
            echo "scheduling chkdsk /f at the next boot"; T=60 s 'cmd /c "echo Y| chkdsk C: /f"' | tail -3
            s 'shutdown /r /t 0' >/dev/null; sleep 90; up && echo "back up with a session $(date +%T)" || echo "no session after the chkdsk boot $(date +%T)"
        fi
        up && echo "session present" || echo "no session" ;;
    offlinefix) # chkdsk /f at the next boot (Repair-Volume -OfflineScanAndFix, locale-free), the
                # reboot, a long wait for autochk, then sfc again and the session check.
        T=120 s 'Repair-Volume -DriveLetter C -OfflineScanAndFix; "dirty: " + (fsutil dirty query C:)' | tail -3
        s 'shutdown /r /t 0' >/dev/null; sleep 120
        for i in $(seq 1 60); do s hostname 2>/dev/null | grep -qi . && break; sleep 10; done; echo "ssh back $(date +%T)"
        T=120 s 'Get-WinEvent -FilterHashtable @{LogName="Application"; ProviderName="Microsoft-Windows-Wininit"; StartTime=(Get-Date).AddMinutes(-30)} -ErrorAction SilentlyContinue | Select-Object -First 1 | ForEach-Object { ($_.Message -split "`n" | Select-Object -Last 25) -join "`n" }' | tr -d '\000' | tail -25
        T=2400 s 'sfc /scannow' | tr -d '\000' | tail -4
        up && echo "session present $(date +%T)" || echo "no session $(date +%T)" ;;
    diag) # Which file took C92's failed write (bytes 78885019648-78885511167 of the disk), the
          # crashing service's faulting module, autochk's last result, the dirty bit.
        T=300 s '$p = Get-Partition -DriveLetter C; $cs = (Get-Volume -DriveLetter C).AllocationUnitSize; $a = 78885019648; $b = 78885511167; $c1 = [math]::Floor(($a - $p.Offset) / $cs); $c2 = [math]::Floor(($b - $p.Offset) / $cs); "partition offset {0}, cluster {1}, clusters {2}-{3}" -f $p.Offset, $cs, $c1, $c2; $list = @(); for ($c = $c1; $c -le $c2; $c += 4) { $list += $c }; $list += $c2; fsutil volume querycluster C: @list' | tr -d '\000' | tail -40
        T=120 s 'Get-WinEvent -LogName Application -FilterXPath "*[System[(EventID=1000)]]" -MaxEvents 30 -ErrorAction SilentlyContinue | Where-Object { $_.Message -match "CoreMessaging|svchost" } | Select-Object -First 3 | ForEach-Object { "{0:HH:mm:ss} {1}" -f $_.TimeCreated, (($_.Message -split "`n" | Select-Object -First 8) -join " | ") }' | tr -d '\000' | cut -c1-600
        T=120 s 'Get-WinEvent -LogName Application -FilterXPath "*[System[(EventID=1001)]]" -MaxEvents 10 -ErrorAction SilentlyContinue | Where-Object { $_.ProviderName -match "Wininit|Chkdsk" } | Select-Object -First 2 | ForEach-Object { "{0:HH:mm:ss} {1}" -f $_.TimeCreated, (($_.Message -split "`n" | Select-Object -Last 20) -join " | ") }' | tr -d '\000' | cut -c1-1500
        s 'fsutil dirty query C:' | tr -d '\000' ;;
    diag2) # After autochk: the directories whose $I30 it repaired, its full report, the service
           # errors of this boot, then sfc on the clean volume.
        T=120 s 'foreach ($id in "0x2B982","0x2B983") { "file $id :"; fsutil file queryfilenamebyid C:\ $id }' | tr -d '\000' | tail -6
        T=120 s '$e = Get-WinEvent -LogName Application -FilterXPath "*[System[(EventID=1001)]]" -MaxEvents 10 -ErrorAction SilentlyContinue | Where-Object { $_.ProviderName -match "Wininit" } | Select-Object -First 1; $e.Message' | tr -d '\000' | grep -avE '^\s*$' | tail -60
        T=120 s 'Get-WinEvent -LogName System -MaxEvents 400 -ErrorAction SilentlyContinue | Where-Object { $_.Id -in 7000,7001,7009,7022,7023,7024,7031,7034 -and $_.TimeCreated -gt (Get-Date).AddMinutes(-20) } | Select-Object -First 12 | ForEach-Object { "{0:HH:mm:ss} {1} {2}" -f $_.TimeCreated, $_.Id, ($_.Message -replace "\s+"," ").Substring(0, [Math]::Min(220, $_.Message.Length)) }' | tr -d '\000'
        T=120 s 'Get-WinEvent -FilterHashtable @{LogName="Application"; ProviderName="Application Error"} -MaxEvents 5 -ErrorAction SilentlyContinue | ForEach-Object { "{0:HH:mm:ss} {1}" -f $_.TimeCreated, (($_.Message -split "`n" | Select-Object -First 6) -join " | ") }' | tr -d '\000' | cut -c1-500
        T=2400 s 'sfc /scannow' | tr -d '\000' | tail -4
        up && echo "session present $(date +%T)" || echo "no session $(date +%T)" ;;
    chkdskf) # The classic scheduling: chkdsk /f at the next boot, the reboot, 10 min for autochk.
        T=60 s 'cmd /c "echo Y| chkdsk C: /f"' | tr -d '\000' | tail -3
        s 'shutdown /r /t 0' >/dev/null; sleep 600
        for i in $(seq 1 60); do s hostname 2>/dev/null | grep -qi . && break; sleep 10; done; echo "ssh back $(date +%T)"
        s 'fsutil dirty query C:' | tr -d '\000'
        up && echo "session present $(date +%T)" || echo "no session $(date +%T)" ;;
    refused) echo "compressible-kind refusals: $(grep -ciE 'REFUSED.*compressible|compressible kind' gpu-trace.log) (trace), $(grep -ciE 'REFUSED.*compressible|compressible kind' gpu.log) (log)"; grep -iE 'compress' gpu.log | head -3 | cut -c1-200 ;;
    events) # Application crashes and hangs (WER) and display/driver events of the last 3 hours.
        T=120 s '$t = (Get-Date).AddHours(-3); Get-WinEvent -FilterHashtable @{LogName="Application"; StartTime=$t} -ErrorAction SilentlyContinue | Where-Object { $_.ProviderName -match "Application Error|Windows Error Reporting|Application Hang" } | Select-Object -First 10 | ForEach-Object { "{0:HH:mm:ss} {1} {2}: {3}" -f $_.TimeCreated, $_.ProviderName, $_.Id, (($_.Message -split "`n" | Select-Object -First 8) -join " | ") }; Get-WinEvent -FilterHashtable @{LogName="System"; StartTime=$t} -ErrorAction SilentlyContinue | Where-Object { $_.ProviderName -match "Display|nvlddmkm|Dxgkrnl" } | Select-Object -First 10 | ForEach-Object { "{0:HH:mm:ss} {1} {2}: {3}" -f $_.TimeCreated, $_.ProviderName, $_.Id, (($_.Message -split "`n" | Select-Object -First 3) -join " | ") }; "events done"' | tr -d '\000' | cut -c1-600 ;;
    cacheclear) # The user's NVIDIA and D3D shader caches (C100 wrote Heaven's 16 MiB cache 5 s before
                # the reboot that killed the VM; every Heaven run after it crashed).
        T=120 s 'foreach ($d in @("C:\Users\silas\AppData\Local\NVIDIA\DXCache", "C:\Users\silas\AppData\Local\D3DSCache")) { if (Test-Path $d) { $fs = Get-ChildItem $d -Recurse -File -ErrorAction SilentlyContinue; Remove-Item "$d\*" -Recurse -Force -ErrorAction SilentlyContinue; "{0}: {1} file(s) removed, {2} left" -f $d, $fs.Count, (Get-ChildItem $d -Recurse -File -ErrorAction SilentlyContinue).Count } }' | tr -d '\000' ;;
    ids) # The six files whose $I30 indexes autochk repaired (C98), and C:\found.000.
        T=120 s 'foreach ($id in @("0x2A07A", "0x2A0ED", "0x2B10C", "0x2B982", "0x2B983", "0x2BF9B")) { $r = (fsutil file queryFileNameById C:\ $id 2>&1) -join " "; "{0}: {1}" -f $id, $r }; if (Test-Path C:\found.000) { Get-ChildItem C:\found.000 -Recurse -Force -ErrorAction SilentlyContinue | Select-Object -First 30 | ForEach-Object { "found.000: {0} {1}" -f $_.FullName, $_.Length } } else { "no C:\found.000" }' | tr -d '\000' ;;
    driverhash) # SHA-256 of the installed display driver's DriverStore files, against the 617.14
                # package's Display.Driver (extracted on the host, tmp-nv617/host-sha256.txt).
        T=600 s '$d = Get-ChildItem C:\Windows\System32\DriverStore\FileRepository -Directory -Filter "nv_dispi.inf_amd64_*" | Select-Object -First 1; "dir " + $d.Name; Get-ChildItem $d.FullName -Recurse -File | ForEach-Object { "{0}  {1}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name.ToLower() }' | tr -d '\000\r' > "$OUT/guest-driver-sha256.txt"
        python3 - "$OUT/guest-driver-sha256.txt" /mnt/vmstore/caraxes/tmp-nv617/host-sha256.txt <<'PY'
import sys
g = {}; hdr = []
for l in open(sys.argv[1], errors='replace'):
    p = l.split()
    if len(p) == 2 and len(p[0]) == 64: g.setdefault(p[1], set()).add(p[0])
    elif l.strip(): hdr.append(l.strip())
h = {}
for l in open(sys.argv[2]):
    p = l.split()
    h.setdefault(p[1], set()).add(p[0])
same = [n for n in g if n in h and g[n] & h[n]]
diff = [n for n in g if n in h and not (g[n] & h[n])]
only_g = [n for n in g if n not in h]
print(" | ".join(hdr[:2]))
print(f"driver files: {len(g)} in the guest, {len(same)} identical to the package, {len(diff)} different, {len(only_g)} not in the package")
for n in diff[:20]: print("DIFFERENT", n)
for n in only_g[:10]: print("guest only", n)
PY
        ;;
    apphash) # Heaven's data archives against the same files of the Linux package (tmp-nv617).
        T=300 s 'foreach ($n in @("core.ung", "heaven.ung")) { $f = "C:\Program Files (x86)\Unigine\Heaven Benchmark 4.0\data\$n"; if (Test-Path $f) { "{0}  data/{1}" -f (Get-FileHash $f -Algorithm SHA256).Hash.ToLower(), $n } else { "missing data/$n" } }' | tr -d '\000\r' | while read -r h n; do w=$(grep -F "  $n" /mnt/vmstore/caraxes/tmp-nv617/heaven-data-sha256.txt | cut -c1-64); [ "$h" = "$w" ] && echo "heaven $n: identical to the package" || echo "heaven $n: guest $h, package $w"; done ;;
    gpuclients) # The GPU's clients now: processes with GPU engine time (per-process counters),
                # Steam and the Futuremark/UL SystemInfo service.
        T=120 s '$c = (Get-Counter "\GPU Engine(*)\Utilization Percentage" -ErrorAction SilentlyContinue).CounterSamples; $by = @{}; foreach ($x in $c) { if ($x.InstanceName -match "pid_(\d+)_.*engtype_(\w+)") { $k = [int]$Matches[1]; if (-not $by.ContainsKey($k)) { $by[$k] = @{} }; $by[$k][$Matches[2]] = [math]::Round($by[$k][$Matches[2]] + $x.CookedValue, 1) } }; foreach ($k in $by.Keys) { $n = (Get-Process -Id $k -ErrorAction SilentlyContinue).ProcessName; "gpu client pid {0} {1}: {2}" -f $k, $n, (($by[$k].GetEnumerator() | Where-Object { $_.Value -gt 0 } | ForEach-Object { "$($_.Key) $($_.Value)%" }) -join ", ") }; Get-Process steam*, msedge*, *SystemInfo*, 3DMark* -ErrorAction SilentlyContinue | ForEach-Object { "process {0} {1} started {2:HH:mm:ss}" -f $_.Id, $_.ProcessName, $_.StartTime }; Get-Service | Where-Object { $_.DisplayName -match "SystemInfo|Futuremark|UL " } | ForEach-Object { "service {0} ({1}): {2}, start {3}" -f $_.Name, $_.DisplayName, $_.Status, $_.StartType }' | tr -d '\000' | cut -c1-200 ;;
    quiet) # For this session only: Steam's and Edge's processes and the SystemInfo service stop
           # (autostart and installation untouched; the next boot starts them again).
        T=120 s 'Get-Process steam*, msedge* -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; Get-Service | Where-Object { $_.DisplayName -match "SystemInfo|Futuremark" -and $_.Status -eq "Running" } | ForEach-Object { Stop-Service -Name $_.Name -Force -ErrorAction SilentlyContinue; "stopped service {0}: {1}" -f $_.Name, (Get-Service -Name $_.Name).Status }; Start-Sleep 3; "steam/edge processes left: " + (Get-Process steam*, msedge* -ErrorAction SilentlyContinue | Measure-Object).Count' | tr -d '\000' ;;
    present-bg=*) # A second D3D11 client in the background: present-probe windowed 800x600, a clear
                  # and a present per vblank, for N seconds (the next step runs beside it).
        PACK_WAIT_S=$(( ${st#present-bg=} + 120 )) timeout 300 python3 CH-build/run44.py upack present-probe.ps1 -Seconds ${st#present-bg=} -Width 800 -Height 600 2>&1 | tail -1 ;;
    present-status=*) timeout 60 python3 CH-build/run44.py upack present-probe.ps1 status -Seconds ${st#present-status=} -Width 800 -Height 600 2>&1 | tr -d '\000' | tail -8 ;;
    config) # The device's configuration block and the removed variables it warned about.
        grep -a "config: " gpu.log | cut -c1-300 ;;
    sleep=*) sleep ${st#sleep=} ;;
    *) echo "unknown step $st" ;;
    esac
done
echo "FLRs: $(grep -c 'flr: ' gpu-trace.log); withdrawals: $(grep -c 'withdrawal [0-9]' gpu.log)"
grep "interrupt vectors by the guest" gpu.log | tail -1 | cut -c1-600
