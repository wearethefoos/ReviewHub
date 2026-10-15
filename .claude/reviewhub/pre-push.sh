#!/bin/sh
# ReviewHub pre-push hook (installed by `reviewhub init --pre-push`; re-run it to update).
# Gets pushes from agent sessions security-reviewed: in Claude Code it tells the
# agent, in a terminal it starts a headless Claude Code review, and it does
# nothing when CI reviews the pull request. It never blocks a push.
dir="$(cd "$(dirname "$0")" && pwd)"
"$dir/bin/reviewhub" pre-push "$@" || true
exit 0
