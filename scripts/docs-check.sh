#!/bin/sh
# Claude Code Stop hook for this repository: when the branch changes what users see (the CLI, the core, the plugin,
# the workflow templates or the extension), Claude is asked once to check that README.md, plugin/README.md and the
# landing page still describe it. Once per state of those changes, so finishing again without new changes is quiet.
input=$(cat)
case "$input" in *'"stop_hook_active":true'* | *'"stop_hook_active": true'*) exit 0 ;; esac
cd "${CLAUDE_PROJECT_DIR:-.}" 2>/dev/null || exit 0
base=$(git merge-base HEAD origin/main 2>/dev/null) || exit 0

files=$(
  { git diff --name-only "$base"; git ls-files --others --exclude-standard; } |
    grep -E '^(crates/(cli|core)/src/|crates/cli/templates/|plugin/|extension/src/)' |
    grep -vE '(\.test\.tsx?|_test\.rs)$|/tests?/|/generated/|README\.md$|/landing/' |
    sort -u
)
[ -n "$files" ] || exit 0

# The state of those changes: committed, staged, unstaged and untracked.
state=$({ git diff "$base" -- $files; for f in $files; do [ -f "$f" ] && git hash-object "$f"; done; } | git hash-object --stdin)
stamp="$(git rev-parse --git-dir)/reviewhub/docs-check"
[ "$(cat "$stamp" 2>/dev/null)" = "$state" ] && exit 0
mkdir -p "$(dirname "$stamp")" && printf '%s\n' "$state" >"$stamp"

count=$(printf '%s\n' "$files" | wc -l | tr -d ' ')
example=$(printf '%s\n' "$files" | head -3 | tr '\n' ' ')
cat <<EOF
{"decision":"block","reason":"This branch changes what users see ($count files, e.g. ${example% }). Before finishing, check that README.md, plugin/README.md and the landing page (extension/src/landing/Landing.tsx) still describe it, and update them where they don't (retake the docs/screenshots when the review page changes visibly). If they already do, say so in one line and finish."}
EOF
