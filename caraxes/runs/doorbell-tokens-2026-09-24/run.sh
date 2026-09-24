#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Doorbell token pool, pure emulation on vm1 (docs/caraxes/doorbell-token-pool.md,
# "Verification"). Steps: T (253 tokens: registration, driver load, bench-trap with pool tokens
# and other values, nvprobe 2), hot-remove, re-add with a new server (64 tokens), down.
set -uo pipefail
R=/home/silas/git/Caraxes                                   # the main checkout: VM script, ROM
T=/home/silas/git/Caraxes/.claude/worktrees/agent-abcf6071de2ca99bb   # this tree
O=$T/caraxes/runs/doorbell-tokens-2026-09-24
source "$O/env.sh"
W=/mnt/vmstore/caraxes/$VM
V=("$R/tools/caraxes-vm.sh")
BDF=/sys/bus/pci/devices/0000:00:05.0
step() { echo "== $* ($(date +%T))"; }

step "T: up with CARAXES_DOORBELL_TOKENS=253"
export CARAXES_DOORBELL_TOKENS=253
mkdir -p "$O/T"
"${V[@]}" up > "$O/T/up.txt" 2>&1 && "${V[@]}" wait >> "$O/T/up.txt" 2>&1 || { cat "$O/T/up.txt"; exit 1; }
grep -E 'doorbell|vfio-user-nvgpu: tu106' "$W/gpu.log" | cut -c1-400 > "$O/T/gpu-start.txt"
grep -E 'ioeventfd|region I/O|max_msg_fds' "$W/ch.log" > "$O/T/ch-ioeventfd.txt"
echo "ch.log: $(grep -c 'datamatch Some' "$O/T/ch-ioeventfd.txt") datamatch ioeventfd line(s)"; grep 'sub-region(s) registered' "$O/T/ch-ioeventfd.txt"

step "T: driver load"
"${V[@]}" probe tok-T > "$O/T/probe.txt" 2>&1; cat "$O/T/probe.txt"
cp "$W/probe-tok-T/guest-dmesg.txt" "$O/T/guest-dmesg.txt" 2>/dev/null

step "T: bench-trap on the doorbell (pool slot 0, slot 252, a value outside the pool)"
"${V[@]}" ssh 'mkdir -p ~/bench; cat > ~/bench/bench-trap.c' < "$T/caraxes/tools/bench-trap.c"
L0=$(wc -l < "$W/gpu-trace.log")
"${V[@]}" ssh "cd ~/bench && gcc -O2 -o bench-trap bench-trap.c &&
  sudo ./bench-trap $BDF/resource0 20000 0xbb0090 0 0xca700000 &&
  sudo ./bench-trap $BDF/resource0 20000 0xbb0090 0 0xca7000fc &&
  sudo ./bench-trap $BDF/resource0 20000 0xbb0090 0 0x12345 &&
  sudo ./bench-trap $BDF/resource0 20000 0x1400" | tee "$O/T/bench-trap.txt"
sleep 1
tail -n +"$((L0 + 1))" "$W/gpu-trace.log" > "$W/bench-trace.tmp"
{
  echo "# gpu-trace.log during the bench: trapped doorbell writes by value, then eventfd events"
  grep 'W bar0 0x00bb0090' "$W/bench-trace.tmp" | awk '{print $6}' | sort | uniq -c | sort -rn | head
  echo "trapped doorbell lines: $(grep -c 'W bar0 0x00bb0090' "$W/bench-trace.tmp")"
  echo "eventfd events: $(grep -c 'doorbell ioeventfd:' "$W/bench-trace.tmp"), by token:"
  grep -oE 'guest write\(s\) of token 0x[0-9a-f]+' "$W/bench-trace.tmp" | sort | uniq -c
  echo "writes counted by the eventfds: $(grep -oE '[0-9]+ guest write\(s\) of token' "$W/bench-trace.tmp" | awk '{s+=$1} END {print s}')"
  echo "stale notes: $(grep -c 'stale' "$W/bench-trace.tmp")"
} > "$O/T/bench-trace-summary.txt"
rm -f "$W/bench-trace.tmp"; cat "$O/T/bench-trace-summary.txt"

