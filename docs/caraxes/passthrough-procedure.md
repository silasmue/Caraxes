<!-- SPDX-License-Identifier: MIT -->
# Passthrough procedure for R1/R3 (written, not run)

> 2026-09-29: for R4 (and R1/R3 with it), sections 2-4 are replaced by
> [experiments.md](experiments.md) R4 and `caraxes/tools/passthrough/`. Two corrections to
> this file: the port 00:03.1 has reset_method `pm`, so its `reset` resets the port, not the
> card (the secondary bus reset is its `reset_subordinate`); and QEMU's log trace backend
> writes timestamps only with `-msg timestamp=on`.

This is preparation only. Nothing in this file has been executed against the
host driver, the card's registers, or the Wayland session. Every command below
that changes host state is a **plan** for a future session run over SSH in
tmux from a second machine, per the operator's decision on 2026-09-23 ("no
passthrough for the moment, we need to prepare that better") and the safety
envelope in [experiments.md](experiments.md). Section 1's read-only checks
were run now, from the desktop session itself, because they cannot break
anything; their real output is recorded below so the rest of the procedure is
grounded in this machine's actual state, not assumptions. All host paths below
are absolute to this machine (hostname `manacor`); command output was captured
2026-09-23, roughly 11:00-11:25 CEST.

Read first: [experiments.md](experiments.md) (R1/R3 definitions, safety
table), [kickoff.md](kickoff.md) rule 1 and section 5 step 6 (guest
preparation), [environment.md](environment.md) (host inventory),
[open-questions.md](open-questions.md) Q1/Q3 (what R1/R3 resolve).

## 1. Preconditions

Each row: what to check, the read-only command, and what was actually found.
Re-run every command in this section again right before the real attempt --
driver state, session state and module state can all have moved on since
2026-09-23.

### 1.1 IOMMU enabled, and the card's group membership

```sh
find /sys/kernel/iommu_groups/ -type l | grep '2d:00'
ls /sys/kernel/iommu_groups/ | wc -l
ls /sys/class/iommu/
```

Found:

```
/sys/kernel/iommu_groups/28/devices/0000:2d:00.2
/sys/kernel/iommu_groups/28/devices/0000:2d:00.0
/sys/kernel/iommu_groups/28/devices/0000:2d:00.3
/sys/kernel/iommu_groups/28/devices/0000:2d:00.1
```

33 groups total; `/sys/class/iommu/` has one entry, `ivhd0` (AMD-Vi; the CPU
is `AuthenticAMD`). Group 28 contains exactly the card's four functions and
nothing else -- a clean group, no unrelated device has to be pulled along.
`CONFIG_AMD_IOMMU=y`, `CONFIG_INTEL_IOMMU=y` and `CONFIG_IOMMU_SUPPORT=y` are
built in (not modules); no `iommu`/`amd_iommu` argument is on the kernel
command line and none is needed -- the groups already exist at this boot, so
IOMMU translation is active without a reboot. **Verdict: satisfied.**

### 1.2 Which modules/drivers hold the card, right now

```sh
lsmod | grep -E 'nvidia|vfio|drm'
lspci -nnk -s 2d:00
for f in 0 1 2 3; do readlink -f /sys/bus/pci/devices/0000:2d:00.$f/driver; done
```

Found -- driver per function:

| Function | Device | Driver in use |
|---|---|---|
| `0000:2d:00.0` | VGA `10de:1f02` | `nvidia` |
| `0000:2d:00.1` | HD audio `10de:10f9` | `snd_hda_intel` |
| `0000:2d:00.2` | USB 3.1 `10de:1ada` | `xhci_hcd` |
| `0000:2d:00.3` | UCSI `10de:1adb` | `nvidia-gpu` (`i2c-nvidia-gpu.ko`, mainline I2C driver, not the proprietary stack) |

Loaded NVIDIA modules and their reference counts: `nvidia_drm` (68 refs),
`nvidia_modeset` (22 refs, held by `nvidia_drm`), `nvidia_uvm` (0 refs),
`nvidia` (538 refs, held by `nvidia_uvm` and `nvidia_modeset`). The 538 refs
on `nvidia` come from every process below holding `/dev/nvidia*` open (1.7).
**Verdict: informational; this is the state step 2 has to undo.**

### 1.3 Is `vfio-pci` available

```sh
modinfo vfio-pci
modinfo vfio
```

Found: `/lib/modules/7.2.4-arch1-2/kernel/drivers/vfio/pci/vfio-pci.ko.zst`,
in-tree, depends on `vfio-pci-core` and `vfio`, not currently loaded (absent
from `lsmod`, and `/sys/bus/pci/drivers/vfio-pci/` does not exist yet).
**Verdict: satisfied; `modprobe vfio-pci` in step 2.7 is what creates that
driver directory.**

### 1.4 Kernel config bits

