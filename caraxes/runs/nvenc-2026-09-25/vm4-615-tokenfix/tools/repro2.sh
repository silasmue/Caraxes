#!/bin/bash
# repro2.sh <label> [tries]: a long vkcube, then the CUDA NVENC probe, then up to <tries> second vkcubes
# (rmioctl-log3: timestamps, frees, memory escapes, slow ioctls) with a stall sampler on each:
# every 1 s the vkcube threads' state/wchan/syscall, and at +6 s and +9 s a gdb snapshot (if gdb exists).
L=$1; N=${2:-3}; O=~/cxrun/$L; mkdir -p $O
U=$(id -u); wd=wayland-0; for c in /run/user/$U/wayland-*; do case $c in *.lock) continue;; esac; [ -S "$c" ] && { wd=${c##*/}; break; }; done
export XDG_RUNTIME_DIR=/run/user/$U WAYLAND_DISPLAY=$wd DISPLAY=:0
sudo sh -c "dmesg -w > $O/kmsg.txt 2>&1 & echo \$! > $O/dmesg.pid"
LOG=~/nvenc/rmioctl-log3.so
sample() { # <pid> <outfile>
  local p=$1 f=$2 i=0
  while kill -0 $p 2>/dev/null && [ $i -lt 40 ]; do
    { echo "== +$i s $(date +%s.%N)"; for t in /proc/$p/task/*; do echo "$(basename $t) $(cat $t/comm 2>/dev/null) st=$(awk '{print $3}' $t/stat 2>/dev/null) wchan=$(cat $t/wchan 2>/dev/null) sys=$(cat $t/syscall 2>/dev/null | cut -d' ' -f1-3)"; done; } >> $f
    if [ $i = 6 ] || [ $i = 9 ]; then command -v gdb >/dev/null && sudo timeout 20 gdb -batch -p $p -ex 'thread apply all bt 8' -ex 'info registers' -ex 'x/24i $pc-48' > $f.gdb$i 2>&1; sudo cat /proc/$p/maps > $f.maps$i; fi
    sleep 1; i=$((i+1))
  done
}
( LD_PRELOAD=$LOG timeout 150 vkcube-wayland --c 20000 > $O/vk-long.out 2> $O/vk-long.rmlog; echo "vk long rc=$?" >> $O/summary.txt ) &
sleep 8
(cd ~/nvenc && setsid nohup timeout 120 ./nvenc-probe -n 7200 -r 60 -o /tmp/probe-$L.es > $O/probe.txt 2>&1 < /dev/null &)
sleep 12; echo "probe pid $(pgrep -x nvenc-probe)" >> $O/summary.txt
for k in $(seq $N); do
  s=$(date +%s.%N)
  LD_PRELOAD=$LOG timeout 60 vkcube-wayland --c 30 > $O/vk-$k.out 2> $O/vk-$k.rmlog & vp=$!
  sleep 0.3; cp=$(pgrep -P $vp -x vkcube-wayland || echo $vp); sample $cp $O/vk-$k.sample &
  wait $vp; rc=$?
  n57=$(grep -c 'status=0x57' $O/vk-$k.rmlog); echo "vk $k rc=$rc $(echo "$(date +%s.%N) - $s" | bc)s $n57 x 0x57 start=$s" >> $O/summary.txt
  [ "$n57" -gt 0 ] && break
  sleep 3
done
wait
sleep 1; sudo kill $(cat $O/dmesg.pid)
cat $O/summary.txt
