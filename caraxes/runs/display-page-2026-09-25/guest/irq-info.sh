#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The desktop series of 2026-09-25 (guest): where the device's interrupts land in the guest
# (docs/caraxes/intr-page.md §11.4): the nvidia lines of /proc/interrupts with their CPU
# affinity, the nvidia kernel threads with their CPU and scheduling class, and the halt/idle
# settings that decide how fast a halted vCPU answers an MSI.
echo "== $(date -u +%FT%TZ) $(nproc) CPUs, load $(cat /proc/loadavg)"
head -1 /proc/interrupts
grep -iE 'nvidia|nvkms' /proc/interrupts
for irq in $(grep -iE 'nvidia' /proc/interrupts | cut -d: -f1 | tr -d ' '); do
    echo "irq $irq affinity $(cat /proc/irq/"$irq"/smp_affinity_list 2>/dev/null) effective $(cat /proc/irq/"$irq"/effective_affinity_list 2>/dev/null)"
done
ps -eLo pid,psr,cls,rtprio,ni,comm | grep -iE 'nv|irq/' | head -20
echo "cpuidle: $(cat /sys/devices/system/cpu/cpuidle/current_driver 2>/dev/null) governor $(cat /sys/devices/system/cpu/cpuidle/current_governor_ro 2>/dev/null)"
echo "irqbalance: $(systemctl is-active irqbalance 2>/dev/null)"