```sh
zcat /proc/config.gz | grep -E \
  'CONFIG_VFIO_PCI[= ]|CONFIG_VFIO_PCI_CORE|CONFIG_VFIO_IOMMU_TYPE1|CONFIG_VFIO=|CONFIG_IOMMU_SUPPORT=|CONFIG_AMD_IOMMU=|CONFIG_INTEL_IOMMU=|CONFIG_IO_STRICT_DEVMEM|CONFIG_STRICT_DEVMEM'
```

Found: `CONFIG_VFIO=m`, `CONFIG_VFIO_PCI=m`, `CONFIG_VFIO_PCI_CORE=m`,
`CONFIG_VFIO_IOMMU_TYPE1=m`, `CONFIG_VFIO_MDEV=m`, `CONFIG_IOMMU_SUPPORT=y`,
`CONFIG_AMD_IOMMU=y`, `CONFIG_INTEL_IOMMU=y`, `CONFIG_STRICT_DEVMEM=y`,
`CONFIG_IO_STRICT_DEVMEM=y`. The last two are why open question Q1's
`/dev/mem` and `resource0` reads failed with EPERM/EINVAL while `nvidia` owns
the BAR -- they are also why R1/R3 (going through `vfio-pci` and
`/dev/vfio/<group>` instead) is the sanctioned path, not a workaround. **Verdict:
satisfied.**

### 1.5 A second machine with SSH access

Not verifiable from this host. What *was* checked here, and is a blocker:

```sh
systemctl is-active sshd; systemctl is-enabled sshd
```

Found: `inactive` / `disabled`. **This host cannot currently be reached over
SSH at all.** Before anything else, the operator has to enable and start
`sshd` here (or confirm some other remote-shell path already exists) and
confirm a second machine, on the same network, with credentials for this
host, actually exists and has been tested. Until both are true, "run it from
an SSH session in tmux from another machine" is not an available option and
the rest of this procedure must not be started. **Verdict: blocked -- operator
action required, tracked in section 5.**

### 1.6 The Wayland session and the display manager in use

```sh
loginctl list-sessions
loginctl show-session <id> -p Type -p State -p TTY -p Remote -p Service -p Scope -p Leader
systemctl status display-manager --no-pager
systemctl list-unit-files --type=service | grep -iE 'gdm|sddm|lightdm|greetd|ly\.service'
ps -o pid,ppid,cmd --ppid <login-pid>
```

Found, and this is the main deviation from the assumption behind step 2 of
this task: **there is no display manager on this machine.**
`systemctl status display-manager` answers "Unit display-manager.service
could not be found", and no `gdm`/`sddm`/`lightdm`/`greetd`/`ly` unit is
installed. `loginctl list-sessions` shows two sessions for `silas`: session 1
is a `manager`-class session with no TTY (systemd's own per-user manager,
leader pid 745, not a login session); session 2 is the real one -- `seat0`,
`tty1`, `Type=wayland`, `Class=user`, `Service=login`, `Scope=session-2.scope`,
`Leader=699`. Walking the process tree from that leader: `login` (699) ->
`zsh` (979) -> `start-hyprland` (1082) -> `Hyprland --watchdog-fd 4` (1085).
`start-hyprland` is a real binary at `/usr/bin/start-hyprland` (the Hyprland
package's own launcher), not a script. No `exec`/`hyprland`/`start-hyprland`
line was found in `~/.zshrc`, `~/.bash_profile`, or `~/.zprofile` (the last
does not exist), and no `systemctl --user` unit runs it either
(`graphical-session.target` is loaded but `inactive dead`). **The session was
started by the operator logging in on the physical tty1 console and typing
`start-hyprland`, not by any automatic mechanism this machine records.**
Consequence for section 2 ("log out the Wayland session and stop the display
manager"): there is no display-manager service to stop; the equivalent action
is ending session 2 (`loginctl terminate-session 2`, or the compositor's own
exit path). Consequence for section 4 (recovery): nothing on this machine
will restart Hyprland automatically either -- see 4's last step.

Also checked, because the task named it as a specific risk (framebuffer
console holding the card) and it turned out to be real:

```sh
cat /proc/fb
for v in /sys/class/vtconsole/vtcon*; do echo "$v: $(cat $v/name), bind=$(cat $v/bind)"; done
sudo cat /sys/module/nvidia_drm/parameters/modeset
modinfo nvidia_drm | grep -A1 parm
zcat /proc/config.gz | grep -E 'CONFIG_DRM_SIMPLEDRM|CONFIG_FB_EFI|CONFIG_SYSFB_SIMPLEFB'
ls /sys/bus/platform/devices/ | grep -i fb
```

Found: `/proc/fb` shows `0 nvidia-drmdrmfb`. `vtcon0` is the "(S) dummy
device" (`bind=0`); `vtcon1` is the "(M) frame buffer device" and is
`bind=1` -- the text console is currently attached to `nvidia_drm`'s DRM
fbdev emulation. `nvidia_drm`'s `modeset` parameter reads `Y` (the module
default; nothing in `/etc/modprobe.d`, `/usr/lib/modprobe.d`, or
`/proc/cmdline` overrides it -- the only NVIDIA-related modprobe.d file is
`nvidia-sleep.conf`, which sets unrelated suspend options). Kernel support for
a fallback console exists (`CONFIG_DRM_SIMPLEDRM=y`, `CONFIG_FB_EFI=y`,
`CONFIG_SYSFB_SIMPLEFB=y`), but no `efi-framebuffer` platform device is
present and no `simpledrm` module is loaded right now: the generic boot
framebuffer already yielded its platform device to `nvidia_drm` at boot (this
is the kernel's normal "hand off simplefb to the real driver" behaviour, and
it is a one-way handoff -- the platform device is removed, not just unbound).
**Consequence: when `nvidia_drm` is unloaded later, no fallback console will
silently appear.** The physical display will most likely go to no-signal and
stay there until `nvidia_drm` reloads and re-registers its fbdev; `vtcon1`
must be explicitly unbound from the framebuffer before `rmmod nvidia_drm`
(step 2.5) or the unload can fail with the module "in use" by the console
layer. **Verdict: blocked in one respect (no SSH in), confirmed-real risk in
another (fbcon); both are addressed in sections 2 and 4, not blockers to
writing the plan.**

### 1.7 Which processes hold `/dev/nvidia*`

```sh
sudo lsof /dev/nvidia*
```

This is a read-only listing (`lsof` does not modify anything it inspects);
`sudo` is needed only because some of the listed processes are not owned by
the calling shell's session. Found 262 open-file lines across seven process
names, all under `silas`, all ordinary desktop processes: `Hyprland` (1085,
the compositor itself, holds `/dev/nvidia0`, `/dev/nvidiactl` and
`/dev/nvidia-modeset`), `Xwayland`, `kitty`, `firefox`, `Discord`/`electron`,
`spotify`. No `nvidia-persistenced` (checked separately, below: loaded,
disabled, inactive) and no container runtime holding the device. In other
words, on this machine, "stop anything using `/dev/nvidia*`" and "log out the
Wayland session" are almost the same action -- essentially every holder is a
child of, or the compositor of, session 2 itself.

```sh
systemctl status nvidia-persistenced --no-pager
```

Found: `loaded (...disabled; preset: disabled)`, `Active: inactive (dead)`.
Nothing to stop there. **Verdict: informational; the full holder list is the
input to step 2.2.**

### 1.8 Reset capability (used by section 4, checked here so the plan does not
invent it)

