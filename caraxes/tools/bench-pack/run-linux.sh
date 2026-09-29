#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
#
# Runs the render-benchmark pack against the Linux guest (Ubuntu 24.04, GNOME,
# NVIDIA 615) over SSH from the host. Drives Blender (CPU/CUDA/OptiX x
# monster/junkshop/classroom), Unigine Heaven 4.0 and Unigine Superposition
# 1.1, all from their free/Basic downloads — no account, no GUI clicking.
# Writes one RESULT line per test to stdout and to $OUT_DIR/results.tsv, and
# keeps every raw log under $OUT_DIR/logs/.
#
# This script only touches the GUEST over SSH; it never starts a VM and never
# touches host GPU/driver state itself (README "What this script needs").
#
# Required env:
#   LINUX_GUEST_HOST     ssh destination, e.g. caraxes@192.168.100.20
# Optional env (defaults shown):
#   SSH_OPTS              ""                          extra ssh/scp/rsync flags
#   BENCH_PACK_DIR         /mnt/vmstore/caraxes/bench-pack   host-side payload (see fetch-and-seed.sh)
#   GUEST_BENCH_DIR        ~/bench-pack                guest-side staging directory
#   OUT_DIR                ./bench-pack-out/linux      where results/logs land on the HOST
#   BLENDER_SCENES         "monster junkshop classroom"
#   BLENDER_FULL_DEVICES   "CUDA OPTIX"                run every scene on these
#   BLENDER_SANITY_DEVICE  "CPU"                       run ONE scene only, as a sanity check
#   BLENDER_SANITY_SCENE   "monster"
#   HEAVEN_PRESETS         "basic extreme"             see quality_flags_heaven()
#   SUPERPOSITION_PRESETS  "1080p_medium 1080p_high"
#   LINUX_DISPLAY          auto-discovered via loginctl if unset
#   LINUX_XAUTHORITY       auto-discovered if unset
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck source=lib-common.sh
source "$SCRIPT_DIR/lib-common.sh"

: "${LINUX_GUEST_HOST:?set LINUX_GUEST_HOST, e.g. caraxes@192.168.100.20}"
SSH_OPTS=${SSH_OPTS:-}
BENCH_PACK_DIR=${BENCH_PACK_DIR:-/mnt/vmstore/caraxes/bench-pack}
GUEST_BENCH_DIR=${GUEST_BENCH_DIR:-'$HOME/bench-pack'}
OUT_DIR=${OUT_DIR:-./bench-pack-out/linux}
BLENDER_SCENES=${BLENDER_SCENES:-"monster junkshop classroom"}
BLENDER_FULL_DEVICES=${BLENDER_FULL_DEVICES:-"CUDA OPTIX"}
BLENDER_SANITY_DEVICE=${BLENDER_SANITY_DEVICE:-"CPU"}
BLENDER_SANITY_SCENE=${BLENDER_SANITY_SCENE:-"monster"}
HEAVEN_PRESETS=${HEAVEN_PRESETS:-"basic extreme"}
SUPERPOSITION_PRESETS=${SUPERPOSITION_PRESETS:-"1080p_medium 1080p_high"}
BENCH_HOST_LABEL="linux-guest"

mkdir -p "$OUT_DIR/logs"
ssh_c() { ssh $SSH_OPTS "$LINUX_GUEST_HOST" "$@"; }
scp_back() { scp $SSH_OPTS "$LINUX_GUEST_HOST:$1" "$2"; }

# ---------------------------------------------------------------------------
# Stage: make sure the guest has the CLI/Blender/scene cache and the two
# Unigine .run installers. Cheap to re-run (rsync, skips what's already there).
# ---------------------------------------------------------------------------
stage() {
  echo "== staging payload to $LINUX_GUEST_HOST:$GUEST_BENCH_DIR =="
  ssh_c "mkdir -p $GUEST_BENCH_DIR"
  rsync -az $SSH_OPTS \
    "$BENCH_PACK_DIR/downloads/blender/cli/benchmark-launcher-cli-3.3.0-linux.tar.gz" \
    "$BENCH_PACK_DIR/downloads/unigine/heaven/Unigine_Heaven-4.0.run" \
    "$BENCH_PACK_DIR/downloads/unigine/superposition/Unigine_Superposition-1.1.run" \
    "$LINUX_GUEST_HOST:$GUEST_BENCH_DIR/"
  rsync -az $SSH_OPTS "$BENCH_PACK_DIR/cache-seed/linux/" \
    "$LINUX_GUEST_HOST:\$HOME/.cache/"
  ssh_c "cd $GUEST_BENCH_DIR && [ -x benchmark-launcher-cli ] || tar xzf benchmark-launcher-cli-3.3.0-linux.tar.gz"
  ssh_c "cd $GUEST_BENCH_DIR && [ -d Unigine_Heaven-4.0 ] || (chmod +x Unigine_Heaven-4.0.run && ./Unigine_Heaven-4.0.run --nox11)"
  ssh_c "cd $GUEST_BENCH_DIR && [ -d Unigine_Superposition-1.1 ] || (chmod +x Unigine_Superposition-1.1.run && ./Unigine_Superposition-1.1.run --nox11)"
}

