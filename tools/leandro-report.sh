#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The report collector: what a bug report needs from a machine that runs Leandro VirtIO or
# Leandro VFIO, in one tarball.
#
# Usage: tools/leandro-report.sh [--vm STATE_DIR]... [--guest SSH_TARGET]... [--since TIME]
#                                [--redact] [--key FILE] [--lines N] [--trace-mb N] -o OUT.tar.zst
#        tools/leandro-report.sh --self-test
#   --vm DIR      a VM state directory: Leandro VFIO (gpu.log, ch.log, serial.log, gpu-trace.log;
#                 a Windows state directory's `current` names its run folder) or Leandro VirtIO
#                 (nvrm.log or backend.log, ch.log or hypervisor.log, serial.log)
#   --guest T     a running guest over ssh (user@address); Linux or Windows (PowerShell)
#   --key FILE    the ssh key for --guest (default $LEA_VM_DIR/id_leandro when present)
#   --since TIME  the host kernel log window, in journalctl's syntax (default "2 hours ago")
#   --redact      also replace host names, user names, home directories, IP and MAC
#                 addresses, UUIDs, serial numbers and other GPU processes' names
#   --lines N     lines per log tail (default 300)
#   --trace-mb N  the size cap of the device trace tail (default 20)
#   --self-test   run the collectors against tools/testdata/leandro-report (no GPU, no VM)
#
# Host facts, the kernel log, per state directory the device, hypervisor and serial logs, per
# guest its driver state (docs/reporting.md). Read-only: it needs no root,
# never writes to a GPU, a module or a parameter, and under sudo runs the same steps. Every
# step is time-limited; a failed step is noted in MANIFEST.txt and skipped. E-mail addresses
# are always replaced and credential lines dropped; the tarball is not written while one
# remains. Next to OUT: OUT.sha256 and <OUT without .tar.zst>.manifest.txt.
# Another card needs a capture instead: caraxes/tools/lab-kit.sh
# (docs/wiki/contributing-a-capture.md).
set -uo pipefail
export LC_ALL=C

TOOL="leandro-report 1 (2026-09-30)"
SELF=$(readlink -f "${BASH_SOURCE[0]}")
LEA_ROOT=$(cd "$(dirname "$SELF")/.." && pwd -P)
export LEA_ROOT

die() { printf 'leandro-report: %s\n' "$*" >&2; exit 2; }
have() { command -v "$1" >/dev/null 2>&1; }
usage() { sed -n '7,20p' "$SELF"; exit "${1:-0}"; }
# t SECONDS CMD...: one external command under its own time limit.
t() { local s=$1; shift; timeout -k 3 "$s" "$@"; }

# Credential lines (dropped whole) and e-mail addresses (replaced): always.
CRED_RE='passw(or)?d|passphrase|autoadminlogon|defaultpassword|defaultusername|autologon'
MAIL_RE='[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}'
OCTET='(25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])'
IPV4_RE="\\b($OCTET\\.){3}$OCTET\\b"
# Names that are project accounts or common words in the logs, never redacted as identities.
COMMON_NAMES=' root user admin guest test leandro caraxes nvidia localhost ubuntu linux debian arch host vm gpu '

# ---- helpers the steps share ------------------------------------------------------------------

