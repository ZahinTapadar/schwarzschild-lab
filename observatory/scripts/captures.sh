#!/bin/sh
# Reproduce every documented render (native CLI). Output tree:
#   out/presentation/   final renders, ≥ 1024×640 (frames/ and progress/ inside)
#   out/diagnostics/    fate, parity, winding, τ, … at 1024–1280 px, with legends
#   out/data/           EXR, FITS, JSON and checkpoints of the presentation renders
#   out/validation/     sweeps at 512 px (never presentation)
#   out/debug/          correspondence overlays (never presentation)
#   out/benchmarks/     timing runs, checkpoint test (small)
#   out/fixtures/       128 px regression renders (compared with fixtures/)
set -e
cd "$(dirname "$0")/.."
B=./target/release/observatory
cargo build --release -p observatory-cli
rm -rf out && mkdir -p out

# --- presentation renders (telescope mode)
$B render presets/first-light-single.json        --tree out --name first-light-single --views radiance,fate,redshift,winding,noise
$B render presets/first-light-hdr.json           --tree out --name first-light-hdr --snap 8,64 --views radiance
$B render presets/lensed-hii-offset.json         --tree out --name lensed-hii-offset --snap 8,32,128 --views radiance,tau,nebula,fate,noise,parity
$B render presets/lensed-hii-offset.json         --tree out --name lensed-hii-offset-unlensed --unlensed --views radiance
$B render presets/einstein-ring-structured.json  --tree out --name einstein-ring-structured --views radiance,tau,nebula
$B render presets/einstein-ring.json             --tree out --name einstein-ring-exact --views radiance,tau,nebula
$B render presets/einstein-ring.json             --tree out --name einstein-ring-exact-unlensed --unlensed --views radiance
$B render presets/narrowband-nebula.json         --tree out --name narrowband-nebula-sho --views band,radiance,tau,nebula
$B render presets/deep-field-frame.json          --tree out --name deep-field-frame --views radiance,noise,variance
$B render presets/deep-field-stack.json          --tree out --name deep-field-stack --snap 32,128 --views radiance,noise

# --- diagnostics: the local-observer validation scene at 1280×800, the volume before lensing
$B render presets/critical-curve.json --tree out --name critical-curve --views fate,winding,parity,invariant,steps,redshift
$B inspect volumes/lensed-nebula-96.obsvol --out out/diagnostics/volume-inspect

# --- debug only: source-feature correspondence (landmarks), lensed and unlensed
$B render presets/lensed-hii-offset.json --tree out/debug --name offset-landmarks --set landmarks=true --set output_class=validation --spp 32 --views correspondence
$B render presets/lensed-hii-offset.json --tree out/debug --name offset-landmarks-unlensed --unlensed --set landmarks=true --set output_class=validation --spp 32 --views correspondence
$B render presets/einstein-ring.json --tree out/debug --name exact-landmarks --set landmarks=true --set output_class=validation --spp 32 --views correspondence

# --- validation sweeps at 512 px
V=out/validation
for b in 0 7 21 48 103; do
  $B render presets/validation/finite-source.json --tree $V/finite-source-offset-sweep --name disc-$b --w 512 --h 512 --set output_class=validation --set "source_offset_arcmin=[$b,0]" --views parity
done
$B render presets/validation/finite-source.json --tree $V/finite-source-offset-sweep --name disc-48-unlensed --w 512 --h 512 --set output_class=validation --set "source_offset_arcmin=[48,0]" --unlensed
for o in 0 3 8; do
  $B render presets/einstein-ring.json --tree $V/source-offset-sweep --name nebula-offset-$o --w 512 --h 320 --spp 64 --set output_class=validation --set "source_offset_arcmin=[$o,0]" --views tau
done
for d in 1000 2500 6000; do
  $B render presets/einstein-ring.json --tree $V/source-distance-sweep --name nebula-distance-$d --w 512 --h 320 --spp 64 --set output_class=validation --set "source_distance_pc=$d" --views tau
done
for s in 1.0 0.5 0.25; do
  $B render presets/einstein-ring-structured.json --tree $V/volume-step-convergence --name step-$s --w 512 --h 320 --spp 64 --set output_class=validation --set "volume_step=$s" --views nebula
done

# --- benchmarks: time to 256 / 1,024 / 4,096 spp, and the same scene at three resolutions
$B render presets/first-light-single.json --tree out/benchmarks --name bench-4096 --w 320 --h 200 --spp 4096 --pass 64 --snap 256,1024,4096 --set output_class=fixture
for r in "640 400" "1024 640" "1600 1000"; do
  set -- $r
  $B render presets/first-light-single.json --tree out/benchmarks --name resolution-$1 --w $1 --h $2 --spp 16
done
# checkpoint / resume reproducibility: 0→64 in one go vs 0→32 + resume →64
$B render presets/lensed-hii-offset.json --tree out/benchmarks --name ck-full --w 160 --h 100 --spp 64 --set output_class=fixture
$B render presets/lensed-hii-offset.json --tree out/benchmarks --name ck-half --w 160 --h 100 --spp 32 --set output_class=fixture
$B resume out/benchmarks/ck-half-160.ckpt --tree out/benchmarks --name ck-resumed --w 160 --h 100 --spp 64
cmp out/benchmarks/ck-full-160.exr out/benchmarks/ck-resumed-160.exr && echo "checkpoint resume: EXR (all radiance + sensor channels) bit-identical"

# --- fixtures: 128 px checks and image regression (out/fixtures vs fixtures/)
$B fixtures
