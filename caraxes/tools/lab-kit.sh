#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
#
# Caraxes lab kit: one self-contained pack a person runs unattended on a machine with an NVIDIA
# GPU (a rented cloud GPU VM, or the workplace Blackwell card), and one tarball to carry home.
# Operator instructions: docs/caraxes/gsp-plan/lab-kit.md.
#
#   caraxes/tools/lab-kit.sh pack [options]      at home: build the pack (tarball + .sha256)
#   ./lab-kit.sh run OUTDIR [options]            on the lab machine, from the unpacked pack
#   ./lab-kit.sh verify TARBALL                  anywhere: check a result tarball and its sums
#
# pack options (at home; nothing here touches a GPU or the network):
#   -o DIR               where the pack tarball goes (default: target/lab-kit)
#   --ogkm DIR           open-gpu-kernel-modules tree of the driver the lab runs (default:
#                        $CARAXES_OGKM, else vendor/open-gpu-kernel-modules); its version is
#                        the pack's driver version, and the sweep list is made from it
#   --leandro DIR        the Leandro checkout with crates/nvrm-trace (default: ../Leandro)
#   --leandro-test DIR   the Leandro-Test checkout with probe/c (default: ../Leandro-Test)
#   --cuda DIR           CUDA toolkit for cuda.h (default: $CUDA_HOME, /opt/cuda, /usr/local/cuda)
#   --no-tracer          leave the tracer and the probes out (no traces on the lab machine)
#
# run options (on the lab machine; read-only towards card and driver except where said):
#   --device N           the GPU to sweep and trace (RM device instance; default 0)
#   --bdf BDF            passed to lab-capture.sh: capture only this GPU
#   --rom                passed to lab-capture.sh: read the PCI option ROM (sudo; the one write
#                        of the kit, the sysfs ROM enable bit, undone right after the read)
#   --prom               passed to lab-capture.sh: read the VBIOS through the BAR0 PROM window
#                        (sudo, read-only mapping)
#   --priv               also run the privileged getters of the sweep as root (sudo; read-only)
#   --no-sudo            never call sudo (the sudo items are recorded as skipped)
#   --no-traces          skip the traced probes
#   --keep-edid          keep EDIDs as captured (default: the serial number fields are zeroed)
#
# Result of run: OUTDIR/lab-kit-<chip>-<stamp>.tar.gz and its .sha256; the tarball holds
# MANIFEST.txt, SHA256SUMS, run.log and one directory per step. Identifiers of the machine and
# its users (host name, user name, home directory, e-mail addresses, GPU UUIDs, board serial
# numbers, MAC and IP addresses, machine id, file-system UUIDs, process names and PIDs) are
# replaced in every text file before the tarball is written, and a final check searches every
# file, binary ones included, for each collected identifier and refuses to write the tarball if
# one is left. The kit calls no network tool and sends nothing anywhere.
set -uo pipefail
export LC_ALL=C

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
KIT_VERSION="lab-kit 1 (2026-09-24)"
SPDX='# SPDX-License-Identifier: MIT'

usage() { sed -n '5,42p' "$0"; }
die() { echo "lab-kit: $*" >&2; exit 2; }
have() { command -v "$1" >/dev/null 2>&1; }

# max GLIBC_x.y a dynamic ELF needs, from its version-needed section (python, no binutils)
glibc_need() {
    python3 - "$1" <<'PY'
import re, sys
d = open(sys.argv[1], "rb").read()
if d[:4] != b"\x7fELF":
    print("-"); sys.exit()
vs = sorted({tuple(int(x) for x in m.group(1).split(b".")) for m in re.finditer(rb"GLIBC_(\d+\.\d+(?:\.\d+)?)\x00", d)})
print(".".join(map(str, vs[-1])) if vs else "static")
PY
}

