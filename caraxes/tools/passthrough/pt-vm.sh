#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# R4 VM runner (docs/caraxes/experiments.md, R4): the Windows 11 guest of the Caraxes runs
# (and the Linux 615 guest of pass C) with the real RTX 2070 through vfio-pci. Runs as the
# operator, not root, after `sudo pt-host.sh to-vfio` and, once in the tmux shell,
# `sudo prlimit --pid $$ --memlock=unlimited:unlimited`. Every VM start is the operator's.
#
#   pt-vm.sh prepare win|linux              R4 copies: disk overlay(s), OVMF vars, TPM state
#   pt-vm.sh preflight win|linux|win-ch A|B checks only
#   pt-vm.sh cmdline win|linux|win-ch A|B   print the full VM command line
#   pt-vm.sh start win|linux|win-ch A|B NAME  start it; run folder $PT_DIR/runs/NAME
#   pt-vm.sh trace on|off                   pass B: the per-access events on or off (QMP)
#   pt-vm.sh mark TEXT                      host-time marker (qemutrace2gpu.py --markers)
#   pt-vm.sh status                         VM, MSIs/s, CPU, memory, trace size
#   pt-vm.sh stop                           guest shutdown, then ACPI, then quit; logs kept
#   pt-vm.sh qmp '{"execute":...}'          one raw QMP command
#
# Pass A: plain vfio-pci (BARs mmapped, MSIs and the quirks' ioeventfds in KVM), trace-A.events.
# Pass B: .0 with x-no-mmap=on (every BAR access traps to QEMU), x-no-kvm-msi=on (each MSI
#         passes QEMU: vfio_msi_interrupt), x-no-kvm-ioeventfd=on (the MSI re-arm writes to the
#         config mirror 0x88704 stay traced); trace-B.events from boot (PT_TRACE_FROM=boot) or
#         from `trace on` (PT_TRACE_FROM=qmp).
# win-ch: the same disk under Cloud Hypervisor with the card (--device), pass A only: the
#         failing runs' hypervisor with the real card (control).
# Env: PT_DIR (/mnt/vmstore/caraxes/r4), PT_MEM (8G), PT_CPUS (win 4, linux 8), PT_ROMFILE
#      (romfile= for .0), PT_TRACE_FROM (boot), PT_STOP_WAIT (300 s), PT_CH (CH binary).
set -euo pipefail

ME=$(readlink -f "$0")
HERE=$(dirname "$ME")
R4=${PT_DIR:-/mnt/vmstore/caraxes/r4}
KIT=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain
WIN_BASE=$KIT/win11.qcow2
DESK=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b
LEA_VM=/mnt/vmstore/leandro/vm
CODE=/usr/share/edk2/x64/OVMF_CODE.secboot.4m.fd
CH=${PT_CH:-/mnt/vmstore/caraxes/ch-readonly-mmap/target/release/cloud-hypervisor}
CH_FW=/mnt/vmstore/caraxes/win11/CLOUDHV.fd
FUNCS=(0000:2d:00.0 0000:2d:00.1 0000:2d:00.2 0000:2d:00.3)
GROUP=28
SYS=/sys/bus/pci/devices
MEM=${PT_MEM:-8G}
MEM_GIB=${MEM%G}
WIN_KEY=$KIT/CH-build/id_win
WIN_IP=192.168.107.2
LIN_KEY=$LEA_VM/id_leandro
LIN_IP=192.168.106.26
HOT=(vfio_region_read vfio_region_write vfio_pci_read_config vfio_pci_write_config vfio_msi_interrupt)
SSH_OPTS=(-F /dev/null -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR
          -o ConnectTimeout=5 -o BatchMode=yes)

die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
say() { printf '%s %s\n' "$(date +%T)" "$*"; }
drv_of() { local l; l=$(readlink "$SYS/$1/driver" 2>/dev/null) && basename "$l"; }
alive() { [[ -n ${1:-} && -e /proc/$1 ]] && [[ $(awk '/^State:/{print $2}' "/proc/$1/status") != Z ]]; }
stamp() { stat -c '%s %Y' "$1"; }
iso_now() { date -u +%Y-%m-%dT%H:%M:%S.%6NZ; }
shq() {  # print a command line that pastes back into a shell
    local a out=()
    for a in "$@"; do
        if [[ $a =~ ^[A-Za-z0-9_./:=,@+%-]+$ ]]; then out+=("$a"); else out+=("'${a//\'/\'\\\'\'}'"); fi
    done
    echo "${out[*]}"
}
current() { [[ -f $R4/current ]] || die "no run yet ($R4/current)"; echo "$R4/runs/$(cat "$R4/current")"; }
mem_avail_gib() { awk '/^MemAvailable/{printf "%d", $2 / 1048576}' /proc/meminfo; }
free_gib() { df --output=avail -BG "$R4" | tail -1 | tr -dc 0-9; }