```sh
for f in 0 1 2 3; do
  p=/sys/bus/pci/devices/0000:2d:00.$f/reset
  [ -e "$p" ] && echo "0000:2d:00.$f: reset file present" || echo "0000:2d:00.$f: none"
done
ls -la /sys/bus/pci/devices/0000:00:03.1/reset
lspci -s 00:03.1 -nnk
```

Found: a per-function FLR `reset` file exists only for `0000:2d:00.0`; not for
`.1`/`.2`/`.3`. The upstream bridge, `0000:00:03.1` ("AMD Starship/Matisse GPP
Bridge", driver `pcieport`), also has a `reset` file -- writing to it performs
a secondary-bus reset of everything below it, which per 1.1 is exactly and
only the card's four functions. That is the group-wide fallback used in
section 4 if `.0`'s own FLR is not enough.

## 2. Taking the card away from the host, in order

Nothing in this section has been run. Every command needs `sudo`; this
machine has passwordless `sudo` for `silas` (`sudo -n true` succeeds). Do not
skip a check. Re-verify 1.1-1.8 immediately before starting, in the same SSH
session that will run this section, since driver and session state can have
changed.

### 2.1 Confirm the remote setup

- Command: none (human/procedural). Confirm: connected over SSH, inside
  `tmux`, from the second machine (not from a shell inside the Wayland
  session -- that shell dies with the session in step 2.3).
- Check: `tty` shows a pty, not the physical console; `loginctl` still shows
  session 2 as a *separate* session from this SSH one.
- Rollback: n/a, nothing changed yet.

### 2.2 Save and close what 1.7 found

- Command: none scriptable -- this is closing/saving state inside `kitty`,
  `firefox`, `Discord`, `spotify` by hand, or accepting their state will be
  lost. Re-run `sudo lsof /dev/nvidia*` right before 2.3 to get the current
  holder list, since it can differ from the 2026-09-23 snapshot in 1.7.
- Check: operator confirms each named application has nothing unsaved.
- Rollback: n/a.

### 2.3 End the Wayland session

- Command: from inside the session, the compositor's own exit binding (if
  configured); otherwise from the SSH session:
  ```sh
  loginctl terminate-session 2
  ```
- Expected: the physical console goes to a text login prompt (or blank, given
  1.6); `loginctl list-sessions` no longer lists session 2.
