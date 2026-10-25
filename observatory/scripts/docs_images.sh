#!/bin/sh
# Build the README figures in docs/ from the capture tree in out/ (run
# captures.sh first). Final figures use only out/presentation (≥ 1024 px renders)
# and out/diagnostics; validation figures use out/validation (512 px) and are
# shown only in the validation section. Nothing is ever enlarged except the
# labelled nearest-neighbour crops of the resolution comparison. Fixtures are
# never used. Needs ImageMagick and cwebp.
set -e
cd "$(dirname "$0")/.."
rm -f docs/*.webp
mkdir -p docs
T=$(mktemp -d)
FONT=${FONT:-/System/Library/Fonts/HelveticaNeue.ttc}
P=out/presentation
D=out/diagnostics
V=out/validation

webp() { cwebp -quiet -q "${3:-90}" "$1" -o "$2"; }

# label a tile in its top-left corner (legend bands below stay readable)
tile() { magick "$1" -resize "$4" -gravity NorthWest -fill '#e8edf2' -undercolor '#000000b0' -font "$FONT" -pointsize 14 -annotate +6+6 " $2 " "$3"; }

# strip OUT WIDTH label file label file ... → one row of tiles WIDTH px wide
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

# --- full-resolution presentation renders (1024 × 640, unscaled)
webp $P/first-light-hdr-1024.png docs/first-light-hdr.webp 92
webp $P/lensed-hii-offset-1024.png docs/lensed-hii-offset.webp 92
webp $P/einstein-ring-structured-1024.png docs/einstein-ring-structured.webp 92
webp $P/narrowband-nebula-sho-1024-annotated.png docs/narrowband-nebula-sho.webp 92
webp $P/deep-field-stack-1024.png docs/deep-field-stack.webp 92

# --- comparisons (downscaled side by side)
strip docs/comparison.webp 512 \
  "real-time simulator (local observer, artistic disk)" docs/src/realtime.png \
  "Observatory: HDR composite at 20,000 M, physical disk" $P/first-light-hdr-1024.png
strip docs/first-light-pair.webp 512 \
  "physical single exposure: 20 us through ND 6 (calibration)" $P/first-light-single-1024.png \
  "synthetic HDR composite of 6 registered frames" $P/first-light-hdr-1024.png
strip docs/first-light-frames.webp 340 \
  "frame 1: 20 us, ND 6" $P/frames/first-light-hdr-1024-frame0.png \
  "frame 2: 20 ms, ND 6" $P/frames/first-light-hdr-1024-frame1.png \
  "frame 3: 20 us" $P/frames/first-light-hdr-1024-frame2.png
strip docs/first-light-frames-2.webp 340 \
  "frame 4: 20 ms" $P/frames/first-light-hdr-1024-frame3.png \
  "frame 5: 2 s" $P/frames/first-light-hdr-1024-frame4.png \
  "frame 6: 120 s" $P/frames/first-light-hdr-1024-frame5.png
strip docs/first-light-diagnostics.webp 512 \
  "diagnostic: disk frequency ratio g" $D/first-light-single-redshift-1024.png \
  "diagnostic: ray fate" $D/first-light-single-fate-1024.png

strip docs/lensed-pair.webp 512 \
  "same field, lensing off" $P/lensed-hii-offset-unlensed-1024.png \
  "lensed: outer arc + mirrored counter-image" $P/lensed-hii-offset-1024.png
strip docs/lensed-diagnostics.webp 340 \
  "diagnostic: dust optical depth" $D/lensed-hii-offset-tau-1024.png \
  "diagnostic: image parity" $D/lensed-hii-offset-parity-1024.png \
  "diagnostic: line radiance" $D/lensed-hii-offset-nebula-1024.png
strip docs/correspondence.webp 512 \
  "DEBUG: landmarks, lensing off" out/debug/offset-landmarks-unlensed-1024-correspondence.png \
  "DEBUG: the same landmarks, lensed" out/debug/offset-landmarks-1024-correspondence.png

strip docs/einstein-exact.webp 340 \
  "lensing off" $P/einstein-ring-exact-unlensed-1024.png \
  "exact alignment: symmetry test" $P/einstein-ring-exact-1024.png \
  "diagnostic: dust optical depth" $D/einstein-ring-exact-tau-1024.png

strip docs/narrowband-planes.webp 340 \
  "[S II] plane" $P/narrowband-nebula-sho-band-sii-1024.png \
  "H-alpha plane" $P/narrowband-nebula-sho-band-ha-1024.png \
  "[O III] plane" $P/narrowband-nebula-sho-band-oiii-1024.png
strip docs/narrowband-visible.webp 512 \
  "SHO: FALSE COLOUR" $P/narrowband-nebula-sho-1024.png \
  "visible-light rendering of the same spectra" $P/narrowband-nebula-sho-visible-1024.png

strip docs/deep-field-pair.webp 512 \
  "physical detector frame: 1 x 3600 s" $P/deep-field-frame-1024.png \
  "stacked survey composite: 36 x 300 s" $P/deep-field-stack-1024.png

# --- diagnostics at 1280 × 800 (local observer)
strip docs/critical-curve-1.webp 427 \
  "radiance check (not a picture)" $D/critical-curve-radiance-check-1280.png \
  "fate" $D/critical-curve-fate-1280.png \
  "parity" $D/critical-curve-parity-1280.png
strip docs/critical-curve-2.webp 427 \
  "winding" $D/critical-curve-winding-1280.png \
  "first-integral drift" $D/critical-curve-invariant-1280.png \
  "disk redshift g" $D/critical-curve-redshift-1280.png
magick out/diagnostics/volume-inspect.png -resize 1280x "$T/vol.png" && webp "$T/vol.png" docs/volume-inspect.webp

# --- validation (512 px)
strip docs/validation-discs.webp 256 \
  "beta = 0" $V/finite-source-offset-sweep/disc-0-512.png \
  "0.1 theta_E" $V/finite-source-offset-sweep/disc-7-512.png \
  "0.3 theta_E" $V/finite-source-offset-sweep/disc-21-512.png \
  "0.7 theta_E" $V/finite-source-offset-sweep/disc-48-512.png \
  "1.5 theta_E" $V/finite-source-offset-sweep/disc-103-512.png
strip docs/validation-nebula.webp 427 \
  "aligned" $V/source-offset-sweep/nebula-offset-0-512-tau.png \
  "3' off axis" $V/source-offset-sweep/nebula-offset-3-512-tau.png \
  "8' off axis" $V/source-offset-sweep/nebula-offset-8-512-tau.png
strip docs/validation-distance.webp 427 \
  "source at 1.0 kpc" $V/source-distance-sweep/nebula-distance-1000-512-tau.png \
  "2.5 kpc" $V/source-distance-sweep/nebula-distance-2500-512-tau.png \
  "6.0 kpc" $V/source-distance-sweep/nebula-distance-6000-512-tau.png
strip docs/validation-step.webp 427 \
  "volume step 1 voxel" $V/volume-step-convergence/step-1.0-512-nebula.png \
  "0.5 voxel" $V/volume-step-convergence/step-0.5-512-nebula.png \
  "0.25 voxel" $V/volume-step-convergence/step-0.25-512-nebula.png

# --- resolution: the same 4.8′ × 3′ patch around the shadow, rendered natively
# at three sizes, shown as nearest-neighbour crops at a common display size
for r in 640 1024 1600; do
  h=$((r * 10 / 16)); cw=$((r / 10)); ch=$((h / 10))
  f=out/benchmarks/resolution-$r-$r.png; [ -f "$f" ] || f=out/benchmarks/presentation/resolution-$r-$r.png
  magick "$f" -gravity center -crop ${cw}x${ch}+0+0 +repage -filter point -resize 400x250! "$T/crop$r.png"
done
strip docs/resolution.webp 400 \
  "640 x 400 (crop, nearest neighbour)" "$T/crop640.png" \
  "1024 x 640" "$T/crop1024.png" \
  "1600 x 1000" "$T/crop1600.png"

rm -rf "$T"
ls -la docs