qmp() {  # qmp SOCKET JSON...: send each command after the handshake, print the replies
    python3 - "$@" <<'PY'
import json, socket, sys
s = socket.socket(socket.AF_UNIX)
s.settimeout(60)
s.connect(sys.argv[1])
f = s.makefile('rw')
def reply():
    while True:
        m = json.loads(f.readline())
        if 'event' not in m:
            return m
reply()
rc = 0
for c in ['{"execute": "qmp_capabilities"}'] + sys.argv[2:]:
    f.write(c + '\n')
    f.flush()
    r = reply()
    if 'qmp_capabilities' not in c:
        print(json.dumps(r))
    rc |= 'error' in r
sys.exit(rc)
PY
}

check_base() {  # check_base DIR BASE: the overlay's base must be unchanged since prepare
    [[ -f $1/base.stamp ]] || die "$1 not prepared: $ME prepare"
    [[ $(stamp "$2") == "$(cat "$1/base.stamp")" ]] ||
        die "$2 changed since prepare (a Caraxes run wrote it): rm -r $1 and prepare again"
}

prepare() {
    case $1 in
    win)
        local w=$R4/win
        if [[ -f $w/r4.qcow2 ]]; then check_base "$w" "$WIN_BASE"; say "ok (already): $w"; return; fi
        pgrep -f "swtpm socket.*$KIT/swtpm" >/dev/null && die "the kit's swtpm runs (a Caraxes Windows VM is up): stop it first"
        [[ -e $KIT/ch.pid || -e $KIT/qemu.pid ]] && die "$KIT has a ch.pid/qemu.pid: run44.py stop first"
        mkdir -p "$w/swtpm"
        say "overlay $w/r4.qcow2 on $WIN_BASE (the base stays as the Caraxes runs left it)"
        qemu-img create -q -f qcow2 -F qcow2 -b "$WIN_BASE" "$w/r4.qcow2"
        cp "$KIT/OVMF_VARS.4m.fd" "$w/OVMF_VARS.4m.fd"
        cp "$KIT/swtpm/tpm2-00.permall" "$w/swtpm/"
        stamp "$WIN_BASE" > "$w/base.stamp"
        ;;
    linux)
        local l=$R4/linux
        if [[ -f $l/rootfs.qcow2 ]]; then check_base "$l" "$DESK/rootfs.qcow2"; say "ok (already): $l"; return; fi
        [[ -e $DESK/ch.pid ]] && alive "$(cat "$DESK/ch.pid")" && die "desktop-b runs: bring it down first"
        mkdir -p "$l"
        say "overlays $l/rootfs.qcow2 and games.qcow2 on desktop-b's (Ubuntu 24.04, 615 open driver, GNOME)"
        qemu-img create -q -f qcow2 -F qcow2 -b "$DESK/rootfs.qcow2" "$l/rootfs.qcow2"
        qemu-img create -q -f qcow2 -F qcow2 -b "$DESK/games.qcow2" "$l/games.qcow2"
        stamp "$DESK/rootfs.qcow2" > "$l/base.stamp"
        ;;
    *) die "prepare win|linux" ;;
    esac
    ls -la "$R4/$1"
}