- Check:
  ```sh
  loginctl list-sessions
  sudo lsof /dev/nvidia* | wc -l
  ```
  Expect the second command to print `0` (or just a header line).
- Rollback: nothing destructive yet; if the session will not end,
  `loginctl kill-session 2` then, as a last resort,
  `pkill -u silas -f Hyprland`.

### 2.4 Confirm nothing still holds `/dev/nvidia*`

- Command: `sudo lsof /dev/nvidia*`
- Expected: empty (or header only).
- Check: if any process remains, identify it (`ps -p <pid>`) and stop it
  explicitly; do not proceed to 2.5 with open handles, `rmmod` will fail.
- Rollback: n/a.

### 2.5 Unbind the console framebuffer from `nvidia_drm`

- Command:
  ```sh
  cat /sys/class/vtconsole/vtcon1/name   # confirm it is still "(M) frame buffer device"
  echo 0 | sudo tee /sys/class/vtconsole/vtcon1/bind
  ```
- Expected: the write succeeds; the physical display may blank immediately
  (expected, see 1.6).
- Check: `cat /sys/class/vtconsole/vtcon1/bind` reads `0`.
- Rollback: `echo 1 | sudo tee /sys/class/vtconsole/vtcon1/bind` -- only
  meaningful while `nvidia_drm` is still loaded; once it is not (2.6), this
  step's rollback moves to section 4.

### 2.6 Unload the NVIDIA modules, in this order

- Command:
  ```sh
  sudo modprobe -r nvidia_drm
  sudo modprobe -r nvidia_modeset
  sudo modprobe -r nvidia_uvm
  sudo modprobe -r nvidia
  ```
  (`modprobe -r` refuses each step cleanly if refcounts are still non-zero,
  which is a better failure mode here than `rmmod`.)
- Expected: each command exits 0 with no output.
- Check: `lsmod | grep nvidia` prints nothing.
- Rollback:
  ```sh
  sudo modprobe nvidia && sudo modprobe nvidia_uvm && \
  sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm
  ```
  The console will very likely stay blank even after this (1.6); do not treat
  a blank screen here as failure by itself -- check `lsmod` and
  `nvidia-smi` instead, then see section 4's console-rebind step.

### 2.7 Bind all four functions to `vfio-pci`

- Command:
  ```sh
  sudo modprobe vfio-pci
  for f in 0 1 2 3; do
    bdf=0000:2d:00.$f
    echo vfio-pci | sudo tee /sys/bus/pci/devices/$bdf/driver_override
    drv=/sys/bus/pci/devices/$bdf/driver
    [ -e "$drv" ] && echo "$bdf" | sudo tee "$drv/unbind"
    echo "$bdf" | sudo tee /sys/bus/pci/drivers_probe
  done
  ```
- Expected: no errors; each `driver_override` write and each `unbind`/
  `drivers_probe` write succeeds (a `tee` failure shows up as a write error,
  not a silent no-op).
- Check:
  ```sh
  lspci -nnk -s 2d:00
  ```
  All four functions should show `Kernel driver in use: vfio-pci`.
- Rollback: see section 4 (unbind from `vfio-pci`, clear `driver_override`,
  `drivers_probe` back to the original drivers) -- do not improvise a
  different order here, use section 4's sequence.

### 2.8 Verify the group is fully bound

- Command:
  ```sh
  ls /sys/bus/pci/drivers/vfio-pci/
  ls -la /dev/vfio/
  ```
- Expected: all four `0000:2d:00.*` entries under the driver directory; a new
  node `/dev/vfio/28` (matching the group number from 1.1).
- Check: if `/dev/vfio/28` is missing, one function is still bound elsewhere
  -- re-check `lspci -nnk -s 2d:00` and repeat 2.7 for the stuck function.
- Rollback: section 4.

At this point the card belongs to `vfio-pci` and R1/R3 can run. The desktop
is down; nothing further in section 2 should be attempted without moving on
to section 3 or straight to section 4.

## 3. R1 and R3 run recipes

R1 (full VBIOS through the PROM window) and R3 (complete MMIO trace of a real
init) are one capture: the same trace that gives the VBIOS bytes also gives
the boot-register values and the whole register sequence. Both need QEMU
(already installed, `qemu-system-x86_64 --version` reports `QEMU emulator
version 11.1.1`), not Cloud Hypervisor -- per
[vfio-user.md](vfio-user.md), Cloud Hypervisor has no MMIO tracing for
passthrough devices.

### 3.1 Guest image

