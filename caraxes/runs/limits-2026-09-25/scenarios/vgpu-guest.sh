# SPDX-License-Identifier: MIT
# Sourced by limits-check.sh (vm8).
# vgpu-guest: the TU106 whose NV_PMC_BOOT_1 says "SR-IOV virtual function"
# (CARAXES_COSTUME=rtx2070vf). What the stock guest does with it: the probe log, whether
# RmInitAdapter runs, and what nvidia-smi then sees.
dmesg | grep -E "NVRM|nvidia" | grep -v "NVRM: VM:" | head -n 40
step smi $SMI
step vgpu $SMI vgpu
step q-virt $SMI -q
