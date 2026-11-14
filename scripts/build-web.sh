#!/usr/bin/env bash
# Builds web/jarsnitch.wasm. Needs the wasm32-unknown-unknown target:
#   rustup target add wasm32-unknown-unknown
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p jarsnitch-wasm --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/jarsnitch_wasm.wasm web/jarsnitch.wasm
echo "built web/jarsnitch.wasm ($(wc -c < web/jarsnitch.wasm) bytes)"
# The page's "Try a sample" jars: a made-up plugin from fixtures/ whose code never runs.
mkdir -p web/samples
cp fixtures/jars/sample-v1.jar web/samples/SampleShop-1.4.0.jar
cp fixtures/jars/sample-v2.jar web/samples/SampleShop-1.4.1.jar
# The docs pages: docs/guide, docs/*.md and CHANGELOG.md -> web/docs/. Fails on a broken link.
node scripts/build-docs.mjs
