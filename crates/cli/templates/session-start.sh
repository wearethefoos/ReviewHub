#!/bin/sh
# ReviewHub SessionStart hook (installed by `reviewhub init`; re-run it to update).
# Puts the pinned ReviewHub CLI on PATH for this Claude Code session, locally or
# in the cloud, and downloads it ahead of first use. In repositories that use the
# pre-push hook, installs it in this clone (cloud sessions start from a fresh one).
# Says when a newer ReviewHub release is out.
here="$(cd "$(dirname "$0")" && pwd)"
dir="$here/bin"
if [ -f "$here/git-pre-push" ] && hooks="$(git rev-parse --path-format=absolute --git-path hooks 2>/dev/null)"; then
  hook="$hooks/pre-push"
  if [ ! -e "$hook" ] || grep -q '^# reviewhub:' "$hook" 2>/dev/null; then
    mkdir -p "$hooks" && cp "$here/git-pre-push" "$hook" && chmod +x "$hook"
  fi
fi
if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
  echo "export PATH=\"$dir:\$PATH\"" >>"$CLAUDE_ENV_FILE"
fi
if ! "$dir/reviewhub" --version >/dev/null 2>&1; then
  echo "ReviewHub: couldn't install the CLI yet; it will retry when first used." >&2
  exit 0
fi
# Tells the user when a newer ReviewHub release exists and how to update this repository (checked once a day).
"$dir/reviewhub" update-check --for repo --hook 2>/dev/null
exit 0
