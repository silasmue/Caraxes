#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The gate's rendering check (2026-09-30): a guest screenshot of a game against a reference frame
# of a known-good run, by colour statistics, not pixels, because the camera moves. A frame the
# GPU rendered wrongly (the "neon" SotTR frames on Linux: surfaces read in the wrong format) is
# saturated and bright all over. The evening's lfpool SotTR frame has 4.7 % of its pixels above
# 0.7 HSL saturation and a mean grey of 0.17; the broken frame has 74.5 % and 0.72.
# Usage: frame-check.sh SHOT.png [REF.png]   (REF default /mnt/vmstore/caraxes/ref/sottr-ref-E-171514.png)
# Prints the statistics of both and PASS or FAIL. The exit status is 0 on PASS.
set -uo pipefail
shot=${1:?shot} ref=${2:-/mnt/vmstore/caraxes/ref/sottr-ref-E-171514.png}
stats() {
    local f=$1
    local sat hi grey
    sat=$(magick "$f" -colorspace HSL -channel G -separate -format "%[fx:mean]" info:) || return 1
    hi=$(magick "$f" -colorspace HSL -channel G -separate -threshold 70% -format "%[fx:mean]" info:) || return 1
    grey=$(magick "$f" -colorspace gray -format "%[fx:mean]" info:) || return 1
    echo "$sat $hi $grey"
}
read -r s_sat s_hi s_grey < <(stats "$shot") || { echo "FAIL: $shot unreadable"; exit 1; }
read -r r_sat r_hi r_grey < <(stats "$ref") || { echo "FAIL: reference $ref unreadable"; exit 1; }
printf 'shot %s: saturation mean %.3f, above 0.7 %.3f, grey mean %.3f\n' "$shot" "$s_sat" "$s_hi" "$s_grey"
printf 'ref  %s: saturation mean %.3f, above 0.7 %.3f, grey mean %.3f\n' "$ref" "$r_sat" "$r_hi" "$r_grey"
# Pass within generous bounds of the reference: at most 4x its share of highly saturated pixels
# (and at most 25 %), a grey mean at most 0.25 above it, a shot that is not black.
awk -v sh="$s_hi" -v rh="$r_hi" -v sg="$s_grey" -v rg="$r_grey" 'BEGIN {
    ok = (sh <= 4 * rh || sh <= 0.08) && sh <= 0.25 && sg <= rg + 0.25 && sg >= 0.02
    print (ok ? "PASS" : "FAIL") ": rendering statistics " (ok ? "within" : "outside") " the reference bounds"
    exit ok ? 0 : 1 }'