# ---------------------------------------------------------------------------
# GNOME session discovery: Heaven/Superposition are X11 (OpenGL) apps and run
# fine under XWayland, but only if DISPLAY/XAUTHORITY point at the already
# logged-in GNOME session (there is nothing to render into otherwise).
# ---------------------------------------------------------------------------
discover_display() {
  if [ -n "${LINUX_DISPLAY:-}" ]; then return; fi
  echo "== discovering the GNOME session's DISPLAY on the guest =="
  local sess
  sess=$(ssh_c "loginctl list-sessions --no-legend 2>/dev/null | awk '\$3==\"seat0\"||\$1{print \$1; exit}'" || true)
  if [ -n "$sess" ]; then
    LINUX_DISPLAY=$(ssh_c "loginctl show-session $sess -p Display --value 2>/dev/null" || true)
    local uid
    uid=$(ssh_c "loginctl show-session $sess -p User --value 2>/dev/null" || true)
    [ -n "${LINUX_XAUTHORITY:-}" ] || LINUX_XAUTHORITY=$(ssh_c "find /run/user/$uid -maxdepth 2 -iname '*Xauthority*' 2>/dev/null | head -1" || true)
  fi
  LINUX_DISPLAY=${LINUX_DISPLAY:-:0}
  echo "   DISPLAY=$LINUX_DISPLAY XAUTHORITY=${LINUX_XAUTHORITY:-<unset>}"
}

guest_env_prefix() {
  printf 'export DISPLAY=%q;' "$LINUX_DISPLAY"
  [ -n "${LINUX_XAUTHORITY:-}" ] && printf 'export XAUTHORITY=%q;' "$LINUX_XAUTHORITY"
  return 0
}

# ---------------------------------------------------------------------------
# Blender (benchmark-launcher-cli, official non-interactive CLI, no account).
# One small anonymous metadata GET happens per invocation even with a warm
# cache (see README "Known offline limitation") — everything else is local.
# ---------------------------------------------------------------------------
run_blender() {
  local device="$1" scene="$2"
  local remote_json="$GUEST_BENCH_DIR/out-blender-${device}-${scene}.json"
  local remote_log="$GUEST_BENCH_DIR/out-blender-${device}-${scene}.log"
  local local_log="$OUT_DIR/logs/blender-${device}-${scene}.log"
  local local_json="$OUT_DIR/logs/blender-${device}-${scene}.json"
  echo "== Blender $device / $scene =="
  local t0 t1
  t0=$(date +%s)
  ssh_c "cd $GUEST_BENCH_DIR && ./benchmark-launcher-cli benchmark $scene --blender-version 5.2.0 --device-type $device --json -v 2 >'$remote_json' 2>'$remote_log'" || true
  t1=$(date +%s)
  scp_back "$remote_json" "$local_json" || true
  scp_back "$remote_log" "$local_log" || true
  read -r spm rtime <<<"$(parse_blender_result "$local_json" "$scene" 2>/dev/null || echo 'NA NA')"
  result blender "$scene" "$device" "$spm" NA NA "$((t1 - t0))" "$local_log"
}

# ---------------------------------------------------------------------------
# Unigine Heaven 4.0 — the free Basic edition has no licensed "command line
# automation" (that is an Advanced/Pro feature), but the raw engine binary
# does accept standard engine startup flags. This invocation is the one
# Phoronix Test Suite's pts/unigine-heaven profile has used for years against
# the same free .run download (verified against its install.sh/downloads.xml,
# both fetched 2026-09-29). "-extern_define" quality/tessellation names below
# come from the Phoronix forum thread on tuning Heaven 4.0, not from Unigine's
# own (GUI-only) manual, since the manual never documents a headless mode.
# ---------------------------------------------------------------------------
quality_flags_heaven() {
  case "$1" in
    basic)   echo "-extern_define PHORONIX,RELEASE,QUALITY_HIGH,TESSELLATION_NORMAL" ;;
    extreme) echo "-extern_define PHORONIX,RELEASE,QUALITY_ULTRA,TESSELLATION_EXTREME" ;;
    *) echo "-extern_define PHORONIX,RELEASE" ;;
  esac
}

