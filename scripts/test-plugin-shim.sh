#!/usr/bin/env bash
# Tests plugin/bin/reviewhub against a local fake release server.
# Usage: scripts/test-plugin-shim.sh <path to a reviewhub binary for this machine>
set -euo pipefail

binary=$(realpath "$1")
repo=$(pwd)
tmp=$(mktemp -d)
trap 'kill "$server" 2>/dev/null || true; rm -rf "$tmp"' EXIT

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) asset=reviewhub-x86_64-unknown-linux-musl ;;
  Linux-aarch64) asset=reviewhub-aarch64-unknown-linux-musl ;;
  Darwin-arm64) asset=reviewhub-aarch64-apple-darwin ;;
  Darwin-x86_64) asset=reviewhub-x86_64-apple-darwin ;;
  *) echo "unsupported platform for this test"; exit 1 ;;
esac

release() { # version [tamper]
  mkdir -p "$tmp/releases/v$1"
  cp "$binary" "$tmp/releases/v$1/$asset"
  (cd "$tmp/releases/v$1" && { sha256sum "$asset" 2>/dev/null || shasum -a 256 "$asset"; } > SHA256SUMS)
  if [ "${2:-}" = tamper ]; then echo junk >> "$tmp/releases/v$1/$asset"; fi
}
plugin() { # version
  rm -rf "$tmp/plugin" && cp -r "$repo/plugin" "$tmp/plugin"
  sed -i.bak "s/\"version\": \"[^\"]*\"/\"version\": \"$1\"/" "$tmp/plugin/.claude-plugin/plugin.json"
}

release 1.0.0
release 1.1.0
release 1.2.0 tamper
port=$((20000 + RANDOM % 20000))
(cd "$tmp/releases" && exec python3 -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1) &
server=$!
sleep 1

export REVIEWHUB_RELEASE_BASE_URL="http://127.0.0.1:$port" REVIEWHUB_CACHE_DIR="$tmp/cache" NO_PROXY=127.0.0.1 no_proxy=127.0.0.1
failures=0
check() { # name, command...
  local name=$1
  shift
  if "$@" >/dev/null 2>"$tmp/err"; then echo "ok   $name"; else echo "FAIL $name: $(cat "$tmp/err")"; failures=$((failures + 1)); fi
}
fails() { # name, command...
  local name=$1
  shift
  if "$@" >/dev/null 2>&1; then echo "FAIL $name: succeeded"; failures=$((failures + 1)); else echo "ok   $name"; fi
}

plugin 1.0.0
check "installs the release matching the plugin version" "$tmp/plugin/bin/reviewhub" --version
check "binary is cached" test -x "$tmp/cache/v1.0.0/reviewhub"
kill "$server"
check "runs from the cache without the network" "$tmp/plugin/bin/reviewhub" --version
(cd "$tmp/releases" && exec python3 -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1) &
server=$!
sleep 1
plugin 1.1.0
touch -t 202001010000 "$tmp/cache/v1.0.0"
check "a plugin update installs the matching binary" "$tmp/plugin/bin/reviewhub" --version
check "and removes versions unused for 30 days" test ! -e "$tmp/cache/v1.0.0"
mkdir -p "$tmp/repo/.claude/reviewhub/bin" && cp "$repo/plugin/bin/reviewhub" "$tmp/repo/.claude/reviewhub/bin/" && echo 1.0.0 > "$tmp/repo/.claude/reviewhub/bin/VERSION"
check "a repository launcher runs the version in its VERSION file" "$tmp/repo/.claude/reviewhub/bin/reviewhub" --version
check "next to the plugin's version, without removing it" test -x "$tmp/cache/v1.1.0/reviewhub"
check "both stay installed" test -x "$tmp/cache/v1.0.0/reviewhub"
plugin 1.2.0
fails "refuses a binary that doesn't match SHA256SUMS" "$tmp/plugin/bin/reviewhub" --version
check "keeps the working install" test -x "$tmp/cache/v1.1.0/reviewhub"
plugin 9.9.9
check "while a new release is still building, runs the newest installed version" "$tmp/plugin/bin/reviewhub" --version
check "without installing anything for it" test ! -e "$tmp/cache/v9.9.9/reviewhub"
fails "fails clearly for an unreleased version when nothing is installed" env REVIEWHUB_CACHE_DIR="$tmp/empty-cache" "$tmp/plugin/bin/reviewhub" --version
check "REVIEWHUB_BIN overrides the download" env REVIEWHUB_BIN="$binary" "$tmp/plugin/bin/reviewhub" --version
check "REVIEWHUB_CACHED_ONLY exits quietly when nothing is installed" test -z "$(REVIEWHUB_CACHED_ONLY=1 REVIEWHUB_CACHE_DIR="$tmp/empty-cache2" "$tmp/plugin/bin/reviewhub" --version)"
check "without downloading anything" test ! -e "$tmp/empty-cache2/v9.9.9"
# The update check compares the latest release with the plugin's version, not the stand-in CLI's.
now=$(date +%s)
echo "{\"tag\":\"v9.9.9\",\"checked\":$now}" > "$tmp/cache/latest-release.json"
check "no update notice when the plugin is current, even on a stand-in CLI" test -z "$("$tmp/plugin/bin/reviewhub" update-check --for plugin --hook 2>/dev/null)"
echo "{\"tag\":\"v10.0.0\",\"checked\":$now}" > "$tmp/cache/latest-release.json"
check "an update notice for the plugin when a newer release is out" grep -q "ReviewHub 10.0.0 is available (you have 9.9.9)" <("$tmp/plugin/bin/reviewhub" update-check --for plugin --hook 2>/dev/null)

[ "$failures" -eq 0 ]
