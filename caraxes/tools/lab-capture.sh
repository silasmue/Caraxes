#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
#
# Caraxes lab capture kit: read-only facts about an NVIDIA GPU and its host,
# for building a synthetic identity of that chip later (decision D14). The
# operator checklist is docs/caraxes/lab-blackwell.md.
#
#   caraxes/tools/lab-capture.sh [options]
#
#   -o DIR          parent directory for the capture (default: current directory)
#   --bdf BDF       capture only this GPU (default: every NVIDIA display controller)
#   --rom           also read the PCI option ROM through sysfs. Needs sudo; this is
#                   the only step that writes anything to the card: the sysfs "rom"
#                   switch sets the ROM BAR decode enable bit, and it is cleared
#                   again right after the read. Off unless given.
#   --prom          also read the VBIOS through the BAR0 PROM window (NV_PROM_DATA,
#                   BAR0 0x300000, 1 MiB) the way the driver and nova-core read it:
#                   sysfs resource0 opened read-only and mapped PROT_READ, aligned
#                   32-bit loads, nothing written. This is the only copy of the full
#                   NVIDIA image chain (the FWSEC image behind the PCI last-image
#                   flag). Needs sudo, and a BAR0 no driver holds on kernels with
#                   CONFIG_IO_STRICT_DEVMEM (else recorded as failed, nothing
#                   read); off unless given (operator decision of 2026-09-24,
#                   docs/caraxes/blackwell-gb206.md section 6).
#   --no-sudo       never call sudo, not even "sudo -n true" (sudo-only items are
#                   recorded as skipped)
#   --no-rm-tools   skip part 2 (the RM query tools built with rustc)
#   --ogkm DIR      open-gpu-kernel-modules tree at the running driver version, for
#                   gsp-abi-sizes in part 2 (default: $CARAXES_OGKM, else
#                   vendor/open-gpu-kernel-modules when its version.mk matches the
#                   running driver); the field lists follow the running
#                   driver's major version (gsp-abi-sizes.sh --abi: 610 lists below
#                   615, 615 lists from 615 on)
#   --walk-rom FILE only run the ROM image chain walk on FILE and exit
#   -h, --help      this text
#
# Part 1 (core) needs bash, coreutils, pciutils and nvidia-smi; python3 is used for
# the ROM walk and the CSV transposition when present. Part 2 needs rustc (and cc
# for the ABI check and gsp-abi-sizes) and this repository around the script.
#
# Towards the card and the driver everything is read-only: file reads under /proc
# and /sys, lspci, nvidia-smi queries, and RM getter controls on a private RM
# client in part 2. No module is loaded or unloaded, no register is read or
# written, nothing resets the card. The one exception is --rom (see above);
# --prom reads BAR0 registers (the PROM window only) and writes none.
#
# Result: DIR/lab-capture-<chip>-<host>-<stamp>/ with MANIFEST.txt and
# SHA256SUMS, and the tarball DIR/lab-capture-<chip>-<host>-<stamp>.tar.gz with
# its .sha256 next to it. Usernames, the home directory and e-mail addresses are
# replaced in every text file; board serial numbers are redacted, and so are the
# per-card bytes of the config-space dump (PBI mailbox data, Device Serial Number).
set -uo pipefail
umask 022
export LC_ALL=C

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../.." 2>/dev/null && pwd)
KIT_VERSION="lab-capture 1 (2026-09-24)"
SPDX='# SPDX-License-Identifier: MIT'

parent=$PWD
only_bdf=""
want_rom=0
want_prom=0
no_sudo=0
rm_tools=1
ogkm_dir=${CARAXES_OGKM:-}
walk_only=""

usage() { sed -n '4,51p' "$0"; }

while [ $# -gt 0 ]; do
    case $1 in
        -o) parent=$2; shift 2 ;;
        --bdf) only_bdf=$2; shift 2 ;;
        --rom) want_rom=1; shift ;;
        --prom) want_prom=1; shift ;;
        --no-sudo) no_sudo=1; shift ;;
        --no-rm-tools) rm_tools=0; shift ;;
        --ogkm) ogkm_dir=$2; shift 2 ;;
        --walk-rom) walk_only=$2; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
done

have() { command -v "$1" >/dev/null 2>&1; }

