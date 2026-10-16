#!/bin/sh
# Test coverage for ReviewHub's metrics report, run by
# .github/workflows/reviewhub-metrics.yml. Written by `reviewhub init` for the
# test stacks it found; it's yours to edit, and `reviewhub init` won't
# overwrite it. Each block writes an lcov file to $REVIEWHUB_COVERAGE_DIR as
# <name>.info; a failing block doesn't stop the others.
#
# Locally: sh .github/reviewhub-coverage.sh, then
#   reviewhub metrics --lcov coverage-reviewhub/*.info --base origin/main
set -u
out="${REVIEWHUB_COVERAGE_DIR:-$(pwd)/coverage-reviewhub}"
mkdir -p "$out"
status=0
# Rust
(cargo llvm-cov --workspace --lcov --output-path "$out/rust.info") || status=1

# JavaScript/TypeScript (Vitest): extension. Its tests load the reviewhub-core wasm module, so build that first
# (with the wasm target and the wasm-bindgen version Cargo.lock pins, installed if missing).
(
  rustup target add wasm32-unknown-unknown &&
    want=$(cargo metadata --format-version 1 --locked | jq -r '.packages[] | select(.name == "wasm-bindgen") | .version') &&
    { [ "$(wasm-bindgen --version 2>/dev/null | cut -d ' ' -f 2)" = "$want" ] || cargo install wasm-bindgen-cli --locked --version "$want"; } &&
    cd extension && npm ci && npm run wasm &&
    npx vitest run --coverage --coverage.reporter=lcov --coverage.reportsDirectory="$out/node-extension"
) || status=1
[ -f "$out/node-extension/lcov.info" ] && mv "$out/node-extension/lcov.info" "$out/node-extension.info"
exit $status