preflight() {
    local vm=$1 pass=$2 f l need
    [[ $vm == win || $vm == linux || $vm == win-ch ]] || die "vm is win, linux or win-ch"
    [[ $pass == A || $pass == B ]] || die "pass is A or B"
    [[ $vm == win-ch && $pass == B ]] && die "win-ch has no MMIO trace: pass A only"
    for f in "${FUNCS[@]}"; do
        [[ $(drv_of "$f") == vfio-pci ]] || die "$f is on '$(drv_of "$f")': sudo $HERE/pt-host.sh to-vfio"
    done
    [[ -r /dev/vfio/$GROUP && -w /dev/vfio/$GROUP ]] || die "/dev/vfio/$GROUP is not $(id -un)'s (pt-host.sh to-vfio chowns it)"
    l=$(ulimit -l)
    need=$(((MEM_GIB + 1) * 1048576))
    [[ $l == unlimited ]] || ((l >= need)) ||
        die "memlock ${l} KiB < ${need} KiB: in this shell run: sudo prlimit --pid \$\$ --memlock=unlimited:unlimited"
    (($(mem_avail_gib) >= MEM_GIB + 6)) || die "MemAvailable $(mem_avail_gib) GiB < ${MEM_GIB} + 6 GiB"
    pgrep -a '^(qemu-system-x86|cloud-hypervis|vfio-user-nvgp|vhost-user-nvr)' && die "another VM or GPU device runs (above)"
    if [[ -f $R4/current && -f $R4/runs/$(cat "$R4/current")/vm.pid ]]; then
        alive "$(cat "$R4/runs/$(cat "$R4/current")/vm.pid")" && die "run $(cat "$R4/current") is up: $ME stop"
    fi
    if [[ $vm == linux ]]; then
        ip link show caraxes6 >/dev/null 2>&1 || die "no tap caraxes6 (recreate it as for desktop-b)"
        check_base "$R4/linux" "$DESK/rootfs.qcow2"
        for f in "$LEA_VM/guest-image/vmlinuz" "$LEA_VM/guest-image/initrd" "$DESK/seed.img"; do [[ -f $f ]] || die "no $f"; done
    else
        ip link show caraxes7 >/dev/null 2>&1 || die "no tap caraxes7 (recreate it as for the Windows runs)"
        check_base "$R4/win" "$WIN_BASE"
        for f in "$CODE" "$R4/win/OVMF_VARS.4m.fd" "$R4/win/swtpm/tpm2-00.permall"; do [[ -f $f ]] || die "no $f"; done
        [[ $vm == win-ch ]] && { [[ -x $CH ]] || die "no $CH"; [[ -f $CH_FW ]] || die "no $CH_FW"; }
    fi
    local want=5
    [[ $pass == B ]] && want=20
    (($(free_gib) >= want)) || die "$R4 has $(free_gib) GiB free (pass $pass wants $want)"
    say "preflight ok: $vm pass $pass, MemAvailable $(mem_avail_gib) GiB, $R4 free $(free_gib) GiB, memlock $l"
}

