#!/bin/sh
# Installs the ReviewHub CLI from GitHub releases, verifying its SHA-256.
#
#   curl -fsSL https://raw.githubusercontent.com/wearethefoos/ReviewHub/main/install.sh | sh
#
# Arguments after `sh -s --` are run with the installed CLI, e.g. to set up the
# repository in the current directory:
#
#   curl -fsSL https://raw.githubusercontent.com/wearethefoos/ReviewHub/main/install.sh | sh -s -- init
#
# Environment:
#   REVIEWHUB_VERSION      version to install, e.g. 0.2.0 (default: the latest release)
#   REVIEWHUB_INSTALL_DIR  where to put the binary (default: ~/.local/bin)
# Inside Claude Code, the ReviewHub plugin installs and updates the CLI by itself.
set -eu

repo=wearethefoos/ReviewHub
dir="${REVIEWHUB_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
  echo "install.sh: $*" >&2
  exit 1
}
download() {
  if command -v curl >/dev/null 2>&1; then curl -fsSL --retry 3 -o "$2" "$1" 2>/dev/null; else wget -q -t 3 -O "$2" "$1"; fi
}
token() {
  if [ -n "${GITHUB_TOKEN:-}" ]; then echo "$GITHUB_TOKEN"; elif [ -n "${GH_TOKEN:-}" ]; then echo "$GH_TOKEN"; elif command -v gh >/dev/null 2>&1; then gh auth token 2>/dev/null || true; fi
}
api_curl() {
  if [ -n "$auth" ]; then curl -fsSL --retry 3 -H "Authorization: Bearer $auth" "$@" 2>/dev/null; else curl -fsSL --retry 3 "$@" 2>/dev/null; fi
}
# Same as plugin/bin/reviewhub: release assets through the GitHub API, for private repositories.
api_download() { # name file
  command -v curl >/dev/null 2>&1 || return 1
  api="${GITHUB_API_URL:-https://api.github.com}"
  auth=$(token)
  [ -f "$tmp/release.json" ] || api_curl -o "$tmp/release.json" "$api/repos/$repo/releases/tags/v$version" || return 1
  id=$(tr -d ' \t\r\n' <"$tmp/release.json" | tr ',{}[]' '\n\n\n\n\n' | awk -v name="\"name\":\"$1\"" '
    /^"url":".*\/releases\/assets\/[0-9]+"$/ { n = $0; sub(/"$/, "", n); sub(/.*\//, "", n); last = n }
    $0 == name && last != "" { print last; exit }')
  [ -n "$id" ] || return 1
  api_curl -H "Accept: application/octet-stream" -o "$2" "$api/repos/$repo/releases/assets/$id"
}
fetch() { # name file
  if [ -n "${REVIEWHUB_RELEASE_BASE_URL:-}" ]; then
    download "$REVIEWHUB_RELEASE_BASE_URL/v$version/$1" "$2"
  else
    download "https://github.com/$repo/releases/download/v$version/$1" "$2" || api_download "$1" "$2"
  fi
}
sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d ' ' -f 1; else shasum -a 256 "$1" | cut -d ' ' -f 1; fi
}

version="${REVIEWHUB_VERSION:-}"
if [ -z "$version" ]; then
  # github.com/<repo>/releases/latest redirects to .../releases/tag/v<version>;
  # for a private repository, ask the API instead.
  url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$repo/releases/latest" 2>/dev/null || true)
  case "$url" in
    */releases/tag/v*) version=${url##*/v} ;;
    *)
      auth=$(token)
      version=$(api_curl "${GITHUB_API_URL:-https://api.github.com}/repos/$repo/releases/latest" | tr -d ' \t\r\n' | tr ',' '\n' | sed -n 's/^"tag_name":"v\(.*\)"$/\1/p')
      [ -n "$version" ] || fail "can't find the latest release of $repo"
      ;;
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
if ! fetch SHA256SUMS "$tmp/SHA256SUMS"; then
  [ -n "${REVIEWHUB_VERSION:-}" ] || fail "v$version has no binaries yet; a new release takes a few minutes to build. Try again shortly, or set REVIEWHUB_VERSION to an earlier version."
  fail "can't download SHA256SUMS for v$version"
fi
fetch "$asset" "$tmp/$asset" || fail "can't download $asset"
expected=$(awk -v name="$asset" '$2 == name || $2 == "*" name { print $1 }' "$tmp/SHA256SUMS")
[ -n "$expected" ] && [ "$expected" = "$(sha256 "$tmp/$asset")" ] || fail "checksum mismatch for $asset; not installing it"

mkdir -p "$dir"
chmod +x "$tmp/$asset"
mv -f "$tmp/$asset" "$dir/reviewhub$exe"
"$dir/reviewhub$exe" --version
case ":$PATH:" in *":$dir:"*) ;; *) echo "Add $dir to your PATH to run reviewhub." ;; esac

if [ $# -gt 0 ]; then
  echo
  "$dir/reviewhub$exe" "$@"
fi
