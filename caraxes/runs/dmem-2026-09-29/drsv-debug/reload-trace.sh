set -u
sudo systemctl stop gdm3; sleep 3
for m in nvidia_drm nvidia_modeset nvidia_uvm nvidia; do [ -d /sys/module/$m ] && sudo rmmod $m; done
lsmod | grep -c "^nvidia" 
sudo modprobe ecc
sudo dmesg -C
sudo insmod /opt/caraxes/display-reserve/$(uname -r)/nvidia.ko NVreg_GpuInitOnProbe=1 NVreg_UseKernelSuspendNotifiers=1 NVreg_TemporaryFilePath=/var/tmp NVreg_EnableS0ixPowerManagement=1 NVreg_PreserveVideoMemoryAllocations=1 NVreg_RegistryDwords="RmDisableGlobalCeUtils=1;RmRcWatchdog=0;RmDisplayReserve=1" NVreg_RmMsg="display_reserve.c,stdmemDumpInputAllocParams" && echo insmod-ok
sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm modeset=1 && echo modeset-drm-ok
bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1
nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader
sudo systemctl start gdm3; sleep 40
nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader
sudo dmesg > /tmp/drsv-dmesg.txt; grep -c "stdmemConstruct input" /tmp/drsv-dmesg.txt

