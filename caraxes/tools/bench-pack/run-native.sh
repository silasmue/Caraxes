#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
#
# Runs the same render-benchmark pack directly on the HOST, as the native
# reference point for the Linux/Windows guest numbers. It is the same
# Blender/Heaven/Superposition downloads, driven the same way as run-linux.sh,
# just without SSH — everything happens in the caller's own desktop session.
#
# IMPORTANT: this script is a normal GPU client (Vulkan/OpenGL/CUDA/OptiX
# userspace calls only) and never loads/unloads a kernel module, never writes
# an NVIDIA register and never touches driver configuration. It still
# allocates GPU memory and drives the display for real, so per the project's
# "host GPU state is off limits" rule: run this only with the user's explicit
# go-ahead for that session, and report that it ran (what, and for how long).
# This script is not invoked by anything else in this pack automatically.
#
# Optional env (defaults shown):
#   BENCH_PACK_DIR         /mnt/vmstore/caraxes/bench-pack
#   NATIVE_STAGE_DIR        ~/bench-pack-native          local extraction dir
#   OUT_DIR                 ./bench-pack-out/native
#   BLENDER_SCENES          "monster junkshop classroom"
#   BLENDER_FULL_DEVICES    "CUDA OPTIX"
#   BLENDER_SANITY_DEVICE   "CPU"
#   BLENDER_SANITY_SCENE    "monster"
#   HEAVEN_PRESETS          "basic extreme"
#   SUPERPOSITION_PRESETS   "1080p_medium 1080p_high"
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck source=lib-common.sh
source "$SCRIPT_DIR/lib-common.sh"

BENCH_PACK_DIR=${BENCH_PACK_DIR:-/mnt/vmstore/caraxes/bench-pack}
NATIVE_STAGE_DIR=${NATIVE_STAGE_DIR:-"$HOME/bench-pack-native"}
OUT_DIR=${OUT_DIR:-./bench-pack-out/native}
BLENDER_SCENES=${BLENDER_SCENES:-"monster junkshop classroom"}
BLENDER_FULL_DEVICES=${BLENDER_FULL_DEVICES:-"CUDA OPTIX"}
BLENDER_SANITY_DEVICE=${BLENDER_SANITY_DEVICE:-"CPU"}
BLENDER_SANITY_SCENE=${BLENDER_SANITY_SCENE:-"monster"}
HEAVEN_PRESETS=${HEAVEN_PRESETS:-"basic extreme"}
SUPERPOSITION_PRESETS=${SUPERPOSITION_PRESETS:-"1080p_medium 1080p_high"}
BENCH_HOST_LABEL="native-host-$(hostname)"

mkdir -p "$OUT_DIR/logs" "$NATIVE_STAGE_DIR"

stage() {
  echo "== staging payload into $NATIVE_STAGE_DIR =="
  cd "$NATIVE_STAGE_DIR"
  [ -x benchmark-launcher-cli ] || tar xzf "$BENCH_PACK_DIR/downloads/blender/cli/benchmark-launcher-cli-3.3.0-linux.tar.gz"
  [ -d Unigine_Heaven-4.0 ] || { cp "$BENCH_PACK_DIR/downloads/unigine/heaven/Unigine_Heaven-4.0.run" . && chmod +x Unigine_Heaven-4.0.run && ./Unigine_Heaven-4.0.run --nox11; }
  [ -d Unigine_Superposition-1.1 ] || { cp "$BENCH_PACK_DIR/downloads/unigine/superposition/Unigine_Superposition-1.1.run" . && chmod +x Unigine_Superposition-1.1.run && ./Unigine_Superposition-1.1.run --nox11; }
  mkdir -p "$HOME/.cache"
  rsync -a "$BENCH_PACK_DIR/cache-seed/linux/" "$HOME/.cache/" 2>/dev/null || \
    cp -rn "$BENCH_PACK_DIR/cache-seed/linux/." "$HOME/.cache/"
}

