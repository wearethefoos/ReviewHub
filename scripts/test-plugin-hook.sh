#!/usr/bin/env bash
# Tests for plugin/scripts/pr-created.sh. Run from the repository root.
set -euo pipefail

hook="$PWD/plugin/scripts/pr-created.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/optin" "$tmp/plain"
echo "Pull requests here are reviewed with ReviewHub." > "$tmp/optin/CLAUDE.md"

failures=0
expect() {
  local name=$1 project=$2 input=$3 want=$4 got
  got=$(printf '%s' "$input" | CLAUDE_PROJECT_DIR="$project" "$hook")
  if { [ "$want" = reminds ] && [[ "$got" == *review-manifest* ]]; } || { [ "$want" = silent ] && [ -z "$got" ]; }; then
    echo "ok   $name"
  else
    echo "FAIL $name: expected $want, got: $got"
    failures=$((failures + 1))
  fi
}

expect "reminds after gh pr create"          "$tmp/optin" '{"tool_name":"Bash","tool_input":{"command":"cd app && gh pr create --fill"}}' reminds
expect "reminds after the GitHub MCP tool"   "$tmp/optin" '{"tool_name":"mcp__github__create_pull_request","tool_input":{}}' reminds
expect "silent for other Bash commands"      "$tmp/optin" '{"tool_name": "Bash", "tool_input": {"command": "ls"}}' silent
expect "silent in repos that don't opt in"   "$tmp/plain" '{"tool_name":"Bash","tool_input":{"command":"gh pr create"}}' silent

[ "$failures" -eq 0 ]