build_cmd() {  # build_cmd VM PASS RUN -> the array CMD
    local vm=$1 pass=$2 run=$3 o0 ev
    local vfio=(-device pcie-root-port,id=rp1,bus=pcie.0,chassis=1,slot=1)
    o0="vfio-pci,host=${FUNCS[0]},bus=rp1,addr=00.0,multifunction=on"
    [[ $pass == B ]] && o0+=",x-no-mmap=on,x-no-kvm-msi=on,x-no-kvm-ioeventfd=on"
    [[ -n ${PT_ROMFILE:-} ]] && o0+=",romfile=$PT_ROMFILE"
    vfio+=(-device "$o0"
           -device "vfio-pci,host=${FUNCS[1]},bus=rp1,addr=00.1"
           -device "vfio-pci,host=${FUNCS[2]},bus=rp1,addr=00.2"
           -device "vfio-pci,host=${FUNCS[3]},bus=rp1,addr=00.3")
    ev=$HERE/trace-A.events
    [[ $pass == B && ${PT_TRACE_FROM:-boot} == boot ]] && ev=$HERE/trace-B.events
    local common=(-pidfile "$run/vm.pid" -qmp "unix:$run/qmp.sock,server=on,wait=off"
                  -display none -vga none -monitor none
                  -msg timestamp=on -D "$run/qemu-trace.log" -trace "events=$ev")
    case $vm in
    win)
        CMD=(qemu-system-x86_64 -name "r4-win-$pass" -enable-kvm -machine q35,accel=kvm,smm=on
             -cpu host,hv_relaxed,hv_vapic,hv_spinlocks=0x1fff,hv_time -smp "${PT_CPUS:-4}" -m "$MEM"
             -global driver=cfi.pflash01,property=secure,value=on
             -drive "if=pflash,format=raw,readonly=on,file=$CODE"
             -drive "if=pflash,format=raw,file=$R4/win/OVMF_VARS.4m.fd"
             -chardev "socket,id=chrtpm,path=$R4/win/swtpm/sock" -tpmdev emulator,id=tpm0,chardev=chrtpm
             -device tpm-crb,tpmdev=tpm0
             -drive "file=$R4/win/r4.qcow2,if=virtio,format=qcow2,cache=writeback,discard=unmap"
             -drive "file=$KIT/iso/virtio-win.iso,media=cdrom,readonly=on"
             -device virtio-net-pci,netdev=n0 -netdev "user,id=n0,hostfwd=tcp:127.0.0.1:3389-:3389"
             -device virtio-net-pci,netdev=n1,mac=02:00:00:00:00:47
             -netdev tap,id=n1,ifname=caraxes7,script=no,downscript=no
             -device virtio-serial-pci -chardev "socket,id=qga0,path=$run/qga.sock,server=on,wait=off"
             -device virtserialport,chardev=qga0,name=org.qemu.guest_agent.0
             -device qemu-xhci -device usb-tablet -device usb-kbd -rtc base=utc
             "${vfio[@]}" "${common[@]}")
        ;;
    linux)
        CMD=(qemu-system-x86_64 -name "r4-linux-$pass" -enable-kvm -machine q35,accel=kvm -cpu host
             -smp "${PT_CPUS:-8}" -m "$MEM"
             -kernel "$LEA_VM/guest-image/vmlinuz" -initrd "$LEA_VM/guest-image/initrd"
             -append "root=/dev/vda1 rw console=ttyS0 net.ifnames=0 modprobe.blacklist=caraxes_dma_range"
             -drive "file=$R4/linux/rootfs.qcow2,if=virtio,format=qcow2,cache=writeback,discard=unmap"
             -drive "file=$DESK/seed.img,if=virtio,format=raw,readonly=on"
             -drive "file=$R4/linux/games.qcow2,if=virtio,format=qcow2,cache=writeback"
             -device virtio-net-pci,netdev=n1,mac=02:00:00:00:00:26
             -netdev tap,id=n1,ifname=caraxes6,script=no,downscript=no
             -serial "file:$run/serial.log" -rtc base=utc
             "${vfio[@]}" "${common[@]}")
        ;;
    win-ch)
        CMD=("$CH" -v --api-socket "$run/ch.sock" --cpus "boot=${PT_CPUS:-4},kvm_hyperv=on"
             --memory "size=$MEM" --kernel "$CH_FW"
             --disk "path=$R4/win/r4.qcow2,image_type=qcow2,backing_files=on"
             --net "tap=caraxes7,mac=02:00:00:00:00:47" --rng src=/dev/urandom
             --tpm "socket=$R4/win/swtpm/sock" --serial "file=$run/serial.log" --console off
             --device "path=$SYS/${FUNCS[0]}/" --device "path=$SYS/${FUNCS[1]}/"
             --device "path=$SYS/${FUNCS[2]}/" --device "path=$SYS/${FUNCS[3]}/")
        ;;
    esac
}

swtpm_up() {
    local d=$R4/win/swtpm
    if [[ -f $d/swtpm.pid ]] && alive "$(cat "$d/swtpm.pid")"; then return; fi
    rm -f "$d/sock"
    swtpm socket --tpm2 --tpmstate "dir=$d" --ctrl "type=unixio,path=$d/sock" --daemon \
        --pid "file=$d/swtpm.pid" --log "file=$d/swtpm.log"
    sleep 0.5
    [[ -S $d/sock ]] || die "swtpm did not start ($d/swtpm.log)"
}