Reuse the overlay `tools/caraxes-vm.sh` already prepared for E0/E1
(`$CARAXES_STORE/vm0/rootfs.qcow2`, falling back to
`target/caraxes/vm0/rootfs.qcow2`): it already has `nvidia.ko` 610.57.04 built
in-guest from `vendor/open-gpu-kernel-modules` against the Ubuntu
610.57.04 image's headers, with `gsp_tu10x.bin` installed, per
[kickoff.md](kickoff.md) section 5 step 6. That overlay's driver was never
loaded against real hardware, so it is exactly what R1/R3 need. If a fresh
overlay is preferred instead of reusing `vm0`'s, repeat kickoff.md section 5
step 6 against a new `qemu-img create -f qcow2 -b <base> -F qcow2 <new>.qcow2`
(base image `guest-baked-20260801-610.57.04-20260820-105733.qcow2` under
`/mnt/vmstore/leandro/vm/`). Kernel and initrd are
`/mnt/vmstore/leandro/vm/guest-image/vmlinuz` and `.../initrd`. Make sure
`tools/caraxes-vm.sh down` has been run first (or use a second tap) -- the
tap `caraxes0` and the overlay's disk cannot be attached to two VMs at once.

### 3.2 Confirm the trace event names for this QEMU build

```sh
qemu-system-x86_64 -trace help | grep -i vfio
```

This is read-only (it only prints the compiled-in event list, nothing runs).
Confirmed present in this build (194 `vfio`-prefixed events total, in part):
`vfio_region_read`, `vfio_region_write`, `vfio_pci_read_config`,
`vfio_pci_write_config`, `vfio_pci_reset`, `vfio_pci_load_rom`,
`vfio_pci_size_rom`. The exact names the task mentioned
(`vfio_region_read`, `vfio_region_write`) exist unchanged in 11.1.1.

### 3.3 Launch QEMU with the real card

```sh
W=$CARAXES_STORE/vm0   # or target/caraxes/vm0, whichever 3.1 used
cat > /tmp/caraxes-r1r3-events.txt <<'EVENTS'
vfio_region_read
vfio_region_write
vfio_pci_read_config
vfio_pci_write_config
vfio_pci_reset
EVENTS

qemu-system-x86_64 \
  -enable-kvm -cpu host -smp 8 -m 4G -machine q35 \
  -kernel /mnt/vmstore/leandro/vm/guest-image/vmlinuz \
  -initrd /mnt/vmstore/leandro/vm/guest-image/initrd \
  -append 'root=/dev/vda1 rw console=ttyS0 net.ifnames=0' \
  -drive file=$W/rootfs.qcow2,if=virtio,format=qcow2 \
  -netdev tap,id=net0,ifname=caraxes0,script=no,downscript=no \
  -device virtio-net-pci,netdev=net0 \
  -device vfio-pci,host=0000:2d:00.0,x-no-mmap=on,multifunction=on \
  -device vfio-pci,host=0000:2d:00.1,x-no-mmap=on \
  -device vfio-pci,host=0000:2d:00.2,x-no-mmap=on \
  -device vfio-pci,host=0000:2d:00.3,x-no-mmap=on \
  -trace events=/tmp/caraxes-r1r3-events.txt,file=$W/r1r3-trace.log \
  -serial file=$W/r1r3-serial.log -display none -nographic
```

`x-no-mmap=on` on `.0` is the part that matters for the trace: without it,
QEMU maps BAR0 straight into the guest and most accesses never reach the
`vfio_region_*` trace points (fast path, no trap) -- exactly the "no mmap"
requirement from the task. The other three functions are passed through
mainly so the whole IOMMU group can be exercised together and the guest sees
the real multi-function device; they do not need tracing.

### 3.4 In the guest

```sh
lspci -nn   # expect the real 10de:1f02, not the synthetic device
sudo modprobe nvidia NVreg_GpuInitOnProbe=1 NVreg_ResmanDebugLevel=0
dmesg | tail -n 100
```

Unlike E0/E1 (synthetic device, deliberately incomplete), this is the real
card with FWSEC and GSP actually executing, so a clean `RmInitAdapter
succeeded` (or a real failure past the GFW wait) is possible, not just the
predicted failure points. Give it a bounded window -- E0/E1 took well under
4 s to reach their respective failures against the emulator; budget several
times that against real firmware before treating it as hung, but do not wait
indefinitely: if there is no forward progress in `dmesg` and no new lines in
`r1r3-trace.log` for on the order of a minute, stop.

### 3.5 Stop condition and clean shutdown

Power the guest off deliberately once either `RmInitAdapter` resolves
(success or failure) or the timeout above is hit -- do not `kill -9` the QEMU
process while the guest driver may be mid-transaction with the card:

```sh
echo 'system_powerdown' | sudo -u silas qemu-system-x86_64 ... # or, if a QMP/monitor socket was added to the command line, use that
```

If no monitor was attached, a plain guest-side `sudo poweroff` is simpler and
just as clean for this purpose; wait for the QEMU process to exit on its own
before moving to section 4.

### 3.6 Reconstructing the VBIOS from the trace

Per [registers-turing.md](registers-turing.md), the PROM window is
`NV_PROM_DATA` at BAR0 offset `0x300000`, up to 1 MiB (`0x300000`-`0x3FFFFF`),
read as consecutive 32-bit little-endian words in ascending address order
(confirmed against the synthetic image in the E1 result: 32930 dword reads
from `0x300000` to `0x3201FC`, in order, covering the whole image). Filter
`r1r3-trace.log` for `vfio_region_read` lines whose region is BAR0 (region
index 0) and whose offset falls in that range, sort by offset, and pack the
values as little-endian `u32`.

