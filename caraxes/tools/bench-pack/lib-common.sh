# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
#
# Shared helpers for run-linux.sh and run-native.sh: the machine-readable
# result line and small parsers for Unigine's log output. Sourced, not run.

# result <benchmark> <preset> <device_api> <score> <fps_avg> <fps_min> <duration_s> <logfile>
# Appends one tab-separated line to $OUT_DIR/results.tsv and echoes it. Any
# missing numeric field is written as "NA" rather than left empty, so the file
# stays awk/cut-friendly.
result() {
  local benchmark="$1" preset="$2" device_api="$3" score="${4:-NA}" fps_avg="${5:-NA}" \
        fps_min="${6:-NA}" duration_s="${7:-NA}" logfile="${8:-NA}"
  local ts host line
  ts=$(date -Is)
  host=${BENCH_HOST_LABEL:-$(hostname)}
  line=$(printf 'RESULT\tts=%s\thost=%s\tbenchmark=%s\tpreset=%s\tdevice_api=%s\tscore=%s\tfps_avg=%s\tfps_min=%s\tduration_s=%s\tlog=%s' \
    "$ts" "$host" "$benchmark" "$preset" "$device_api" "$score" "$fps_avg" "$fps_min" "$duration_s" "$logfile")
  echo "$line"
  mkdir -p "$(dirname "$OUT_DIR/results.tsv")"
  [ -f "$OUT_DIR/results.tsv" ] || echo "# ts host benchmark preset device_api score fps_avg fps_min duration_s log" > "$OUT_DIR/results.tsv"
  echo "$line" >> "$OUT_DIR/results.tsv"
}

# parse_heaven_log <logfile> -> prints "score fps min max" (space separated,
# NA for anything not found). Matches the on-screen widget fields documented
# in the Basic Edition manual (FPS, Score, Min FPS, Max FPS) as they appear in
# the raw stdout/log of the heaven_x64/heaven_x86 binary.
parse_heaven_log() {
  local log="$1"
  local score fps minfps maxfps
  score=$(grep -oP '(?<=^Score:\s)[0-9.]+' "$log" | tail -1); score=${score:-NA}
  fps=$(grep -oP '(?<=^FPS:\s)[0-9.]+' "$log" | tail -1); fps=${fps:-NA}
  minfps=$(grep -oP '(?<=^Min FPS:\s)[0-9.]+' "$log" | tail -1); minfps=${minfps:-NA}
  maxfps=$(grep -oP '(?<=^Max FPS:\s)[0-9.]+' "$log" | tail -1); maxfps=${maxfps:-NA}
  echo "$score $fps $minfps $maxfps"
}

# parse_superposition_log <logfile> -> "score fps min max", from the
# automation log Superposition itself writes to
# ~/.Superposition/automation/log*.txt (results-definition.xml confirms the
# field names: FPS, Min FPS, Max FPS, Score).
parse_superposition_log() {
  local log="$1"
  local score fps minfps maxfps
  score=$(grep -oP '(?<=^Score:\s)[0-9.]+' "$log" | tail -1); score=${score:-NA}
  fps=$(grep -oP '(?<=^FPS:\s)[0-9.]+' "$log" | tail -1); fps=${fps:-NA}
  minfps=$(grep -oP '(?<=^Min FPS:\s)[0-9.]+' "$log" | tail -1); minfps=${minfps:-NA}
  maxfps=$(grep -oP '(?<=^Max FPS:\s)[0-9.]+' "$log" | tail -1); maxfps=${maxfps:-NA}
  echo "$score $fps $minfps $maxfps"
}

# parse_blender_json <jsonfile> <scene> -> "samples_per_minute total_render_time"
# benchmark-launcher-cli --json output is a JSON array; picked apart with
# python3 (present on both guests, avoids a jq dependency).
parse_blender_result() {
  local jsonfile="$1" scene="$2"
  python3 - "$jsonfile" "$scene" <<'PYEOF'
import json, sys
path, scene = sys.argv[1], sys.argv[2]
try:
    data = json.load(open(path))
except Exception:
    print("NA NA")
    raise SystemExit
for entry in data:
    if entry.get("scene", {}).get("label") == scene:
        stats = entry.get("stats", {})
        print(f'{stats.get("samples_per_minute", "NA")} {stats.get("total_render_time", "NA")}')
        break
else:
    print("NA NA")
PYEOF
}
