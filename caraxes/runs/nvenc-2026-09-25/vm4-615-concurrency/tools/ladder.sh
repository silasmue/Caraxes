#!/bin/bash
# ladder.sh <label> <holder cmd...>: Vulkan device-create probe (vkcomp 5 s) before, during and after
# a CUDA holder process. Outputs as grun.sh (~/conc/out/<label>/pN-*.out, *.rm).
L=$1; shift; D=~/conc/out/$L; rm -rf "$D"; mkdir -p "$D"; cd ~/conc || exit 1
P=$HOME/conc/rmioctl-log.so
vk() { LD_PRELOAD=$P timeout 120 ./vkcomp 5 > "$D/$1-vk.out" 2> "$D/$1-vk.rm"; echo "rc=$?" >> "$D/$1-vk.out"; }
echo "== ladder $L at $(date +%s.%N): $*"
vk p1-before
LD_PRELOAD=$P "$@" > "$D/p2-hold.out" 2> "$D/p2-hold.rm" & hp=$!
sleep 10; echo "holder pid $hp alive=$(kill -0 $hp 2>/dev/null && echo 1 || echo 0)"; nvidia-smi --query-compute-apps=pid,name,used_memory --format=csv,noheader > "$D/smi.txt" 2>&1
vk p3-during
kill $hp 2>/dev/null; wait $hp; echo "rc=$? (killed after the during probe)" >> "$D/p2-hold.out"
sleep 3; vk p4-after
for f in "$D"/*.out; do echo "-- $(basename "$f"): $(grep -E '^START|^device created|HOLDING|^INFO NVENC (session|encoder)|^ENC' "$f" | tr '\n' ' ' | cut -c1-200)"; grep -E '^RESULT|^rc=' "$f" | tr '\n' ' '; echo; done
echo "== c5c0 allocs"; for f in "$D"/*-vk.rm; do echo "$(basename $f): $(grep -c 'class=0xc5c0' $f) c5c0, failing: $(grep 'class=0xc5c0' $f | grep -vc 'status=0 ') ; $(grep 'status=0x[1-9a-f]' $f | cut -c1-120 | head -2 | tr '\n' ';')"; done
echo "== end $(date +%s.%N)"