The exact field layout of `vfio_region_read`/`vfio_region_write` lines is
defined in QEMU's `hw/vfio/region.c` (or `common.c`), which is **not** part
of the files fetched into `vendor/qemu-vfio-user/` (only `container.c`,
`device.c`, `pci.c`, `proxy.c` and `hw-vfio-pci.c` were pulled in, per
[environment.md](environment.md) -- none of them define this trace point).
Do not trust a hardcoded field order below without checking a handful of real
lines first:

```sh
grep vfio_region_read caraxes-r1r3-trace.log | head -5
```

Upstream QEMU's usual shape for this event is close to
`(<name>:region<index>+0x<offset>, <size>) = 0x<value>` for reads and
`(<name>:region<index>+0x<offset>, 0x<value>, <size>)` for writes, but treat
that as a starting guess, not a verified fact, and adjust the field indices
in the script below to match whatever the first `head -5` actually shows.
Sketch (Python; adjust the regex group order per the confirmed format before
trusting the output):

```python
import re, struct

PROM_LO, PROM_HI = 0x300000, 0x400000
line_re = re.compile(
    r"vfio_region_read\(.*?region(?P<region>\d+)\+0x(?P<off>[0-9a-fA-F]+),"
    r".*?\)\s*=\s*0x(?P<val>[0-9a-fA-F]+)"
)

words = {}
with open("r1r3-trace.log") as f:
    for line in f:
        m = line_re.search(line)
        if not m or int(m["region"]) != 0:
            continue
        off = int(m["off"], 16)
        if PROM_LO <= off < PROM_HI:
            words[off] = int(m["val"], 16)

if not words:
    raise SystemExit("no PROM reads matched -- check the trace line format first")

lo, hi = min(words), max(words) + 4
buf = bytearray(hi - lo)
for off, val in words.items():
    buf[off - lo:off - lo + 4] = struct.pack("<I", val)

with open("vbios-full.bin", "wb") as f:
    f.write(buf)
print(f"{len(buf)} bytes, {lo:#x}-{hi:#x}, {len(words)} words captured")
```

Store the result at `caraxes/reference/rtx2070/vbios-full.bin`. **Note:**
`.gitignore` currently ignores only `caraxes/reference/rtx2070/rom.bin` and
`caraxes/reference/rtx2070/synth-vbios.bin` in that directory (checked
2026-09-23) -- it does **not** yet list `vbios-full.bin`. Add that line (or
otherwise keep the file out of `git add`) before running R1; it is a raw
firmware dump of the real card and should not be staged by accident.

### 3.7 Boot registers (resolves open questions Q1 and Q3)

Filter the same trace for BAR0 region reads at offsets `0x0` (`NV_PMC_BOOT_0`),
`0x4` (`NV_PMC_BOOT_1`) and `0xA00` (`NV_PMC_BOOT_42`). Per
[init-map.md](init-map.md) and the E0 result these are read in three rounds
(probe, start, `RmInitAdapter`), so nine lines total are expected; all three
rounds should agree. [environment.md](environment.md) records the predicted
values from the datasheet encoding (`BOOT_0 = 0x166000A1`, `BOOT_1 = 0`,
`BOOT_42` chip id `0x166` with revision A1) -- record what the real card
actually returns next to those predictions, and update
[open-questions.md](open-questions.md) Q1 and Q3 with the measured values and
the trace file's location. (This document does not edit `open-questions.md`
itself -- that update is the follow-up action after a real run.)

## 4. Recovery

Run this whether or not section 3 finished cleanly. Order matters and is the
reverse of section 2.

1. **Confirm the guest is down.** `ps` for the `qemu-system-x86_64` process
   from 3.3; if it is still running, shut it down as in 3.5 before touching
   host driver state.
2. **Unbind all four functions from `vfio-pci` and clear the override:**
   ```sh
   for f in 0 1 2 3; do
     bdf=0000:2d:00.$f
     echo "$bdf" | sudo tee /sys/bus/pci/drivers/vfio-pci/unbind
     echo | sudo tee /sys/bus/pci/devices/$bdf/driver_override
   done
   ```
   (An empty line clears `driver_override` -- the kernel treats a
   whitespace-only write as "no override set".)
3. **Let the driver core re-match the original drivers:**
   ```sh
   for f in 0 1 2 3; do echo 0000:2d:00.$f | sudo tee /sys/bus/pci/drivers_probe; done
   ```
   Check: `lspci -nnk -s 2d:00` matches the 1.2 table again (`nvidia`,
   `snd_hda_intel`, `xhci_hcd`, `nvidia-gpu`). If `.0` does not pick up
   `nvidia` on its own, `sudo modprobe nvidia` explicitly (module aliases
   should already trigger this from `drivers_probe`, but do not assume it
   silently worked).
