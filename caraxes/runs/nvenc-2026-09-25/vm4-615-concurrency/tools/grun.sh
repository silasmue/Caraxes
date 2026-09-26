#!/bin/bash
# grun.sh <label> <spec>...: guest side of one matrix cell. spec = kind:delay:secs[:arg],
# kind cu|vk|egl. Every process runs under rmioctl-log.so (stderr = its RM escapes).
L=$1; shift; D=~/conc/out/$L; rm -rf "$D"; mkdir -p "$D"; cd ~/conc || exit 1
echo "== cell $L at $(date +%s.%N) specs: $*"
echo "== nvidia users"; sudo fuser -v /dev/nvidia* 2>&1 | awk 'NR>1{print $NF, $0}' | awk '{print $NF}' | sort | uniq -c | tr '\n' ' '; echo
pgrep -a -x 'gnome-shell|Xwayland|sunshine|steam|gdm-wayland-ses' | cut -c1-60 | tr '\n' ';'; echo
i=0; pids=()
for s in "$@"; do
  IFS=: read -r k dl secs arg <<< "$s"; i=$((i+1))
  case $k in cu) cmd="./cuwork $secs ${arg:-20000}";; vk) cmd="./vkcomp $secs ${arg:-4096}";; egl) cmd="./eglwork $secs";; esac
  ( sleep "$dl"; LD_PRELOAD=$HOME/conc/rmioctl-log.so timeout $((secs+90)) $cmd > "$D/p$i-$k.out" 2> "$D/p$i-$k.rm"; echo "rc=$?" >> "$D/p$i-$k.out" ) &
  pids+=($!)
done
sleep 5; nvidia-smi --query-compute-apps=pid,name,used_memory --format=csv,noheader > "$D/smi.txt" 2>&1
wait "${pids[@]}"
for f in "$D"/*.out; do echo "-- $(basename "$f"): $(grep -E '^START|^device |^GL_RENDERER|^context|^device created' "$f" | tr '\n' ' ')"; grep -E '^RESULT|^rc=' "$f" | tr '\n' ' '; echo; done
echo "== rm status!=0"; grep -h 'status=0x[1-9a-f]' "$D"/*.rm | cut -c1-160 | sort | uniq -c | head -8
echo "== dmesg tail"; sudo dmesg | tail -5
echo "== end $(date +%s.%N)"