start() {
    local vm=$1 pass=$2 name=$3 run
    preflight "$vm" "$pass"
    run=$R4/runs/$name
    [[ -e $run ]] && die "$run exists: pick a new name"
    mkdir -p "$run"
    echo "$name" > "$R4/current"
    build_cmd "$vm" "$pass" "$run"
    [[ $vm == linux ]] || swtpm_up
    {
        echo "start $(iso_now) vm=$vm pass=$pass trace_from=${PT_TRACE_FROM:-boot} mem=$MEM"
        echo "host $(uname -r); $(qemu-system-x86_64 --version | head -1); tree $(git -C "$HERE" rev-parse --short HEAD 2>/dev/null)"
        shq "${CMD[@]}"
    } > "$run/start.txt"
    sudo -n dmesg 2>/dev/null | wc -l > "$run/dmesg.lines" || true
    lspci -nnk -s 2d:00 > "$run/lspci-before.txt" 2>&1 || true
    say "starting $vm pass $pass -> $run"
    if [[ $vm == win-ch ]]; then
        setsid nohup "${CMD[@]}" > "$run/ch.log" 2>&1 < /dev/null &
        echo $! > "$run/vm.pid"
    else
        setsid nohup "${CMD[@]}" > "$run/qemu.log" 2>&1 < /dev/null &
    fi
    local launcher=$! i
    for ((i = 0; i < 100; i++)); do
        [[ -s $run/vm.pid ]] && alive "$(cat "$run/vm.pid")" && break
        alive "$launcher" || { tail -n 20 "$run"/*.log; die "the VM exited at start"; }
        sleep 0.1
    done
    [[ -s $run/vm.pid ]] || die "no $run/vm.pid after 10 s"
    sleep 5   # VFIO pins the guest RAM and opens the group after the pid file is written
    alive "$(cat "$run/vm.pid")" || { tail -n 20 "$run"/*.log; die "the VM exited at start"; }
    setsid nohup "$ME" watch "$run" > /dev/null 2>&1 < /dev/null &
    echo $! > "$run/watch.pid"
    say "up: pid $(cat "$run/vm.pid"); host samples in $run/hostwatch.csv"
    if [[ $vm == linux ]]; then
        say "next: ssh -i $LIN_KEY leandro@$LIN_IP (after ~1 min)"
    else
        say "next: python3 $HERE/pt-guest.py wait   (SSH to $WIN_IP)"
    fi
}

watch() {  # the host sampler, 1 s: MSIs of 2d:00.0, VM CPU and RSS, memory, trace size, disk
    local run=$1 pid t prev_msi='' prev_ticks='' msi ticks rss avail tr free alerted=0 dm dt e
    pid=$(cat "$run/vm.pid")
    echo "t,msi_total,msi_per_s,vm_cpu_pct,vm_rss_mib,mem_available_mib,trace_mib,vmstore_free_gib" > "$run/hostwatch.csv"
    while alive "$pid"; do
        t=$(iso_now)
        msi=$(awk '/vfio-(msi|msix|intx).*0000:2d:00\.0/ {for (i = 2; i <= NF; i++) if ($i ~ /^[0-9]+$/) s += $i} END {print s + 0}' /proc/interrupts)
        ticks=$(awk '{print $14 + $15}' "/proc/$pid/stat" 2>/dev/null || echo 0)
        rss=$(awk '/^VmRSS/{printf "%d", $2 / 1024}' "/proc/$pid/status" 2>/dev/null || echo 0)
        avail=$(awk '/^MemAvailable/{printf "%d", $2 / 1024}' /proc/meminfo)
        tr=$(( $(stat -c %s "$run/qemu-trace.log" 2>/dev/null || echo 0) >> 20 ))
        free=$(free_gib)
        dm=0 dt=0
        [[ -n $prev_msi ]] && dm=$((msi - prev_msi)) dt=$((ticks - prev_ticks))
        echo "$t,$msi,$dm,$dt,$rss,$avail,$tr,$free" >> "$run/hostwatch.csv"
        prev_msi=$msi prev_ticks=$ticks
        if ((free < 10 && !alerted)); then
            echo "$t ALERT $R4 free ${free} GiB: per-access trace events off" >> "$run/alerts.txt"
            for e in "${HOT[@]}"; do
                qmp "$run/qmp.sock" "{\"execute\":\"trace-event-set-state\",\"arguments\":{\"name\":\"$e\",\"enable\":false}}" > /dev/null 2>&1 || true
            done
            alerted=1
        fi
        ((avail < 2048)) && echo "$t ALERT MemAvailable ${avail} MiB" >> "$run/alerts.txt"
        sleep 1
    done
}

trace() {
    local run on e
    run=$(current)
    [[ $1 == on || $1 == off ]] || die "trace on|off"
    [[ -S $run/qmp.sock ]] || die "no QMP socket (win-ch has no trace)"
    if [[ $1 == on ]]; then
        on=true
        mapfile -t ev < <(grep -vE '^\s*(#|$)' "$HERE/trace-B.events")
    else
        on=false
        ev=("${HOT[@]}")
    fi
    local cmds=()
    for e in "${ev[@]}"; do cmds+=("{\"execute\":\"trace-event-set-state\",\"arguments\":{\"name\":\"$e\",\"enable\":$on}}"); done
    qmp "$run/qmp.sock" "${cmds[@]}" > /dev/null
    mark "trace $1"
    say "trace $1 (${#ev[@]} event patterns); log $(du -h "$run/qemu-trace.log" | cut -f1)"
}

mark() {
    local run
    run=$(current)
    printf '%s %s\n' "$(iso_now)" "$*" >> "$run/markers.txt"
}

status() {
    local run pid
    run=$(current)
    pid=$(cat "$run/vm.pid" 2>/dev/null || true)
    say "run $run: $(alive "$pid" && echo "up, pid $pid" || echo down); $(head -1 "$run/start.txt")"
    say "host: $(tail -1 "$run/hostwatch.csv" 2>/dev/null)   (t,msi_total,msi_per_s,vm_cpu_pct,rss_mib,avail_mib,trace_mib,free_gib)"
    [[ -f $run/alerts.txt ]] && tail -n 3 "$run/alerts.txt"
    [[ -f $run/markers.txt ]] && tail -n 3 "$run/markers.txt"
    return 0
}

stop() {
    local run pid i vm
    run=$(current)
    pid=$(cat "$run/vm.pid" 2>/dev/null || true)
    vm=$(sed -n 's/.* vm=\([^ ]*\) .*/\1/p' "$run/start.txt")
    if alive "$pid"; then
        say "guest shutdown over SSH"
        if [[ $vm == linux ]]; then
            timeout 20 ssh "${SSH_OPTS[@]}" -i "$LIN_KEY" "leandro@$LIN_IP" 'sudo poweroff' || true
        else
            timeout 20 ssh "${SSH_OPTS[@]}" -i "$WIN_KEY" "$(cat "$KIT/CH-build/guest-user")@$WIN_IP" 'Stop-Computer -Force' || true
        fi
        for ((i = 0; i < ${PT_STOP_WAIT:-300}; i++)); do alive "$pid" || break; sleep 1; done
    fi
    if alive "$pid"; then
        say "still up: ACPI power button"
        if [[ $vm == win-ch ]]; then
            curl -s --unix-socket "$run/ch.sock" -X PUT http://localhost/api/v1/vm.power-button || true
        else
            qmp "$run/qmp.sock" '{"execute":"system_powerdown"}' || true
        fi
        for ((i = 0; i < 120; i++)); do alive "$pid" || break; sleep 1; done
    fi
    if alive "$pid"; then
        say "still up: quit (hard)"
        if [[ $vm == win-ch ]]; then
            curl -s --unix-socket "$run/ch.sock" -X PUT http://localhost/api/v1/vmm.shutdown || true
        else
            qmp "$run/qmp.sock" '{"execute":"quit"}' || true
        fi
        for ((i = 0; i < 30; i++)); do alive "$pid" || break; sleep 1; done
    fi
    if alive "$pid"; then
        grep -qE 'qemu-system|cloud-hypervisor' "/proc/$pid/cmdline" || die "pid $pid is not the VM"
        say "still up: SIGTERM $pid"
        kill "$pid"
        for ((i = 0; i < 30; i++)); do alive "$pid" || break; sleep 1; done
    fi
    alive "$pid" && die "VM $pid still runs: do not touch the host drivers; see the R4 recovery"
    [[ -f $run/watch.pid ]] && kill "$(cat "$run/watch.pid")" 2>/dev/null || true
    if [[ $vm != linux && -f $R4/win/swtpm/swtpm.pid ]] && alive "$(cat "$R4/win/swtpm/swtpm.pid")"; then
        kill "$(cat "$R4/win/swtpm/swtpm.pid")"
    fi
    if [[ -s $run/dmesg.lines ]]; then
        sudo -n dmesg 2>/dev/null | tail -n +"$(($(cat "$run/dmesg.lines") + 1))" > "$run/host-kernel.log" || true
    fi
    lspci -nnk -s 2d:00 > "$run/lspci-after.txt" 2>&1 || true
    say "down: $run ($(du -sh "$run" | cut -f1)); host kernel lines: $(wc -l < "$run/host-kernel.log" 2>/dev/null || echo ?)"
    grep -E 'vfio|2d:00|DMAR|AMD-Vi|IOMMU' "$run/host-kernel.log" 2>/dev/null | tail -n 10 || true
    say "next: another pass, or sudo $HERE/pt-host.sh restore"
}

case ${1:-} in
    prepare) prepare "${2:-}" ;;
    preflight) preflight "${2:-}" "${3:-}" ;;
    cmdline) build_cmd "${2:-win}" "${3:-A}" "$R4/runs/NAME"; shq "${CMD[@]}" ;;
    start) [[ -n ${4:-} ]] || die "start win|linux|win-ch A|B NAME"; start "$2" "$3" "$4" ;;
    trace) trace "${2:-}" ;;
    mark) shift; mark "$@" ;;
    status) status ;;
    stop) stop ;;
    qmp) shift; qmp "$(current)/qmp.sock" "$@" ;;
    watch) watch "$2" ;;
    *) sed -n '2,/^set -euo/p' "$ME" | sed '$d'; exit 1 ;;
esac
