#!/usr/bin/env bash
# Sets the version of the CLI, the Claude plugin and the extension. They are
# released together: the plugin downloads the CLI binary of its own version.
#   scripts/bump-version.sh 0.2.0
set -euo pipefail
cd "$(dirname "$0")/.."

version=${1:?usage: scripts/bump-version.sh <version>}
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version must look like 1.2.3 (Chrome requires plain numbers)" >&2; exit 1; }

sed -i.bak "/^\[workspace.package\]/,/^\[/s/^version = \"[^\"]*\"/version = \"$version\"/" Cargo.toml
for file in plugin/.claude-plugin/plugin.json extension/package.json extension/public/manifest.json; do
  sed -i.bak "s/^  \"version\": \"[^\"]*\"/  \"version\": \"$version\"/" "$file"
done
find . -name '*.bak' -not -path './node_modules/*' -not -path './extension/node_modules/*' -delete

cargo update --workspace --quiet
(cd extension && npm install --package-lock-only --silent)
scripts/check-version.sh "$version"