# RUN: the run folder a Windows state directory's `current` names (run44.py), else empty.
vm_resolve() {
    RUN=""
    local c=""
    if [[ -f $D/current ]]; then
        read -r c < "$D/current" || true
        [[ -n $c && $c != */* && -d $D/$c ]] && RUN=$D/$c
    fi
}

# The newer of $D/<name> and $RUN/<name>, when either exists.
vm_file() {
    local a=$D/$1 b=""
    [[ -n $RUN ]] && b=$RUN/$1
    if [[ -n $b && -f $b ]] && { [[ ! -f $a ]] || [[ $b -nt $a ]]; }; then echo "$b"
    elif [[ -f $a ]]; then echo "$a"; fi
}

# "<role> <pid>" of the VM's processes from its pid files and start.json.
vm_pids() {
    local f p pid
    for f in gpu:gpu.pid ch:ch.pid backend:backend.pid hypervisor:hypervisor.pid input:input.pid; do
        p=$(vm_file "${f#*:}")
        [[ -n $p ]] || continue
        pid=""; read -r pid < "$p" || true
        [[ $pid =~ ^[0-9]+$ ]] && echo "${f%%:*} $pid"
    done
    p=$(vm_file start.json)
    [[ -n $p ]] && grep -oE '"(win_device_pid|ch_pid)"[[:space:]]*:[[:space:]]*[0-9]+' "$p" \
        | sed -E 's/^"win_device_pid"[^0-9]*/gpu /; s/^"ch_pid"[^0-9]*/ch /'
}

# The running device process of the VM, if its pid is alive and is a device binary.
vm_device_alive() {
    local role pid exe
    while read -r role pid; do
        [[ $role == gpu || $role == backend ]] || continue
        [[ -d /proc/$pid ]] || continue
        exe=$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null | cut -d' ' -f1)
        [[ $exe == *vfio-user-nvgpu* || $exe == *vhost-user-nvrm* ]] && { echo "$pid"; return 0; }
    done < <(vm_pids | sort -u)
    return 1
}

# First occurrence of each line with its count, in order.
dedupe() {
    awk '{ if (!($0 in c)) o[++n] = $0; c[$0]++ } END { for (i = 1; i <= n; i++) printf "%6d  %s\n", c[o[i]], o[i] }'
}

# ---- steps (each runs in its own time-limited child: `leandro-report.sh --step NAME ...`) -----

host_system() {
    mkdir -p "$R/host"
    {
        echo "== os"; ( . /etc/os-release 2>/dev/null && echo "${PRETTY_NAME:-unknown}" ) || echo "no /etc/os-release"
        echo "== uname -a"; uname -a
        echo "== kernel command line"; cat /proc/cmdline
        echo "== cpu"
        if have lscpu; then
            t 10 lscpu | grep -E '^(Architecture|CPU\(s\)|Model name|Thread|Core|Socket|NUMA node\(s\)|Virtualization|L3)'
        else
            grep -m1 'model name' /proc/cpuinfo
        fi
        echo "== memory"; grep -E '^(MemTotal|MemAvailable|SwapTotal|SwapFree|HugePages_Total|Hugepagesize):' /proc/meminfo
        echo "== uptime (s)"; cut -d' ' -f1 /proc/uptime
        echo "== run as"
        if [[ $EUID -eq 0 ]]; then echo "root${SUDO_USER:+ (sudo from $SUDO_USER)}: the same read-only steps"; else echo "uid $EUID, no root"; fi
    } > "$R/host/system.txt"
    echo "host/system.txt: kernel $(uname -r), $(awk '/^MemTotal/ { printf "%d GiB RAM", $2 / 1048576 }' /proc/meminfo)"
}

host_pci() {
    have lspci || { echo "lspci missing (pciutils)"; return 3; }
    mkdir -p "$R/host"
    t 20 lspci -nnk > "$R/host/lspci-nnk.txt" 2>&1
    t 20 lspci -vv -nn -D -d 10de: > "$R/host/lspci-vv-nvidia.txt" 2>&1
    local bdf g n=0
    for bdf in $(t 10 lspci -D -n -d 10de: | grep -oE "^[0-9a-f]{4}:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]"); do
        n=$((n + 1))
        if g=$(readlink "/sys/bus/pci/devices/$bdf/iommu_group" 2>/dev/null); then
            g=${g##*/}
            echo "$bdf: IOMMU group $g: $(ls "/sys/kernel/iommu_groups/$g/devices" | tr '\n' ' ')"
        else
            echo "$bdf: no IOMMU group (IOMMU off)"
        fi
        g=$(readlink "/sys/bus/pci/devices/$bdf/driver" 2>/dev/null) && echo "  driver: ${g##*/}"
    done > "$R/host/iommu.txt"
    echo "$n NVIDIA function(s): host/lspci-nnk.txt, lspci-vv-nvidia.txt, iommu.txt"
}

host_nvidia() {
    mkdir -p "$R/host"
    local got=() rc f
    if [[ -r /proc/driver/nvidia/version ]]; then
        cat /proc/driver/nvidia/version > "$R/host/nvidia-version.txt"; got+=(version)
    fi
    if [[ -r /proc/driver/nvidia/params ]]; then
        cat /proc/driver/nvidia/params > "$R/host/nvidia-params.txt"; got+=(params)
    fi
    if have nvidia-smi; then
        t 45 nvidia-smi -q > "$R/host/nvidia-smi-q.txt" 2>&1; rc=$?
        if ((rc == 0)); then got+=("nvidia-smi -q"); else got+=("nvidia-smi -q (exit $rc)"); fi
    fi
    ((${#got[@]})) || { echo "no NVIDIA driver loaded and no nvidia-smi"; return 3; }
    printf -v f '%s, ' "${got[@]}"
    echo "driver $(grep -oE '[0-9]{3}\.[0-9]+\.[0-9]+' /proc/driver/nvidia/version 2>/dev/null | head -n 1): ${f%, }"
}

vmm_candidates() {
    local D RUN role pid p
    command -v cloud-hypervisor qemu-system-x86_64 2>/dev/null
    echo "${LEA_CH:-/mnt/vmstore/leandro/cloud-hypervisor/bin/cloud-hypervisor}"
    while IFS= read -r D; do
        [[ -n $D ]] || continue
        vm_resolve
        p=$(vm_file env.json)
        [[ -n $p ]] && grep -oE '"(WIN_CH|LEA_CH|CH_BIN)"[[:space:]]*:[[:space:]]*"[^"]*"' "$p" | sed -E 's/.*"([^"]*)"$/\1/'
        while read -r role pid; do
            [[ $role == ch || $role == hypervisor ]] || continue
            [[ -d /proc/$pid ]] && tr '\0' '\n' < "/proc/$pid/cmdline" 2>/dev/null | head -n 1
        done < <(vm_pids | sort -u)
    done <<< "$VMS"
}

host_vmm() {
    mkdir -p "$R/host"
    local c n=0
    local -A seen=()
    while IFS= read -r c; do
        [[ -n $c && -x $c && -f $c && -z ${seen[$c]:-} ]] || continue
        seen[$c]=1; n=$((n + 1))
        printf '%s: %s\n' "$c" "$(t 10 "$c" --version 2>&1 < /dev/null | head -n 2 | tr '\n' ' ')"
    done < <(vmm_candidates) > "$R/host/vmm-versions.txt"
    ((n)) || { echo "no Cloud Hypervisor or QEMU found"; return 3; }
    echo "host/vmm-versions.txt: $n binaries"
}

host_klog() {
    mkdir -p "$R/host"
    local pat='NVRM|Xid|nvidia|vfio|kvm|ublk' src raw=$S/klog.raw
    if [[ -n ${KLOG_FIXTURE:-} ]]; then
        src="the self-test's kernel log"; cat "$KLOG_FIXTURE" > "$raw"
    elif have journalctl && [[ -n $(t 20 journalctl -k -n 1 -q --no-pager 2>/dev/null) ]]; then
        src="journalctl -k --since \"$SINCE\""
        t 50 journalctl -k --since "$SINCE" -q --no-pager -o short-iso > "$raw" 2> "$S/klog.err" \
            || { echo "journalctl failed: $(head -n 1 "$S/klog.err")"; return 1; }
    elif t 20 dmesg --time-format iso --since "$SINCE" > "$raw" 2> /dev/null; then
        src="dmesg --since \"$SINCE\" (no journal access)"
    elif t 20 dmesg --time-format iso > "$raw" 2> /dev/null; then
        src="dmesg, the whole ring buffer (this dmesg has no --since)"
    else
        echo "none: journalctl shows this user no kernel lines and dmesg is restricted (kernel.dmesg_restrict=$(cat /proc/sys/kernel/dmesg_restrict 2>/dev/null)); run with sudo or join the systemd-journal group" | tee "$S/klog.source"
        return 3
    fi
    echo "$src" > "$S/klog.source"
    grep -aiE "$pat" "$raw" > "$S/klog.matched"
    { echo "# $src: the lines matching $pat (any case), the last 20000"; tail -n 20000 "$S/klog.matched"; } > "$R/host/kernel-log.txt"
    echo "$src: $(wc -l < "$S/klog.matched") of $(wc -l < "$raw") lines"
    rm -f "$raw"
}

host_resources() {
    mkdir -p "$R/host"
    local d
    {
        echo "== memory"; grep -E '^(MemTotal|MemAvailable|SwapTotal|SwapFree):' /proc/meminfo
        echo "== disk (df -hP)"
        {
            while IFS= read -r d; do [[ -n $d && -d $d ]] && echo "$d"; done <<< "$VMS"
            for d in "${CARAXES_STORE:-/mnt/vmstore/caraxes}" /mnt/vmstore/leandro "$OUTDIR"; do [[ -d $d ]] && echo "$d"; done
        } | sort -u | while IFS= read -r d; do
            printf '%s: %s\n' "$d" "$(t 10 df -hP "$d" 2>&1 | tail -n 1)"
        done
    } > "$R/host/resources.txt"
    echo "host/resources.txt: MemAvailable $(awk '/^MemAvailable/ { printf "%d MiB", $2 / 1024 }' /proc/meminfo)"
}

repo_state() {
    have git || { echo "git missing"; return 3; }
    local d
    d=$(GIT_OPTIONAL_LOCKS=0 t 20 git -C "$LEA_ROOT" describe --always --dirty --abbrev=12 2>/dev/null) \
        || { echo "$LEA_ROOT is not a git checkout"; return 3; }
    {
        echo "describe: $d"
        echo "branch: $(git -C "$LEA_ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null)"
        echo "head: $(git -C "$LEA_ROOT" log -1 --format='%H %cI %s' 2>/dev/null)"
        echo "DRIVER_VERSION: $(cat "$LEA_ROOT/DRIVER_VERSION" 2>/dev/null)"
        echo "CH_VERSION: $(cat "$LEA_ROOT/CH_VERSION" 2>/dev/null)"
        echo "changed files:"; GIT_OPTIONAL_LOCKS=0 t 20 git -C "$LEA_ROOT" status --porcelain 2>/dev/null | head -n 50
    } > "$R/repo.txt"
    echo "repo.txt: $d"
}

binary_candidates() {
    local D RUN i=0 role pid p k v
    while IFS= read -r D; do
        [[ -n $D ]] || continue
        i=$((i + 1)); vm_resolve
        while read -r role pid; do
            [[ ($role == gpu || $role == backend) && -d /proc/$pid ]] || continue
            printf 'vm-%d %s pid %s (running)\t/proc/%s/exe\n' "$i" "$role" "$pid" "$pid"
        done < <(vm_pids | sort -u)
        p=$(vm_file env.json)
        if [[ -n $p ]]; then
            grep -oE '"[A-Z_]*SERVER"[[:space:]]*:[[:space:]]*"[^"]*"' "$p" \
                | sed -E 's/^"([A-Z_]*)"[[:space:]]*:[[:space:]]*"(.*)"$/\1\t\2/' \
                | while IFS=$'\t' read -r k v; do printf 'vm-%d env.json %s\t%s\n' "$i" "$k" "$v"; done
        fi
        [[ -f $D/vfio-user-nvgpu ]] && printf 'vm-%d state directory copy\t%s\n' "$i" "$D/vfio-user-nvgpu"
    done <<< "$VMS"
}

binaries() {
    local label p h n=0
    local -A seen=()
    while IFS=$'\t' read -r label p; do
        [[ -n $p && -z ${seen[$p]:-} ]] || continue
        seen[$p]=1
        if [[ ! -e $p ]]; then
            printf 'missing  %s  (%s)\n' "$p" "$label"
        elif h=$(t 60 sha256sum < "$p" 2>/dev/null); then
            n=$((n + 1))
            printf '%s  %s bytes  %s  %s  (%s)\n' "${h%% *}" "$(stat -L -c %s "$p")" \
                "$(stat -L -c %y "$p" | cut -d. -f1)" "$p" "$label"
        else
            printf 'unreadable  %s  (%s)\n' "$p" "$label"
        fi
    done < <(binary_candidates) > "$R/binaries.txt"
    [[ -s $R/binaries.txt ]] || { rm -f "$R/binaries.txt"; echo "no device binary named in the state directories"; return 3; }
    echo "binaries.txt: $n hashed"
}

vm_logs() {
    local i=$1 D=$2 RUN o=$R/vm-$1 kind dev vmm f p n alive start=0 pids
    [[ -d $D ]] || { echo "$D: no such directory"; return 1; }
    vm_resolve
    mkdir -p "$o"
    kind="Leandro VFIO"; dev=$(vm_file gpu.log)
    if [[ -z $dev ]]; then
        kind="Leandro VirtIO"; dev=$(vm_file nvrm.log)
        [[ -z $dev ]] && dev=$(vm_file backend.log)
        [[ -z $dev ]] && kind="unknown (no gpu.log, nvrm.log or backend.log)"
    fi
    vmm=$(vm_file ch.log); [[ -z $vmm ]] && vmm=$(vm_file hypervisor.log)
    alive=$(vm_device_alive) || alive=""
    {
        echo "state directory: $D"
        echo "run folder: ${RUN:-none}"
        echo "kind: $kind"
        echo "device log: ${dev:-none}"
        echo "hypervisor log: ${vmm:-none}"
        echo "processes:"
        while read -r f p; do
            if [[ -d /proc/$p ]]; then
                echo "  $f pid $p: running: $(tr '\0' ' ' < "/proc/$p/cmdline" 2>/dev/null)"
            else
                echo "  $f pid $p: gone"
            fi
        done < <(vm_pids | sort -u)
        p=$(vm_file start.json)
        [[ -n $p ]] && grep -oE '"(binary|sha256)"[[:space:]]*:[[:space:]]*"[^"]*"' "$p" | sed 's/^/start.json: /'
    } > "$o/source.txt"

    local notes=("$kind")
    if [[ -n $dev ]]; then
        if [[ $kind == "Leandro VFIO" ]]; then
            grep -a 'vfio-user-nvgpu: config:' "$dev" > "$o/config.txt"
            if [[ -s $o/config.txt ]]; then
                notes+=("config $(wc -l < "$o/config.txt") lines")
            else
                { echo "# no config: block (a device binary before the settings table); the first 40 lines:"; head -n 40 "$dev"; } > "$o/config.txt"
                notes+=("no config block")
            fi
            if [[ -n $alive ]]; then
                echo "# the device is running (pid $alive): no exit summary yet" > "$o/exit-summary.txt"
                notes+=("device running")
            else
                start=$(awk '/Connection closed/ { c = NR } /soak exit:/ { s = NR } END { print (s ? (c && c < s ? c + 1 : s) : 0) }' "$dev")
                if ((start > 0)); then
                    tail -n +"$start" "$dev" | head -n 5000 > "$o/exit-summary.txt"
                    notes+=("exit summary $(wc -l < "$o/exit-summary.txt") lines")
                else
                    echo "# no exit summary: the device was killed or crashed before it wrote one (see the log tail)" > "$o/exit-summary.txt"
                    notes+=("no exit summary")
                fi
            fi
        else
            { echo "# the backend's start lines (it writes no exit summary; see the log tail)"; head -n 20 "$dev"; } > "$o/config.txt"
        fi
        {
            echo "# $dev: ERROR, WARN, panic and fail-closed lines, first occurrence with count"
            grep -aE 'ERROR|WARN|panicked|fail[- ]closed|FAIL_CLOSED' "$dev" | dedupe | head -n 300
            if [[ -n $vmm ]]; then
                echo "# $vmm: error, warn and panic lines"
                grep -aiE 'error|warn|panic' "$vmm" | dedupe | head -n 100
            fi
        } > "$o/errors.txt"
        notes+=("$(grep -acE 'ERROR|WARN|panicked|fail[- ]closed|FAIL_CLOSED' "$dev") error or warning lines")
    fi
    n=0
    for f in "$dev" "$vmm" "$(vm_file serial.log)" "$(vm_file input.log)" "$(vm_file load.log)"; do
        [[ -n $f && -f $f ]] || continue
        if [[ $f == "$dev" ]] && ((start > 1)); then
            # The device log's tail ends where its exit summary begins.
            { echo "# the last $NLINES lines before the exit summary (line $start)"; head -n $((start - 1)) "$f" | tail -n "$NLINES"; } > "$o/$(basename "$f").tail"
        else
            tail -n "$NLINES" "$f" > "$o/$(basename "$f").tail"
        fi
        n=$((n + 1))
    done
    notes+=("$n tails")
    for f in env.json start.json launch.log up.log checkpoint.json; do
        p=$(vm_file "$f")
        [[ -n $p ]] && head -c 262144 "$p" > "$o/$f"
    done
    if [[ -f $S/klog.matched ]]; then
        pids=$(vm_pids | awk '$1 == "gpu" || $1 == "backend" { print $2 }' | sort -u | paste -sd'|')
        {
            echo "# host Xid lines of the device (pid ${pids:-unknown}), window: $(cat "$S/klog.source" 2>/dev/null)"
            [[ -n $pids ]] && grep -aE "Xid.*pid=($pids)([^0-9]|$)" "$S/klog.matched"
        } > "$o/xid.txt"
        notes+=("$(($(wc -l < "$o/xid.txt") - 1)) Xid")
    else
        echo "# no host kernel log (see the host-klog step)" > "$o/xid.txt"
    fi
    printf -v n '%s, ' "${notes[@]}"
    echo "vm-$i: ${n%, }"
}

vm_trace() {
    local i=$1 D=$2 RUN o=$R/vm-$1 tr size
    vm_resolve
    tr=$(vm_file gpu-trace.log)
    [[ -n $tr ]] || { echo "no gpu-trace.log (tracing off, or Leandro VirtIO)"; return 3; }
    mkdir -p "$o"
    size=$(stat -c %s "$tr")
    if ((size > TRACE_BYTES)); then
        tail -c "$TRACE_BYTES" "$tr" | tail -n +2 > "$o/gpu-trace.tail"   # drop the cut first line
    else
        cat "$tr" > "$o/gpu-trace.tail"
    fi
    echo "vm-$i/gpu-trace.tail: the last $(stat -c %s "$o/gpu-trace.tail") of $size bytes (compressed at packing)"
}

# The guest scripts. Linux: POSIX sh; Windows: PowerShell, sent as -EncodedCommand.
GUEST_LINUX='echo "== hostname"; uname -n
echo "== uname -a"; uname -a
echo "== os"; (. /etc/os-release 2>/dev/null; echo "${PRETTY_NAME:-unknown}")
echo "== driver"; cat /proc/driver/nvidia/version 2>&1; lsmod 2>/dev/null | grep -E "^(nvidia|nouveau|nova|virtio_nvrm|nvrm_nodes)"
echo "== nvidia-smi -q"; smi=$(command -v nvidia-smi || echo /opt/nvrm/bin/nvidia-smi)
if [ -x "$smi" ]; then timeout 45 "$smi" -q 2>&1; else echo "no nvidia-smi"; fi
echo "== kernel log: NVRM, Xid, nvidia, nouveau, nova (the last 500)"
log=$(dmesg 2>/dev/null || sudo -n dmesg 2>/dev/null || journalctl -k -b -q --no-pager 2>/dev/null)
if [ -n "$log" ]; then printf "%s\n" "$log" | grep -aE "NVRM|Xid|nvidia|nouveau|nova|virtio_nvrm" | tail -n 500; else echo "kernel log not readable"; fi'

GUEST_WINDOWS='$ErrorActionPreference = "SilentlyContinue"
$since = (Get-Date).AddHours(-24)
function L($e) { $m = ($e.Message -replace "\s+", " "); if ($m.Length -gt 600) { $m = $m.Substring(0, 600) }
  "{0:s} {1} id {2} {3}: {4}" -f $e.TimeCreated, $e.ProviderName, $e.Id, $e.LevelDisplayName, $m }
"== hostname"; $env:COMPUTERNAME
"== user"; $env:USERNAME
"== os"; (Get-CimInstance Win32_OperatingSystem | Format-List Caption, Version, BuildNumber, LastBootUpTime | Out-String).Trim()
"== display adapters and driver versions"
(Get-CimInstance Win32_VideoController | Format-List Name, DriverVersion, DriverDate, Status, ConfigManagerErrorCode, PNPDeviceID | Out-String).Trim()
"== display-driver events, System log, 24 h"
foreach ($p in "nvlddmkm", "Display", "Microsoft-Windows-DxgKrnl", "Microsoft-Windows-WER-SystemErrorReporting") {
  $ev = @(Get-WinEvent -FilterHashtable @{ LogName = "System"; ProviderName = $p; StartTime = $since } -MaxEvents 200)
  "-- ${p}: $($ev.Count) event(s)"; $ev | ForEach-Object { L $_ } }
"-- Kernel-Power 41, a restart without a clean shutdown"
Get-WinEvent -FilterHashtable @{ LogName = "System"; ProviderName = "Microsoft-Windows-Kernel-Power"; Id = 41; StartTime = $since } | ForEach-Object { L $_ }
"== Windows Error Reporting, Application log, 24 h: LiveKernelEvent, BlueScreen, display"
Get-WinEvent -FilterHashtable @{ LogName = "Application"; ProviderName = "Windows Error Reporting"; StartTime = $since } -MaxEvents 300 |
  Where-Object { $_.Message -match "LiveKernelEvent|BlueScreen|nvlddmkm|dxgkrnl|Display" } | ForEach-Object { L $_ }
"== LiveKernelReports and WER report folders, 24 h"
foreach ($d in "$env:SystemRoot\LiveKernelReports", "$env:ProgramData\Microsoft\Windows\WER\ReportArchive", "$env:ProgramData\Microsoft\Windows\WER\ReportQueue") {
  Get-ChildItem $d -Recurse | Where-Object { $_.LastWriteTime -gt $since } | ForEach-Object { "{0:s} {1,12} {2}" -f $_.LastWriteTime, $_.Length, $_.FullName } }
exit 0'

guest() {
    local j=$1 target=$2 o=$R/guest-$1 os rc enc extra=()
    have ssh || { echo "ssh missing"; return 3; }
    local opts=(-n -o BatchMode=yes -o ConnectTimeout=5 -o StrictHostKeyChecking=no
                -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR)
    [[ -n $KEY ]] && opts+=(-i "$KEY")
    [[ -n ${SSH_EXTRA:-} ]] && read -ra extra <<< "$SSH_EXTRA" && opts+=("${extra[@]}")
    os=$(t 20 ssh "${opts[@]}" "$target" uname -s 2> "$S/ssh-$j.err"); rc=$?
    if ((rc == 255 || rc == 124 || rc == 137)); then
        echo "guest-$j unreachable (ssh exit $rc): $(tr -d '\r' < "$S/ssh-$j.err" | grep -v '^$' | head -n 1)"
        return 1
    fi
    mkdir -p "$o"
    echo "target: $target" > "$o/source.txt"
    if [[ $os == Linux* ]]; then
        echo "system: Linux" >> "$o/source.txt"
        t 120 ssh "${opts[@]}" "$target" "$GUEST_LINUX" > "$o/guest.txt" 2>&1; rc=$?
    else
        if ! have iconv || ! have base64; then echo "guest-$j: iconv or base64 missing for the PowerShell command"; return 1; fi
        echo "system: Windows (PowerShell)" >> "$o/source.txt"
        enc=$(printf '%s' "$GUEST_WINDOWS" | iconv -f UTF-8 -t UTF-16LE | base64 -w0)
        t 150 ssh "${opts[@]}" "$target" "powershell -NoProfile -NonInteractive -EncodedCommand $enc" > "$o/guest.txt" 2>&1; rc=$?
        sed -i 's/\r$//' "$o/guest.txt"
    fi
    # The guest's own names join the identities --redact replaces (word-bounded).
    sed -n '/^== hostname$/{n;p}; /^== user$/{n;p}' "$o/guest.txt" | tr -d '\r' >> "$S/ids.guest"
    ((rc == 0)) || { echo "guest-$j: the collection exited $rc (partial guest.txt)"; return 1; }
    echo "guest-$j: $(sed -n 's/^system: //p' "$o/source.txt"), $(wc -l < "$o/guest.txt") lines"
}

if [[ ${1:-} == --step ]]; then
    shift
    case ${1:-} in
        host_system|host_pci|host_nvidia|host_vmm|host_klog|host_resources|repo_state|binaries|vm_logs|vm_trace|guest) "$@"; exit $? ;;
        *) exit 2 ;;
    esac
fi

# ---- the run ---------------------------------------------------------------------------------

MANIFEST_LINES=()
step() {  # NAME LIMIT FUNCTION ARGS...
    local name=$1 limit=$2 t0=$SECONDS rc st note log=$S/step-$1.log
    shift 2
    timeout -k 5 "$limit" bash "$SELF" --step "$@" > "$log" 2>&1 < /dev/null; rc=$?
    case $rc in 0) st=ok ;; 3) st=skipped ;; 124|137) st=timeout ;; *) st=failed ;; esac
    note=$(grep -v '^[[:space:]]*$' "$log" | tail -n 1)
    [[ $st == timeout ]] && note="stopped after ${limit} s${note:+; $note}"
    printf '  %-8s %-14s %3s s\n' "$st" "$name" "$((SECONDS - t0))"
    MANIFEST_LINES+=("$(printf '%-8s %-14s %4s s  %s' "$st" "$name" "$((SECONDS - t0))" "$note")")
}
skip() {  # NAME REASON
    printf '  %-8s %-14s\n' skipped "$1"
    MANIFEST_LINES+=("$(printf '%-8s %-14s %4s s  %s' skipped "$1" 0 "$2")")
}

