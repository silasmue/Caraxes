<!-- SPDX-License-Identifier: GPL-2.0-only -->
# Reading guide: `caraxes_dma_range`

This walks the four source files in this directory in the order you'd naturally read
them, and for every non-obvious kernel mechanism says what it is, why this module
needs it here, what else could have been used, and where to go read the kernel's own
code. It assumes you already know C and are comfortable in a kernel source tree, but
have not written a Linux kernel module before.

Line numbers below are cited against two trees: this machine's own installed headers
under `/usr/lib/modules/7.2.4-arch1-2/build/include/` (read directly, exact), and the
`v6.8` tag of `torvalds/linux` on GitHub (fetched and grepped, exact for that tag; the
guest's `6.8.0-136-generic` is an Ubuntu build and may shift a line or two). Where
neither was available, file and function only.

**Why this module exists** (read first: `docs/vfio/sysmem-iova.md` §5, removal
plan, and §3(b)/§4, design and rationale; short version in this directory's
`reference.md` and atop `caraxes_dma_range.c`): the Leandro VFIO guest has no vIOMMU, so every
DMA address the guest kernel hands the passthrough-shaped NVIDIA GPU is a guest
physical address, but the physical GPU on the host lives in a translated IOMMU domain
and only reaches the IOVAs the host mapped guest RAM at. This module installs a
per-device translation table (`dev->dma_range_map`) so the guest kernel's own DMA API
does that translation for every driver that binds, with **no NVIDIA symbols and no
kernel or driver patch**. It is decision D13: temporary, removed once one of three
conditions in sysmem-iova.md §5 holds (stock ACPI `_DMA`, host-side IOVA=GPA, or a
model change that stops guest addresses reaching the GPU at all).

## The files

| File | Role |
|---|---|
| `caraxes_dma_range.c` | The module itself |
| `caraxes_dma_range_pci.h` | Device lookup, shared with the test module |
| `caraxes_dma_range_test.c` | A second module that exercises the DMA API against the installed map |
| `Makefile` | Out-of-tree kbuild boilerplate for both modules |
| `reference.md` | Operator-facing reference: parameters, behaviour, readback, build/load order |

## Module parameters: the plain ones, and the one that couldn't be plain

`bdf` and `force` (lines 41–49) use the ordinary `module_param(name, type, perm)`
macro — a **module parameter** is a variable the module-loading code (`insmod`, or
`modprobe` reading `options` lines) can set from a `name=value` argument, which sysfs
also exposes under `/sys/module/<name>/parameters/<name>` per `perm`. It's the
standard way an out-of-tree module takes configuration with no companion userspace
program, config file, or ioctl — exactly the "no patch, no extra moving part" brief
this module is under. See `include/linux/moduleparam.h` (doc comment at lines
114–140 on this host) and `kernel/params.c` for the plumbing.

`ranges` cannot be a plain `charp` parameter. A `charp` parameter is set through
`param_set_charp()`, which caps the string at 1024 bytes:

```c
/* kernel/params.c:262-270, v6.8 */
size_t len, maxlen = 1024;
len = strnlen(val, maxlen + 1);
if (len == maxlen + 1)
	return -ENOSPC;             /* "%s: string parameter too long\n" */
```

64 entries of `cpu_start:dma_start:size` in hex is about 2.5 KiB, comfortably over
that — not theoretical: `caraxes/runs/dma-range-2026-09-23/commands.txt` records that
the first attempt used a plain `charp` and `insmod` failed with `ranges: string
parameter too long` for a 65-entry table, which is what motivated the custom
parameter ops (`a-refusals.txt`, the run after the fix, instead fails that same table
with "Argument list too long" — `insmod`'s own command-line ceiling, not the
kernel's, and not this module's problem to solve).

The fix (lines 71–101) is a **custom `struct kernel_param_ops`**: `.set`, `.get`, and
`.free` callbacks on a plain `char *ranges`, registered with
`module_param_cb(name, ops, arg, perm)` instead of `module_param`. `cdr_ranges_set()`
just `kstrdup()`s the value with no length limit; `cdr_ranges_get()` prints it back;
`cdr_ranges_free()` runs when the module unloads (or load fails) so the last copy
doesn't leak. `struct kernel_param_ops` is three function pointers plus a flags word
(`include/linux/moduleparam.h:60-69`). Patching `param_set_charp()`'s limit was never
on the table (a kernel patch, and it would raise the limit for every `charp`
parameter, not just this one); splitting `ranges` into several numbered parameters
would still need an arbitrary cap and is uglier to parse. A callback pair on a plain
pointer is the minimal way to get an unbounded string through the parameter machinery.

## Parsing, validating, and proving coverage

`cdr_parse()` and `cdr_parse_one()` (lines 121–185) are ordinary string splitting —
`strsep()` on `,` then `:`, `kstrtou64()` base 16 per field — nothing kernel-specific.
`cdr_validate()` (192–239) checks each range is a non-zero multiple of `SZ_2M`
(`include/linux/sizes.h`), both starts are `PAGE_ALIGNED`, nothing wraps the address
space, and no two ranges overlap in CPU or DMA space; a range whose CPU and DMA starts
differ by something not a multiple of 2 MiB loads but with a `pr_warn`, since it would
misalign the GPU's big (2 MiB) pages (sysmem-iova.md §2). Every one of these refusal
messages appears verbatim in `caraxes/runs/dma-range-2026-09-23/a-refusals.txt` from
the real test run — a good companion while reading this function.

The interesting kernel mechanism is `cdr_check_coverage()` (241–275): the table must
translate **every** System RAM page, because dma-direct's own lookup
(`translate_dma_to_phys()`, next section) returns `DMA_MAPPING_ERROR` for an uncovered
address, and — worse — the guest's 6.8 kernel has a real infinite loop for a RAM range
a map covers only *partway*. That loop lives in `check_ram_in_range_map()`,
`kernel/dma/direct.c:595-618` (v6.8), called by `dma_direct_all_ram_mapped()` a few
lines below it, which backs `dma_addressing_limited()`:

```c
const struct bus_dma_region *bdr = NULL;   /* declared once, outside the loop */
while (start_pfn < end_pfn) {
	for (m = dev->dma_range_map; PFN_DOWN(m->size); m++)
		/* bdr = m; break; on a match */;
	if (!bdr) return 1;
	start_pfn = PFN_DOWN(bdr->cpu_start) + PFN_DOWN(bdr->size);
}
```

`bdr` is never reset inside the `while`. If the first map entry covers `start_pfn` but
ends before `end_pfn`, the loop advances `start_pfn` to that entry's end; if no entry
covers the new `start_pfn` (a gap), the inner `for` finishes without a `break`, but
`bdr` still holds the *previous* match — so `if (!bdr) return 1;` never fires, and
`start_pfn` is recomputed to exactly the value it already had. The function spins
forever with interrupts and an RCU read lock (via `walk_system_ram_range()`) held.
`cdr_check_coverage()` exists so this module's own load path can never hand the guest
kernel a map that hits it: "cover every RAM page or refuse to load" isn't stylistic
caution, it decides whether the box gets stuck.

To do the coverage walk, the natural function is `walk_system_ram_range()` —
`kernel/resource.c:507-529` (v6.8) — but it is **not exported** (no `EXPORT_SYMBOL*`
follows it, confirmed by grep against that file), so an out-of-tree module can't link
it. The exported equivalent is `walk_iomem_res_desc()`, `kernel/resource.c:411`,
`EXPORT_SYMBOL_GPL` at line 416, declared in `include/linux/ioport.h:441` on this
host; `cdr_check_coverage()` calls it with `IORESOURCE_SYSTEM_RAM | IORESOURCE_BUSY`
and `IORES_DESC_NONE` (constants at `ioport.h:75` and `:137`) — exactly the flag pair
`walk_system_ram_range()` itself passes internally to `find_next_iomem_res()`. Both
walk the same `/proc/iomem` tree; the module only had to re-supply the flags
`walk_system_ram_range()` would otherwise hard-code for it.

## Finding the device, and who owns the reference

`cdr_find_pci_dev()` (`caraxes_dma_range_pci.h:18-46`) either parses a `[DDDD:]BB:DD.F`
string with `sscanf()` and calls `pci_get_domain_bus_and_slot(domain, bus, devfn)`, or
scans by class: `pci_get_class(PCI_CLASS_DISPLAY_VGA << 8, NULL)`, then the
3D-controller class, filtering on `pdev->vendor == PCI_VENDOR_ID_NVIDIA`. Both are
declared in `include/linux/pci.h:1308-1310` on this host. `pci_get_domain_bus_and_slot()`
is `drivers/pci/search.c:228-240` (v6.8): it walks `for_each_pci_dev()` and returns
the match *without* an extra `pci_dev_get()`, because `for_each_pci_dev()` itself
calls `pci_get_device()` on every step, which already takes the reference — the one
returned is the one the caller now owns. `pci_get_class()` (`search.c:351-364`) is a
thin wrapper deferring to `pci_get_dev_by_id()` (`search.c:268-282`), which calls
`bus_find_device(&pci_bus_type, from, id, match_pci_dev_by_id)` (`bus_find_device()`
takes the reference on a match) and then unconditionally `pci_dev_put(from)`s the
*previous* candidate. That's why `cdr_find_pci_dev()`'s
`while ((pdev = pci_get_class(classes[i], pdev)))` loop is reference-safe as written:
every rejected candidate is put by the next call, and only the final match, returned
to `cdr_init()`, needs the caller's own `pci_dev_put()` — done on every exit path of
`cdr_init()` (`unlock:` label) and in `cdr_exit()`. The alternative, walking
`pci_bus_type`'s device list with `bus_for_each_dev()` and a private match function,
is what these helpers already do; existing as generic, exported, reference-counted
searches is why this module needs no NVIDIA-specific code to find "the" GPU.

## `device_lock`: why a private mutex would not do

`struct device` carries its own `struct mutex mutex`, and `device_lock()`/
`device_unlock()` (`include/linux/device.h:1102-1120` on this host) are just
`mutex_lock(&dev->mutex)`/`mutex_unlock(&dev->mutex)`. The driver core takes this same
lock around every probe, remove, and bus-notifier call — the notifier declaration in
`include/linux/device/bus.h:275-277` says so explicitly ("bus notifiers are likely to
be called with the device lock already held"). A module-private mutex could serialize
this module's *own* accesses to `cdr_table`/`cdr_installed`, but could never stop a
bind from racing the install: only holding `dev->mutex` itself, the same lock
`really_probe()` holds across `dma_configure()` and `->probe()`, guarantees "no driver
can start probing between my check and my install." That's why `cdr_init()` takes
`device_lock(dev)` before checking `dev->driver` and `dev->dma_range_map`, and why
`cdr_lock` (lines 51–54) is a second, narrower mutex nested inside it, protecting this
module's globals against its own bus-notifier callback (which runs with `dev->mutex`
already held by the driver core), not against the driver core itself.

## Installing the map, and how dma-direct reads it

`struct device` has a `const struct bus_dma_region *dma_range_map` field (used, not
declared, in this file — declared in `include/linux/device.h`); `struct bus_dma_region`
itself is in `include/linux/dma-direct.h:20-24` on this host: three fields,
`cpu_start`, `dma_start`, `size` (no `offset` — see below).

`cdr_install()` (278–289) builds the copy the device gets with `kmemdup(cdr_table,
(cdr_n + 1) * sizeof(*map), GFP_KERNEL)` — `kmemdup()` is `kmalloc()` then `memcpy()`
in one call (`mm/util.c:127-136`, v6.8; on this host a macro at
`include/linux/string.h:302` forwards to `kmemdup_noprof()`, a post-6.8
allocation-profiling wrapper with the same semantics). The array is zero-terminated
(`kcalloc(CDR_MAX_RANGES + 1, ...)` in `cdr_parse()`, one slot more than entries, left
zero), the loop-termination convention every map reader uses:
`for (m = dev->dma_range_map; m->size; m++)`.

That reader is `translate_phys_to_dma()` (`include/linux/dma-direct.h:26-40`):

```c
for (m = dev->dma_range_map; m->size; m++) {
	u64 offset = paddr - m->cpu_start;
	if (paddr >= m->cpu_start && offset < m->size)
		return m->dma_start + offset;
}
return DMA_MAPPING_ERROR;
```

`phys_to_dma()` (same file, 99–102) calls this whenever `dev->dma_range_map` is
non-`NULL`; every `dma_map_page()`/`dma_alloc_coherent()` the bound driver makes for
this device goes through it. `dma_to_phys()` (104-115) does the reverse via
`translate_dma_to_phys()` (42–55) — what `caraxes_dma_range_test.c` checks against.
`bus_dma_region` has **no `offset` field** on this host or the guest's 6.8, but it did
through 6.7 — `include/linux/dma-direct.h:20-25` at the `v6.7` tag has a fourth
`u64 offset;` member, and `translate_phys_to_dma()` there computes `paddr - m->offset`
instead of `m->dma_start + offset`. That's what `cdr_parse_one()`'s
`LINUX_VERSION_CODE` guard (below) fills in for pre-6.8 kernels.

This mechanism — a per-device array of CPU-to-DMA ranges baked generically into
dma-direct — is also precisely what firmware-described DMA ranges (Open Firmware
`dma-ranges`, ACPI `_DMA`) populate through `of_dma_get_range()`/`acpi_dma_get_range()`
before a driver probes: option (a) in sysmem-iova.md §3 is "firmware does this instead
of a guest module," and this module is that substitute until Cloud Hypervisor can
emit `_DMA` (sysmem-iova.md §5, item 1). Replacing `dev->dma_ops` with a custom set
adding a fixed offset would mean reimplementing every mapping operation instead of
reusing dma-direct's — no lighter alternative plugs into the same generic path.

## The bus notifier, and who actually frees the map

A single install at module-load time would only last until the bound driver unbinds:
the driver core's `device_unbind_cleanup()`, `drivers/base/dd.c:547-557` (v6.8),
unconditionally runs `kfree(dev->dma_range_map); dev->dma_range_map = NULL;` (plus
`devres_release_all()` and `arch_teardown_dma_ops()`) on every unbind *and* every
failed probe. Inside `really_probe()` (`dd.c:603` onward) it's reached both from the
`test_remove` re-probe path (line 698) and the general failure unwind at
`pinctrl_bind_failed:` (line 724, after `BUS_NOTIFY_DRIVER_NOT_BOUND` at line 719);
also from `__device_release_driver()` (defined at 1241) at line 1277, just before
`BUS_NOTIFY_UNBOUND_DRIVER` at line 1283. So a map this module installed survives
exactly one bind/unbind cycle unless something reinstalls it.

That something is `bus_register_notifier(&pci_bus_type, &cdr_nb)`
(`include/linux/device/bus.h:259` on this host), registered under `device_lock()` so
no bind can slip in before it (comment at line 544). `cdr_notify()` (315–378) handles
five events, all declared in the `enum bus_notifier_event` at
`include/linux/device/bus.h:281-290`:

- **`BUS_NOTIFY_BIND_DRIVER`** — raised inside `driver_sysfs_add()` (`dd.c:439`),
  called from `really_probe()` at line 645, *after* `dma_configure()` (lines 639–640)
  and *before* `call_driver_probe()` (line 658). If the device has no map (the driver
  core just freed the last one, or this is the first bind), `cdr_notify()` reinstalls
  a fresh `kmemdup()` copy right here — this is what makes `rmmod nvidia; modprobe
  nvidia` keep working without reloading this module. The real run's log shows it:
  `nvidia 0000:00:05.0: re-installed 4 ranges before nvidia probes`
  (`caraxes/runs/dma-range-2026-09-23/b-identity-nvidia.txt:163`).
- **`BUS_NOTIFY_BOUND_DRIVER`** — raised in `driver_bound()` (`dd.c:420`), after a
  successful probe. Handled below (module pin).
- **`BUS_NOTIFY_DRIVER_NOT_BOUND`**/**`BUS_NOTIFY_UNBOUND_DRIVER`** — both mean "the
  driver core just freed the map"; `cdr_notify()` forgets `cdr_installed` either way
  and drops the module pin if held. Same log:
  `pci 0000:00:05.0: driver unbound; the driver core freed the map, the next bind
  gets a new copy` (line 159).
- **`BUS_NOTIFY_DEL_DEVICE`** — logged only; the driver core's `device_release()`
  also `kfree()`s `dma_range_map` when the device itself goes away.

The alternative — re-inserting this module before every future `modprobe nvidia`,
forever — is fine for a one-shot experiment, not for a harness reloading the driver
repeatedly across VM runs.

## Pinning the module while a driver is bound

`try_module_get()`/`module_put()` (`include/linux/module.h`, declared 709/719, inline
fallbacks 722-729 on this host) add or drop a reference on `THIS_MODULE`; while the
count is positive, `rmmod` fails with `ERROR: Module ... is in use`. `cdr_notify()`
takes one unconditional `__module_get(THIS_MODULE)` (line 347) the first time it sees
`BUS_NOTIFY_BOUND_DRIVER`, and gives it back with `module_put()` at
`UNBOUND_DRIVER`/`DRIVER_NOT_BOUND`. `__module_get()` rather than the failable
`try_module_get()` is correct here: the module can't be mid-unload while its own
notifier callback is running, so there's nothing to check. The real run confirms the
pin: `rmmod: ERROR: Module caraxes_dma_range is in use` (`b-identity-nvidia.txt:51`),
while nvidia was still bound. The alternative — refusing `rmmod` in `cdr_exit()` by
checking `dev->driver` — has a race (a bind could start between check and unload);
pinning the module makes the kernel's own unload path enforce it instead.

## Sysfs readback, and why the read-only parameters still need a `.set`

`table`, `live`, and `status` (402–485) are `module_param_cb()` parameters whose `.set`
is `cdr_ro_set()`, which just `return -EPERM;`. All three are `0444` already, so a
sysfs `write()` can never reach `.set` — sysfs itself checks the mode before
`param_attr_store()` runs (`kernel/params.c:576-594`, which also checks
`if (!attribute->param->ops->set) return -EPERM;` before calling it). But
`module_param_cb()` parameters can also be set at `insmod` time from the command
line, and *that* path, `parse_one()` (`kernel/params.c:117-149`), calls
`params[i].ops->set(val, &params[i])` with **no null check** once the name matches: a
`.set == NULL` ops struct used with `module_param_cb()` would crash the first time
someone typed `insmod caraxes_dma_range.ko table=x`. `cdr_ro_set()` exists purely to
turn that into a clean `-EPERM`. `cdr_table_get()` prints the table this module would
install; `cdr_live_get()` takes `device_lock()` first (so the driver core can't free
`dma_range_map` mid-print) and prints what the device carries now; `cdr_status_get()`
reports device/driver/map/pin state as one line. `cdr_print()` (393–400) formats each
range as `0xcpu:0xdma:0xsize`, so `table` joined with commas is a valid `ranges=`
value again — reference.md's `diff "$P/table" "$P/live"` check relies on that.

## Version guards, error paths, and the `-Werror` build

The one `#if LINUX_VERSION_CODE < KERNEL_VERSION(6, 8, 0)` (lines 143–146) fills the
`offset` field `struct bus_dma_region` carried through 6.7 (previous section).
`LINUX_VERSION_CODE`/`KERNEL_VERSION()` come from `<linux/version.h>`, generated by
kbuild from the build tree (`include/generated/uapi/linux/version.h` defines
`LINUX_VERSION_CODE 459268` on this host, i.e. `KERNEL_VERSION(7,2,4)`). Two source
files, or a Makefile `#define` keyed off `uname -r`, would be more machinery for one
field.

Every error path in `cdr_init()`/`cdr_exit()` unwinds exactly what was acquired: a
failed `cdr_install()` after `bus_register_notifier()` succeeded rolls the notifier
back out and restores `dev->dma_range_map` (lines 545–555); the `unlock:`/
`free_table:` labels drop `device_lock()`, then `pci_dev_put()`, then
`kfree(cdr_table)`, in that order, on every failure branch, so no path leaks the PCI
reference or the table. `caraxes/runs/dma-range-2026-09-23/commands.txt` records the
real build command: `make -C /lib/modules/$(uname -r)/build M=$PWD KCFLAGS=-Werror
W=1 modules` — `-Wall` plus kbuild's extra warning tier promoted to errors, noting "no
warnings with `W=1` `-Werror`". `tools/check.sh` doesn't build guest kernel modules
(`.github/workflows/check.yml` does that, plain `KCFLAGS=-Werror`, for the sibling
`nvrm_nodes`/`virtio_nvrm` modules); it checks the SPDX header and repo invariants.

## The test module, briefly

`caraxes_dma_range_test.c` is a second, independent module proving the installed map
actually changes what the DMA API returns, without touching the GPU itself — with no
vIOMMU, `dma_map_page()`/`dma_alloc_coherent()` on this device only *compute* an
address via `phys_to_dma()`, never talking to hardware. It refuses to load if a driver
is bound (`device_lock()` held for its whole lifetime, so nothing can bind meanwhile),
reads `dev->dma_range_map` directly, sets a 47-bit DMA mask (matching `nvidia.ko` for
this GPU) with `dma_set_mask_and_coherent()`, then for 256 fresh pages, 8 coherent
allocations, and the first/middle/last page of every range, maps it and checks the
returned `dma_addr_t` equals guest-physical-address-plus-offset, and that
`dma_to_phys()` inverts it. It logs one line per range and `PASS`/`FAIL`
(`caraxes/runs/dma-range-2026-09-23/b-identity-nvidia.txt:121-126` has a real PASS
run); its `module_init` returns `-EIO` on failure, so `insmod`'s exit code is the
test result.

## Experiments to try

All of these are done in the vm1 guest, where the sources are at
`~/caraxes_dma_range` (copy them there the way `commands.txt` does, or edit in place
if they're already there), building with
`make -C /lib/modules/$(uname -r)/build M=$PWD KCFLAGS=-Werror W=1 modules`. Reset the
GPU model between attempts with
`echo 1 | sudo tee /sys/bus/pci/devices/0000:00:05.0/reset` before `modprobe nvidia`, as
`tools/caraxes-vm.sh probe` does. Do this only in the guest — never against the host's
NVIDIA state.

1. **Drop coverage of one entry.** Load the identity table missing its last range
   (`ranges=0:0:40000000,40000000:40000000:40000000,80000000:80000000:40000000`, no
   `[4 GiB, 5 GiB)` entry). Expect `insmod` to fail with `Invalid parameters` and
   `dmesg` to show `System RAM [...]: 0x100000000 is not covered by any range` —
   `cdr_check_coverage()` refusing before the map is ever installed.
2. **Load, test, read back.** Load the identity table, run
   `sudo insmod caraxes_dma_range_test.ko`, check `dmesg` for `PASS (0 failures)`,
   then `sudo rmmod caraxes_dma_range_test` and diff
   `/sys/module/caraxes_dma_range/parameters/{table,live}` — identical, confirming
   what's on the device matches what this module thinks it installed.
3. **Break big-page alignment on purpose.** Shift one range's `dma_start` by an odd
   multiple of 4 KiB relative to its `cpu_start`, e.g.
   `ranges=0:1000:40000000,40000000:40000000:40000000,...`. Expect the load to
   *succeed* but log `cpu and dma start differ modulo 2 MiB; GPU big pages lose
   their alignment` — a soft warning, not a refusal; compare to experiment 1's hard
   failure and see which `cdr_validate()` checks return `-EINVAL` versus `pr_warn()`.
4. **Watch the bind/unbind cycle.** With the map installed and no driver bound, run
   `sudo modprobe nvidia`, `sudo rmmod nvidia_uvm nvidia`, `sudo modprobe nvidia`
   again, watching `dmesg -w`. Expect `... bound, DMA range map in place`, then
   `driver unbound; the driver core freed the map, the next bind gets a new copy`,
   then `re-installed N ranges before nvidia probes` — the whole
   `BUS_NOTIFY_BIND_DRIVER`/`BOUND_DRIVER`/`UNBOUND_DRIVER` cycle in one run, with
   `cat .../parameters/status` confirming `map=ours` after each reload.
5. **Try to unload while pinned.** With nvidia bound (`status` shows `pinned=1`),
   run `sudo rmmod caraxes_dma_range` and expect `Module caraxes_dma_range is in
   use`. Then `sudo rmmod nvidia_uvm nvidia` and retry — it now succeeds, showing
   `try_module_get()`/`module_put()` doing what the file's top comment promises.

## Pitfalls this module already avoids

- **Partial coverage on a 6.8 kernel hangs the box.** `check_ram_in_range_map()`
  (`kernel/dma/direct.c:595-618`, v6.8) loops forever if a System RAM range is
  covered at its start but not to its end, because its `bdr` match variable is never
  reset between outer-loop iterations. `cdr_check_coverage()` requires whole-range
  coverage before install, so this module's own load path can't trigger it — a table
  edited to pass validation and still leave a gap remains dangerous on this kernel;
  fixed upstream by 7.x's `dma_find_range()` per this directory's `reference.md`.
- **Changing the map under a live mapping.** `cdr_init()` refuses to install
  (`-EEXIST`, or `-EBUSY` if a driver is bound) unless `force=1`, and never while a
  driver is bound — a bound driver's cached DMA addresses would silently translate
  to the wrong physical page if the table changed under it.
- **The driver core owns the installed copy, not this module.** `device_unbind_cleanup()`
  (`drivers/base/dd.c:547-557`) `kfree()`s `dev->dma_range_map` on every unbind and
  failed probe, unconditionally, whoever installed it. The module never assumes
  `cdr_installed` is still valid after any bus-notifier event; it treats
  `UNBOUND_DRIVER`/`DRIVER_NOT_BOUND` as "the driver core just freed this," full
  stop — the real run's log confirms the probe-failure variant happens too
  (`commands.txt`: "driver probe failed; the driver core freed the map, the next
  bind gets a new copy").
- **`rmmod` while a driver is bound.** Would leave the driver holding a pointer into
  memory this module just freed. The `try_module_get()`/`module_put()` pin around
  `BOUND_DRIVER`/`UNBOUND_DRIVER` makes the kernel's own refcount check refuse the
  unload, instead of relying on this module remembering to check.
- **A read-only sysfs parameter with a `NULL` `.set`.** Would `NULL`-deref the first
  time someone passed that parameter's name on the `insmod` command line: `parse_one()`
  (`kernel/params.c:117-149`) calls `ops->set()` unconditionally once the name
  matches, before any `0444` sysfs-mode check applies. `cdr_ro_set()` turns that into
  a plain `-EPERM`.
