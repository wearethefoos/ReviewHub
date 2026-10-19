#!/bin/sh
# Tests scripts/docs-check.sh, the Stop hook that asks Claude to keep the README and the landing page up to date.
set -eu
hook="$(cd "$(dirname "$0")" && pwd)/docs-check.sh"
repo=$(mktemp -d)
trap 'rm -rf "$repo"' EXIT
cd "$repo"
git init -q -b main
git -c user.name=t -c user.email=t@example.com commit -q --allow-empty -m base
git update-ref refs/remotes/origin/main HEAD
mkdir -p crates/cli/src crates/cli/tests extension/src/landing

failed=0
check() {
  want=$1 name=$2 input=${3:-'{}'}
  got=$(printf '%s' "$input" | CLAUDE_PROJECT_DIR="$repo" sh "$hook")
  if { [ "$want" = asks ] && printf '%s' "$got" | grep -q '"decision":"block"'; } || { [ "$want" = quiet ] && [ -z "$got" ]; }; then
    echo "ok   $name"
  else
    echo "FAIL $name: $got"
    failed=1
  fi
}

check quiet "no changes"
echo 'fn a() {}' >crates/cli/tests/commands.rs
echo '# Docs' >README.md
check quiet "only tests and docs changed"
echo 'fn main() {}' >crates/cli/src/main.rs
check asks "a user-facing change asks to check the docs"
check quiet "asks once per state of the changes"
echo 'fn main() { run() }' >>crates/cli/src/main.rs
check asks "asks again after further changes"
echo '// more' >>crates/cli/src/main.rs
check quiet "not while Claude is already continuing for this hook" '{"stop_hook_active":true}'
git update-ref -d refs/remotes/origin/main
echo '// again' >>crates/cli/src/main.rs
check quiet "quiet without origin/main"
exit $failed
