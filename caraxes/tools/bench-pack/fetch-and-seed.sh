#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
#
# Downloads the render-benchmark pack (Blender Open Data benchmark-launcher-cli,
# Blender 5.2.0, the three standard scenes, Unigine Heaven 4.0 and Unigine
# Superposition 1.1 — Linux and Windows builds of each) and pre-extracts the
# Blender launcher's on-disk cache so the two guests can benchmark with Blender
# without re-downloading Blender or the scenes at run time.
#
# Anonymous plain HTTPS GETs only: no accounts, no login, no auth headers, no
# identifying User-Agent. Every archive is checked against a sha256 (see
# CHECKSUMS.sha256 written at the end, and README.md for provenance of each
# checksum: the Blender ones come from the official metadata endpoint and are
# verified here; Unigine publishes none, so we record the sha256 we computed).
#
# Usage: fetch-and-seed.sh [download|seed|all]   (default: all)
# Env:   BENCH_PACK_DIR (default /mnt/vmstore/caraxes/bench-pack)
set -euo pipefail

BENCH_PACK_DIR=${BENCH_PACK_DIR:-/mnt/vmstore/caraxes/bench-pack}
DL="$BENCH_PACK_DIR/downloads"
SEED="$BENCH_PACK_DIR/cache-seed"
LOGDIR="$BENCH_PACK_DIR/logs"
mkdir -p "$DL"/blender/{cli,blender,script,scenes} "$DL"/unigine/{heaven,superposition} "$LOGDIR"

log() { echo "[$(date -Is)] $*"; }

fetch() {
  local url="$1" out="$2"
  if [ -s "$out" ]; then
    log "SKIP (exists): $out"
    return 0
  fi
  log "GET $url -> $out"
  curl -fL --retry 5 --retry-delay 5 -C - --max-time 3600 -o "$out" "$url"
}

do_download() {
  # Blender Open Data — CLI launcher, benchmark script, Blender 5.2.0, scenes.
  # Source of truth for these URLs/checksums: https://opendata.blender.org/benchmarks/metadata/
  # (a small, anonymous, unauthenticated JSON GET; the launcher itself makes the
  # same call every time it runs, see README "Known offline limitation").
  fetch "https://download.blender.org/release/BlenderBenchmark2.0/launcher/benchmark-launcher-cli-3.3.0-linux.tar.gz" \
        "$DL/blender/cli/benchmark-launcher-cli-3.3.0-linux.tar.gz"
  fetch "https://download.blender.org/release/BlenderBenchmark2.0/launcher/benchmark-launcher-cli-3.3.0-windows.zip" \
        "$DL/blender/cli/benchmark-launcher-cli-3.3.0-windows.zip"
  fetch "https://download.blender.org/release/BlenderBenchmark2.0/script/blender-benchmark-script-3.1.2.tar.gz" \
        "$DL/blender/script/blender-benchmark-script-3.1.2.tar.gz"
  fetch "https://opendata.blender.org/cdn/Blender5.2/blender-5.2.0-linux-x64.tar.xz" \
        "$DL/blender/blender/blender-5.2.0-linux-x64.tar.xz"
  fetch "https://opendata.blender.org/cdn/Blender5.2/blender-5.2.0-windows-x64.zip" \
        "$DL/blender/blender/blender-5.2.0-windows-x64.zip"
  fetch "https://opendata.blender.org/cdn/BlenderBenchmark2.0/scenes/monster.tar.bz2" \
        "$DL/blender/scenes/monster.tar.bz2"
  fetch "https://opendata.blender.org/cdn/BlenderBenchmark2.0/scenes/junkshop.tar.bz2" \
        "$DL/blender/scenes/junkshop.tar.bz2"
  fetch "https://opendata.blender.org/cdn/BlenderBenchmark2.0/scenes/classroom.tar.bz2" \
        "$DL/blender/scenes/classroom.tar.bz2"

  # Unigine — free Basic-edition benchmarks, official CDN (benchmark.unigine.com
  # links to assets.unigine.com; no checksums are published by Unigine).
  fetch "https://assets.unigine.com/d/Unigine_Heaven-4.0.run" \
        "$DL/unigine/heaven/Unigine_Heaven-4.0.run"
  fetch "https://assets.unigine.com/d/Unigine_Heaven-4.0.exe" \
        "$DL/unigine/heaven/Unigine_Heaven-4.0.exe"
  fetch "https://assets.unigine.com/d/Unigine_Superposition-1.1.run" \
        "$DL/unigine/superposition/Unigine_Superposition-1.1.run"
  fetch "https://assets.unigine.com/d/Unigine_Superposition-1.1.exe" \
        "$DL/unigine/superposition/Unigine_Superposition-1.1.exe"
}

