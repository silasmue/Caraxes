#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Codec matrix on the Linux guest (the Windows twin: codec-matrix.ps1). Files in ~/codec:
# clips, their host software framemd5, and FF (ffmpeg dir, default: the one on PATH).
# One line per test: "MATRIX <test> PASS|FAIL|NEG <detail>".
SECS=${SECS:-30}; ONLY=${1:-}
D=~/codec; cd $D || exit 1
FF=${FF:-$(command -v ffmpeg)}; FP=${FP:-$(command -v ffprobe)}
N=$((60 * SECS))
want() { [[ -z $ONLY || $1 =~ $ONLY ]]; }
frames() { grep -oE 'frame= *[0-9]+' "$1" | tail -1 | grep -oE '[0-9]+' || echo 0; }
fps() { local n t; n=$(frames "$1"); t=$(grep -oE 'rtime=[0-9.]+s' "$1" | tail -1 | tr -dc '0-9.'); [[ -n $t ]] && python3 -c "print(f'$n frames {float($t):.2f} s {$n/float($t):.1f} fps')" || echo "$n frames"; }
fallback() { grep -qiE 'Failed setup for format|hwaccel initialisation returned error|not supported|No device available|Unsupported|Error while decoding|Failed to create' "$1"; }
hashes() { grep -v '^#' "$1" | awk -F, '{gsub(/ /,"",$NF); print $NF}'; }
echo "ffmpeg: $($FF -hide_banner -version | head -1)"
nvidia-smi --query-gpu=name,driver_version --format=csv,noheader
for c in h264 hevc; do
    want enc-$c || continue
    o=$D/enc-$c.mp4; rm -f $o
    $FF -hide_banner -benchmark -y -hwaccel cuda -hwaccel_output_format cuda -i $D/clip-h264.mp4 -t $SECS -c:v ${c}_nvenc -preset p4 -tune ll -b:v 20M $o > log-enc-$c.txt 2>&1; rc=$?
    n=$($FP -v error -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of csv=p=0 $o 2>/dev/null)
    v=$($FF -hide_banner -v error -i $o -f null - 2>&1 | head -3)
    st=FAIL; [[ $rc == 0 && $n == "$N" && -z $v ]] && st=PASS
    echo "MATRIX enc-$c-nvenc (NVDEC->CUDA->NVENC, 1080p60 ${SECS}s): exit $rc, $(fps log-enc-$c.txt), out frames $n $st"
    [[ $st == PASS ]] || tail -4 log-enc-$c.txt
    o=$D/enc-$c-cpu.mp4; rm -f $o
    $FF -hide_banner -benchmark -y -f lavfi -i testsrc2=size=1920x1080:rate=60:duration=$SECS -c:v ${c}_nvenc -preset p4 -tune ll -b:v 20M $o > log-enc-$c-cpu.txt 2>&1; rc=$?
    st=FAIL; [[ $rc == 0 && $(frames log-enc-$c-cpu.txt) == "$N" ]] && st=PASS
    echo "MATRIX enc-$c-nvenc-cpusrc (testsrc2 on the CPU, uploaded): exit $rc, $(fps log-enc-$c-cpu.txt) $st"
done
if want enc-av1; then
    $FF -hide_banner -y -f lavfi -i testsrc2=size=1920x1080:rate=60:duration=1 -c:v av1_nvenc $D/enc-av1.mp4 > log-enc-av1.txt 2>&1; rc=$?
    st=FAIL; [[ $rc != 0 ]] && grep -qiE 'not supported|No capable devices|unsupported' log-enc-av1.txt && st=NEG
    echo "MATRIX enc-av1-nvenc (TU106 has no AV1 encode): exit $rc, $(grep -iE 'nvenc|support|capab|Error' log-enc-av1.txt | head -2 | tr '\n' '|') $st"
fi
for c in h264 hevc vp9 av1; do
    clip=$(ls $D/clip-$c.* | grep -v framemd5 | head -1)
    ref=$clip.sw.framemd5
    if want dec-$c-cuda; then
        $FF -hide_banner -benchmark -hwaccel cuda -hwaccel_output_format cuda -i $clip -f null - > log-dec-$c-speed.txt 2>&1; rc=$?
        fb=0; fallback log-dec-$c-speed.txt && fb=1
        $FF -hide_banner -hwaccel cuda -i $clip -pix_fmt yuv420p -f framemd5 $D/dec-$c.framemd5 > log-dec-$c-md5.txt 2>&1; rc2=$?
        fallback log-dec-$c-md5.txt && fb=1
        same=$(paste -d' ' <(hashes $ref) <(hashes $D/dec-$c.framemd5) | awk '$1==$2' | wc -l); tot=$(hashes $ref | wc -l); got=$(hashes $D/dec-$c.framemd5 | wc -l)
        if [[ $c == av1 ]]; then st="FAIL(av1 claimed by hw?)"; [[ $fb == 1 || $rc != 0 ]] && st=NEG
        else st=FAIL; [[ $rc == 0 && $fb == 0 && $same == "$tot" && $got == "$tot" ]] && st=PASS; fi
        echo "MATRIX dec-$c-cuda: exit $rc/$rc2, speed $(fps log-dec-$c-speed.txt), framemd5 $same/$tot equal, fallback=$fb $st"
        [[ $st == PASS ]] || grep -iE 'hwaccel|Failed|support|Error' log-dec-$c-speed.txt | head -3
    fi
    if want dec-$c-cuvid; then
        $FF -hide_banner -benchmark -c:v ${c}_cuvid -i $clip -f null - > log-dec-$c-cuvid.txt 2>&1; rc=$?
        if [[ $c == av1 ]]; then st="FAIL(unexpected success)"; [[ $rc != 0 ]] && st=NEG
        else st=FAIL; [[ $rc == 0 && $(frames log-dec-$c-cuvid.txt) == "$N" ]] && st=PASS; fi
        echo "MATRIX dec-$c-cuvid: exit $rc, $(fps log-dec-$c-cuvid.txt) $st"
        [[ $st == PASS ]] || grep -iE 'cuvid|CUDA|support|Error' log-dec-$c-cuvid.txt | head -3
    fi
done
echo "guest Xid: $(sudo dmesg | grep -c 'NVRM: Xid')"
echo "done $(date +%T)"
