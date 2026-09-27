# Ubuntu under QEMU: bounded VMM comparison

This harness runs the same Ubuntu 24.04 / stock 615.71.09 / GNOME Wayland image and the same
Caraxes backend profile as `alpha-2026-09-27/desktop.sh`, but substitutes QEMU 11.1.1 for Cloud
Hypervisor. It is a diagnostic comparison. Cloud Hypervisor remains the preferred target VMM.

The comparison fixes 4 GiB shared memfd RAM, 4 vCPUs, 2 GiB virtual VRAM, reservation slot 0,
a 32 MiB BAR1 pool, two 16 MiB host BAR1 windows, GPA guest DMA without `caraxes_dma_range`, the
display model, host execution, and the same lazy allocation and poll settings. Direct kernel/initrd
boot avoids making firmware behavior another variable. `-vga none -display none` makes the modeled
NVIDIA display the guest's only DRM/display device; use SSH and Sunshine/Moonlight for control.

Do not run it beside `desktop-a`: both configurations request reservation slot 0 and the same 2 GiB
of host VRAM. The launcher refuses while the desktop backend or Cloud Hypervisor is alive. It also
uses a dedicated `caraxes5` tap (`192.168.105.1/24` on this host), guest address
`192.168.105.24`, and separate state under `alpha-2026-09-27/ubuntu-qemu-a`.

After stopping the Cloud Hypervisor desktop, create the isolated child disk and cloud-init seed,
then start the comparison:

```sh
/mnt/vmstore/caraxes/alpha-2026-09-27/desktop.sh down
caraxes/runs/ubuntu-qemu-2026-09-27/run.sh setup
caraxes/runs/ubuntu-qemu-2026-09-27/run.sh preflight
caraxes/runs/ubuntu-qemu-2026-09-27/run.sh up
caraxes/runs/ubuntu-qemu-2026-09-27/run.sh ssh 'nvidia-smi && systemctl --user status sunshine'
caraxes/runs/ubuntu-qemu-2026-09-27/run.sh down
```

`setup` is intentionally separate and refuses while `desktop-a` is active. It also edits the
installed systemd-networkd and NetworkManager MAC/IP settings in the child overlay; changing only
cloud-init would not affect this already initialized image. No child of the active desktop image is
created merely by checking out this artifact.

There is one expected client difference. Stock QEMU 11.1.1 defines
`VFIO_USER_DEVICE_GET_REGION_IO_FDS` but does not send it. Therefore the 64 doorbell-token
ioeventfds and MSI re-arm ioeventfd advertised by this backend are not installed in KVM; those
writes use the backend's trapped fallback. Cloud Hypervisor with Caraxes patches 0005/0006 installs
them. QEMU does support fd-backed DMA maps and sparse mmap areas, so shared memfd RAM, the BAR1
pool, BAR2 memfd, interrupt pages, and display page remain comparable. Treat frame rate and CPU
results as a diagnostic VMM split, with the missing ioeventfd fast path recorded as a confounder.