4. **Reload the NVIDIA modules in forward order** (opposite of 2.6):
   ```sh
   sudo modprobe nvidia && sudo modprobe nvidia_uvm && \
   sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm
   ```
5. **Rebind the console framebuffer**, once `nvidia_drm` is back:
   ```sh
   cat /proc/fb            # wait for "nvidia-drmdrmfb" to reappear
   echo 1 | sudo tee /sys/class/vtconsole/vtcon1/bind
   ```
6. **Verify:**
   ```sh
   lspci -nnk -s 2d:00
   nvidia-smi
   cat /proc/fb
   ```
   If the monitor itself shows no signal even though the above all look
   correct, the DRM connector may need a nudge -- check
   `cat /sys/class/drm/card*/status` (read-only) for the connector state,
   and if all else fails, physically wake/replug the display.
7. **Restart the desktop session.** As found in 1.6, nothing on this machine
   restarts Hyprland automatically -- there is no display manager, no
   shell-profile `exec`, and no `systemctl --user` unit for it. The
   guaranteed way is logging in on the physical tty1 console and running
   `start-hyprland`, same as normally. **This step cannot be done from the
   SSH/tmux session that ran the rest of this procedure** -- it needs a body
   at the keyboard, or physical access, or a pre-arranged way for the
   operator to trigger it themselves. Treat this as the plan's known gap, not
   something to script around with a guess.

### If the card does not come back

- Retry `modprobe nvidia` once; check `dmesg` for the actual failure (Xid
  code, timeout, or GSP-related message) before doing anything else.
- Function-level reset, function `.0` only (the only one with a `reset` file,
  per 1.8):
  ```sh
  echo 1 | sudo tee /sys/bus/pci/devices/0000:2d:00.0/reset
  ```
- Group-wide fallback: a secondary-bus reset at the upstream bridge, which
  per 1.1/1.8 only affects this card's four functions:
  ```sh
  echo 1 | sudo tee /sys/bus/pci/devices/0000:00:03.1/reset
  ```
  Do this only while all four functions are unbound from any active user (no
  VM attached, `vfio-pci` or no driver), then repeat steps 3-6 above.
- Last resort: `systemctl reboot`, run interactively by a human who has
  decided the desktop is not coming back cleanly -- not something an
  unattended script should trigger silently (see the watchdog skeleton
  below: it stops short of a reboot and instead alerts).

### Unattended, timeout-driven variant (script skeleton)

This restores host driver and PCI state even if the SSH session, tmux, or the
whole agent session running this plan disappears mid-run. It does **not**
solve step 7 above (nothing here can log in on tty1) -- its job is to
guarantee sections 2 and 3 do not leave the card in an unbound, half-migrated
state forever.

```sh
#!/usr/bin/env bash
# Skeleton only -- fill in before use, this has not been run.
set -euo pipefail
BUDGET_SECS=1800          # whole run must finish inside this
HEARTBEAT=/run/caraxes/heartbeat
mkdir -p /run/caraxes

recover() {
    # Section 4, steps 1-6. Idempotent: safe to call from a state where
    # some of this already happened.
    : # qemu shutdown, vfio-pci unbind, drivers_probe, modprobe forward
      # order, vtcon1 rebind -- the exact commands from section 4.
}
trap recover EXIT TERM INT

run_experiment() {
    touch "$HEARTBEAT"
    : # section 2 (detach), section 3 (R1/R3 run) -- touch "$HEARTBEAT"
      # again after each step so the watchdog knows the main script is
      # still alive and making progress, not just still running.
}

# Start the watchdog FIRST, detached from this shell and from the SSH
# session, before run_experiment does anything destructive. It must
# survive the parent dying; systemd-run --user (or setsid + nohup, if
# systemd-run is unavailable) both satisfy that.
systemd-run --user --unit=caraxes-passthrough-watchdog \
    --property=Type=oneshot \
    /bin/bash -c '
        deadline=$(( $(date +%s) + '"$BUDGET_SECS"' ))
        while [ "$(date +%s)" -lt "$deadline" ]; do
            sleep 15
            [ -f "'"$HEARTBEAT"'" ] || continue
            # stale heartbeat (no touch in the last 60s) => main script
            # is stuck or gone; force recovery independently.
            if [ "$(( $(date +%s) - $(stat -c %Y "'"$HEARTBEAT"'") ))" -gt 60 ]; then
                : # run the same section-4 commands here, standalone --
                  # this copy must not depend on anything run_experiment
                  # set up in-process.
                exit 0
            fi
        done
        : # budget exceeded with no stale-heartbeat trigger yet => force
          # recovery here too, then stop; do not reboot from this path.
    '

run_experiment
```

Two independent triggers for recovery: the `trap` in the main script (covers
a clean failure or an operator-sent signal) and the detached watchdog unit
(covers the main script, its shell, or the whole SSH/tmux session vanishing
without a trapped signal -- e.g. the network dropping). Both call the same
section-4 sequence; recovery must be safe to run twice.

