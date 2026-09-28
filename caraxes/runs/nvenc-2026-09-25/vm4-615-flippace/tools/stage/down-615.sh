#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The SotTR stage on the 615 guest (vm4-615, env-615.sh): down.sh with SOTTR_DRIVER=615.
SOTTR_DRIVER=615 exec "$(dirname "$0")/down.sh" "$@"