step "T: nvprobe 2 (cuInit, context, cuMemAlloc + H2D/D2H) if the image has it"
L1=$(wc -l < "$W/gpu-trace.log")
"${V[@]}" ssh 'cd ~/bench 2>/dev/null && ls nvprobe >/dev/null 2>&1 || { echo "no ~/bench/nvprobe"; exit 0; }
  [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
  [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
  sudo modprobe nvidia-uvm; M=$(grep nvidia-uvm /proc/devices | cut -d" " -f1)
  sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools; sudo mknod -m 666 /dev/nvidia-uvm c $M 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c $M 1
  timeout 180 ./nvprobe 2; echo "nvprobe 2 rc=$?"' 2>&1 | tail -20 | tee "$O/T/nvprobe2.txt"
tail -n +"$((L1 + 1))" "$W/gpu-trace.log" > "$W/nvprobe-trace.tmp"
{
  echo "# gpu-trace.log during nvprobe 2"
  echo "hand-outs:"; grep -oE 'doorbell tokens: chid [0-9]+ gets (back its )?pool token 0x[0-9a-f]+ \(was 0x[0-9a-f]+[^)]*\)' "$W/nvprobe-trace.tmp" | sort | uniq -c
  echo "refusals:"; grep -oE 'doorbell tokens: (chid [0-9]+: work-submit|chid [0-9]+ has no error|pool of).{0,160}' "$W/nvprobe-trace.tmp" | sort | uniq -c | head
  echo "rings by pool token (eventfd):"; grep -oE 'doorbell token 0x[0-9a-f]+ \(eventfd\) -> chid [0-9]+' "$W/nvprobe-trace.tmp" | sort | uniq -c
  echo "rings by pool token (trapped):"; grep -oE 'doorbell token 0x[0-9a-f]+ \(trapped\) -> chid [0-9]+' "$W/nvprobe-trace.tmp" | sort | uniq -c
  echo "trapped doorbell writes by value:"; grep 'W bar0 0x00bb0090' "$W/nvprobe-trace.tmp" | awk '{print $6}' | sort | uniq -c | sort -rn | head -40
} > "$O/T/nvprobe2-trace-summary.txt"
rm -f "$W/nvprobe-trace.tmp"; cat "$O/T/nvprobe2-trace-summary.txt"

step "hot-remove"
mkdir -p "$O/removal-readd"
"${V[@]}" ssh 'sudo rmmod nvidia_uvm 2>/dev/null; sudo rmmod nvidia; echo rmmod rc=$?' | tee "$O/removal-readd/rmmod.txt"
curl -s -o /dev/null -w '%{http_code}\n' --unix-socket "$W/ch.sock" -X PUT http://localhost/api/v1/vm.remove-device -d '{"id":"gpu0"}' | tee "$O/removal-readd/remove.txt"
for _ in $(seq 50); do kill -0 "$(cat "$W/gpu.pid")" 2>/dev/null || break; sleep 0.2; done
echo "server alive after removal: $(kill -0 "$(cat "$W/gpu.pid")" 2>/dev/null && echo yes || echo no)" | tee -a "$O/removal-readd/remove.txt"
grep -E 'device-removed|unregister|Could not' "$W/ch.log" | tail -5 | tee "$O/removal-readd/ch-removal.txt"
grep -E 'doorbell tokens:|exit|ERROR' "$W/gpu.log" | tail -6 | cut -c1-900 > "$O/removal-readd/gpu-summary-T.txt"; cat "$O/removal-readd/gpu-summary-T.txt"

step "re-add with a new server, CARAXES_DOORBELL_TOKENS=64"
REF=$R/caraxes/reference/rtx2070
CARAXES_DOORBELL_TOKENS=64 "$CARAXES_SERVER" --socket "$W/gpu2.sock" --trace "$W/gpu-trace-readd.log" \
  --config "$REF/config-space.txt" --rom "$REF/rom.bin" --vbios "$REF/synth-vbios.bin" --boot-model --host-rm \
  > "$W/gpu-readd.log" 2>&1 &
echo $! > "$W/gpu2.pid"
for _ in $(seq 50); do [[ -S $W/gpu2.sock ]] && break; sleep 0.1; done
L2=$(wc -l < "$W/ch.log")
curl -s -w '\n%{http_code}\n' --unix-socket "$W/ch.sock" -X PUT http://localhost/api/v1/vm.add-user-device -d "{\"socket\":\"$W/gpu2.sock\",\"id\":\"gpu1\"}" | tee "$O/removal-readd/add.txt"
sleep 1
tail -n +"$((L2 + 1))" "$W/ch.log" | grep -E 'ioeventfd|registered|EEXIST|File exists' > "$O/removal-readd/ch-readd.txt"
echo "re-add: $(grep -c 'datamatch Some' "$O/removal-readd/ch-readd.txt") datamatch ioeventfd line(s), $(grep -ciE 'not registered|File exists' "$O/removal-readd/ch-readd.txt") refused"; grep 'sub-region(s) registered' "$O/removal-readd/ch-readd.txt"
"${V[@]}" ssh "sudo modprobe nvidia; echo modprobe rc=\$?; sudo dmesg | grep -E 'RmInitAdapter|Xid' | tail -3; cd ~/bench && sudo ./bench-trap $BDF/resource0 2000 0xbb0090 0 0xca700000 && sudo ./bench-trap $BDF/resource0 2000 0xbb0090 0 0xca70003f" | tee "$O/removal-readd/readd-bench.txt"

step "down"
"${V[@]}" down | tee "$O/down.txt"
for _ in $(seq 50); do kill -0 "$(cat "$W/gpu2.pid")" 2>/dev/null || break; sleep 0.2; done
kill "$(cat "$W/gpu2.pid")" 2>/dev/null; rm -f "$W/gpu2.pid"
grep -E 'doorbell tokens' "$W/gpu-readd.log" | cut -c1-900 | tee "$O/removal-readd/gpu-summary-readd.txt"
echo "done $(date +%T)"
