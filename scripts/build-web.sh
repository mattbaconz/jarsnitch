#!/usr/bin/env bash
# Builds web/jarsnitch.wasm. Needs the wasm32-unknown-unknown target:
#   rustup target add wasm32-unknown-unknown
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p jarsnitch-wasm --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/jarsnitch_wasm.wasm web/jarsnitch.wasm
echo "built web/jarsnitch.wasm ($(wc -c < web/jarsnitch.wasm) bytes)"
