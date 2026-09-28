# shellcheck shell=bash
# SPDX-License-Identifier: MIT
# The guest driver a Caraxes run targets, in one place (sourced by tools/caraxes-vm.sh and
# tools/caraxes-gate.sh, never run). The default is the repository's DRIVER_VERSION;
# CARAXES_DRIVER=610|615 overrides it for one run. caraxes_driver_select sets:
#   CARAXES_DRIVER          the major (610 or 615); CARAXES_DRIVER_EXPLICIT=1 when the caller set it
#   CARAXES_DRIVER_VERSION  the full version (610.57.04 or 615.71.09): what the gates expect
#                           the guest's nvidia-smi to report and what sync-tests writes into
#                           the guest's leandro-test/DRIVER_VERSION
#   CARAXES_GUEST_LIBDIR    the guest directory with libcuda.so.1 and libnvidia-ml.so.1
#   CARAXES_GUEST_SMI       the guest's nvidia-smi of that version
#   CARAXES_GUEST_ABI       not set here: the device reads the guest's ABI at its GSP boot
#                           (docs/research/one-abi-binary-2026-09-28.md). A caller's value
#                           pins the device; with CARAXES_DRIVER=615 it must be 615 or unset
# CARAXES_GUEST_NVRM=<dir> still overrides the guest userspace as <dir>/lib and
# <dir>/bin/nvidia-smi. caraxes_driver_image <vm dir> prints the base image of the driver.
# Guest layouts: docs/caraxes/guest-driver-versions.md section 8.

# Returns non-zero with a message on stderr for anything it does not know (fail closed).
caraxes_driver_select() {
    local root=${LEA_ROOT:?LEA_ROOT unset} file
    CARAXES_DRIVER_EXPLICIT=${CARAXES_DRIVER:+1}
    if [[ -n ${CARAXES_DRIVER:-} ]]; then
        case $CARAXES_DRIVER in
            610) CARAXES_DRIVER_VERSION=610.57.04 ;;
            615) CARAXES_DRIVER_VERSION=615.71.09 ;;
            *) printf 'ERROR: CARAXES_DRIVER=%s: must be 610 (610.57.04) or 615 (615.71.09)\n' "$CARAXES_DRIVER" >&2
               return 1 ;;
        esac
    else
        file=$(tr -d '[:space:]' < "$root/DRIVER_VERSION") || return 1
        case $file in
            610.57.04) CARAXES_DRIVER=610 ;;
            615.71.09) CARAXES_DRIVER=615 ;;
            *) printf 'ERROR: DRIVER_VERSION is %s; tools/lib/driver.sh knows 610.57.04 and 615.71.09\n' "$file" >&2
               return 1 ;;
        esac
        CARAXES_DRIVER_VERSION=$file
    fi
    case $CARAXES_DRIVER in
        610)
            # The baked image's own userspace (/opt/nvrm, docs/caraxes/testing.md section 2).
            CARAXES_GUEST_LIBDIR=/opt/nvrm/lib
            CARAXES_GUEST_SMI=/opt/nvrm/bin/nvidia-smi ;;
        615)
            # NVIDIA's ubuntu2404 packages install into the system paths; the image still
            # carries the 610 userspace in /opt/nvrm, outside the linker's path.
            CARAXES_GUEST_LIBDIR=/usr/lib/x86_64-linux-gnu
            CARAXES_GUEST_SMI=/usr/bin/nvidia-smi
            if [[ -n ${CARAXES_GUEST_ABI:-} && $CARAXES_GUEST_ABI != 615 ]]; then
                printf 'ERROR: CARAXES_DRIVER=615 with CARAXES_GUEST_ABI=%s; unset it or set 615\n' "$CARAXES_GUEST_ABI" >&2
                return 1
            fi ;;
    esac
    if [[ -n ${CARAXES_GUEST_NVRM:-} ]]; then
        CARAXES_GUEST_LIBDIR=$CARAXES_GUEST_NVRM/lib
        CARAXES_GUEST_SMI=$CARAXES_GUEST_NVRM/bin/nvidia-smi
    fi
}

# caraxes_driver_image <vm dir>: the base image of the selected driver. The 610 image lives
# in the v1 VM directory ($LEA_VM_DIR); the 615 image is Caraxes' own (CARAXES_IMAGES,
# default /mnt/vmstore/caraxes/images).
caraxes_driver_image() {
    case $CARAXES_DRIVER in
        610) printf '%s\n' "$1/guest-baked-20260801-610.57.04-20260820-105733.qcow2" ;;
        615) printf '%s\n' "${CARAXES_IMAGES:-/mnt/vmstore/caraxes/images}/guest-baked-2026-09-24-615.71.09.qcow2" ;;
    esac
}
