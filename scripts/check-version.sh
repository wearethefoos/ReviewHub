#!/usr/bin/env bash
# Checks that every component carries the same version, and optionally that it
# matches the given one (the release tag without its "v").
set -euo pipefail
cd "$(dirname "$0")/.."

json_version() { sed -n 's/^  "version": "\([^"]*\)".*/\1/p' "$1" | head -n 1; }
cargo=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' Cargo.toml)
plugin=$(json_version plugin/.claude-plugin/plugin.json)
extension=$(json_version extension/package.json)
manifest=$(json_version extension/public/manifest.json)

echo "Cargo workspace:      $cargo"
echo "Claude plugin:        $plugin"
echo "extension package:    $extension"
echo "extension manifest:   $manifest"

if [ "$cargo" != "$plugin" ] || [ "$cargo" != "$extension" ] || [ "$cargo" != "$manifest" ]; then
  echo "Versions differ. Use scripts/bump-version.sh to change them together." >&2
  exit 1
fi
if [ -n "${1:-}" ] && [ "$1" != "$cargo" ]; then
  echo "Expected version $1 but the sources say $cargo." >&2
  exit 1
fi
