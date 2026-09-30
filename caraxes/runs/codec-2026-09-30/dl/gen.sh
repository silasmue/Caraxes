#!/usr/bin/env bash
# 30 s 1080p60 test clips (testsrc2 with a moving counter), software-encoded on the host.
set -e
S="-f lavfi -i testsrc2=size=1920x1080:rate=60:duration=30"
nice -n 19 ffmpeg -hide_banner -loglevel error -y $S -c:v libx264 -preset veryfast -b:v 20M -pix_fmt yuv420p -g 120 clip-h264.mp4
nice -n 19 ffmpeg -hide_banner -loglevel error -y $S -c:v libx265 -preset ultrafast -b:v 20M -pix_fmt yuv420p -x265-params keyint=120:log-level=error clip-hevc.mp4
nice -n 19 ffmpeg -hide_banner -loglevel error -y $S -c:v libvpx-vp9 -deadline realtime -cpu-used 8 -row-mt 1 -b:v 20M -pix_fmt yuv420p -g 120 clip-vp9.webm
nice -n 19 ffmpeg -hide_banner -loglevel error -y $S -c:v libsvtav1 -preset 12 -b:v 20M -pix_fmt yuv420p -g 120 clip-av1.mp4
for f in clip-*; do nice -n 19 ffmpeg -hide_banner -loglevel error -i $f -pix_fmt yuv420p -f framemd5 $f.sw.framemd5; done
ls -la clip-*