run_heaven() {
  local preset="$1"
  local remote_log="$GUEST_BENCH_DIR/out-heaven-${preset}.log"
  local local_log="$OUT_DIR/logs/heaven-${preset}.log"
  echo "== Heaven $preset =="
  local t0 t1
  t0=$(date +%s)
  ssh_c "$(guest_env_prefix) cd $GUEST_BENCH_DIR/Unigine_Heaven-4.0 && \
    LD_LIBRARY_PATH=bin/x64:\$LD_LIBRARY_PATH ./bin/heaven_x64 \
    -data_path ../ -sound_app null -engine_config ../data/heaven_4.0.cfg \
    -system_script heaven/unigine.cpp -video_mode -1 -video_app opengl \
    -video_fullscreen 0 -video_width 1920 -video_height 1080 \
    $(quality_flags_heaven "$preset") > '$remote_log' 2>&1" || true
  t1=$(date +%s)
  scp_back "$remote_log" "$local_log" || true
  read -r score fps minfps maxfps <<<"$(parse_heaven_log "$local_log" 2>/dev/null || echo 'NA NA NA NA')"
  result heaven "$preset" OpenGL "$score" "$fps" "$minfps" "$((t1 - t0))" "$local_log"
}

# ---------------------------------------------------------------------------
# Unigine Superposition 1.1 — same story as Heaven: free Basic edition, no
# licensed CLI automation, but the raw engine binary honours the same
# engine-level flags. Invocation adapted from pts/unigine-super's install.sh
# (fetched 2026-09-29); quality/resolution flags per its documented presets
# for "1080p Medium" / "1080p High". Results land in
# ~/.Superposition/automation/log*.txt, which Superposition itself writes
# when run with -mode 2 (benchmark) -preset 0 (custom, driven by our flags).
# ---------------------------------------------------------------------------
quality_flags_superposition() {
  case "$1" in
    1080p_medium) echo "-video_width 1920 -video_height 1080 -shaders_quality 1 -textures_quality 1" ;;
    1080p_high)   echo "-video_width 1920 -video_height 1080 -shaders_quality 2 -textures_quality 2" ;;
    *) echo "-video_width 1920 -video_height 1080 -shaders_quality 1 -textures_quality 1" ;;
  esac
}

run_superposition() {
  local preset="$1"
  local remote_log="$GUEST_BENCH_DIR/out-superposition-${preset}.log"
  local local_log="$OUT_DIR/logs/superposition-${preset}.log"
  echo "== Superposition $preset =="
  local t0 t1
  t0=$(date +%s)
  ssh_c "$(guest_env_prefix) rm -rf \$HOME/.Superposition/automation/*; \
    cd $GUEST_BENCH_DIR/Unigine_Superposition-1.1 && ./bin/superposition \
    -sound_app openal -system_script superposition/system_script.cpp \
    -data_path ../ -engine_config ../data/superposition/unigine.cfg \
    -video_mode -1 -project_name Superposition -video_resizable 1 \
    -console_command 'config_readonly 1 && world_load superposition/superposition' \
    -mode 2 -preset 0 $(quality_flags_superposition "$preset"); \
    cat \$HOME/.Superposition/automation/log*.txt > '$remote_log' 2>&1" || true
  t1=$(date +%s)
  scp_back "$remote_log" "$local_log" || true
  read -r score fps minfps maxfps <<<"$(parse_superposition_log "$local_log" 2>/dev/null || echo 'NA NA NA NA')"
  result superposition "$preset" OpenGL "$score" "$fps" "$minfps" "$((t1 - t0))" "$local_log"
}

main() {
  stage
  discover_display

  for scene in $BLENDER_SCENES; do
    for device in $BLENDER_FULL_DEVICES; do
      run_blender "$device" "$scene"
    done
  done
  run_blender "$BLENDER_SANITY_DEVICE" "$BLENDER_SANITY_SCENE"

  for preset in $HEAVEN_PRESETS; do run_heaven "$preset"; done
  for preset in $SUPERPOSITION_PRESETS; do run_superposition "$preset"; done

  echo "== done: $OUT_DIR/results.tsv =="
}
main "$@"