run_blender() {
  local device="$1" scene="$2"
  local json="$OUT_DIR/logs/blender-${device}-${scene}.json"
  local log="$OUT_DIR/logs/blender-${device}-${scene}.log"
  echo "== Blender $device / $scene (native) =="
  local t0 t1
  t0=$(date +%s)
  ( cd "$NATIVE_STAGE_DIR" && ./benchmark-launcher-cli benchmark "$scene" \
      --blender-version 5.2.0 --device-type "$device" --json -v 2 \
      >"$json" 2>"$log" ) || true
  t1=$(date +%s)
  read -r spm rtime <<<"$(parse_blender_result "$json" "$scene" 2>/dev/null || echo 'NA NA')"
  result blender "$scene" "$device" "$spm" NA NA "$((t1 - t0))" "$log"
}

quality_flags_heaven() {
  case "$1" in
    basic)   echo "-extern_define PHORONIX,RELEASE,QUALITY_HIGH,TESSELLATION_NORMAL" ;;
    extreme) echo "-extern_define PHORONIX,RELEASE,QUALITY_ULTRA,TESSELLATION_EXTREME" ;;
    *) echo "-extern_define PHORONIX,RELEASE" ;;
  esac
}

run_heaven() {
  local preset="$1"
  local log="$OUT_DIR/logs/heaven-${preset}.log"
  echo "== Heaven $preset (native) =="
  local t0 t1
  t0=$(date +%s)
  ( cd "$NATIVE_STAGE_DIR/Unigine_Heaven-4.0" && \
    LD_LIBRARY_PATH=bin/x64:${LD_LIBRARY_PATH:-} ./bin/heaven_x64 \
    -data_path ../ -sound_app null -engine_config ../data/heaven_4.0.cfg \
    -system_script heaven/unigine.cpp -video_mode -1 -video_app opengl \
    -video_fullscreen 0 -video_width 1920 -video_height 1080 \
    $(quality_flags_heaven "$preset") ) >"$log" 2>&1 || true
  t1=$(date +%s)
  read -r score fps minfps maxfps <<<"$(parse_heaven_log "$log" 2>/dev/null || echo 'NA NA NA NA')"
  result heaven "$preset" OpenGL "$score" "$fps" "$minfps" "$((t1 - t0))" "$log"
}

quality_flags_superposition() {
  case "$1" in
    1080p_medium) echo "-video_width 1920 -video_height 1080 -shaders_quality 1 -textures_quality 1" ;;
    1080p_high)   echo "-video_width 1920 -video_height 1080 -shaders_quality 2 -textures_quality 2" ;;
    *) echo "-video_width 1920 -video_height 1080 -shaders_quality 1 -textures_quality 1" ;;
  esac
}

run_superposition() {
  local preset="$1"
  local log="$OUT_DIR/logs/superposition-${preset}.log"
  echo "== Superposition $preset (native) =="
  rm -rf "$HOME/.Superposition/automation"/* 2>/dev/null || true
  local t0 t1
  t0=$(date +%s)
  ( cd "$NATIVE_STAGE_DIR/Unigine_Superposition-1.1" && ./bin/superposition \
    -sound_app openal -system_script superposition/system_script.cpp \
    -data_path ../ -engine_config ../data/superposition/unigine.cfg \
    -video_mode -1 -project_name Superposition -video_resizable 1 \
    -console_command 'config_readonly 1 && world_load superposition/superposition' \
    -mode 2 -preset 0 $(quality_flags_superposition "$preset") ) >/dev/null 2>&1 || true
  cat "$HOME/.Superposition/automation"/log*.txt >"$log" 2>/dev/null || true
  t1=$(date +%s)
  read -r score fps minfps maxfps <<<"$(parse_superposition_log "$log" 2>/dev/null || echo 'NA NA NA NA')"
  result superposition "$preset" OpenGL "$score" "$fps" "$minfps" "$((t1 - t0))" "$log"
}

main() {
  echo "!! run-native.sh drives the HOST GPU for real (Blender/Heaven/Superposition windows will open)."
  echo "!! Only continue with the user's explicit go-ahead for this session."
  stage
  for scene in $BLENDER_SCENES; do
    for device in $BLENDER_FULL_DEVICES; do run_blender "$device" "$scene"; done
  done
  run_blender "$BLENDER_SANITY_DEVICE" "$BLENDER_SANITY_SCENE"
  for preset in $HEAVEN_PRESETS; do run_heaven "$preset"; done
  for preset in $SUPERPOSITION_PRESETS; do run_superposition "$preset"; done
  echo "== done: $OUT_DIR/results.tsv =="
}
main "$@"
