#!/usr/bin/env bash
# PostToolUse hook: after Claude opens a pull request in a repository that
# uses ReviewHub, remind it to publish a review manifest. Silent otherwise.
set -euo pipefail

input=$(cat)

# Bash calls only matter when they create a pull request.
if printf '%s' "$input" | grep -Eq '"tool_name"[[:space:]]*:[[:space:]]*"Bash"'; then
  printf '%s' "$input" | grep -q 'gh pr create' || exit 0
fi

# Only repositories that opt in by mentioning ReviewHub in their agent instructions.
project="${CLAUDE_PROJECT_DIR:-$PWD}"
opted_in=false
for file in "$project/CLAUDE.md" "$project/AGENTS.md" "$project/.claude/CLAUDE.md"; do
  if [ -f "$file" ] && grep -qi 'reviewhub' "$file"; then
    opted_in=true
    break
  fi
done
$opted_in || exit 0

cat <<'JSON'
{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"This repository uses ReviewHub. Now that the pull request exists, use the reviewhub:review-manifest skill to write, validate and publish its review manifest."}}
JSON
