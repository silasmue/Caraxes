#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
#
# Check whether the standalone RM query tools (host-facts.rs, fifo-host-query.rs,
# lab-rm-query.rs), which carry hard-coded 610.57.04 layouts, read the right bytes
# on another driver version. Compiles lab-abi-check.c against that version's SDK
# headers and compares the fingerprint with the recorded 610.57.04 one
# (lab-abi-610.57.04.txt next to this script).
#
#   caraxes/tools/lab-abi-check.sh [--print] TARGET...
#
#   TARGET   a driver version (looked up under target/abi-headers/<v>/ and
#            vendor/nvidia-rm-headers/<v>/), or a directory: an
#            open-gpu-kernel-modules tree or its src/common/sdk/nvidia/inc
#   --print  print the fingerprint instead of comparing it
#   With no TARGET, the tree named by $CARAXES_OGKM is checked when it is set.
#
# Exit status: 0 all targets identical, 1 at least one differs, 2 usage or
# header error. Only host files are read; nothing here touches a GPU.
set -uo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)
src="$here/lab-abi-check.c"
ref="$here/lab-abi-610.57.04.txt"
cc=${CC:-cc}

print=0
targets=()
for a in "$@"; do
    case $a in
        --print) print=1 ;;
        -h|--help) sed -n '4,19p' "$0"; exit 0 ;;
        *) targets+=("$a") ;;
    esac
done
[ ${#targets[@]} -gt 0 ] || [ -z "${CARAXES_OGKM:-}" ] || targets=("$CARAXES_OGKM")
[ ${#targets[@]} -gt 0 ] || { sed -n '4,19p' "$0" >&2; exit 2; }
command -v "$cc" >/dev/null || { echo "no C compiler ($cc)" >&2; exit 2; }

# resolve TARGET to an SDK include directory
sdk_dir() {
    local t=$1 d
    for d in "$t/src/common/sdk/nvidia/inc" "$t" \
             "$root/target/abi-headers/$t/src/common/sdk/nvidia/inc" \
             "$root/vendor/nvidia-rm-headers/$t/src/common/sdk/nvidia/inc"; do
        [ -f "$d/nvos.h" ] && [ -f "$d/ctrl/ctrl2080/ctrl2080ce.h" ] && { echo "$d"; return 0; }
    done
    return 1
}

tmp=$(mktemp -d "${TMPDIR:-/tmp}/lab-abi-check.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

# fingerprint INC: compile; a line that fails becomes a "missing" line, retried
# until the program compiles (at most 40 rounds).
fingerprint() {
    local inc=$1 work="$tmp/fp.c" round lines
    cp "$src" "$work"
    for round in $(seq 1 40); do
        if "$cc" -std=gnu11 -w -DNV_LINUX -D__linux__ -I"$inc" -o "$tmp/fp" "$work" 2>"$tmp/err"; then
            "$tmp/fp"
            return 0
        fi
        # the snippet lines "  NN |     S(...)" name the failing item, also when
        # the error itself is reported inside the macro definition
        lines=$(grep -E '^ *[0-9]+ \|     [SOC]\(' "$tmp/err" | sed -E 's/^ *([0-9]+) .*/\1/' | sort -un)
        [ -n "$lines" ] || { cat "$tmp/err" >&2; return 1; }
        local l item
        for l in $lines; do
            item=$(sed -n "${l}p" "$work" | sed -E 's/^ *[SOC]\((.*)\);.*$/\1/; s/ //g; s/"/_/g')
            case $(sed -n "${l}p" "$work") in
                *'    S('*|*'    O('*|*'    C('*)
                    sed -i "${l}s/.*/    printf(\"missing %-58s -\\\\n\", \"$item\");/" "$work" ;;
                *) cat "$tmp/err" >&2; return 1 ;;
            esac
        done
    done
    return 1
}

rc=0
for t in "${targets[@]}"; do
    inc=$(sdk_dir "$t") || { echo "== $t: no complete SDK headers found (need nvos.h and ctrl/ctrl2080/ctrl2080ce.h)"; rc=2; continue; }
    if ! fingerprint "$inc" >"$tmp/out.txt"; then
        echo "== $t: fingerprint did not compile (see errors above)"; rc=2; continue
    fi
    if [ $print = 1 ]; then
        cat "$tmp/out.txt"; continue
    fi
    if diff <(grep -v '^#' "$ref") "$tmp/out.txt" >"$tmp/diff.txt"; then
        echo "== $t: identical to 610.57.04 ($(wc -l <"$tmp/out.txt") items): the tools' layouts hold"
    else
        echo "== $t: DIFFERS from 610.57.04 ($(grep -c '^>' "$tmp/diff.txt") items): the tools' output is unverified for this version"
        # printf, not sed: a directory TARGET contains slashes
        while IFS= read -r l; do
            case $l in
                '< '*) printf '   610.57.04: %s\n' "${l#< }" ;;
                '> '*) printf '   %s: %s\n' "$t" "${l#> }" ;;
            esac
        done <"$tmp/diff.txt"
        [ $rc = 0 ] && rc=1
    fi
done
exit $rc