# ---------------------------------------------------------------------------
# ROM image chain walk (read-only, on a file). Layout per the nova-core VBIOS
# documentation (https://docs.kernel.org/gpu/nova/core/vbios.html) and the
# driver's own parser (kernel_gsp_vbios_tu102.c, kernel_gsp_fwsec.c, 610.57.04),
# as mirrored in caraxes/tools/synth-vbios.py.
# ---------------------------------------------------------------------------
rom_walk() {
    if ! have python3; then
        echo "python3 not found: ROM walk skipped; the dump itself is kept (sha256 in SHA256SUMS)"
        return 3
    fi
    python3 - "$1" <<'PY'
import hashlib, sys
path = sys.argv[1]
d = open(path, "rb").read()
n = len(d)
print("file %s: %d bytes, sha256 %s" % (path.rsplit("/", 1)[-1], n, hashlib.sha256(d).hexdigest()))
if n == 0:
    print("VERDICT: empty dump (the ROM BAR returned nothing)")
    sys.exit(1)

def u8(o):  return d[o] if 0 <= o < n else None
def u16(o): return int.from_bytes(d[o:o + 2], "little") if 0 <= o and o + 2 <= n else None
def u32(o): return int.from_bytes(d[o:o + 4], "little") if 0 <= o and o + 4 <= n else None

ROM_SIGS = {0xAA55: "55AA", 0x4E56: "NV", 0xBB77: "77BB"}
PCIR_SIGS = {0x52494350: "PCIR", 0x5344504E: "NPDS", 0x53494752: "RGIS"}
NPDE = 0x4544504E
TYPES = {0x00: "x86 PC-AT (base VBIOS)", 0x01: "Open Firmware", 0x02: "PA-RISC",
         0x03: "EFI (GOP driver)", 0xE0: "NVIDIA extension (FWSEC container)"}

start = 0
if u32(0) == 0x4947564E:
    fixed1, fixed2 = u32(4), u32(8)
    ver = (fixed1 >> 8) & 0xFF
    print("IFR header 'NVGI' at 0, VERSIONSW %d" % ver)
    if ver in (1, 2):
        start = u32(((fixed1 >> 16) & 0x7FFF) + 4) or 0
    elif ver == 3:
        fs = u32(fixed2 & 0xFFFFF)
        if fs is not None and u32(fs + 4096) == 0x44524652:
            start = u32(fs + 4096 + 8) or 0
    print("  PCI ROM image offset from IFR: 0x%x" % start)

off, images, pci_end, nv_end, stop = start, [], None, None, ""
for _ in range(32):
    sig = u16(off)
    if sig is None:
        stop = "chain continues at 0x%x, past the end of the dump" % off
        break
    if sig not in ROM_SIGS:
        stop = "no ROM signature at 0x%x (found 0x%04x)" % (off, sig)
        break
    ptr = u16(off + 0x18)
    pcir = off + (ptr or 0)
    psig = u32(pcir)
    if psig not in PCIR_SIGS:
        stop = "image at 0x%x: PCI data structure pointer 0x%x has no PCIR signature" % (off, ptr or 0)
        break
    vendor, device = u16(pcir + 4), u16(pcir + 6)
    plen = u16(pcir + 0xA) or 0
    img_len = (u16(pcir + 0x10) or 0) * 512
    code = u8(pcir + 0x14)
    last_pci = bool((u8(pcir + 0x15) or 0) & 0x80)
    npde = (pcir + plen + 0xF) & ~0xF
    sub_len, last_nv, note = img_len, last_pci, "no NPDE"
    if u32(npde) == NPDE:
        rev, elen = u16(npde + 4), u16(npde + 6) or 0
        if rev in (0x100, 0x101):
            sub_len = (u16(npde + 8) or 0) * 512
            if elen >= 0xB:
                last_nv = bool((u8(npde + 0xA) or 0) & 0x80)
            elif sub_len < img_len:
                last_nv = False
            note = "NPDE rev 0x%x, subimage %d bytes, NVIDIA last=%s" % (rev, sub_len, last_nv)
        else:
            note = "NPDE with unknown rev 0x%x (ignored)" % rev
    inside = off + sub_len <= n
    images.append((off, code, sub_len, inside))
    print("image %d @0x%06x: sig %s, %s, vendor %04x device %04x, code type 0x%02x %s, "
          "PCI length %d bytes, PCI last=%s, %s%s"
          % (len(images) - 1, off, ROM_SIGS[sig], PCIR_SIGS[psig], vendor or 0, device or 0,
             code if code is not None else 0, TYPES.get(code, "(other)"), img_len, last_pci, note,
             "" if inside else ", NOT fully inside the dump"))
    if last_pci and pci_end is None:
        pci_end = off + img_len
    if last_nv or sub_len == 0:
        nv_end = off + sub_len
        break
    off += sub_len
else:
    stop = "more than 32 images, walk stopped"

print("images found: %d; PCI view (what sysfs and firmware see) ends at 0x%x; %s"
      % (len(images), pci_end or 0,
         ("NVIDIA view ends at 0x%x" % nv_end) if nv_end is not None else ("NVIDIA view: " + stop)))
if n > (nv_end or pci_end or n):
    tail = d[(nv_end or pci_end):]
    print("bytes after the last image: %d (%s)" % (len(tail), "all 0xFF" if set(tail) <= {0xFF} else "not blank"))

ext = [i for i in images if i[1] == 0xE0]
base = [i for i in images if i[1] == 0x00]
exp_off = (ext[0][0] - start - base[0][2]) if ext and base else 0

bit = d.find(b"\xff\xb8BIT\x00")
vbios_ver = None
falcon_ptr = None
if bit < 0:
    print("BIT header: not found")
else:
    hver, hsize, tsize, tcount = u16(bit + 6), u8(bit + 8), u8(bit + 9), u8(bit + 10)
    print("BIT header @0x%x: version 0x%04x, header %d, token size %d, tokens %d" % (bit, hver, hsize, tsize, tcount))
    toks = []
    for t in range(tcount or 0):
        o = bit + hsize + t * tsize
        tid, dver, dsize = u8(o), u8(o + 1), u16(o + 2)
        dptr = u32(o + 4) if tsize >= 8 else u16(o + 4)
        if tid is None or dptr is None:
            continue
        toks.append("%s(0x%02x)v%d" % (chr(tid) if 0x20 < tid < 0x7F else "?", tid, dver))
        if tid == 0x42 and dver in (1, 2) and dsize and dsize > 5 and u32(dptr) is not None:
            v, oem = u32(dptr), u8(dptr + 4)
            vbios_ver = "%02x.%02x.%02x.%02x.%02x" % ((v >> 24) & 0xFF, (v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF, oem or 0)
        if tid == 0x70 and dver == 2 and dsize and dsize >= 4:
            falcon_ptr = u32(dptr)
    print("BIT tokens: " + " ".join(toks))
    if vbios_ver:
        print("VBIOS version from BIT BIOSDATA: %s" % vbios_ver)
    if falcon_ptr is not None:
        tbl = exp_off + falcon_ptr
        where = ("inside the dump" if ext and u8(tbl) is not None
                 else "in the NVIDIA extension image, which this dump does not contain")
        print("BIT FALCON_DATA v2: FalconUcodeTablePtr 0x%x -> %s" % (falcon_ptr, where))
        if ext and u8(tbl) is not None:
            tv, th, te, tc = u8(tbl), u8(tbl + 1), u8(tbl + 2), u8(tbl + 3)
            apps = []
            for e in range(tc or 0):
                eo = tbl + (th or 0) + e * (te or 0)
                if u8(eo) is not None:
                    apps.append("0x%02x" % u8(eo))
            print("falcon ucode table @0x%x: version %s, %d entries, application ids %s (0x85 FWSEC_PROD, 0x45 FWSEC_DBG, 0x05 FIRMWARE_SEC_LIC)"
                  % (tbl, tv, tc or 0, " ".join(apps)))
    else:
        print("BIT FALCON_DATA v2 token: absent")

pci_ok = pci_end is not None and pci_end <= n
fwsec = bool(ext) and all(i[3] for i in ext)
print("VERDICT: PCI image chain %s; FWSEC (0xE0) image %s"
      % ("complete (the last-image flag is inside the dump)" if pci_ok else "INCOMPLETE",
         "present" if fwsec else ("partly present" if ext else
         "absent (expected for a sysfs read: sysfs and the ROM BAR stop at the PCI last-image flag; "
         "the NVIDIA chain continues only through the BAR0 PROM window, read with --prom)")))
sys.exit(0 if pci_ok else 1)
PY
}

if [ -n "$walk_only" ]; then
    rom_walk "$walk_only"
    exit $?
fi

# ---------------------------------------------------------------------------
# Setup
# ---------------------------------------------------------------------------
stamp=$(date +%Y%m%d-%H%M%S)
host=$(hostname -s 2>/dev/null || cat /proc/sys/kernel/hostname)
me=$(id -un 2>/dev/null || echo "")
home=${HOME:-}
t0=$(date +%s)

# GPUs: NVIDIA (0x10de) display controllers (class 0x03xxxx)
gpus=()
for dev in /sys/bus/pci/devices/*; do
    [ "$(cat "$dev/vendor" 2>/dev/null)" = 0x10de ] || continue
    case $(cat "$dev/class" 2>/dev/null) in 0x03*) ;; *) continue ;; esac
    b=${dev##*/}
    [ -n "$only_bdf" ] && [ "$b" != "$only_bdf" ] && [ "0000:$only_bdf" != "$b" ] && continue
    gpus+=("$b")
done