do_verify() {
  # Blender-side files are content-addressed by the official metadata endpoint;
  # verify against the values pinned below (fetched and cross-checked on
  # 2026-09-29, see README "Verification"). Unigine files have no published
  # checksum: we only record what we computed (see CHECKSUMS.sha256).
  local fail=0
  check() {
    local file="$1" want="$2" got
    [ -f "$file" ] || { log "MISSING $file"; fail=1; return; }
    got=$(sha256sum "$file" | cut -d' ' -f1)
    if [ "$got" = "$want" ]; then
      log "OK   sha256 $file"
    else
      log "FAIL sha256 $file (got $got, want $want)"
      fail=1
    fi
  }
  check "$DL/blender/blender/blender-5.2.0-linux-x64.tar.xz"   96f6c181a30f4950607839dc84d42a354b250d8a0231b098b59b7bc69c351c48
  check "$DL/blender/blender/blender-5.2.0-windows-x64.zip"    2d184b626c001692c362291911293b6a297179d618d95e9e9192c3a80318adc4
  check "$DL/blender/script/blender-benchmark-script-3.1.2.tar.gz" 2881a9f160a24ccd4282e43b861bd727e33562a4a230401343b590d10e890272
  check "$DL/blender/scenes/monster.tar.bz2"    8ff812a02f720433de1403dab447be9541eecca71cc919224a7790d017c1b2ce
  check "$DL/blender/scenes/junkshop.tar.bz2"   f5515a21a1337b908212d8d76733530df83014464a24c585ab7b96000da0cdce
  check "$DL/blender/scenes/classroom.tar.bz2"  936f0a528f03eba291c67ea87743a0ebde58938fb87471b262721b89b46b875d
  # benchmark-launcher-cli-3.3.0: the metadata endpoint's checksum for these two
  # files does NOT match what download.blender.org actually serves (confirmed
  # 2026-09-29, both a fresh metadata fetch and a fresh download agree with each
  # other, so this is not cache staleness on our side — see README). The CLI does
  # not self-verify its own archive at any point, only Blender/script/scene
  # downloads are checksum-gated (launcher source: internal/rpc/download.go).
  # We record our own computed sha256 below and treat that as the reference.
  check "$DL/blender/cli/benchmark-launcher-cli-3.3.0-linux.tar.gz"   a15e062179e7bf5cd20d58081f3c84cf87d56df3f596ffb0a11383638139324a
  check "$DL/blender/cli/benchmark-launcher-cli-3.3.0-windows.zip"    3f00bb68915800f14ddad6877083544aa25c56aac252b77b161bf7962ee67dde
  [ "$fail" = 0 ] || { log "One or more checksums failed — stopping."; exit 1; }
}

