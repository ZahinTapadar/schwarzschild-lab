#!/bin/sh
# Build the browser Observatory into observatory/web and serve it.
#   ./build_web.sh          build + serve on :8766
#   ./build_web.sh --no-serve
# Needs Rust with the wasm32-unknown-unknown target. The volume cache must
# exist (volumes/*.obsvol); regenerate it with:
#   cargo run --release -p observatory-cli -- volume --n 96 --seed 7 --out volumes/lensed-nebula-96.obsvol
set -e
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown -p observatory-wasm
cp target/wasm32-unknown-unknown/release/observatory_wasm.wasm web/observatory.wasm
rm -rf web/presets && mkdir -p web/presets web/volumes
cp presets/*.json web/presets/
cp volumes/*.obsvol web/volumes/
[ "$1" = "--no-serve" ] && exit 0
echo "Observatory at http://localhost:8766"
cd web && python3 -m http.server 8766
