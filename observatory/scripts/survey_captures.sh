#!/bin/sh
# Multiwavelength Survey captures: every survey preset at presentation size.
#   ./scripts/survey_captures.sh            (writes out/survey/{presentation,diagnostics,data})
set -e
cd "$(dirname "$0")/.."
cargo build --release -p observatory-cli
B=./target/release/observatory
$B survey presets/survey/pillars.json --tree out/survey --name pillars --grid 256 --spp 2
$B survey presets/survey/supercluster.json --tree out/survey --name supercluster --spp 2
$B survey presets/survey/cluster-core.json --tree out/survey --name cluster-core --spp 2
