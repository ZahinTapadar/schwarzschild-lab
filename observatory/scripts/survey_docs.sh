#!/bin/sh
# Build the Multiwavelength Survey figures in docs/ from out/survey (run
# survey_captures.sh first). Uses only presentation renders (1024 × 640) and
# diagnostics; strips shrink tiles, nothing is enlarged. Needs ImageMagick and cwebp.
set -e
cd "$(dirname "$0")/.."
mkdir -p docs
T=$(mktemp -d)
FONT=${FONT:-/System/Library/Fonts/HelveticaNeue.ttc}
P=out/survey/presentation
D=out/survey/diagnostics
rm -f docs/survey-*.webp

webp() { cwebp -quiet -q "${3:-90}" "$1" -o "$2"; }
# image area only (drop the legend band), then label and shrink
tile() { magick "$1" -crop 1024x640+0+0 +repage -resize "$4" -gravity NorthWest -fill '#e8edf2' -undercolor '#000000b0' -font "$FONT" -pointsize 14 -annotate +6+6 " $2 " "$3"; }
strip() {
  out=$1; width=$2; shift 2
  files=""
  i=0
  while [ $# -gt 0 ]; do
    tile "$2" "$1" "$T/s$i.png" "${width}x"
    files="$files $T/s$i.png"
    i=$((i + 1)); shift 2
  done
  magick $files -background '#07090c' -gravity West -splice 4x0 -gravity NorthWest +append -gravity West -chop 4x0 "$T/strip.png"
  webp "$T/strip.png" "$out"
}

# pillars
webp $P/pillars-montage.png docs/survey-pillars-montage.webp 88
webp $P/pillars-nircam-composite.png docs/survey-pillars-nircam.webp 92
webp $P/pillars-optical_narrow-composite.png docs/survey-pillars-optical.webp 92
webp $P/pillars-miri-composite.png docs/survey-pillars-miri.webp 92
webp $P/pillars-xray-optical-ir.png docs/survey-pillars-xray-optical-ir.webp 92
webp $P/pillars-radio-submm.png docs/survey-pillars-radio-submm.webp 92
strip docs/survey-pillars-dust.webp 512 "H-alpha 657 nm" $P/pillars-optical_narrow-h-alpha-657.png "F200W 2 um" $P/pillars-nircam-f200w.png "F444W 4.4 um" $P/pillars-nircam-f444w.png
strip docs/survey-pillars-xray.webp 512 "0.5-1.2 keV" $P/pillars-xray_ccd-0-5-1-2-kev.png "2-7 keV" $P/pillars-xray_ccd-2-7-kev.png "850 um" $P/pillars-submm-850-um.png
webp $D/pillars-state.png docs/survey-pillars-state.webp 90
# cluster core
webp $P/cluster-core-bullet-style.png docs/survey-cluster-bullet.webp 92
webp $P/cluster-core-montage.png docs/survey-cluster-montage.webp 88
strip docs/survey-cluster-sz.webp 512 "90 GHz" $P/cluster-core-cmb-90-ghz.png "150 GHz" $P/cluster-core-cmb-150-ghz.png "220 GHz" $P/cluster-core-cmb-220-ghz.png
# supercluster
webp $P/supercluster-montage.png docs/survey-supercluster-montage.webp 88
webp $P/supercluster-gas-and-radio.png docs/survey-supercluster-gas.webp 92
webp $D/supercluster-wedge.png docs/survey-supercluster-wedge.webp 92
strip docs/survey-lensing.webp 768 "40 deg field: noise-limited" $P/supercluster-lensing-convergence.png "1.6 deg cluster core" $P/cluster-core-lensing-convergence.png
rm -rf "$T"
ls -la docs/survey-*.webp
