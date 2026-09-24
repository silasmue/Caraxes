#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
#
# Build and run gsp-abi-sizes.c against a vendored open-gpu-kernel-modules
# tree and print the compiled sizes and field offsets of the CPU<->GSP
# message-queue and boot structures as Markdown.
#
#   caraxes/tools/gsp-abi-sizes.sh [--driver-flags] [--check] [--abi N] [OGKM_DIR]
#
#   OGKM_DIR        default: $CARAXES_OGKM, else vendor/open-gpu-kernel-modules
#   --abi N         driver major version whose field lists to use: below 610
#                   the 595.91.07 lists (pre-MCTP queue element, no vbios
#                   revisions in GspStaticConfigInfo), 610 to 614 the 610.57.04
#                   lists, 615 and above the 615.71.09 lists (GSP_ABI in
#                   gsp-abi-sizes.c). Default: the major version
#                   in OGKM_DIR/version.mk. A mismatch with the tree is
#                   reported on stderr; the tree's headers decide whether it
#                   compiles.
#   --driver-flags  compile with the full include/define set of the driver's
#                   own build (src/nvidia/Makefile:28-138) instead of the
#                   minimal set below
#   --check         compile with the minimal set, with the full driver set,
#                   and (if available) with the other of gcc/clang; fail if
#                   any two outputs differ. Prints the output once.
#   CC=...          compiler for the minimal set (default: cc). The driver
#                   set is always compiled with GCC (GCC=..., default gcc):
#                   under clang it makes nvport pick atomic_c11.h, which
#                   includes a stdatomic.h the open tree does not ship
#                   (nvport/atomic.h:396-397, inline/atomic_c11.h:36-40).
#
# Only host files are read and a temporary directory is written; nothing
# here touches a GPU.
#
# Why these flags. crates/nvrm-sys/wrapper.h only needs the SDK headers
# (crates/xtask/src/abi/mod.rs:22-28, :312-317: -DNV_LINUX -D__linux__ and
# five -I roots). The GSP structures live in RM-internal headers that pull in
# NVOC-generated classes (gsp_static_config.h:40 includes gpu/gpu.h), so they
# need the RM's own include roots and the nvport configuration defines. The
# MINIMAL set below was found by removing one flag at a time from the
# driver's full set while the program still compiled AND printed
# byte-identical output; --check re-verifies that on every run.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)
src="$here/gsp-abi-sizes.c"

mode=minimal
check=0
abi=""
ogkm="$root/vendor/open-gpu-kernel-modules"
[ -z "${CARAXES_OGKM:-}" ] || ogkm=$(cd "$CARAXES_OGKM" && pwd)
while [ $# -gt 0 ]; do
    case "$1" in
        --driver-flags) mode=driver ;;
        --check)        check=1 ;;
        --abi)          abi=$2; shift ;;
        -h|--help)      sed -n '4,30p' "$0"; exit 0 ;;
        -*)             echo "unknown option: $1" >&2; exit 2 ;;
        *)              ogkm=$(cd "$1" && pwd) ;;
    esac
    shift
done

S="$ogkm/src"
N="$S/nvidia"
C="$S/common"
[ -f "$N/inc/kernel/gpu/gsp/gsp_static_config.h" ] ||
    { echo "not an open-gpu-kernel-modules tree: $ogkm" >&2; exit 2; }

# The field-list variant (GSP_ABI): 595 below driver major 610, 610 below 615, 615 from there on.
tree_major=$(sed -n 's/^NVIDIA_VERSION *= *\([0-9]*\)\..*/\1/p' "$ogkm/version.mk" 2>/dev/null)
[ -n "$abi" ] || abi=$tree_major
case $abi in ''|*[!0-9]*) echo "cannot tell the driver major version (--abi N)" >&2; exit 2 ;; esac
if [ -n "$tree_major" ] && [ "$abi" != "$tree_major" ]; then
    echo "warning: --abi $abi but the tree is major $tree_major" >&2
fi
if [ "$abi" -ge 615 ]; then gsp_abi=615; elif [ "$abi" -ge 610 ]; then gsp_abi=610; else gsp_abi=595; fi

if [ "$(uname -m)" != x86_64 ]; then
    echo "warning: host is $(uname -m); the kernel module whose layout matters is x86_64" >&2
fi

minimal_flags() {
    printf '%s\n' \
        -I"$N/interface" \
        -I"$C/sdk/nvidia/inc" \
        -I"$N/arch/nvalloc/common/inc" \
        -I"$N/arch/nvalloc/common/inc/gsp" \
        -I"$N/arch/nvalloc/unix/include" \
        -I"$N/inc" \
        -I"$C/shared/inc" \
        -I"$C/inc" \
        -I"$C/uproc/os/common/include" \
        -I"$C/inc/swref" \
        -I"$N/generated" \
        -I"$C/nvlink/inband/interface" \
        -I"$N/inc/libraries" \
        -I"$N/inc/kernel" \
        -DNVRM \
        -DPORT_IS_KERNEL_BUILD=1 \
        -DPORT_IS_CHECKED_BUILD=0 \
        -DPORT_MODULE_atomic=1 \
        -DPORT_MODULE_debug=1 \
        -DPORT_MODULE_memory=1 \
        -DPORT_MODULE_thread=1
    # 615.71.09 headers call portTimeGetUptimeNanoseconds (generated/g_objtmr_nvoc.h:1192)
    [ "$gsp_abi" = 615 ] && printf '%s\n' -DPORT_MODULE_time=1
    printf '%s\n' -DGSP_ABI="$gsp_abi"
}

