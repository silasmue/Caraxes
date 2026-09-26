#!/bin/bash
# repro.sh <label> <mode>: mode cuda-then-vk | vk-then-cuda | cycles
L=$1; M=$2; O=~/cxrun/$L; mkdir -p $O
U=$(id -u); wd=wayland-0; for c in /run/user/$U/wayland-*; do case $c in *.lock) continue;; esac; [ -S "$c" ] && { wd=${c##*/}; break; }; done
export XDG_RUNTIME_DIR=/run/user/$U WAYLAND_DISPLAY=$wd DISPLAY=:0
sudo sh -c "dmesg -w > $O/kmsg.txt 2>&1 & echo \$! > $O/dmesg.pid"
sleep 1
LOG=~/nvenc/rmioctl-log2.so
vk() { s=$(date +%s.%N); LD_PRELOAD=$LOG timeout 60 vkcube-wayland --c 30 > $O/vk-$1.out 2> $O/vk-$1.rmlog; rc=$?; echo "vk $1 rc=$rc $(echo "$(date +%s.%N) - $s" | bc)s $(grep -c 'status=0x57' $O/vk-$1.rmlog) x 0x57" | tee -a $O/summary.txt; }
cuda_start() { (cd ~/nvenc && setsid nohup timeout ${1:-150} ./nvenc-probe -n 3600 -r 60 -o /tmp/probe-$L.es > $O/probe.txt 2>&1 < /dev/null &); sleep 12; echo "probe pid $(pgrep -x nvenc-probe)" | tee -a $O/summary.txt; }
cuda_wait() { for k in $(seq 40); do pgrep -x nvenc-probe >/dev/null || break; sleep 5; done; echo "probe: $(tail -2 $O/probe.txt | tr '\n' ' ')" | tee -a $O/summary.txt; }
case $M in
cuda-then-vk) vk before; cuda_start 150; vk during; cuda_wait; vk after ;;
vk-then-cuda) (LD_PRELOAD=$LOG timeout 90 vkcube-wayland --c 5000 > $O/vk-long.out 2> $O/vk-long.rmlog; echo "vk long rc=$?" >> $O/summary.txt) & sleep 8; cuda_start 60; vk during2; cuda_wait; wait; vk after ;;
cycles) for i in 1 2 3; do cuda_start 40; vk c$i; cuda_wait; vk n$i; done ;;
esac
sleep 1; sudo kill $(cat $O/dmesg.pid); sleep 1
grep -a 'CXDBG\|Check failed\|Assertion failed\|GspRmAlloc\|Xid\|NVRM: .*rror' $O/kmsg.txt | grep -v 'ioctl(' > $O/kmsg-filtered.txt; wc -l $O/kmsg.txt $O/kmsg-filtered.txt | tee -a $O/summary.txt
