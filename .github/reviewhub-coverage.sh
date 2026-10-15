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

# JavaScript/TypeScript (Vitest): extension
(cd extension && npm ci && npm install --no-save "@vitest/coverage-v8@$(node -p "require('vitest/package.json').version")" && npx vitest run --coverage --coverage.reporter=lcov --coverage.reportsDirectory="$out/node-extension") || status=1
[ -f "$out/node-extension/lcov.info" ] && mv "$out/node-extension/lcov.info" "$out/node-extension.info"
exit $status
