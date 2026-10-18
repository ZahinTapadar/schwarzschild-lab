#!/bin/sh
# Build the Rust geodesic engine to WebAssembly and serve the app.
set -e
cd "$(dirname "$0")/engine"
cargo +stable build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/geodesic.wasm ../web/engine.wasm
cd ../web && echo "Open http://localhost:8765" && python3 -m http.server 8765
