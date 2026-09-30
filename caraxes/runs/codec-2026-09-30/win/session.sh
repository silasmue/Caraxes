#!/usr/bin/env bash
# One Windows session (run inside vmlock.sh windows): start, steps, always stop.
# Usage: session.sh TAG BINARY-cap STEPS...   steps: copy matrix[=regex] nvenc software sleep=N
set -uo pipefail
TAG=$1 BIN=$2; shift 2
C=/mnt/vmstore/caraxes/runs-2026-09-30/codec
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain
OUT=$C/win-$TAG; mkdir -p "$OUT"
cd $K || exit 1
s() { timeout ${T:-60} python3 CH-build/run44.py ssh "$@" 2>&1 | tr -d '\r'; }
MYCH="" MYDEV=""
# Stop only the VM this session started: its Cloud Hypervisor pid (or its device pid) must still
# be the kit's current one; run44.py stop acts on the kit's `current` run.
stop() {
    local cur_ch cur_dev; cur_ch=$(cat $K/ch.pid 2>/dev/null) cur_dev=$(cat $K/gpu.pid 2>/dev/null)
    if [[ -z $MYCH && -z $MYDEV ]] || [[ $(cat $K/current) != "$TAG" ]] ||
       ! [[ ( -n $MYCH && $cur_ch == "$MYCH" ) || ( -n $MYDEV && $cur_dev == "$MYDEV" ) ]]; then
        echo "no VM of mine to stop (mine: ch ${MYCH:-none} dev ${MYDEV:-none}; kit: ch ${cur_ch:-none} dev ${cur_dev:-none}, current $(cat $K/current))"; return
    fi
    echo "stop $(date +%T) ch $MYCH dev $MYDEV"; timeout 400 python3 CH-build/run44.py stop > "$OUT/stop.txt" 2>&1; tail -15 "$OUT/stop.txt"
}
trap stop EXIT
echo "start $TAG $(date +%T) $BIN"
CH_BIN=/mnt/vmstore/caraxes/ch-readonly-mmap/target-0008/release/cloud-hypervisor CH_SERVER=$BIN CH_SLOT=0 \
  CH_DEV_EXTRA="CARAXES_INTR_PAGE=1 CARAXES_DOORBELL_POLL=hybrid WIN_BAR1_POOL_MIB=32 WIN_CPUS=8 CARAXES_PROFILE=1 ${DEV_EXTRA:-}" \
  timeout 300 python3 CH-build/run44.py start $TAG > "$OUT/start.txt" 2>&1; rc=$?
if [[ $(cat $K/current) == "$TAG" ]]; then
    MYCH=$(python3 -c "import json;print(json.load(open('$K/$TAG/start.json')).get('ch_pid',''))" 2>/dev/null)
    MYDEV=$(python3 -c "import json;print(json.load(open('$K/$TAG/start.json')).get('win_device_pid',''))" 2>/dev/null)
    [[ -z $MYDEV && -s $K/gpu.pid ]] && grep -q "device $(cat $K/gpu.pid)" "$OUT/start.txt" && MYDEV=$(cat $K/gpu.pid)
fi
(( rc == 0 )) || { cat "$OUT/start.txt"; exit 1; }
cat "$OUT/start.txt"
for i in $(seq 1 60); do s hostname 2>/dev/null | grep -qi . && break; sleep 5; done
echo "ssh up $(date +%T)"
for st in "$@"; do
    echo "=== $st $(date +%T)"
    case $st in
    copy)
        s 'New-Item -ItemType Directory -Force C:\caraxes\codec | Out-Null; Get-ChildItem C:\caraxes\codec | ForEach-Object { $_.Name + " " + $_.Length }'
        for f in $C/dl/win64/ffmpeg.exe $C/dl/win64/ffprobe.exe $C/dl/clip-* $C/win/codec-matrix.ps1; do
            b=$(basename $f)
            have=$(s "if (Test-Path C:\\caraxes\\codec\\$b) { (Get-Item C:\\caraxes\\codec\\$b).Length } else { 0 }" | tail -1)
            [[ $have == "$(stat -c %s $f)" && $b != *.ps1 ]] && continue
            timeout 600 python3 CH-build/run44.py scp -q $f win:C:/caraxes/codec/$b && echo "copied $b"
        done ;;
    matrix*)
        re=${st#matrix}; re=${re#=}
        only=""; [[ -n $re ]] && only="-Only '$re'"
        T=2400 s "powershell -NoProfile -ExecutionPolicy Bypass -File C:\\caraxes\\codec\\codec-matrix.ps1 $only" | tee "$OUT/matrix.txt" ;;
    nvenc|software)
        bash $C/win/stream.sh "$OUT/stream-$st" $st ${STREAM_S:-150} ;;
    async*)
        timeout 120 python3 CH-build/run44.py scp -q $C/win/nvenc-async.ps1 win:C:/caraxes/codec/nvenc-async.ps1
        variants=${ASYNC_VARIANTS:-"-Mode vram|-Mode vram -Async|-Mode lock -Device d3d11|-Mode lock -Device d3d11 -Async -Sleep 16"}
        IFS='|' read -ra vs <<< "$variants"
        for v in "${vs[@]}"; do
            echo "--- nvenc-async $v"
            T=300 s "powershell -NoProfile -ExecutionPolicy Bypass -File C:\\caraxes\\codec\\nvenc-async.ps1 -Frames ${ASYNC_FRAMES:-300} $v" | tee -a "$OUT/async.txt"
            grep "interrupt vectors by the guest" $K/gpu.log | tail -1 | grep -oE "9: [^;]*" | sed 's/^/vector /' | tee -a "$OUT/async.txt"
        done ;;
    state)
        s 'query session 2>&1 | Out-String; Get-Process explorer,dwm,LogonUI,sunshine,sunshinesvc -ErrorAction SilentlyContinue | ForEach-Object { $_.Name + " " + $_.Id + " s" + $_.SessionId }; $e = $null; [void][System.Management.Automation.Language.Parser]::ParseFile("C:\caraxes\codec\codec-matrix.ps1", [ref]$null, [ref]$e); "matrix parse errors: " + $e.Count; $e | Select-Object -First 3 | ForEach-Object { $_.Extent.StartLineNumber.ToString() + ": " + $_.Message }' ;;
    waitdesk)
        for i in $(seq 1 24); do s 'Get-Process dwm -ErrorAction SilentlyContinue | Measure-Object | ForEach-Object Count' | grep -q '^[1-9]' && { echo "dwm up $(date +%T)"; break; }; sleep 5; done ;;
    sleep=*) sleep ${st#sleep=} ;;
    *) echo "unknown step $st" ;;
    esac
done
grep "interrupt vectors by the guest" $K/gpu.log | tail -1 | cut -c1-1200