# The host's identities for --redact: ids.sub are replaced anywhere, ids.word word-bounded.
collect_ids() {
    local u name w h
    local users=() hosts=()
    if [[ -n ${LEANDRO_REPORT_USER:-} ]]; then users=("$LEANDRO_REPORT_USER")
    else users=("$(id -un)"); [[ -n ${SUDO_USER:-} ]] && users+=("$SUDO_USER"); fi
    if [[ -n ${LEANDRO_REPORT_HOST:-} ]]; then hosts=("$LEANDRO_REPORT_HOST")
    else hosts=("$(uname -n)"); fi
    : > "$S/ids.sub"; : > "$S/ids.word"
    add_id() {
        local v=$1
        [[ -n $v && $COMMON_NAMES != *" ${v,,} "* ]] || return 0
        if ((${#v} >= 4)); then echo "$v" >> "$S/ids.sub"; else echo "$v" >> "$S/ids.word"; fi
    }
    for u in "${users[@]}"; do
        add_id "$u"
        if [[ -n ${LEANDRO_REPORT_USER:-} ]]; then name=${LEANDRO_REPORT_NAME:-}
        else name=$(getent passwd "$u" 2>/dev/null | cut -d: -f5 | cut -d, -f1); fi
        add_id "$name"
        for w in $name; do add_id "$w"; done
    done
    for h in "${hosts[@]}"; do add_id "$h"; add_id "${h%%.*}"; done
}

# One sed script for every file: credentials and e-mail always, the rest with --redact.
scrub_script() {
    local f=$S/scrub.sed id esc
    printf 's/^.*(%s).*$/<line removed: credential keyword>/I\n' "$CRED_RE" > "$f"
    printf 's/%s/<email>/g\n' "$MAIL_RE" >> "$f"
    ((REDACT)) || return 0
    cat >> "$f" <<'SED'
s/\b[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}\b/<uuid>/g
s/\b([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}\b/<mac>/g
s/\b([0-9A-Fa-f]{2}-){5}[0-9A-Fa-f]{2}\b/<mac>/g
s/\b([0-9A-Fa-f]{1,4}:){7}[0-9A-Fa-f]{1,4}\b/<ipv6>/g
s/\b([0-9A-Fa-f]{1,4}:){1,6}(:[0-9A-Fa-f]{1,4}){1,6}\b/<ipv6>/g
s#/home/[^/[:space:]"']+#/home/<user>#g
s#([A-Za-z]:(\\)+Users(\\)+)[^\\[:space:]"']+#\1<user>#g
s/^([[:space:]]*(Serial Number|Board Serial Number|Chassis Serial Number|GPU Fabric GUID)[[:space:]]*:).*$/\1 <redacted>/
/^[[:space:]]{8,}Name[[:space:]]+:.*(vfio-user-nvgpu|vhost-user-nvrm|cloud-hypervisor|qemu)/!s/^([[:space:]]{8,}Name[[:space:]]+:).*$/\1 <process>/
SED
    printf 's/%s/<ip>/g\n' "$IPV4_RE" >> "$f"
    # Longest first, so that a full name goes before its parts.
    while IFS= read -r id; do
        esc=$(printf '%s' "$id" | sed -E 's/[][\.*^$/+?(){}|]/\\&/g')
        printf 's/%s/<id>/gI\n' "$esc" >> "$f"
    done < <(awk '{ print length, $0 }' "$S/ids.sub" | sort -rn | cut -d' ' -f2- | awk 'NF && !s[$0]++')
    while IFS= read -r id; do
        esc=$(printf '%s' "$id" | sed -E 's/[][\.*^$/+?(){}|]/\\&/g')
        printf 's/\\b%s\\b/<id>/gI\n' "$esc" >> "$f"
    done < <(cat "$S/ids.word" "$S/ids.guest" 2>/dev/null | tr -d '\r' | awk -v c="$COMMON_NAMES" 'NF && index(c, " " tolower($0) " ") == 0 && !s[$0]++')
}

# Files that still hold what the scrub removes: "<kind> <file>"; empty when clean.
residual() {
    local id
    grep -rlaE "$MAIL_RE" "$R" | sed 's/^/e-mail address: /'
    grep -rlaiE "$CRED_RE" "$R" | sed 's/^/credential keyword: /'
    ((REDACT)) || return 0
    grep -rlaE "$IPV4_RE" "$R" | sed 's/^/IP address: /'
    while IFS= read -r id; do
        [[ -n $id ]] && grep -rlaiF -- "$id" "$R" | sed 's/^/identity: /'
    done < "$S/ids.sub"
    while IFS= read -r id; do
        [[ -n $id && $COMMON_NAMES != *" ${id,,} "* ]] && grep -rlaiwF -- "$id" "$R" | sed 's/^/identity: /'
    done < <(cat "$S/ids.word" "$S/ids.guest" 2>/dev/null | tr -d '\r')
}

run() {
    local vms=() guests=() arg
    SINCE="2 hours ago"; REDACT=0; KEY=""; NLINES=300; TRACE_MB=20; OUT=""
    while (($#)); do
        arg=$1
        case $arg in
            --vm) vms+=("${2:?--vm needs a directory}"); shift 2 ;;
            --guest) guests+=("${2:?--guest needs an ssh target}"); shift 2 ;;
            --since) SINCE=${2:?--since needs a time}; shift 2 ;;
            --redact) REDACT=1; shift ;;
            --key) KEY=${2:?--key needs a file}; shift 2 ;;
            --lines) NLINES=${2:?}; shift 2 ;;
            --trace-mb) TRACE_MB=${2:?}; shift 2 ;;
            -o) OUT=${2:?-o needs a file}; shift 2 ;;
            -h|--help) usage 0 ;;
            *) printf 'leandro-report: unknown argument %s\n' "$arg" >&2; usage 2 ;;
        esac
    done
    [[ -n $OUT ]] || usage 2
    [[ $OUT == *.tar.zst ]] || die "-o must name a .tar.zst file"
    [[ $NLINES =~ ^[0-9]+$ && $TRACE_MB =~ ^[0-9]+$ ]] || die "--lines and --trace-mb take a number"
    have timeout || die "timeout (coreutils) missing"
    have zstd || die "zstd missing (the tarball is tar with zstd)"
    TRACE_BYTES=${LEANDRO_REPORT_TRACE_BYTES:-$((TRACE_MB << 20))}
    if [[ -z $KEY ]]; then
        # The launcher's LEA_VM_DIR: the environment, else local.env's default, else its own.
        local vmdir=${LEA_VM_DIR:-}
        [[ -z $vmdir && -f $LEA_ROOT/local.env ]] &&
            vmdir=$(sed -nE 's/.*LEA_VM_DIR:?=([^}"[:space:]]+).*/\1/p' "$LEA_ROOT/local.env" | head -n 1)
        vmdir=${vmdir:-/mnt/vmstore/leandro/vm}
        [[ -r $vmdir/id_leandro ]] && KEY=$vmdir/id_leandro
    fi
    OUTDIR=$(dirname "$OUT")
    mkdir -p "$OUTDIR" || die "cannot create $OUTDIR"
    OUTDIR=$(cd "$OUTDIR" && pwd -P); OUT=$OUTDIR/$(basename "$OUT")
    STAGE=$(mktemp -d "$OUTDIR/.leandro-report.XXXXXX") || die "cannot create a staging directory in $OUTDIR"
    trap 'rm -rf "$STAGE"' EXIT
    local top stamp
    stamp=$(date -u +%Y%m%dT%H%M%SZ); top=leandro-report-$stamp
    R=$STAGE/$top; S=$STAGE/.work
    mkdir -p "$R" "$S"
    VMS=""
    for arg in "${vms[@]}"; do VMS+=$(readlink -f "$arg" 2>/dev/null || echo "$arg")$'\n'; done
    KLOG_FIXTURE=${LEANDRO_REPORT_KLOG:-}; SSH_EXTRA=${LEANDRO_REPORT_SSH_EXTRA:-}
    local selftest=${LEANDRO_REPORT_SELFTEST:-0}
    export R S SINCE REDACT KEY NLINES TRACE_BYTES VMS OUTDIR KLOG_FIXTURE SSH_EXTRA

    echo "leandro-report: $TOOL"
    if [[ $EUID -eq 0 ]]; then
        echo "running as root${SUDO_USER:+ (sudo from $SUDO_USER)}: the same read-only steps as without it"
    fi
    collect_ids
    : > "$S/ids.guest"

    step host-system 20 host_system
    if ((selftest)); then
        skip host-pci "self-test"; skip host-nvidia "self-test"; skip host-vmm "self-test"
    else
        step host-pci 40 host_pci
        step host-nvidia 60 host_nvidia
        step host-vmm 40 host_vmm
    fi
    step host-klog 60 host_klog
    step host-resources 40 host_resources
    step repo 30 repo_state
    step binaries 180 binaries
    local i=0 D
    for D in "${vms[@]}"; do
        i=$((i + 1)); D=$(readlink -f "$D" 2>/dev/null || echo "$D")
        step "vm-$i-logs" 120 vm_logs "$i" "$D"
        step "vm-$i-trace" 120 vm_trace "$i" "$D"
    done
    i=0
    for arg in "${guests[@]}"; do
        i=$((i + 1))
        step "guest-$i" 180 guest "$i" "$arg"
    done

    local what="e-mail addresses replaced, credential lines dropped"
    ((REDACT)) && what+="; host and guest names, user names, home directories, IP and MAC addresses, UUIDs, serial numbers and other GPU processes' names replaced"
    {
        echo "# $TOOL"
        echo "created: $stamp"
        echo "run as: $([[ $EUID -eq 0 ]] && echo "root${SUDO_USER:+ via sudo}, the same read-only steps" || echo "a normal user, no root")"
        echo "kernel log: $(cat "$S/klog.source" 2>/dev/null || echo none)"
        echo "vms: ${#vms[@]}, guests: ${#guests[@]}, since: $SINCE, tails: $NLINES lines, trace cap: $((TRACE_BYTES >> 10)) KiB"
        echo "redaction: $what"
        echo
        echo "status   step           time  note"
        printf '%s\n' "${MANIFEST_LINES[@]}"
    } > "$R/MANIFEST.txt"

    # Scrub every file in place, then refuse to pack while anything remains.
    scrub_script
    local f left
    while IFS= read -r -d '' f; do
        sed -E -i -f "$S/scrub.sed" "$f" || die "scrub failed on ${f#"$R"/}"
    done < <(find "$R" -type f -print0)
    left=$(residual | sed "s#$R/##" | sort -u)
    if [[ -n $left ]]; then
        printf 'leandro-report: the scrub left identifiers; no tarball written:\n%s\n' "$left" >&2
        exit 1
    fi
    for f in "$R"/vm-*/gpu-trace.tail; do
        [[ -f $f ]] && { zstd -q -T0 -10 --rm "$f" -o "$f.zst" || die "zstd failed on ${f#"$R"/}"; }
    done
    {
        echo "scrub    ok: $what; residual check clean"
        echo
        echo "files:"
        (cd "$R" && find . -type f ! -name MANIFEST.txt -printf '%10s  %P\n' | sort -k2)
    } >> "$R/MANIFEST.txt"

    tar -C "$STAGE" --owner=0 --group=0 --numeric-owner -cf - "$top" | zstd -q -T0 -10 -f -o "$OUT.part" \
        || die "packing failed"
    mv -f "$OUT.part" "$OUT"
    (cd "$OUTDIR" && sha256sum "$(basename "$OUT")" > "$(basename "$OUT").sha256")
    cp "$R/MANIFEST.txt" "${OUT%.tar.zst}.manifest.txt"
    if [[ $EUID -eq 0 && -n ${SUDO_UID:-} ]]; then
        chown "$SUDO_UID:${SUDO_GID:-$SUDO_UID}" "$OUT" "$OUT.sha256" "${OUT%.tar.zst}.manifest.txt"
    fi

    echo
    sed -n '/^status /,/^files:/p' "$R/MANIFEST.txt" | sed '$d'
    echo "tarball: $OUT ($(du -h "$OUT" | cut -f1), $(find "$R" -type f | wc -l) files)"
    echo "sha256:  $(cut -d' ' -f1 "$OUT.sha256") ($OUT.sha256)"
    echo "manifest: ${OUT%.tar.zst}.manifest.txt"
    echo "Check it before sending (docs/reporting.md); it holds logs of this machine and its VMs."
}

