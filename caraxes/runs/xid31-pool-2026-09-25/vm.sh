#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# tools/caraxes-vm.sh for one guest of this directory's runs in the matrix environment.
# Usage: vm.sh 610|615 ARGS...   (e.g. vm.sh 615 ssh 'dmesg | tail', vm.sh 615 down)
set -uo pipefail
HERE=$(dirname "$(readlink -f "$0")")
source "$HERE/../matrix-2026-09-25/env.sh"
mx_guest "${1:?usage: vm.sh 610|615 ARGS...}" || exit 2
shift
exec "$LEA_TOOLS/caraxes-vm.sh" "$@"