# Device ids whose chip is known here, for machines whose pci.ids does not name them
# (the GB206 run of 2026-09-24 was named "10de-2d04": "NVIDIA Corporation Device [10de:2d04]").
# One "id chip" pair per entry, id in lowercase hex; add an id only with a source.
#   1f02 tu106  RTX 2070 (caraxes/reference/rtx2070/lspci-vvv.txt:2)
#   2d04 gb206  RTX 5060 Ti (caraxes/reference/gb206/rm-tools/lab-rm-query.txt:3-6: GB20x,
#               implementation 6, short name GB206-A)
chip_ids="1f02 tu106
2d04 gb206"

# chip code name from the pci.ids name ("TU106 [GeForce RTX 2070]"), else from the
# table above, else the device id ("10de-<id>", as before)
chip_of() {
    local name id
    name=$(lspci -s "$1" 2>/dev/null)
    name=$(grep -oE '\b(GB|GR|GH|AD|GA|TU|GV|GP)[0-9]{3}[A-Z]?\b' <<<"$name" | head -1)
    if [ -n "$name" ]; then tr 'A-Z' 'a-z' <<<"$name"; return; fi
    id=$(sed 's/^0x//' "/sys/bus/pci/devices/$1/device" 2>/dev/null | tr 'A-Z' 'a-z')
    name=$(awk -v id="$id" '$1 == id { print $2; exit }' <<<"$chip_ids")
    if [ -n "$name" ]; then echo "$name"
    else echo "10de-$id"; fi
}
chip=unknown
[ ${#gpus[@]} -gt 0 ] && chip=$(chip_of "${gpus[0]}")

name="lab-capture-$chip-$host-$stamp"
mkdir -p "$parent" || { echo "cannot create $parent" >&2; exit 2; }
parent=$(cd "$parent" && pwd)
out="$parent/$name"
mkdir -p "$out/host" || exit 2
log="$out/capture.log"
manifest=()

# messages name paths with the home directory as "~"
tilde() { local m="$*"; [ ${#home} -gt 1 ] && m=${m//"$home"/\~}; printf '%s\n' "$m"; }
say() { tilde "$*" | tee -a "$log"; }
# redact_config FILE: replace the per-card bytes of an lspci -xxxx dump with "xx" (the
# device's config parser reads "xx" as 0): the data of the vendor-specific capability,
# which on NVIDIA GPUs is the PBI mailbox (PCI_PBI_CAP_ID 0x09; its DATAOUT register at
# +0x0C returns the GPU UUID to pciPbiReadUuid, vendor/open-gpu-kernel-modules
# src/nvidia/src/kernel/platform/chipset/pci_pbi.c:42-55; on the GB206 capture the
# capability sits at 0x9C and 0xA8-0xAB held four UUID bytes, docs/caraxes/blackwell-gb206.md
# 2.5), and the serial number of the Device Serial Number extended capability (id 0x0003,
# PCIe base spec 7.9.3).
redact_config() {
    have python3 || { say "python3 not found: $1 not redacted"; return 0; }
    python3 - "$1" <<'PY'
import re, sys
path = sys.argv[1]
lines = open(path).read().splitlines()
b, where = {}, {}
for n, l in enumerate(lines):
    m = re.match(r"^([0-9a-f]{2,3}): ((?:[0-9a-fx]{2} ?)+)$", l)
    if m:
        base = int(m.group(1), 16)
        for i, t in enumerate(m.group(2).split()):
            b[base + i] = int(t, 16) if t != "xx" else 0
            where[base + i] = (n, i)
def x(off):
    if off in where:
        n, i = where[off]
        head, toks = lines[n].split(": ", 1)
        t = toks.split()
        t[i] = "xx"
        lines[n] = head + ": " + " ".join(t)
g = lambda o: b.get(o, 0)
off, seen = g(0x34) & 0xFC, 0
while 0x40 <= off < 0x100 and seen < 48:
    if g(off) == 0x09:
        for o in range(off + 3, min(off + max(g(off + 2), 3), 0x100)):
            x(o)
    off, seen = g(off + 1) & 0xFC, seen + 1
off, seen = 0x100, 0
while 0x100 <= off < 0x1000 and seen < 128:
    hdr = g(off) | g(off + 1) << 8 | g(off + 2) << 16 | g(off + 3) << 24
    if hdr in (0, 0xFFFFFFFF):
        break
    if hdr & 0xFFFF == 0x0003:
        for o in range(off + 4, off + 12):
            x(o)
    off, seen = (hdr >> 20) & 0xFFC, seen + 1
open(path, "w").write("\n".join(lines) + "\n")
PY
}

# item NAME STATUS PATH REASON: one manifest row
item() { manifest+=("$(printf '%-34s %-8s %-44s %s' "$1" "$2" "$3" "$4")"); say "[$2] $1${4:+: $4}"; }

# cap FILE -- CMD...: run CMD (60 s cap), output to FILE with a header; returns CMD's status
cap() {
    local f=$1; shift; [ "$1" = -- ] && shift
    mkdir -p "$(dirname "$f")"
    { echo "$SPDX"; echo "# command: $*"; } >"$f"
    timeout 60 "$@" >>"$f" 2>&1
    local rc=$?
    [ $rc = 0 ] || echo "# exit status $rc" >>"$f"
    return $rc
}
rel() { echo "${1#"$out"/}"; }

say "$SPDX"
say "== $KIT_VERSION, $(date -Is), host $host"
say "== output $out"

# sudo: decided once. --no-sudo never runs sudo at all.
sudo_ok=0
if [ $no_sudo = 1 ]; then
    sudo_why="--no-sudo given"
elif ! have sudo; then
    sudo_why="sudo not installed"
elif sudo -n true 2>/dev/null; then
    sudo_ok=1; sudo_why="sudo -n works"
else
    sudo_why="sudo -n true failed (no cached or passwordless sudo)"
fi
say "== sudo: $([ $sudo_ok = 1 ] && echo yes || echo no) ($sudo_why)"
SUDO=(); [ $sudo_ok = 1 ] && SUDO=(sudo -n)

if [ ${#gpus[@]} -eq 0 ]; then
    item "gpus" failed "-" "no NVIDIA display controller found${only_bdf:+ at $only_bdf}"
fi

# ---------------------------------------------------------------------------
# Part 1a: host facts
# ---------------------------------------------------------------------------
H="$out/host"
cap "$H/uname.txt" -- uname -a && item host.uname ok host/uname.txt ""
if [ -r /etc/os-release ]; then cap "$H/os-release.txt" -- cat /etc/os-release; item host.os-release ok host/os-release.txt ""
else item host.os-release failed - "/etc/os-release missing"; fi
cap "$H/cmdline.txt" -- cat /proc/cmdline; item host.cmdline ok host/cmdline.txt "kernel command line (IOMMU, nvidia options)"
cap "$H/boot-time.txt" -- sh -c 'echo "booted: $(uptime -s 2>/dev/null)"; cat /proc/uptime; command -v systemd-analyze >/dev/null && systemd-analyze 2>&1 | head -5'
item host.boot-time ok host/boot-time.txt ""
cap "$H/cpu.txt" -- sh -c 'lscpu 2>/dev/null || grep -m1 "model name" /proc/cpuinfo'; item host.cpu ok host/cpu.txt ""
cap "$H/lsmod.txt" -- sh -c 'cat /proc/modules | sort'; item host.modules ok host/lsmod.txt "loaded module list (/proc/modules)"
if have modinfo; then
    cap "$H/modinfo-nvidia.txt" -- sh -c 'for m in nvidia nvidia_modeset nvidia_drm nvidia_uvm nvidia_peermem nouveau nova_core; do echo "=== $m"; modinfo "$m" 2>&1 | grep -vE "^(signer|sig_key|sig_hashalgo|signature):|^\s+[0-9A-F]{2}:"; done'
    item host.modinfo ok host/modinfo-nvidia.txt "module versions, licence (open or proprietary), firmware names"
else item host.modinfo skipped - "modinfo not installed"; fi

drv=""
if [ -r /proc/driver/nvidia/version ]; then
    drv=$(grep -oE '[0-9]+\.[0-9]+(\.[0-9]+)?' /proc/driver/nvidia/version | head -1)
    mkdir -p "$H/proc-driver-nvidia"
    n=0
    while IFS= read -r f; do
        r=${f#/proc/driver/nvidia/}
        cap "$H/proc-driver-nvidia/${r//\//__}.txt" -- cat "$f" && n=$((n + 1))
    done < <(find /proc/driver/nvidia -type f -readable 2>/dev/null | sort)
    item host.proc-driver-nvidia ok host/proc-driver-nvidia/ "$n files: version, params, registry, gpus/*/information; driver $drv"
else
    item host.proc-driver-nvidia failed - "/proc/driver/nvidia/version missing: is the NVIDIA driver loaded?"
fi
if [ -d /sys/module/nvidia ]; then
    cap "$H/sys-module-nvidia.txt" -- sh -c 'for m in nvidia nvidia_modeset nvidia_drm nvidia_uvm; do d=/sys/module/$m; [ -d $d ] || continue; echo "=== $m"; for f in $d/version $d/taint $d/initstate $d/refcnt $d/parameters/*; do [ -r "$f" ] && [ -f "$f" ] && printf "%s: %s\n" "${f#$d/}" "$(timeout 2 cat "$f" 2>&1 | tr "\n" " ")"; done; done'
    item host.sys-module ok host/sys-module-nvidia.txt "module parameters readable without root"
fi

# firmware files of the running driver version
fwlist="$H/firmware-files.txt"
{
    echo "$SPDX"
    echo "# driver version: ${drv:-unknown}"
    for base in /lib/firmware/nvidia /usr/lib/firmware/nvidia; do
        [ -d "$base" ] || continue
        echo "# versions present under $base: $(ls "$base" 2>/dev/null | tr '\n' ' ')"
    done
} >"$fwlist"
fwdir=""
for base in /lib/firmware/nvidia /usr/lib/firmware/nvidia; do
    [ -n "$drv" ] && [ -d "$base/$drv" ] && { fwdir=$(readlink -f "$base/$drv"); break; }
done
if [ -n "$fwdir" ]; then
    {
        echo "# directory: $fwdir"
        echo "# size_bytes sha256 path"
        find "$fwdir" -type f -printf '%P\n' | sort | while read -r f; do
            printf '%s %s %s\n' "$(stat -c %s "$fwdir/$f")" "$(sha256sum "$fwdir/$f" | cut -d' ' -f1)" "$f"
        done
    } >>"$fwlist"
    item host.firmware ok host/firmware-files.txt "$(find "$fwdir" -type f | wc -l) files in $fwdir with size and sha256"
else
    echo "# no firmware directory for version ${drv:-unknown}" >>"$fwlist"
    item host.firmware failed host/firmware-files.txt "no /lib/firmware/nvidia/${drv:-<version>}/"
fi
# per-chip firmware from linux-firmware (nova-core and nouveau use it, not the NVIDIA driver)
for g in "${gpus[@]}"; do
    c=$(chip_of "$g")
    for base in /lib/firmware/nvidia /usr/lib/firmware/nvidia; do
        [ -d "$base/$c" ] || continue
        {
            echo "# linux-firmware directory for $c: $base/$c (symlinks resolved)"
            echo "# size_bytes sha256 path -> target"
            find -L "$base/$c/" -type f -printf '%P\n' | sort | while read -r f; do
                t=$(readlink -f "$base/$c/$f")
                [ -f "$t" ] && printf '%s %s %s -> %s\n' "$(stat -c %s "$t")" "$(sha256sum "$t" | cut -d' ' -f1)" "$f" "${t#"$base"/}"
            done
        } >>"$fwlist"
        item "host.firmware-$c" ok host/firmware-files.txt "linux-firmware files for $c ($(find -L "$base/$c/" -type f | wc -l) files)"
        break
    done
done

# kernel log since boot, filtered. Streamed, never stored whole: a host with a
# raised RM debug level can carry a kernel log of a gigabyte. Kept: the first line
# (time base), the first 3000 and the last 2000 matching lines, the last 500 Xid lines.
klog="$H/kernel-log.txt"
ksrc=""
kcmd=()
filter='nvrm|nvidia|xid|gsp|nouveau|nova|drm|vfio|iommu|dmar|amd-vi|resiz|bar ?[0-9]|pci 0000|efifb|simpledrm|vesafb|rom'
for g in "${gpus[@]}"; do filter="$filter|${g#0000:}"; done
if have journalctl && [ -n "$(journalctl -k -b 0 -n 1 --no-pager -q 2>/dev/null)" ]; then
    ksrc="journalctl -k -b 0 (as user)"; kcmd=(journalctl -k -b 0 -o short-monotonic --no-pager -q)
elif [ -n "$(dmesg 2>/dev/null | head -1)" ]; then
    ksrc="dmesg (as user)"; kcmd=(dmesg)
elif [ $sudo_ok = 1 ] && [ -n "$(sudo -n dmesg 2>/dev/null | head -1)" ]; then
    ksrc="sudo dmesg"; kcmd=(sudo -n dmesg)
fi
if [ -n "$ksrc" ]; then
    {
        echo "$SPDX"
        echo "# source: $ksrc; filter (case-insensitive): $filter"
        timeout 600 "${kcmd[@]}" 2>/dev/null | awk -v re="$filter" -v H=3000 -v T=2000 -v X=500 '
            NR == 1 { print "# first line of the log (time base):"; print; print "# matching lines:" }
            { l = tolower($0) }
            l ~ /xid/ { xc++; xr[xc % X] = $0 }
            l ~ re {
                m++
                k = $0; sub(/^[^:]*kernel: /, "", k); sub(/^\[[ 0-9.]*\] /, "", k)
                gsub(/0x[0-9a-fA-F]+|[0-9]+/, "#", k)
                if (k in kind) kind[k]++
                else if (nk < 5000) { kind[k] = 1; nk++ }
                if (m <= H) print
                else tr[m % T] = $0
            }
            END {
                if (m > H) {
                    first = (m - T > H) ? m - T + 1 : H + 1
                    if (first > H + 1) print "# ... " first - H - 1 " matching lines omitted ..."
                    for (i = first; i <= m; i++) print tr[i % T]
                }
                print "# total lines " NR ", matching " m + 0 ", Xid lines " xc + 0
                print "# the 40 most frequent kinds of matching line (numbers folded to #):"
                fflush()
                srt = "sort -rn | head -40 | sed \"s/^/#   /\""
                for (k in kind) print kind[k] "\t" k | srt
                close(srt)
                if (xc > 0) {
                    print "# Xid lines (last " (xc < X ? xc : X) "):"
                    for (i = (xc > X ? xc - X + 1 : 1); i <= xc; i++) print xr[i % X]
                }
            }'
    } >"$klog"
    item host.kernel-log ok host/kernel-log.txt "$ksrc; $(tail -n +1 "$klog" | grep -m1 '^# total lines' | sed 's/^# //')"
else
    item host.kernel-log skipped - "kernel log not readable (dmesg_restrict, no journal access) and no sudo"
fi

cap "$H/lspci-nn.txt" -- lspci -Dnn && item host.lspci ok host/lspci-nn.txt ""
cap "$H/lspci-tree.txt" -- lspci -tvnn && item host.lspci-tree ok host/lspci-tree.txt "PCI topology"
cap "$H/iommu-groups.txt" -- sh -c 'for g in /sys/kernel/iommu_groups/*; do [ -d "$g" ] || continue; echo "${g##*/}: $(ls $g/devices | tr "\n" " ")"; done'
item host.iommu-groups ok host/iommu-groups.txt ""

# ---------------------------------------------------------------------------
# Part 1b: per GPU
# ---------------------------------------------------------------------------
for bdf in "${gpus[@]}"; do
    G="$out/gpu-${bdf//:/_}"
    gd=$(rel "$G")
    sys=/sys/bus/pci/devices/$bdf
    mkdir -p "$G"
    say "== GPU $bdf ($(chip_of "$bdf"))"

    # lspci: decoded capabilities, all functions of the slot, upstream port
    if [ $sudo_ok = 1 ]; then
        cap "$G/lspci-vvv.txt" -- sudo -n lspci -vvv -nn -s "$bdf"
        sed -i -E 's/(Device Serial Number ).*/\1<redacted>/' "$G/lspci-vvv.txt"
        item "$bdf.lspci-vvv" ok "$gd/lspci-vvv.txt" "as root: every capability decoded"
    else
        cap "$G/lspci-vvv.txt" -- lspci -vvv -nn -s "$bdf"
        item "$bdf.lspci-vvv" reduced "$gd/lspci-vvv.txt" "as user: capabilities past 0x40 show <access denied>; run with sudo for the full list"
    fi
    slot=${bdf%.*}
    cap "$G/lspci-functions.txt" -- lspci -vvv -nn -s "$slot"
    item "$bdf.lspci-functions" ok "$gd/lspci-functions.txt" "all functions of the slot (audio, USB, UCSI), as user"
    up=$(basename "$(dirname "$(readlink -f "$sys")")")
    if [[ $up =~ ^[0-9a-f]{4}: ]]; then
        cap "$G/lspci-upstream.txt" -- lspci -vvv -nn -s "$up"
        item "$bdf.lspci-upstream" ok "$gd/lspci-upstream.txt" "upstream port $up (link partner)"
    fi

    # config space
    if [ $sudo_ok = 1 ]; then
        cap "$G/config-space.txt" -- sudo -n lspci -xxxx -s "$bdf"
        redact_config "$G/config-space.txt"
        item "$bdf.config-space" ok "$gd/config-space.txt" "4096 bytes (lspci -xxxx as root), PBI data and DSN redacted"
    else
        cap "$G/config-space.txt" -- lspci -xxxx -s "$bdf"
        nb=$(grep -cE '^[0-9a-f]+0: ' "$G/config-space.txt")
        item "$bdf.config-space" reduced "$gd/config-space.txt" "$((nb * 16)) bytes only: the kernel shows unprivileged readers the first 64 bytes; the extended space needs sudo"
    fi

    # BARs and resizable BAR
    {
        echo "$SPDX"
        echo "# sysfs resource table (start end flags) with sizes"
        i=0
        while read -r s e f; do
            if [ "$((s))" != 0 ] || [ "$((e))" != 0 ]; then
                printf 'resource%-2d start=%s end=%s flags=%s size=%d MiB (%d bytes)\n' $i "$s" "$e" "$f" $(((e - s + 1) >> 20)) $((e - s + 1))
            fi
            i=$((i + 1))
        done <"$sys/resource"
        echo "# resizable BAR, sysfs resourceN_resize (bitmask of supported sizes, bit n = 2^n MiB)"
        for r in "$sys"/resource*_resize; do
            [ -r "$r" ] && echo "${r##*/}: $(cat "$r" 2>&1)"
        done
        echo "# from lspci (Region lines and the Resizable BAR capability)"
        grep -E 'Region [0-9]|Expansion ROM|Resizable BAR|BAR [0-9]: current size|supported:' "$G/lspci-vvv.txt"
    } >"$G/bars.txt"
    item "$bdf.bars" ok "$gd/bars.txt" "BAR sizes, resizable-BAR sizes"

    # sysfs attribute files of the device (never config, rom, resourceN, vpd, reset)
    {
        echo "$SPDX"
        echo "# $sys: readable attribute files; excluded: config rom resource[N] vpd reset remove rescan (binary, device access or write-only)"
        for f in "$sys"/* "$sys"/power/* "$sys"/link/* "$sys"/aer/*; do
            n=${f#"$sys"/}
            [ -f "$f" ] && [ -r "$f" ] || continue
            case ${n##*/} in
                config|rom|vpd|reset|reset_method|remove|rescan|resource[0-9]|resource[0-9]_wc|new_id|remove_id|bind|unbind|label) continue ;;
            esac
            printf '%s: %s\n' "$n" "$(timeout 2 cat "$f" 2>&1 | tr '\n' ' ' | sed 's/ *$//')"
        done
        for l in driver iommu_group firmware_node physfn; do
            [ -e "$sys/$l" ] && echo "$l -> $(readlink "$sys/$l" | sed 's|.*/||')"
        done
        [ -d "$sys/msi_irqs" ] && echo "msi_irqs: $(ls "$sys/msi_irqs" | wc -l) vectors"
        echo "reset_method: $(cat "$sys/reset_method" 2>/dev/null)"
    } >"$G/sysfs-attributes.txt"
    item "$bdf.sysfs" ok "$gd/sysfs-attributes.txt" "attribute files under $sys"

    # /proc/driver/nvidia/gpus/<bdf>/information
    if [ -r "/proc/driver/nvidia/gpus/$bdf/information" ]; then
        cap "$G/proc-information.txt" -- cat "/proc/driver/nvidia/gpus/$bdf/information"
        item "$bdf.proc-information" ok "$gd/proc-information.txt" "model, VBIOS version, GPU firmware version"
    fi

    # nvidia-smi
    if have nvidia-smi; then
        cap "$G/nvidia-smi-q.txt" -- nvidia-smi -q -i "$bdf"
        sed -i -E 's/^( *(Serial Number|Chassis Serial Number|Board Serial Number) *: ).*/\1<redacted>/' "$G/nvidia-smi-q.txt"
        item "$bdf.nvidia-smi-q" ok "$gd/nvidia-smi-q.txt" "full -q, serial numbers redacted"
        timeout 60 nvidia-smi -q -x -i "$bdf" >"$G/nvidia-smi-q.xml" 2>&1
        sed -i -E 's#<(serial|chassis_serial_number|board_serial_number)>[^<]*</#<\1>redacted</#g' "$G/nvidia-smi-q.xml"
        item "$bdf.nvidia-smi-q-xml" ok "$gd/nvidia-smi-q.xml" "same as XML, serial redacted"
        fields=$(nvidia-smi --help-query-gpu 2>/dev/null | grep -oE '^"[a-z0-9_.]+"' | tr -d '"' | paste -sd, -)
        nf=$(tr ',' '\n' <<<"$fields" | grep -c .)
        if timeout 60 nvidia-smi -i "$bdf" --query-gpu="$fields" --format=csv >"$G/nvidia-smi-query-gpu.csv" 2>&1; then
            qmode="one query, $nf fields"
        else
            # an unknown field fails the whole query: fall back to one query per field
            qmode="per-field queries, $nf fields"
            {
                printf '%s' "${fields//,/, }"; echo
                sep=""
                for fl in ${fields//,/ }; do
                    printf '%s%s' "$sep" "$(timeout 10 nvidia-smi -i "$bdf" --query-gpu="$fl" --format=csv,noheader 2>&1 | head -1)"; sep=", "
                done; echo
            } >"$G/nvidia-smi-query-gpu.csv"
        fi
        # redact serial columns, then transpose to "field: value"
        if have python3; then
            python3 - "$G/nvidia-smi-query-gpu.csv" "$G/nvidia-smi-query-gpu.txt" "$SPDX" <<'PY'
import sys
src, dst, spdx = sys.argv[1:4]
lines = open(src).read().splitlines()
if len(lines) >= 2:
    keys = [k.strip() for k in lines[0].split(",")]
    vals = [v.strip() for v in lines[1].split(", ")]
    if len(vals) != len(keys):
        vals = [v.strip() for v in lines[1].split(",")]
    for i, k in enumerate(keys):
        if "serial" in k and i < len(vals):
            vals[i] = "<redacted>"
    open(src, "w").write(", ".join(keys) + "\n" + ", ".join(vals) + "\n")
    with open(dst, "w") as f:
        f.write(spdx + "\n# nvidia-smi --query-gpu, one field per line\n")
        for i, k in enumerate(keys):
            f.write("%s: %s\n" % (k, vals[i] if i < len(vals) else "<missing>"))
PY
        fi
        item "$bdf.nvidia-smi-query-gpu" ok "$gd/nvidia-smi-query-gpu.csv" "$qmode (.txt: transposed), serial redacted"
    else
        item "$bdf.nvidia-smi" failed - "nvidia-smi not installed"
    fi

    # PCI option ROM (optional, sudo)
    if [ $want_rom = 0 ]; then
        item "$bdf.rom" skipped - "not requested (--rom): the VBIOS is what the synthetic identity is built from"
    elif [ $sudo_ok = 0 ]; then
        item "$bdf.rom" skipped - "--rom needs sudo ($sudo_why)"
    else
        romf="$G/rom.bin"
        # the only write of the kit: enable ROM decode, read, disable again (also on exit)
        trap 'sudo -n sh -c "echo 0 > $sys/rom" 2>/dev/null' EXIT INT TERM
        if sudo -n sh -c "echo 1 > '$sys/rom'"; then
            sudo -n dd if="$sys/rom" bs=64k status=none >"$romf" 2>"$G/rom-dd.err"
            rc=$?
            sudo -n sh -c "echo 0 > '$sys/rom'"
            trap - EXIT INT TERM
            if [ $rc = 0 ] && [ -s "$romf" ]; then
                { echo "$SPDX"; rom_walk "$romf"; } >"$G/rom-walk.txt" 2>&1
                verdict=$(grep '^VERDICT' "$G/rom-walk.txt" | sed 's/^VERDICT: //')
                item "$bdf.rom" ok "$gd/rom.bin" "$(stat -c %s "$romf") bytes; walk: ${verdict:-see rom-walk.txt}"
                say "$(sed 1d "$G/rom-walk.txt")"
            else
                item "$bdf.rom" failed "$gd/rom-dd.err" "dd from sysfs rom failed (status $rc)"
            fi
            rm -f "$G/rom-dd.err"
        else
            trap - EXIT INT TERM
            item "$bdf.rom" failed - "could not enable $sys/rom"
        fi
    fi

    # VBIOS through the BAR0 PROM window (optional, sudo, read-only). NV_PROM_DATA(i) is
    # BAR0 0x300000 + i (src/common/inc/swref/published/turing/tu102/dev_ext_devices.h:27
    # in vendor/open-gpu-kernel-modules 610.57.04, the only definition, used for every
    # chip); the kernel RM reads it with 32-bit loads (s_promRead32,
    # src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_vbios_tu102.c:74-86, the
    # image walk :275-417), and nova-core scans the same window, at most 1 MiB, 32 bits
    # at a time (nova-core vbios.rs, https://docs.kernel.org/gpu/nova/core/vbios.html).
    # The window is read whole (1 MiB) and the chain walk says where the images end.
    if [ $want_prom = 0 ]; then
        item "$bdf.prom" skipped - "not requested (--prom): the full NVIDIA image chain (FWSEC) is only in the BAR0 PROM window"
    elif [ $sudo_ok = 0 ]; then
        item "$bdf.prom" skipped - "--prom needs sudo ($sudo_why)"
    elif ! have python3; then
        item "$bdf.prom" skipped - "--prom needs python3"
    else
        promf="$G/prom.bin"
        sudo -n python3 - "$sys/resource0" >"$promf" 2>"$G/prom.err" <<'PY'
import mmap, os, sys
# NV_PROM_DATA(i) = BAR0 0x300000 + i (tu102/dev_ext_devices.h:27); nova-core's scan limit.
PROM, WINDOW = 0x300000, 0x100000
fd = os.open(sys.argv[1], os.O_RDONLY | os.O_SYNC)
if os.fstat(fd).st_size < PROM + WINDOW:
    sys.exit("resource0 is %d bytes, smaller than the PROM window end 0x%x" % (os.fstat(fd).st_size, PROM + WINDOW))
# Read-only mapping of the window alone; no store is ever issued. With
# CONFIG_IO_STRICT_DEVMEM the kernel refuses to map a BAR a driver has claimed
# (docs/caraxes/open-questions.md Q1): then the read needs the card without a driver,
# which this kit never arranges.
try:
    mm = mmap.mmap(fd, WINDOW, mmap.MAP_SHARED, mmap.PROT_READ, offset=PROM)
except OSError as e:
    sys.exit("mmap of resource0 refused (%s): a driver holds BAR0 and the kernel has "
             "CONFIG_IO_STRICT_DEVMEM; the PROM read needs the card without a driver" % e)
words = memoryview(mm).cast("I")
first = words[0]
if first == 0xFFFFFFFF:
    sys.exit("PROM window reads all ones (memory decoding off or the device is in reset)")
if first & 0xFFFF != 0xAA55 and first != 0x4947564E:
    sys.exit("PROM window does not start with 55 AA or the IFR signature NVGI (0x%08x)" % first)
out = bytearray(WINDOW)
for i in range(WINDOW // 4):
    out[4 * i:4 * i + 4] = words[i].to_bytes(4, "little")
words.release()
mm.close()
os.close(fd)
sys.stdout.buffer.write(out)
PY
        rc=$?
        if [ $rc = 0 ] && [ -s "$promf" ]; then
            { echo "$SPDX"; echo "# source: BAR0 PROM window (NV_PROM_DATA 0x300000, 1 MiB) via sysfs resource0, read-only"; rom_walk "$promf"; } >"$G/prom-walk.txt" 2>&1
            verdict=$(grep '^VERDICT' "$G/prom-walk.txt" | sed 's/^VERDICT: //')
            item "$bdf.prom" ok "$gd/prom.bin" "$(stat -c %s "$promf") bytes (PROM window); walk: ${verdict:-see prom-walk.txt}"
            say "$(sed 1,2d "$G/prom-walk.txt")"
            rm -f "$G/prom.err"
        else
            rm -f "$promf"
            item "$bdf.prom" failed "$gd/prom.err" "PROM read failed (status $rc): $(head -c 200 "$G/prom.err" 2>/dev/null)"
        fi
    fi

    # display side: DRM connectors of this GPU, EDIDs
    D="$G/display"
    mkdir -p "$D"
    nconn=0; nedid=0
    {
        echo "$SPDX"
        for m in nvidia_drm nvidia_modeset; do
            for p in /sys/module/$m/parameters/*; do [ -r "$p" ] && echo "$m.${p##*/}: $(cat "$p" 2>&1)"; done
        done
        for card in /sys/class/drm/card[0-9]*; do
            case ${card##*/} in *-*) continue ;; esac
            [ "$(readlink -f "$card/device")" = "$(readlink -f "$sys")" ] || continue
            echo "=== ${card##*/} driver $(basename "$(readlink -f "$card/device/driver")")"
            for c in "$card"-*; do
                [ -d "$c" ] || continue
                cn=${c##*/}; nconn=$((nconn + 1))
                echo "--- $cn status=$(cat "$c/status" 2>/dev/null) enabled=$(cat "$c/enabled" 2>/dev/null) dpms=$(cat "$c/dpms" 2>/dev/null)"
                echo "modes: $(tr '\n' ' ' <"$c/modes" 2>/dev/null)"
                # sysfs reports size 0 for every attribute: read, then look. The
                # file is the EDID the kernel already holds; no DDC/AUX transfer.
                cat "$c/edid" >"$D/$cn.edid.bin" 2>/dev/null
                if [ -s "$D/$cn.edid.bin" ]; then
                    nedid=$((nedid + 1))
                    echo "edid: $(stat -c %s "$D/$cn.edid.bin") bytes -> $cn.edid.bin"
                    od -An -tx1 -v "$D/$cn.edid.bin" | sed 's/^/  /'
                    if have edid-decode; then edid-decode "$D/$cn.edid.bin" >"$D/$cn.edid-decode.txt" 2>&1; fi
                else
                    rm -f "$D/$cn.edid.bin"
                fi
            done
        done
    } >"$D/connectors.txt"
    echo "$nconn $nedid" >"$D/.n"
    read -r nconn nedid <"$D/.n"; rm -f "$D/.n"
    item "$bdf.display" ok "$gd/display/" "$nconn connectors, $nedid EDIDs"
done

# ---------------------------------------------------------------------------
# GSP firmware family in use (from the architecture; the driver maps it in
# src/nvidia/arch/nvalloc/common/inc/nv-firmware.h)
# ---------------------------------------------------------------------------
{
    echo "$SPDX"
    for bdf in "${gpus[@]}"; do
        G="$out/gpu-${bdf//:/_}"
        arch=$(sed -n 's/^ *Product Architecture *: *//p' "$G/nvidia-smi-q.txt" 2>/dev/null | head -1)
        gspv=$(sed -n 's/^ *GSP Firmware Version *: *//p' "$G/nvidia-smi-q.txt" 2>/dev/null | head -1)
        pfw=$(sed -n 's/^GPU Firmware:[[:space:]]*//p' "/proc/driver/nvidia/gpus/$bdf/information" 2>/dev/null)
        c=$(chip_of "$bdf")
        case $c in
            tu10*|tu11*|ga100) fam="tu10x (gsp_tu10x.bin, LIBOS2)" ;;
            ga10*|ad10*|gh10*|gb*|gr*) fam="ga10x (gsp_ga10x.bin, LIBOS3; shared GA10x through GB20x in 610.57.04 and 615.71.09)" ;;
            *) fam="unknown for chip $c" ;;
        esac
        echo "$bdf chip=$c architecture=${arch:-?} gsp_firmware_version=${gspv:-?} proc_gpu_firmware=${pfw:-?} family=$fam"
        [ -n "$gspv" ] && [ "$gspv" != "N/A" ] && echo "  GSP firmware is in use (nvidia-smi reports a GSP firmware version)"
        [ -n "$fwdir" ] && ls "$fwdir" | grep -E '^gsp_' | sed 's/^/  present: /'
    done
    echo "# The family is derived from the chip name (pci.ids or the script's device-id table) with"
    echo "# the mapping of nv-firmware.h in 610.57.04 and 615.71.09; check it against nv-firmware.h"
    echo "# of the running driver version when that is another one."
} >"$H/gsp-firmware.txt"
item host.gsp-family ok host/gsp-firmware.txt "$(sed -n '2p' "$H/gsp-firmware.txt" | grep -oE 'family=[a-z0-9]+' )"

# ---------------------------------------------------------------------------
# Part 2: RM query tools (optional; rustc and this repository)
# ---------------------------------------------------------------------------
R="$out/rm-tools"
if [ $rm_tools = 0 ]; then
    item rm-tools skipped - "--no-rm-tools given"
elif [ ! -f "$repo/caraxes/tools/host-facts.rs" ] || [ ! -f "$repo/caraxes/tools/fifo-host-query.rs" ]; then
    item rm-tools skipped - "the Caraxes repository is not around this script"
elif ! have rustc; then
    item rm-tools skipped - "rustc not installed (part 2 only; part 1 is complete without it)"
elif [ ${#gpus[@]} -eq 0 ] || [ ! -e /dev/nvidiactl ]; then
    item rm-tools skipped - "no NVIDIA GPU or no /dev/nvidiactl"
else
    mkdir -p "$R"
    bin="$repo/target/lab-capture"
    mkdir -p "$bin"
    # 1. ABI check of the hard-coded 610.57.04 layouts against the running version
    verdict="unverified"
    if [ "$drv" = 610.57.04 ]; then
        verdict="verified (running driver is 610.57.04, the version the tools were written for)"
        echo "$SPDX" >"$R/abi-check.txt"; echo "$verdict" >>"$R/abi-check.txt"
    elif have cc || have gcc; then
        cap "$R/abi-check.txt" -- "$repo/caraxes/tools/lab-abi-check.sh" "$drv"
        case $? in
            0) verdict="verified (lab-abi-check: identical layouts on $drv)" ;;
            1) verdict="UNVERIFIED (lab-abi-check: layouts differ on $drv, see abi-check.txt)" ;;
            *) verdict="UNVERIFIED (no SDK headers for $drv in this checkout, see abi-check.txt)" ;;
        esac
    else
        verdict="UNVERIFIED (no C compiler for lab-abi-check)"
        echo "$SPDX" >"$R/abi-check.txt"; echo "$verdict" >>"$R/abi-check.txt"
    fi
    item rm-tools.abi-check ok rm-tools/abi-check.txt "$verdict"
    # 2. build and run the three rustc-only tools (getters only; host-facts in "facts" mode)
    for t in lab-rm-query fifo-host-query host-facts; do
        if cap "$R/build-$t.txt" -- rustc -O --edition 2021 "$repo/caraxes/tools/$t.rs" -o "$bin/$t"; then
            rm -f "$R/build-$t.txt"
            args=(); [ $t = host-facts ] && args=(facts)
            {
                echo "$SPDX"
                echo "# $t ${args[*]}; driver ${drv:-?}; ABI $verdict"
            } >"$R/$t.txt"
            timeout 120 "$bin/$t" "${args[@]}" >>"$R/$t.txt" 2>&1
            rc=$?
            [ $rc = 0 ] && st=ok || st=failed
            item "rm-tools.$t" $st "rm-tools/$t.txt" "exit $rc; ABI ${verdict%% *}"
        else
            item "rm-tools.$t" failed "rm-tools/build-$t.txt" "rustc build failed"
        fi
    done
    # 3. gsp-abi-sizes: struct layouts of the driver version (no GPU access)
    tree=""
    if [ -n "$ogkm_dir" ]; then tree=$ogkm_dir
    elif vend_ver=$(sed -n 's/^NVIDIA_VERSION *= *//p' "$repo/vendor/open-gpu-kernel-modules/version.mk" 2>/dev/null);
         [ -n "$drv" ] && [ "$drv" = "$vend_ver" ] && [ -f "$repo/vendor/open-gpu-kernel-modules/src/nvidia/inc/kernel/gpu/gsp/gsp_static_config.h" ]; then
        tree="$repo/vendor/open-gpu-kernel-modules"
    fi
    if [ -z "$tree" ]; then
        item rm-tools.gsp-abi-sizes skipped - "needs an open-gpu-kernel-modules tree at ${drv:-the running version} (--ogkm DIR or CARAXES_OGKM); the vendored one is ${vend_ver:-unknown}"
    elif ! have cc; then
        item rm-tools.gsp-abi-sizes skipped - "no C compiler"
    elif abiargs=(); [ -n "$drv" ] && abiargs=(--abi "${drv%%.*}");
         cap "$R/gsp-abi-sizes.md" -- "$repo/caraxes/tools/gsp-abi-sizes.sh" "${abiargs[@]}" "$tree"; then
        # --abi: the field lists follow the running driver's major version (610 lists
        # below 615, 615 lists from 615 on; gsp-abi-sizes.sh, GSP_ABI in gsp-abi-sizes.c)
        sed -i '1s/.*/<!-- SPDX-License-Identifier: MIT -->/; 2s/^# \(.*\)/<!-- \1 -->/' "$R/gsp-abi-sizes.md"
        item rm-tools.gsp-abi-sizes ok rm-tools/gsp-abi-sizes.md "tree ${tree#"$repo"/}${drv:+, field lists for major ${drv%%.*}}"
    else
        item rm-tools.gsp-abi-sizes failed rm-tools/gsp-abi-sizes.md "did not compile against ${tree#"$repo"/} (see file)"
    fi
fi

# ---------------------------------------------------------------------------
# Scrub, manifest, sums, tarball, verify
# ---------------------------------------------------------------------------
while IFS= read -r f; do
    [ -n "$home" ] && [ ${#home} -gt 1 ] && sed -i "s#$home#~#g" "$f"
    [ -n "$me" ] && [ "$me" != root ] && [ ${#me} -ge 2 ] && sed -i -E "s/\\b$me\\b/<user>/g" "$f"
    sed -i -E 's/[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/<email>/g' "$f"
done < <(find "$out" -type f \( -name '*.txt' -o -name '*.csv' -o -name '*.xml' -o -name '*.md' -o -name '*.log' \))

elapsed=$(($(date +%s) - t0))
{
    echo "$SPDX"
    echo "# $KIT_VERSION"
    echo "# created $(date -Is) on $host, kernel $(uname -r), driver ${drv:-none}, chip $chip, ${#gpus[@]} GPU(s): ${gpus[*]}"
    echo "# options: rom=$want_rom prom=$want_prom sudo=$sudo_ok ($sudo_why) rm_tools=$rm_tools; runtime ${elapsed}s"
    echo "# status: ok = complete, reduced = captured with less detail (usually for lack of sudo),"
    echo "#         skipped = not captured (reason given), failed = tried and failed"
    printf '%-34s %-8s %-44s %s\n' "# item" status path note
    printf '%s\n' "${manifest[@]}"
} >"$out/MANIFEST.txt"
# the log is scrubbed like the rest
sed -i -E 's/[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/<email>/g' "$log"
[ -n "$me" ] && [ "$me" != root ] && sed -i -E "s/\\b$me\\b/<user>/g" "$log"

(cd "$out" && find . -type f ! -name SHA256SUMS -printf '%P\n' | sort | xargs -d '\n' sha256sum >SHA256SUMS)
tarball="$parent/$name.tar.gz"
tar -C "$parent" -czf "$tarball" "$name"
(cd "$parent" && sha256sum "$name.tar.gz" >"$name.tar.gz.sha256")

# verify: unpack into a temporary directory and check every sum
vt=$(mktemp -d "${TMPDIR:-/tmp}/lab-capture-verify.XXXXXX")
if tar -C "$vt" -xzf "$tarball" && (cd "$vt/$name" && sha256sum --quiet -c SHA256SUMS); then
    ver="tarball verified: $(tar -tzf "$tarball" | grep -vc '/$') files, all sums match"
else
    ver="TARBALL VERIFY FAILED"
fi
rm -rf "$vt"
tilde "== $ver"
tilde "== counts: $(printf '%s\n' "${manifest[@]}" | awk '{print $2}' | sort | uniq -c | tr -s ' ' | tr '\n' ' ')"
tilde "== directory $out"
tilde "== tarball   $tarball ($(du -h "$tarball" | cut -f1)), sha256 $(cut -d' ' -f1 "$parent/$name.tar.gz.sha256")"
tilde "== bring back the tarball and its .sha256; see docs/caraxes/lab-blackwell.md"
[ "$ver" = "TARBALL VERIFY FAILED" ] && exit 1
exit 0
