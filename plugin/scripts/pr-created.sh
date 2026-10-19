#!/usr/bin/env bash
# PostToolUse hook: after Claude opens a pull request in a repository that
# uses ReviewHub, remind it to publish a review manifest and to make sure the
# pull request gets a security review. Silent otherwise.
set -euo pipefail

input=$(cat)

# Bash calls only matter when their command creates a pull request (not when
# their output merely mentions `gh pr create`).
if printf '%s' "$input" | grep -Eq '"tool_name"[[:space:]]*:[[:space:]]*"Bash"'; then
  printf '%s' "$input" | grep -Eq '"command"[[:space:]]*:[[:space:]]*"([^"\\]|\\.)*gh pr create' || exit 0
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
{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"This repository uses ReviewHub. Now that the pull request exists: (1) Use the reviewhub:review-manifest skill to write, validate and publish its review manifest. (2) Have it security-reviewed: run `reviewhub security-review <owner/repo#number>` in the background (it takes several minutes; don't run it with a short timeout). It does nothing when a CI workflow reviews the pull request; otherwise an independent headless Claude Code reviewer reviews it and the command publishes the report. Don't review the change or publish a security report yourself. When it finishes, check it printed `Published the security report` or why it didn't; if it failed, tell the user."}}
JSON
