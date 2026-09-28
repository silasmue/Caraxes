#!/bin/bash
# ladder2.sh <label> <probe cu|vk|egl> <holder shell command>: probe (5 s) before, during (holder up 12 s) and
# after a holder, on the desktop's Wayland session. rmioctl logs as ladder.sh.
L=$1; PR=$2; shift 2; D=~/conc/out/$L; rm -rf "$D"; mkdir -p "$D"; cd ~/conc || exit 1
U=$(id -u); export XDG_RUNTIME_DIR=/run/user/$U WAYLAND_DISPLAY=wayland-0 DISPLAY=:0
P=$HOME/conc/rmioctl-log.so
probe() { case $PR in cu) c="./cuwork 5";; vk) c="./vkcomp 5";; egl) c="./eglwork 5";; esac
  LD_PRELOAD=$P timeout 120 $c > "$D/$1-$PR.out" 2> "$D/$1-$PR.rm"; echo "rc=$?" >> "$D/$1-$PR.out"; }
echo "== ladder2 $L probe=$PR at $(date +%s.%N): $*"
probe p1-before
setsid bash -c "LD_PRELOAD=$P exec bash $*" > "$D/p2-hold.out" 2> "$D/p2-hold.rm" < /dev/null & hp=$!
sleep 12; echo "holder pid $hp alive=$(kill -0 $hp 2>/dev/null && echo 1 || echo 0)"; nvidia-smi --query-compute-apps=pid,name,used_memory --format=csv,noheader > "$D/smi.txt" 2>&1
probe p3-during
echo "holder alive after probe=$(kill -0 $hp 2>/dev/null && echo 1 || echo 0)"
kill -- -$hp 2>/dev/null; sleep 1; kill -9 -- -$hp 2>/dev/null; wait $hp 2>/dev/null
sleep 3; probe p4-after
for f in "$D"/*.out; do echo "-- $(basename "$f"): $(grep -E '^START|^device created|^context|HOLDING|priority' "$f" | tr '\n' ' ' | cut -c1-220)"; grep -E '^RESULT|^rc=' "$f" | tr '\n' ' '; echo; done
echo "== failing allocs"; for f in "$D"/*.rm; do echo "$(basename $f): $(grep -c 'class=0xc5c0' $f) c5c0; $(grep 'RMALLOC' $f | grep -v 'status=0 ' | cut -c1-110 | head -2 | tr '\n' ';') slow: $(grep -c ^SLOW $f)"; done
echo "== end $(date +%s.%N)"
