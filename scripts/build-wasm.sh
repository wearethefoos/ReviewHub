#!/usr/bin/env bash
# Builds reviewhub-core for the extension: extension/src/wasm/reviewhub_wasm{.js,_bg.wasm,.d.ts}.
# Needs the wasm32-unknown-unknown target and wasm-bindgen-cli matching the crate version:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version "$(cargo metadata --format-version 1 | jq -r '.packages[] | select(.name=="wasm-bindgen") | .version')"
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build -p reviewhub-wasm --target wasm32-unknown-unknown --profile release-wasm --locked
wasm-bindgen --target web --out-dir extension/src/wasm \
  target/wasm32-unknown-unknown/release-wasm/reviewhub_wasm.wasm

if command -v wasm-opt >/dev/null; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    extension/src/wasm/reviewhub_wasm_bg.wasm -o extension/src/wasm/reviewhub_wasm_bg.wasm
fi
ls -l extension/src/wasm/reviewhub_wasm_bg.wasm
