#!/bin/sh
# Sonification videos: each image with its audio twin and the scan drawn on
# it, as NASA's sonification videos do. Run survey_captures.sh first.
# Needs ffmpeg and ImageMagick.
set -e
cd "$(dirname "$0")/.."
P=out/survey/presentation
A=out/survey/audio
OUT=out/survey/video
mkdir -p $OUT
T=$(mktemp -d)
D=30

enc() { echo "-c:v libx264 -preset veryslow -tune stillimage -crf 26 -pix_fmt yuv420p -r 30 -c:a aac -b:a 128k -movflags +faststart"; }

# sweep IN.png AUDIO.wav OUT.mp4 PIC_H Y0 TILE_W "X0 X0 ..." (one line per tile column)
sweep() {
  in=$1; wav=$2; out=$3; ph=$4; y0=$5; tw=$6; xs=$7
  chain="[0:v]format=rgba[b0]"
  k=0
  for x0 in $xs; do
    chain="$chain;[b$k][1:v]overlay=x='$x0+$tw*t/$D':y=$y0:eval=frame[b$((k + 1))]"
    k=$((k + 1))
  done
  chain="$chain;[b$k]pad=ceil(iw/2)*2:ceil(ih/2)*2[v]"
  ffmpeg -loglevel error -y -loop 1 -framerate 30 -i "$in" \
    -f lavfi -i "color=c=white:s=2x$ph:r=30,format=rgba,colorchannelmixer=aa=0.8" \
    -i "$wav" -filter_complex "$chain" -map "[v]" -map 2:a $(enc) -t $D "$out"
}

# radar IN.png AUDIO.wav OUT.mp4 PIC_W PIC_H: an arm turning clockwise from 12 o'clock
radar() {
  in=$1; wav=$2; out=$3; pw=$4; ph=$5
  s=$(python3 -c "import math; print(2 * math.ceil(math.hypot($pw, $ph) / 2))")
  magick -size ${s}x${s} xc:none -stroke 'rgba(255,255,255,0.85)' -strokewidth 2 -draw "line $((s / 2)),$((s / 2)) $((s / 2)),0" "$T/arm.png"
  ffmpeg -loglevel error -y -loop 1 -framerate 30 -i "$in" -loop 1 -framerate 30 -i "$T/arm.png" -i "$wav" -filter_complex \
    "[0:v]format=rgba,split[a][b];[a]crop=$pw:$ph:0:0[pic];[b]crop=iw:ih-$ph:0:$ph[leg];[1:v]rotate=a='2*PI*t/$D':c=none:ow=$s:oh=$s[arm];[pic][arm]overlay=x=$((pw / 2 - s / 2)):y=$((ph / 2 - s / 2)):eval=frame[p2];[p2][leg]vstack,pad=ceil(iw/2)*2:ceil(ih/2)*2[v]" \
    -map "[v]" -map 2:a $(enc) -t $D "$out"
}

# montage tiles: 512 × 320 with a 26 px label, 4 px gaps, 3 columns
sweep $P/pillars-montage.png $A/pillars-survey.wav $OUT/pillars-survey.mp4 700 4 512 "4 520 1036"
sweep $P/pillars-nircam-composite.png $A/pillars-nircam.wav $OUT/pillars-nircam.mp4 640 0 1024 "0"
sweep $P/pillars-xray-optical-ir.png $A/pillars-survey.wav $OUT/pillars-xray-optical-ir.mp4 640 0 1024 "0"
sweep $P/cluster-core-bullet-style.png $A/cluster-core-survey.wav $OUT/cluster-core-survey.mp4 640 0 1024 "0"
radar $P/supercluster-gas-and-radio.png $A/supercluster-survey.wav $OUT/supercluster-survey.mp4 1024 640
rm -rf "$T"
ls -la $OUT