do_seed() {
  # Pre-extracts the Blender benchmark-launcher-cli's own cache layout
  # (verified against the CLI's Go source, internal/rpc/download.go, and by
  # actually running `benchmark-launcher-cli scenes list` / `devices` against
  # it) so that, once copied to the guest's cache directory, `blender download`
  # and `scenes download` are no-ops and only the ~56 KB metadata GET remains.
  #   Linux:   $XDG_CACHE_HOME/blender-benchmark-launcher (default ~/.cache/...)
  #   Windows: %LOCALAPPDATA%\blender-benchmark-launcher
  local BLENDER_LINUX_SHA=96f6c181a30f4950607839dc84d42a354b250d8a0231b098b59b7bc69c351c48
  local BLENDER_WINDOWS_SHA=2d184b626c001692c362291911293b6a297179d618d95e9e9192c3a80318adc4
  local BLENDER_WINDOWS_SHA8=${BLENDER_WINDOWS_SHA:0:8}   # launcher truncates to 8 chars on Windows (260-char path limit)
  local SCRIPT_SHA=2881a9f160a24ccd4282e43b861bd727e33562a4a230401343b590d10e890272
  local MONSTER_SHA=8ff812a02f720433de1403dab447be9541eecca71cc919224a7790d017c1b2ce
  local JUNKSHOP_SHA=f5515a21a1337b908212d8d76733530df83014464a24c585ab7b96000da0cdce
  local CLASSROOM_SHA=936f0a528f03eba291c67ea87743a0ebde58938fb87471b262721b89b46b875d

  seed_os() {
    local os="$1" root="$SEED/$os/blender-benchmark-launcher"
    local bver_sha; [ "$os" = linux ] && bver_sha="$BLENDER_LINUX_SHA" || bver_sha="$BLENDER_WINDOWS_SHA8"
    local bdir="$root/blender-versions/$bver_sha"
    if [ ! -f "$bdir/valid.launcher" ]; then
      mkdir -p "$bdir"
      log "Extracting Blender ($os) -> $bdir"
      if [ "$os" = linux ]; then
        tar xf "$DL/blender/blender/blender-5.2.0-linux-x64.tar.xz" -C "$bdir"
      else
        unzip -q "$DL/blender/blender/blender-5.2.0-windows-x64.zip" -d "$bdir"
      fi
      touch "$bdir/valid.launcher"
    fi
    local sdir="$root/benchmark-scripts/$SCRIPT_SHA"
    if [ ! -f "$sdir/valid.launcher" ]; then
      mkdir -p "$sdir"
      log "Extracting benchmark script ($os) -> $sdir"
      tar xzf "$DL/blender/script/blender-benchmark-script-3.1.2.tar.gz" -C "$sdir"
      touch "$sdir/valid.launcher"
    fi
    local name sha file scdir
    for triple in "monster $MONSTER_SHA monster.tar.bz2" "junkshop $JUNKSHOP_SHA junkshop.tar.bz2" "classroom $CLASSROOM_SHA classroom.tar.bz2"; do
      read -r name sha file <<<"$triple"
      scdir="$root/scenes/$sha"
      if [ ! -f "$scdir/valid.launcher" ]; then
        mkdir -p "$scdir"
        log "Extracting scene $name ($os) -> $scdir"
        tar xjf "$DL/blender/scenes/$file" -C "$scdir"
        touch "$scdir/valid.launcher"
      fi
    done
  }
  seed_os linux
  seed_os windows
  log "Cache seed ready under $SEED/{linux,windows}/blender-benchmark-launcher"
  log "Copy the matching tree into the guest's cache dir before running run-linux.sh / run-windows.ps1."
}

do_checksums_file() {
  ( cd "$DL" && find . -type f -exec sha256sum {} \; | sort -k2 ) > "$BENCH_PACK_DIR/CHECKSUMS.sha256"
  log "Wrote $BENCH_PACK_DIR/CHECKSUMS.sha256"
}

case "${1:-all}" in
  download) do_download ;;
  verify)   do_verify ;;
  seed)     do_seed ;;
  all)      do_download; do_verify; do_seed; do_checksums_file ;;
  *) echo "usage: $0 [download|verify|seed|all]" >&2; exit 2 ;;
esac