# ---- self-test ---------------------------------------------------------------------------------

self_test() {
    have zstd || { echo "SKIP: zstd missing"; exit 77; }
    local fx=$LEA_ROOT/tools/testdata/leandro-report tmp k pass=0 fail=0
    [[ -d $fx/vm ]] || { echo "no fixture at $fx"; exit 1; }
    tmp=$(mktemp -d "${TMPDIR:-/tmp}/leandro-report-test.XXXXXX") || exit 1
    SELFTEST_TMP=$tmp
    trap 'rm -rf "$SELFTEST_TMP"' EXIT
    cp -r "$fx/vm" "$tmp/vm"
    # A trace longer than the test's cap of 16 KiB, with an e-mail address in its last line.
    for ((k = 0; k < 4000; k++)); do printf 'bar0 R 0x%06x = 0x%08x\n' $((k * 4)) "$k"; done > "$tmp/vm/gpu-trace.log"
    echo "trace end: a note from fixtureuser@example.org" >> "$tmp/vm/gpu-trace.log"
    check() {  # DESCRIPTION COMMAND...
        local d=$1; shift
        if "$@" > /dev/null 2>&1; then pass=$((pass + 1)); else fail=$((fail + 1)); echo "FAIL: $d"; fi
    }
    absent() { ! grep -rqaiE -- "$1" "$2"; }
    collect() {  # LABEL ARGS...
        local label=$1; shift
        LEANDRO_REPORT_SELFTEST=1 LEANDRO_REPORT_KLOG=$fx/kernel.log LEANDRO_REPORT_TRACE_BYTES=16384 \
        LEANDRO_REPORT_USER=fixtureuser LEANDRO_REPORT_NAME="Fixture Person" \
        LEANDRO_REPORT_HOST=fixturehost.example.lan LEANDRO_REPORT_SSH_EXTRA="-F /dev/null -o ProxyCommand=false" \
            bash "$SELF" --vm "$tmp/vm" --guest tester@guest.invalid --lines 50 "$@" \
            -o "$tmp/$label.tar.zst" > "$tmp/$label.out" 2>&1
        local rc=$?
        check "$label: exit 0 (rc $rc)" test "$rc" -eq 0
        mkdir -p "$tmp/$label"
        check "$label: sha256 matches" bash -c 'cd "$1" && sha256sum -c "$2.tar.zst.sha256"' _ "$tmp" "$label"
        zstd -dc "$tmp/$label.tar.zst" 2> /dev/null | tar -C "$tmp/$label" -xf - 2> /dev/null
        U=$(echo "$tmp/$label"/leandro-report-*)
        : > "$tmp/$label.trace"
        [[ -f $U/vm-1/gpu-trace.tail.zst ]] && zstd -dc "$U/vm-1/gpu-trace.tail.zst" > "$tmp/$label.trace"
        check "$label: MANIFEST.txt beside the tarball" test -s "$tmp/$label.manifest.txt"
    }

    collect plain
    local M=$U/MANIFEST.txt
    check "plain: MANIFEST.txt" test -s "$M"
    check "plain: vm-1-logs ok" grep -qE '^ok +vm-1-logs' "$M"
    check "plain: vm-1-trace ok" grep -qE '^ok +vm-1-trace' "$M"
    check "plain: host-klog ok" grep -qE '^ok +host-klog' "$M"
    check "plain: host-nvidia skipped" grep -qE '^skipped +host-nvidia' "$M"
    check "plain: an unreachable guest is noted, the run goes on" grep -qE '^(failed +guest-1 .*unreachable|skipped +guest-1 .*ssh missing)' "$M"
    check "plain: scrub line" grep -qE '^scrub +ok' "$M"
    check "plain: config block" test "$(grep -c 'config:' "$U/vm-1/config.txt")" -eq 3
    check "plain: exit summary starts after the close" grep -q '^vfio-user-nvgpu: device lock holds' "$U/vm-1/exit-summary.txt"
    check "plain: exit summary has the soak line" grep -q 'soak exit:' "$U/vm-1/exit-summary.txt"
    check "plain: WARNING line" grep -q 'WARNING guest registry' "$U/vm-1/errors.txt"
    check "plain: ERROR line counted" grep -qE '^ +2  vfio-user-nvgpu: ERROR' "$U/vm-1/errors.txt"
    check "plain: Xid of the device's pid" grep -q 'pid=4999999,' "$U/vm-1/xid.txt"
    check "plain: no Xid of another pid" absent 'pid=4999998' "$U/vm-1/xid.txt"
    check "plain: tails" test -s "$U/vm-1/serial.log.tail" -a -s "$U/vm-1/ch.log.tail" -a -s "$U/vm-1/gpu.log.tail"
    check "plain: env.json and launch.log" test -s "$U/vm-1/env.json" -a -s "$U/vm-1/launch.log"
    check "plain: trace tail capped" test "$(stat -c %s "$tmp/plain.trace")" -le 16384
    check "plain: trace tail ends with the last line" grep -q '^trace end: a note from <email>$' "$tmp/plain.trace"
    check "plain: no e-mail address" absent "$MAIL_RE" "$tmp/plain"
    check "plain: no e-mail address in the trace" absent "$MAIL_RE" "$tmp/plain.trace"
    check "plain: no credential value" absent 'hunter2' "$tmp/plain"
    check "plain: no auto-logon line" absent 'autoadminlogon' "$tmp/plain"
    check "plain: doorbell tokens kept" grep -q 'CARAXES_DOORBELL_TOKENS' "$U/vm-1/env.json"
    check "plain: host name kept without --redact" grep -rq 'fixturehost' "$U"

    collect redacted --redact
    check "redacted: scrub line" grep -qE '^scrub +ok' "$U/MANIFEST.txt"
    for k in fixtureuser fixturehost 'Fixture Person' '192\.168\.77\.5' '10\.0\.2\.15' '02:00:00:00:00:99' \
             '0b6c1a4e-2f3d-4c5b-8a9e-1f2e3d4c5b6a' 'fe80::1' 'hunter2' "$MAIL_RE"; do
        check "redacted: no $k" absent "$k" "$tmp/redacted"
        check "redacted: no $k in the trace" absent "$k" "$tmp/redacted.trace"
    done
    check "redacted: placeholders" grep -rqF '/home/<user>/' "$U"
    check "redacted: <id>" grep -rqF '<id>' "$U"
    check "redacted: <ip>" grep -rqF '<ip>' "$U"
    check "redacted: PCI address kept" grep -rqF '0000:01:00' "$U"
    check "redacted: version kept" grep -rqF '615.71.09' "$U"
    check "redacted: tar entries owned by 0/0" test "$(zstd -dc "$tmp/redacted.tar.zst" 2> /dev/null | tar -tvf - 2> /dev/null | awk '{ print $2 }' | sort -u)" = 0/0

    echo "self-test: $pass passed, $fail failed"
    ((fail == 0))
}

case ${1:-} in
    --self-test) self_test ;;
    '') usage 2 ;;
    *) run "$@" ;;
esac