# ===========================================================================
# pack
# ===========================================================================
do_pack() {
    local repo outdir ogkm leandro ltest cuda notracer=0
    repo=$(cd "$here/../.." && pwd)
    outdir=$repo/target/lab-kit
    ogkm=${CARAXES_OGKM:-$repo/vendor/open-gpu-kernel-modules}
    leandro=$repo/../Leandro
    ltest=$repo/../Leandro-Test
    cuda=${CUDA_HOME:-}
    while [ $# -gt 0 ]; do
        case $1 in
            -o) outdir=$2; shift 2 ;;
            --ogkm) ogkm=$2; shift 2 ;;
            --leandro) leandro=$2; shift 2 ;;
            --leandro-test) ltest=$2; shift 2 ;;
            --cuda) cuda=$2; shift 2 ;;
            --no-tracer) notracer=1; shift ;;
            *) die "pack: unknown option $1" ;;
        esac
    done
    [ -f "$ogkm/version.mk" ] || die "pack: no open-gpu-kernel-modules tree at $ogkm (--ogkm)"
    local drv major
    drv=$(sed -n 's/^NVIDIA_VERSION *= *//p' "$ogkm/version.mk")
    major=${drv%%.*}
    for t in python3 rustc cc sha256sum tar; do have $t || die "pack: $t not found"; done
    if [ -z "$cuda" ]; then
        for c in /opt/cuda /usr/local/cuda /usr/lib/cuda; do [ -f "$c/include/cuda.h" ] && { cuda=$c; break; }; done
    fi

    local name="caraxes-lab-kit-$drv"
    local stage
    stage=$(mktemp -d "${TMPDIR:-/tmp}/lab-kit-pack.XXXXXX") || die "pack: mktemp"
    local P=$stage/$name
    mkdir -p "$P/bin" "$P/lib" "$P/probe" || die "pack: mkdir"
    local notes=()
    echo "pack: driver $drv from $ogkm"

    # 1. the sweep list of this driver version
    python3 "$repo/caraxes/tools/sweep-list.py" --ogkm "$ogkm" --out "$P/sweep-$drv.tsv" \
        || die "pack: sweep-list.py failed"

    # 2. the rustc-only tools, statically linked (no glibc requirement on the lab machine)
    local t
    for t in lab-rm-query fifo-host-query host-facts lab-sweep; do
        rustc -O --edition 2021 -C target-feature=+crt-static "$repo/caraxes/tools/$t.rs" -o "$P/bin/$t" \
            || die "pack: rustc $t failed"
    done
    notes+=("tools: lab-rm-query fifo-host-query host-facts lab-sweep (static)")

    # 3. the tracer and the probes
    if [ $notracer = 0 ]; then
        local feat=()
        [ "$major" != 615 ] && feat=(--no-default-features --features "v$major")
        if [ -f "$leandro/crates/nvrm-trace/Cargo.toml" ] && have cargo \
            && (cd "$leandro" && cargo build --offline --release -p nvrm-trace "${feat[@]}" >/dev/null 2>&1); then
            cp "$leandro/target/release/libnvrm_trace.so" "$P/lib/" && notes+=("tracer: nvrm-trace, ABI v$major")
        else
            notes+=("tracer: NOT BUILT (need $leandro and cargo with the crates cached; --offline)")
        fi
        local src=$ltest/probe/c p
        if [ -d "$src" ] && [ -n "$cuda" ]; then
            # -std=gnu11 keeps the C23 symbol versions (__isoc23_*) out, so the probes run
            # on older distributions than the build host's
            for p in nvprobe managedprobe hostregprobe; do
                cc -std=gnu11 -O1 -I"$cuda/include" "$src/$p.c" -o "$P/probe/$p" -L"$cuda/lib64" -lcuda 2>/dev/null \
                    && notes+=("probe: $p") || notes+=("probe: $p NOT BUILT")
            done
            cc -std=gnu11 -O1 -I"$cuda/include" "$src/oclprobe.c" -o "$P/probe/oclprobe" -lOpenCL 2>/dev/null \
                && notes+=("probe: oclprobe") || notes+=("probe: oclprobe NOT BUILT")
            cc -std=gnu11 -O1 "$src/vkalloc.c" -o "$P/probe/vkalloc" -lvulkan 2>/dev/null \
                && notes+=("probe: vkalloc") || notes+=("probe: vkalloc NOT BUILT")
            cp "$ltest/probe/kernels/kernels.ptx" "$P/probe/" 2>/dev/null && notes+=("probe: kernels.ptx (sm_75, JITs forward)")
        else
            notes+=("probes: NOT BUILT (need $src and a CUDA toolkit for cuda.h)")
        fi
    fi

    # 4. the scripts
    cp "$repo/caraxes/tools/lab-kit.sh" "$repo/caraxes/tools/lab-capture.sh" "$P/"
    chmod 755 "$P/lab-kit.sh" "$P/lab-capture.sh"

    # 5. what the binaries need, and the pack's own description
    {
        echo "$SPDX"
        echo "# file glibc (the highest GLIBC_ symbol version the file needs; static = none)"
        for f in "$P"/bin/* "$P"/lib/* "$P"/probe/*; do
            [ -f "$f" ] || continue
            case $f in *.ptx) continue ;; esac
            echo "${f#"$P"/} $(glibc_need "$f")"
        done
    } >"$P/REQUIRES.txt"
    {
        echo "$SPDX"
        echo "kit: $KIT_VERSION"
        echo "driver: $drv"
        echo "built: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "caraxes: $(git -C "$repo" rev-parse --short HEAD 2>/dev/null || echo unknown)"
        printf 'note: %s\n' "${notes[@]}"
    } >"$P/KIT.txt"
    (cd "$P" && find . -type f ! -name SHA256SUMS -printf '%P\n' | sort | xargs -d '\n' sha256sum >SHA256SUMS)

    mkdir -p "$outdir" || die "pack: mkdir $outdir"
    tar -C "$stage" -czf "$outdir/$name.tar.gz" "$name" || die "pack: tar"
    (cd "$outdir" && sha256sum "$name.tar.gz" >"$name.tar.gz.sha256")
    rm -rf "$stage"
    printf '  %s\n' "${notes[@]}"
    echo "pack: $outdir/$name.tar.gz"
    echo "pack: $(cut -d' ' -f1 "$outdir/$name.tar.gz.sha256")  (sha256)"
}

# ===========================================================================
# verify
# ===========================================================================
do_verify() {
    local tb=${1:-}
    [ -f "$tb" ] || die "verify: no tarball $tb"
    if [ -f "$tb.sha256" ]; then
        (cd "$(dirname "$tb")" && sha256sum -c --quiet "$(basename "$tb").sha256") || die "verify: tarball sum mismatch"
    fi
    local vt
    vt=$(mktemp -d "${TMPDIR:-/tmp}/lab-kit-verify.XXXXXX")
    tar -C "$vt" -xzf "$tb" || { rm -rf "$vt"; die "verify: cannot unpack"; }
    local d
    d=$(find "$vt" -mindepth 1 -maxdepth 1 -type d | head -1)
    if (cd "$d" && sha256sum --quiet -c SHA256SUMS); then
        echo "verify: $(find "$d" -type f | wc -l) files, all sums match"
        sed -n '1,200p' "$d/MANIFEST.txt" 2>/dev/null
        rm -rf "$vt"
    else
        rm -rf "$vt"
        die "verify: file sums do not match"
    fi
}

# ===========================================================================
# run
# ===========================================================================
do_run() {
    local parent=${1:-}
    [ -n "$parent" ] || die "run: OUTDIR missing (usage: lab-kit.sh run OUTDIR [options])"
    shift
    local device=0 bdf="" rom=0 prom=0 priv=0 nosudo=0 notraces=0 keepedid=0
    while [ $# -gt 0 ]; do
        case $1 in
            --device) device=$2; shift 2 ;;
            --bdf) bdf=$2; shift 2 ;;
            --rom) rom=1; shift ;;
            --prom) prom=1; shift ;;
            --priv) priv=1; shift ;;
            --no-sudo) nosudo=1; shift ;;
            --no-traces) notraces=1; shift ;;
            --keep-edid) keepedid=1; shift ;;
            *) die "run: unknown option $1" ;;
        esac
    done
    [ -f "$here/KIT.txt" ] || die "run: not an unpacked pack (no KIT.txt next to the script); build one with 'lab-kit.sh pack'"
    local kitdrv
    kitdrv=$(sed -n 's/^driver: //p' "$here/KIT.txt")
    mkdir -p "$parent" || die "run: cannot create $parent"
    parent=$(cd "$parent" && pwd)
    local stamp W
    stamp=$(date +%Y%m%d-%H%M%S)
    W=$parent/lab-kit-work-$stamp
    ( umask 077; mkdir -p "$W" ) || die "run: mkdir"
    local log=$W/run.log ids=$W/.identifiers
    local manifest=() N=9 step=0 t0 T0
    T0=$(date +%s)
    item() { manifest+=("$(printf '%-24s %-8s %-36s %s' "$1" "$2" "$3" "$4")"); }
    begin() { step=$((step + 1)); t0=$(date +%s); printf '[%d/%d] %s ... ' "$step" "$N" "$1" | tee -a "$log"; }
    end() { printf '%s (%ss)\n' "$1" "$(($(date +%s) - t0))" | tee -a "$log"; }
    sudo_ok=0
    # sudo only when the operator cached it (sudo -v) and did not say --no-sudo; the kit never asks
    if [ $nosudo = 0 ] && have sudo && sudo -n true 2>/dev/null; then sudo_ok=1; fi

    # ---- 1. preflight -----------------------------------------------------
    begin "preflight: pack sums, tools, driver, glibc"
    local pf=$W/preflight.txt st=ok
    {
        echo "$SPDX"
        echo "# $KIT_VERSION; pack driver $kitdrv"
        (cd "$here" && sha256sum --quiet -c SHA256SUMS) && echo "pack sums: ok" || { echo "pack sums: MISMATCH"; st=failed; }
        for t in bash python3 lspci nvidia-smi sha256sum tar; do
            have $t && echo "tool $t: $(command -v $t)" || echo "tool $t: MISSING"
        done
        local run
        run=$(grep -oE '[0-9]+\.[0-9]+(\.[0-9]+)?' /proc/driver/nvidia/version 2>/dev/null | head -1)
        echo "driver running: ${run:-none}; pack: $kitdrv"
        [ "$run" = "$kitdrv" ] || echo "DRIVER MISMATCH: the sweep sizes are those of $kitdrv; the RM refuses a changed size (NV_ERR_INVALID_ARGUMENT), the traces decode with the $kitdrv ABI"
        local g
        g=$(ldd --version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+$')
        echo "glibc: ${g:-unknown}"
        while read -r f need; do
            case $f in \#*|"") continue ;; esac
            case $need in static|-) echo "needs $f: none"; continue ;; esac
            if [ -n "$g" ] && [ "$(printf '%s\n%s\n' "$need" "$g" | sort -V | tail -1)" = "$g" ]; then
                echo "needs $f: glibc $need, ok"
            else
                echo "needs $f: glibc $need, TOO NEW for ${g:-unknown}: skipped"
                mv "$here/$f" "$here/$f.too-new" 2>/dev/null
            fi
        done <"$here/REQUIRES.txt"
        echo "sudo: $([ $sudo_ok = 1 ] && echo cached || echo 'not used')"
    } >"$pf" 2>&1
    # the identifiers the scrub replaces and the final check looks for; kept out of the tarball
    (
        umask 077
        {
            echo "host $(hostname -s 2>/dev/null)"
            echo "host $(hostname -f 2>/dev/null)"
            echo "host $(cat /proc/sys/kernel/hostname 2>/dev/null)"
            echo "user $(id -un 2>/dev/null)"
            echo "home ${HOME:-}"
            nvidia-smi --query-gpu=uuid --format=csv,noheader 2>/dev/null | sed 's/^/gpuuuid /'
            nvidia-smi --query-gpu=serial --format=csv,noheader 2>/dev/null | sed 's/^/serial /'
            cat /sys/class/net/*/address 2>/dev/null | grep -v '^00:00:00:00:00:00$' | sed 's/^/mac /'
            hostname -I 2>/dev/null | tr ' ' '\n' | sed '/^$/d; s/^/ip /'
            echo "machineid $(cat /etc/machine-id 2>/dev/null)"
            ls /dev/disk/by-uuid 2>/dev/null | sed 's/^/fsuuid /'
            for f in product_uuid product_serial board_serial chassis_serial; do
                [ -r "/sys/class/dmi/id/$f" ] && echo "dmi $(cat "/sys/class/dmi/id/$f" 2>/dev/null)"
            done
        } | awk 'NF == 2 && length($2) >= 3' | sort -u >"$ids"
    )
    end "$st, $(wc -l <"$ids") identifiers collected"
    item preflight "$st" preflight.txt "driver, glibc, pack sums"

    # ---- 2. lab-capture.sh part 1 ------------------------------------------
    begin "capture: config space, lspci, nvidia-smi, /proc, firmware, display (lab-capture.sh)"
    local capargs=(-o "$W/lc" --no-rm-tools)
    [ -n "$bdf" ] && capargs+=(--bdf "$bdf")
    [ $rom = 1 ] && capargs+=(--rom)
    [ $prom = 1 ] && capargs+=(--prom)
    [ $sudo_ok = 0 ] && capargs+=(--no-sudo)
    if bash "$here/lab-capture.sh" "${capargs[@]}" >>"$log" 2>&1; then st=ok; else st=failed; fi
    local lc
    lc=$(find "$W/lc" -mindepth 1 -maxdepth 1 -type d -name 'lab-capture-*' | head -1)
    if [ -n "$lc" ]; then
        mv "$lc" "$W/capture"
        rm -rf "$W/lc"   # its own tarball holds the unscrubbed files: never carried
    fi
    local chip
    chip=$(sed -n 's/.*chip \([a-z0-9-]*\),.*/\1/p' "$W/capture/MANIFEST.txt" 2>/dev/null | head -1)
    chip=${chip:-unknown}
    end "$st, chip $chip"
    item capture "$st" capture/ "lab-capture.sh ${capargs[*]:2}"

    # ---- 3. RM identity tools ----------------------------------------------
    begin "rm-tools: identity, FIFO and host facts (getters on a private RM client)"
    mkdir -p "$W/rm-tools"
    st=ok
    for t in lab-rm-query fifo-host-query host-facts; do
        local a=(); [ $t = host-facts ] && a=(facts)
        if [ -x "$here/bin/$t" ]; then
            { echo "$SPDX"; echo "# $t ${a[*]}; kit driver $kitdrv"; timeout 120 "$here/bin/$t" "${a[@]}" 2>&1; } >"$W/rm-tools/$t.txt" || st=partial
        else
            st=partial
        fi
    done
    end "$st"
    item rm-tools "$st" rm-tools/ "lab-rm-query, fifo-host-query, host-facts facts"

    # ---- 4. the sweep -------------------------------------------------------
    begin "sweep: every A, B and E getter on the card's own objects"
    mkdir -p "$W/sweep"
    local list
    list=$(ls "$here"/sweep-*.tsv 2>/dev/null | head -1)
    st=failed
    if [ -x "$here/bin/lab-sweep" ] && [ -n "$list" ]; then
        { echo "$SPDX"; timeout 300 "$here/bin/lab-sweep" "$list" --device "$device" 2>&1; } >"$W/sweep/sweep-user.txt" && st=ok
        if [ $priv = 1 ] && [ $sudo_ok = 1 ]; then
            { echo "$SPDX"; timeout 300 sudo -n "$here/bin/lab-sweep" "$list" --device "$device" --priv-only 2>&1; } >"$W/sweep/sweep-priv.txt" || st=partial
        fi
        cp "$list" "$W/sweep/"
    fi
    end "$st, $(grep -vc '^#' "$W/sweep/sweep-user.txt" 2>/dev/null || echo 0) calls"
    item sweep "$st" sweep/ "$([ $priv = 1 ] && echo 'user and privileged passes' || echo 'user pass')"

    # ---- 5. native RM traces --------------------------------------------------
    begin "traces: CUDA, NVML, OpenCL and Vulkan probes under the LD_PRELOAD tracer"
    mkdir -p "$W/traces"
    local tr=$here/lib/libnvrm_trace.so ntr=0 nok=0
    if [ $notraces = 1 ] || [ ! -r "$tr" ]; then
        st=skipped
    else
        # probe: name, command, success line (the matrix criteria, Leandro-Test probe/matrix/*.sh)
        local probes=(
            "cuda-core|$here/probe/nvprobe 2|stage 2 ok"
            "cuda-jit|$here/probe/nvprobe 3|stage 3 ok"
            "cuda-launch|$here/probe/nvprobe 4|stage 4 ok"
            "cuda-managed|$here/probe/managedprobe|"
            "cuda-hostreg|$here/probe/hostregprobe|"
            "nvml|nvidia-smi -q|Driver Version"
            "opencl|$here/probe/oclprobe|"
            "vk-alloc|$here/probe/vkalloc|"
        )
        local entry pn cmd ok bin
        for entry in "${probes[@]}"; do
            IFS='|' read -r pn cmd ok <<<"$entry"
            bin=${cmd%% *}
            if [ "$bin" != nvidia-smi ] && [ ! -x "$bin" ]; then
                echo "$pn: skipped (binary absent or needs a newer glibc)" >>"$W/traces/summary.txt"
                continue
            fi
            ntr=$((ntr + 1))
            local raw=$W/traces/$pn.raw rc out
            # shellcheck disable=SC2086
            out=$(cd "$here/probe" && NVPROBE_PTX="$here/probe/kernels.ptx" LEA_TRACE_FILE="$raw" LD_PRELOAD="$tr" \
                timeout 300 $cmd 2>&1); rc=$?
            printf '%s\n' "$out" >"$W/traces/$pn.out"
            local verdict=ok
            [ $rc = 0 ] || verdict="exit $rc"
            [ -n "$ok" ] && ! grep -q "$ok" <<<"$out" && verdict="$verdict, no '$ok'"
            [ "$verdict" = ok ] && nok=$((nok + 1))
            local meta
            meta=$(printf '{"t":"meta","driver":"%s","gpu":"%s","kernel":"%s","date":"%s","probe":"%s","side":"native","kit":"%s"}' \
                "$(grep -oE '[0-9]+\.[0-9]+\.[0-9]+' /proc/driver/nvidia/version | head -1)" \
                "$(nvidia-smi --query-gpu=name --format=csv,noheader -i "$device" 2>/dev/null | head -1)" \
                "$(uname -r)" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$pn" "$KIT_VERSION")
            [ -f "$raw.jsonl" ] && { echo "$meta"; cat "$raw.jsonl"; } >"$W/traces/$pn.jsonl"
            [ -f "$raw" ] && { echo "# $meta"; cat "$raw"; } >"$W/traces/$pn.tsv"
            rm -f "$raw" "$raw.jsonl"
            echo "$pn: $verdict" >>"$W/traces/summary.txt"
        done
        st=ok; [ $nok -lt $ntr ] && st=partial; [ $ntr = 0 ] && st=skipped
    fi
    end "$st, $nok of $ntr probes passed their criterion"
    item traces "$st" traces/ "$nok/$ntr probes (summary.txt)"

    # ---- 6. display ---------------------------------------------------------------
    begin "display: heads, connectors, modes"
    mkdir -p "$W/display"
    {
        echo "$SPDX"
        nvidia-smi --query-gpu=index,display_mode,display_active --format=csv 2>&1
        for c in /sys/class/drm/card*-*; do
            [ -e "$c/status" ] || continue
            echo "${c##*/}: $(cat "$c/status" 2>/dev/null) enabled=$(cat "$c/enabled" 2>/dev/null) modes=$(tr '\n' ' ' <"$c/modes" 2>/dev/null)"
        done
        grep -E "^#.*0073|"$'\t'"NV0073" "$W/sweep/sweep-user.txt" 2>/dev/null
    } >"$W/display/display.txt"
    if grep -q ': connected' "$W/display/display.txt"; then st=ok; else st=nohead; fi
    end "$st"
    item display "$st" display/ "DRM connectors and the 0073 rows of the sweep (EDIDs are in capture/)"

    # ---- 7. MIG, NVLink, topology, GSP firmware (read-only listings) ---------------------
    begin "extras: MIG profiles, NVLink status, topology, GSP firmware sections"
    mkdir -p "$W/extras"
    local x
    for x in "mig -lgip" "mig -lgipp" "mig -lcip" "nvlink --status" "nvlink --capabilities" "topo -m"; do
        # shellcheck disable=SC2086
        { echo "$SPDX"; echo "# nvidia-smi $x"; timeout 60 nvidia-smi $x 2>&1; } >"$W/extras/nvidia-smi-${x// /_}.txt"
    done
    python3 - "$W/extras/gsp-firmware-sections.txt" <<'PY'
# The ELF section table of every gsp_*.bin of the running driver (read-only on the files; the
# boot model's open item "appVersion", docs/caraxes/gsp-boot-blackwell.md section 6).
import glob, hashlib, re, struct, sys
out = ["# SPDX-License-Identifier: MIT"]
v = re.search(r"(\d+\.\d+(?:\.\d+)?)", open("/proc/driver/nvidia/version").read())
for p in sorted(glob.glob("/lib/firmware/nvidia/%s/gsp_*.bin" % (v.group(1) if v else "*"))):
    d = open(p, "rb").read()
    out.append("%s %d bytes sha256 %s" % (p, len(d), hashlib.sha256(d).hexdigest()))
    if d[:4] != b"\x7fELF" or d[4] != 2:
        out.append("  not an ELF64 image"); continue
    shoff, = struct.unpack_from("<Q", d, 0x28)
    shentsize, shnum, shstrndx = struct.unpack_from("<HHH", d, 0x3A)
    sh = [struct.unpack_from("<IIQQQQIIQQ", d, shoff + i * shentsize) for i in range(shnum)]
    stro = sh[shstrndx][4]
    for s in sh:
        name = d[stro + s[0]: d.index(b"\0", stro + s[0])].decode(errors="replace")
        out.append("  %-32s off %#10x size %#10x" % (name, s[4], s[5]))
open(sys.argv[1], "w").write("\n".join(out) + "\n")
PY
    end ok
    item extras ok extras/ "nvidia-smi mig/nvlink/topo listings, GSP firmware ELF sections"

    # ---- 8. scrub ---------------------------------------------------------------------
    begin "scrub: replace identifiers, zero EDID serials, final check"
    rm -rf "$W/lc"
    if python3 - "$W" "$ids" "$keepedid" >>"$log" 2>&1 <<'PY'
import os, re, sys
root, idf, keepedid = sys.argv[1], sys.argv[2], sys.argv[3] == "1"
GENERIC_USERS = {"root", "ubuntu", "ec2-user", "azureuser", "admin", "cloud-user", "debian", "centos",
                 "user", "gpu", "nvidia", "rocky", "fedora", "opc", "core"}
ids = []
for line in open(idf):
    kind, val = line.rstrip("\n").split(" ", 1)
    if kind == "user" and val in GENERIC_USERS:
        continue
    if kind == "home" and val in ("/", "/root"):
        continue
    ids.append((kind, val))
subs = []  # (bytes pattern, replacement), applied to text files
counter = {}
def label(kind):
    counter[kind] = counter.get(kind, 0) + 1
    return "<%s%d>" % (kind, counter[kind])
for kind, val in sorted(ids, key=lambda kv: -len(kv[1])):
    lab = label(kind).encode()
    b = val.encode()
    subs.append((re.compile(re.escape(b) if kind in ("home", "ip") else rb"(?<![\w.-])" + re.escape(b) + rb"(?![\w-])"), lab))
    # the same value as it appears in hex dumps of the traces and the sweep
    for form in (b.hex(), b.hex().upper()):
        if len(form) >= 8:
            subs.append((re.compile(form.encode()), b"x" * len(form)))
    if kind == "gpuuuid" and val.startswith("GPU-"):
        raw = bytes.fromhex(val[4:].replace("-", ""))
        for form in (raw.hex(), raw.hex().upper()):
            subs.append((re.compile(form.encode()), b"x" * len(form)))
    if kind == "mac":
        for form in (val.replace(":", "").lower(), val.replace(":", "").upper(), val.upper()):
            subs.append((re.compile(re.escape(form.encode())), lab))
GEN = [
    (re.compile(rb"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"), b"<email>"),
    (re.compile(rb"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b"), b"<uuid>"),
    (re.compile(rb"\b(?:[0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}\b"), b"<mac>"),
    (re.compile(rb"^(\s*(?:Process ID|Process Name|Serial Number|Board Serial Number|Chassis Serial Number|GPU Fabric GUID|Hostname)\s*:).*$", re.M), rb"\1 <redacted>"),
    (re.compile(rb"<(pid|process_name|serial|board_serial_number|chassis_serial_number|hostname|gpu_fabric_guid)>[^<]*</"), rb"<\1>redacted</"),
]
TEXT = (".txt", ".csv", ".xml", ".md", ".log", ".jsonl", ".tsv", ".out", ".strace", ".hex")
GEN.append((re.compile(rb"^(\s*(?:Display Product Serial Number|Serial Number|Alphanumeric Data String)\s*:).*$", re.M), rb"\1 <redacted>"))
# EDIDs (lab-capture.sh writes <connector>.edid.bin and an od dump of it into connectors.txt):
# zero the serial number and the serial string descriptor, fix the checksum, and redo the dump.
edids = {}
if not keepedid:
    for d, _, files in os.walk(root):
        for fn in files:
            if not fn.endswith(".edid.bin"):
                continue
            p = os.path.join(d, fn)
            e = bytearray(open(p, "rb").read())
            if len(e) >= 128:
                e[12:16] = b"\0\0\0\0"                       # ID serial number
                for o in (54, 72, 90, 108):                  # display descriptors
                    if e[o:o + 3] == b"\0\0\0" and e[o + 3] == 0xFF:
                        e[o + 5:o + 18] = b"\n" + b" " * 12  # serial string descriptor
                e[127] = (-sum(e[:127])) & 0xFF
                open(p, "wb").write(bytes(e))
            edids[fn] = bytes(e)
def redump(text):
    out, lines, i = [], text.split(b"\n"), 0
    while i < len(lines):
        out.append(lines[i])
        m = re.search(rb"-> (\S+\.edid\.bin)$", lines[i])
        i += 1
        if m and m.group(1).decode() in edids:
            while i < len(lines) and re.match(rb"^\s+(?:[0-9a-f]{2} ?)+$", lines[i]):
                i += 1
            e = edids[m.group(1).decode()]
            out += [b"   " + b" ".join(b"%02x" % x for x in e[k:k + 16]) for k in range(0, len(e), 16)]
    return b"\n".join(out)
n_files = 0
for d, _, files in os.walk(root):
    for fn in files:
        p = os.path.join(d, fn)
        if p == idf or not fn.endswith(TEXT):
            continue
        t = open(p, "rb").read()
        if fn == "connectors.txt" and edids:
            t0, t = t, redump(t)
            if t != t0:
                open(p, "wb").write(t)
        u = t
        for rx, rep in subs:
            u = rx.sub(rep, u)
        for rx, rep in GEN:
            u = rx.sub(rep, u)
        if u != t:
            open(p, "wb").write(u)
            n_files += 1
# final check: every collected value, in every file, binary ones too
left = []
for d, _, files in os.walk(root):
    for fn in files:
        p = os.path.join(d, fn)
        if p == idf:
            continue
        data = open(p, "rb").read()
        for kind, val in ids:
            forms = {val.encode(), val.encode().hex().encode()}
            if kind == "gpuuuid" and val.startswith("GPU-"):
                forms.add(bytes.fromhex(val[4:].replace("-", "")))
            if any(f in data for f in forms if len(f) >= 4):
                if kind in ("host", "user") and not fn.endswith(TEXT):
                    # a short name inside a binary (ROM, EDID) is almost surely a coincidence
                    if not re.search(rb"(?<![\w.-])" + re.escape(val.encode()) + rb"(?![\w-])", data):
                        continue
                left.append((os.path.relpath(p, root), kind))
print("scrub: %d text files changed, %d identifiers checked" % (n_files, len(ids)))
for p, k in sorted(set(left)):
    print("RESIDUAL %s in %s" % (k, p))
sys.exit(1 if left else 0)
PY
    then st=ok; else st=RESIDUAL; fi
    end "$st"
    item scrub "$st" run.log "identifiers replaced; final check $([ "$st" = ok ] && echo clean || echo 'found leftovers, see run.log')"
    if [ "$st" != ok ]; then
        echo "lab-kit: identifiers left after the scrub (run.log names the files, not the values);" >&2
        echo "lab-kit: nothing was packed. The work directory $W is private (mode 700); inspect or delete it." >&2
        exit 3
    fi

    # ---- 9. manifest, sums, tarball -----------------------------------------------------
    begin "pack: manifest, SHA256SUMS, tarball, verify"
    rm -f "$ids"
    local name=lab-kit-$chip-$stamp
    {
        echo "$SPDX"
        echo "# $KIT_VERSION, pack driver $kitdrv"
        echo "# created $(date -Is), kernel $(uname -r), chip $chip, device $device, runtime $(($(date +%s) - T0))s"
        echo "# options: rom=$rom prom=$prom priv=$priv sudo=$sudo_ok traces=$((1 - notraces)) keep_edid=$keepedid"
        printf '%-24s %-8s %-36s %s\n' "# step" status path note
        printf '%s\n' "${manifest[@]}"
    } >"$W/MANIFEST.txt"
    mv "$W" "$parent/$name"
    chmod -R go+rX "$parent/$name"
    (cd "$parent/$name" && find . -type f ! -name SHA256SUMS -printf '%P\n' | sort | xargs -d '\n' sha256sum >SHA256SUMS)
    tar -C "$parent" -czf "$parent/$name.tar.gz" "$name"
    (cd "$parent" && sha256sum "$name.tar.gz" >"$name.tar.gz.sha256")
    local vt vv
    vt=$(mktemp -d "${TMPDIR:-/tmp}/lab-kit-verify.XXXXXX")
    if tar -C "$vt" -xzf "$parent/$name.tar.gz" && (cd "$vt/$name" && sha256sum --quiet -c SHA256SUMS); then
        vv="verified: $(find "$vt/$name" -type f | wc -l) files, all sums match"
    else
        vv="VERIFY FAILED"
    fi
    rm -rf "$vt"
    end "$vv"
    echo "== tarball $parent/$name.tar.gz ($(du -h "$parent/$name.tar.gz" | cut -f1))"
    echo "== sha256  $(cut -d' ' -f1 "$parent/$name.tar.gz.sha256")"
    echo "== carry home the .tar.gz and the .sha256; docs/caraxes/gsp-plan/lab-kit.md section 5 says what to check first"
    [ "$vv" = "VERIFY FAILED" ] && exit 1
    exit 0
}

cmd=${1:-}
[ -n "$cmd" ] || { usage; exit 2; }
shift
case $cmd in
    pack) do_pack "$@" ;;
    run) do_run "$@" ;;
    verify) do_verify "$@" ;;
    -h|--help|help) usage ;;
    *) usage >&2; exit 2 ;;
esac