## 5. Risks and operator decisions

- **Time window.** Detach (section 2) is a couple of minutes; guest boot is
  comparable to the E0/E1 runs (well under a minute to the point
  `modprobe nvidia` is issued); the real driver init in 3.4 has no timing
  precedent (E0/E1 both used the synthetic device, which fails fast by
  design) -- budget generously, on the order of 30-60 minutes end to end
  including recovery, and expect to be away from the physical desktop for
  that whole window.
- **Data to save first.** Exactly the processes 1.7 found holding
  `/dev/nvidia*`: whatever is open in `kitty`, `firefox`, `Discord`,
  `spotify`. Confirm nothing unsaved is open in any of them before 2.3.
- **The second machine and SSH.** Not just "does a second machine exist" --
  1.5 found `sshd` itself `inactive`/`disabled` on this host. That has to be
  fixed, and the second machine's access tested end to end (connect, get a
  shell, start tmux), before scheduling a run. Do this well before the
  window, not as part of the same session.
- **GSP/SEC2 hang risk.** Per the safety table in
  [experiments.md](experiments.md), writes to falcon/WPR registers can hang
  GSP-RM (Xid 119/120) and need a reboot to clear. This procedure only reads
  (traces) the real init sequence; it does not add any register writes beyond
  what the stock driver already does on every normal boot. The residual risk
  is a guest-driver bug or an unexpected interaction between `vfio-pci`'s
  `x-no-mmap` trapping and the driver's timing expectations, not anything
  this plan asks the driver to do differently.
- **The display may not return on its own signal-wise** even after every
  software check in section 4.6 passes (DRM connector hotplug can stay
  stuck) -- be prepared to physically wake or replug the monitor.
- **No automatic desktop restart exists** (1.6): full recovery to "operator
  can use the desktop again" needs a human at the physical console for the
  last step, regardless of how well the scripted part goes. Decide in
  advance who does that and when.
- **`.gitignore` gap** (3.6): add `vbios-full.bin` to it, or otherwise make
  sure it is never staged, before the real VBIOS lands in the working tree.

## 6. Checklist (paste into the tmux session)

```
CARAXES R1/R3 -- pre-flight
[ ] sshd enabled+running on manacor; second machine's access to it tested end to end
[ ] connected via SSH, inside tmux, from the second machine (not a shell inside the Wayland session)
[ ] re-ran section 1 checks in this session; nothing has changed since the plan was written
[ ] kitty / firefox / Discord / spotify: confirmed nothing unsaved open
[ ] time budget agreed with operator (30-60 min), operator reachable for the physical-console step
[ ] watchdog unit started and its heartbeat file confirmed fresh, BEFORE the next section

Detach (section 2)
[ ] 2.3  loginctl terminate-session 2 -> loginctl list-sessions no longer shows it
[ ] 2.4  sudo lsof /dev/nvidia*  -> empty
[ ] 2.5  vtcon1 bind -> 0
[ ] 2.6  modprobe -r nvidia_drm, nvidia_modeset, nvidia_uvm, nvidia -> lsmod clean
[ ] 2.7  driver_override=vfio-pci + unbind + drivers_probe, all four functions
[ ] 2.8  lspci -nnk -s 2d:00 all vfio-pci; /dev/vfio/28 exists

Run (section 3)
[ ] 3.1  overlay ready (vm0 reused, or a fresh one built per kickoff.md 5.6)
[ ] 3.2  qemu-system-x86_64 -trace help confirms vfio_region_read/write exist
[ ] 3.3  QEMU launched with x-no-mmap=on on .0, -trace events=... file=...
[ ] 3.4  guest: modprobe nvidia NVreg_GpuInitOnProbe=1 NVreg_ResmanDebugLevel=0; dmesg watched
[ ] 3.5  guest powered off deliberately, not killed
[ ] 3.6  vbios-full.bin reconstructed, saved under caraxes/reference/rtx2070/, gitignore updated first
[ ] 3.7  BOOT_0/1/42 real values recorded against the environment.md predictions

Recovery (section 4) -- run even on failure
[ ] 4.1  qemu process confirmed gone
[ ] 4.2  vfio-pci unbind + driver_override cleared, all four functions
[ ] 4.3  drivers_probe -> lspci -nnk -s 2d:00 matches the original table
[ ] 4.4  modprobe nvidia, nvidia_uvm, nvidia_modeset, nvidia_drm (forward order)
[ ] 4.5  /proc/fb shows nvidia-drmdrmfb again, then vtcon1 bind -> 1
[ ] 4.6  nvidia-smi works; display has signal (physically check/replug if not)
[ ] 4.7  operator at physical tty1: start-hyprland
[ ] open-questions.md Q1/Q3 updated with the real BOOT_0/1/42 values (follow-up, separate change)
```
