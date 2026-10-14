#!/bin/sh
# Installs the ReviewHub CLI from GitHub releases, verifying its SHA-256.
#
#   curl -fsSL https://raw.githubusercontent.com/wearethefoos/ReviewHub/main/install.sh | sh
#
# Environment:
#   REVIEWHUB_VERSION      version to install, e.g. 0.2.0 (default: the latest release)
#   REVIEWHUB_INSTALL_DIR  where to put the binary (default: ~/.local/bin)
# Inside Claude Code, the ReviewHub plugin installs and updates the CLI by itself.
set -eu

repo=wearethefoos/ReviewHub
dir="${REVIEWHUB_INSTALL_DIR:-$HOME/.local/bin}"
base="${REVIEWHUB_RELEASE_BASE_URL:-https://github.com/$repo/releases/download}"

fail() {
  echo "install.sh: $*" >&2
  exit 1
}
download() {
  if command -v curl >/dev/null 2>&1; then curl -fsSL --retry 3 -o "$2" "$1"; else wget -q -t 3 -O "$2" "$1"; fi
}
sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d ' ' -f 1; else shasum -a 256 "$1" | cut -d ' ' -f 1; fi
}

version="${REVIEWHUB_VERSION:-}"
if [ -z "$version" ]; then
  # github.com/<repo>/releases/latest redirects to .../releases/tag/v<version>
  url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$repo/releases/latest") || fail "can't find the latest release"
  case "$url" in
    */releases/tag/v*) version=${url##*/v} ;;
    *) fail "$repo has no release yet" ;;
  esac
fi

exe=""
case "$(uname -s)" in
  Linux) os=unknown-linux-musl ;;
  Darwin) os=apple-darwin ;;
  MINGW* | MSYS* | CYGWIN*) os=pc-windows-msvc exe=.exe ;;
  *) fail "unsupported operating system: $(uname -s)" ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) arch=x86_64 ;;
  aarch64 | arm64) arch=aarch64 ;;
  *) fail "unsupported CPU architecture: $(uname -m)" ;;
esac
if [ "$os" = apple-darwin ] && [ "$(sysctl -n hw.optional.arm64 2>/dev/null || echo 0)" = 1 ]; then arch=aarch64; fi
[ "$os" = pc-windows-msvc ] && arch=x86_64
asset="reviewhub-$arch-$os$exe"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM
echo "Installing the ReviewHub CLI v$version ($arch-$os) to $dir"
download "$base/v$version/SHA256SUMS" "$tmp/SHA256SUMS" || fail "can't download SHA256SUMS for v$version"
download "$base/v$version/$asset" "$tmp/$asset" || fail "can't download $asset"
expected=$(awk -v name="$asset" '$2 == name || $2 == "*" name { print $1 }' "$tmp/SHA256SUMS")
[ -n "$expected" ] && [ "$expected" = "$(sha256 "$tmp/$asset")" ] || fail "checksum mismatch for $asset; not installing it"

mkdir -p "$dir"
chmod +x "$tmp/$asset"
mv -f "$tmp/$asset" "$dir/reviewhub$exe"
"$dir/reviewhub$exe" --version
case ":$PATH:" in *":$dir:"*) ;; *) echo "Add $dir to your PATH to run reviewhub." ;; esac