# src/nvidia/Makefile:28-58 (includes) and :106-138 (defines), in order.
# Kernel code-generation flags (-mcmodel=kernel, -mno-sse, ...) do not
# change struct layout and would stop a hosted program from linking.
driver_flags() {
    printf '%s\n' \
        -include "$C/sdk/nvidia/inc/cpuopsys.h" \
        -I"$N/kernel/inc" -I"$N/interface" \
        -I"$C/sdk/nvidia/inc" -I"$C/sdk/nvidia/inc/hw" \
        -I"$N/arch/nvalloc/common/inc" -I"$N/arch/nvalloc/common/inc/gsp" \
        -I"$N/arch/nvalloc/common/inc/deprecated" -I"$N/arch/nvalloc/unix/include" \
        -I"$N/inc" -I"$N/inc/os" -I"$C/shared/inc" -I"$C/inc" \
        -I"$C/uproc/os/libos-v2.0.0/include" -I"$C/uproc/os/common/include" \
        -I"$C/inc/swref" -I"$C/inc/swref/published" -I"$N/generated" \
        -I"$C/nvswitch/kernel/inc" -I"$C/nvswitch/interface" -I"$C/nvswitch/common/inc" \
        -I"$C/inc/displayport" -I"$C/nvlink/interface" -I"$C/nvlink/inband/interface" \
        -I"$N/src/mm/uvm/interface" -I"$N/inc/libraries" -I"$N/src/libraries" \
        -I"$N/inc/kernel" \
        -D_LANGUAGE_C -D__NO_CTYPE -DNVRM -DLOCK_VAL_ENABLED=0 \
        -DPORT_ATOMIC_64_BIT_SUPPORTED=1 -DPORT_IS_KERNEL_BUILD=1 -DPORT_IS_CHECKED_BUILD=0 \
        -DPORT_MODULE_atomic=1 -DPORT_MODULE_core=1 -DPORT_MODULE_cpu=1 \
        -DPORT_MODULE_crypto=1 -DPORT_MODULE_debug=1 -DPORT_MODULE_memory=1 \
        -DPORT_MODULE_safe=1 -DPORT_MODULE_string=1 -DPORT_MODULE_sync=1 \
        -DPORT_MODULE_thread=1 -DPORT_MODULE_time=1 -DPORT_MODULE_util=1 \
        -DPORT_MODULE_example=0 -DPORT_MODULE_mmio=0 \
        -DRS_STANDALONE=0 -DRS_STANDALONE_TEST=0 -DRS_COMPATABILITY_MODE=1 \
        -DRS_PROVIDES_API_STATE=0 -DNV_CONTAINERS_NO_TEMPLATES \
        -DINCLUDE_NVLINK_LIB -DINCLUDE_NVSWITCH_LIB \
        -DNV_PRINTF_STRINGS_ALLOWED=1 -DNV_ASSERT_FAILED_USES_STRINGS=1 \
        -DPORT_ASSERT_FAILED_USES_STRINGS=1 \
        -DGSP_ABI="$gsp_abi"
}

tmp=$(mktemp -d "${TMPDIR:-/tmp}/gsp-abi-sizes.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

# build <compiler> <flagset> <tag>: compile and run, output in $tmp/<tag>.out
build() {
    local cc=$1 set=$2 tag=$3
    local -a flags
    [ "$set" = driver ] && cc=${GCC:-gcc}
    mapfile -t flags < <("${set}_flags")
    "$cc" -std=gnu11 -O0 "${flags[@]}" -o "$tmp/$tag" "$src"
    "$tmp/$tag" > "$tmp/$tag.out"
}

cc=${CC:-cc}
build "$cc" "$mode" main

if [ "$check" = 1 ]; then
    other=driver; [ "$mode" = driver ] && other=minimal
    build "$cc" "$other" other
    cmp -s "$tmp/main.out" "$tmp/other.out" ||
        { echo "check failed: minimal and driver flag sets disagree" >&2
          diff "$tmp/main.out" "$tmp/other.out" >&2 || true; exit 1; }
    used=$cc; [ "$mode" = driver ] && used=${GCC:-gcc}
    if "$used" --version | grep -qi clang; then alt=gcc; else alt=clang; fi
    if command -v "$alt" >/dev/null; then
        # always the minimal set: the driver set is GCC-only (see above)
        build "$alt" minimal alt
        cmp -s "$tmp/main.out" "$tmp/alt.out" ||
            { echo "check failed: $alt (minimal flags) disagrees" >&2
              diff "$tmp/main.out" "$tmp/alt.out" >&2 || true; exit 1; }
        echo "<!-- check: identical with $alt, minimal flag set -->"
    fi
    echo "<!-- check: identical with the $other flag set -->"
fi

[ "$mode" = driver ] && cc=${GCC:-gcc}
echo "<!-- compiler: $("$cc" --version | head -1); flags: $mode; tree: ${ogkm#"$root"/} -->"
cat "$tmp/main.out"
