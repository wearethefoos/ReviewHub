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
{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"This repository uses ReviewHub. Now that the pull request exists: (1) Use the reviewhub:review-manifest skill to write, validate and publish its review manifest. (2) Run `reviewhub security-status <owner/repo#number>`. If it prints `session`, run `reviewhub security-plan <owner/repo#number>`. If its `mode` is `batched`, launch one general-purpose subagent per batch, all in one message, each with exactly its batch's `prompt` and on its `model`, and wait for them. Then launch the `security-reviewer` subagent on the plan's `coordinatorModel` with only the instruction \"Review <owner/repo#number> and write the ReviewHub security report.\", adding \"The batch reviews are done: <the plan's planFile>.\" when batched (don't pass it your view of the change). Run `reviewhub validate security <file>` and `reviewhub publish security <file> --repo <owner/repo> --pr <number>` on its report, without editing its findings. If it prints `ci`, a workflow reviews it; do nothing."}}
JSON
