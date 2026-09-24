# SPDX-License-Identifier: MIT
# Guest: load nvidia-uvm and create the device nodes (the image has no nvidia-modprobe);
# the same nodes tools/caraxes-vm.sh uvm makes, plus nvidia-uvm-tools.
sudo modprobe nvidia-uvm
[ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
[ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
u=$(grep nvidia-uvm /proc/devices | cut -d" " -f1)
sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools
sudo mknod -m 666 /dev/nvidia-uvm c "$u" 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c "$u" 1
