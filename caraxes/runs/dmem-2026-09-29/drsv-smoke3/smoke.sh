set -u
t() { echo "$(date +%T) $*"; }
used() { nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader; }
sudo systemctl stop gdm3 2>/dev/null; sleep 3
for m in nvidia_drm nvidia_modeset nvidia_uvm nvidia; do [ -d /sys/module/$m ] && sudo rmmod $m; done
[ -d /sys/module/ecc ] && sudo rmmod ecc && t "ecc unloaded (dependency test)"
sudo dmesg -C
t "== load"; sudo /opt/caraxes/display-reserve/load.sh ${DRSV:-RmDisplayReserve=1} NVreg_RmMsg=display_reserve.c; echo "load rc=$?"
cat /proc/driver/nvidia/version | head -1
sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm modeset=1 && echo modeset-drm-ok
bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1
t "before gdm: $(used)"
sudo systemctl start gdm3
for s in 10 20 30 45; do sleep $(( s == 10 ? 10 : (s == 45 ? 15 : 10) )); t "gdm +${s}s: $(used)"; done
t "== reserve lines"; sudo dmesg | grep -E "display reserve" | tail -20
t "== other NVRM"; sudo dmesg | grep NVRM | grep -v "display reserve" | tail -5
