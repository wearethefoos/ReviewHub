#!/bin/sh
# ReviewHub SessionStart hook (installed by `reviewhub init`; re-run it to update).
# Puts the pinned ReviewHub CLI on PATH for this Claude Code session, locally or
# in the cloud, and downloads it ahead of first use.
dir="$(cd "$(dirname "$0")" && pwd)/bin"
if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
  echo "export PATH=\"$dir:\$PATH\"" >>"$CLAUDE_ENV_FILE"
fi
if ! "$dir/reviewhub" --version >/dev/null 2>&1; then
  echo "ReviewHub: couldn't install the CLI yet; it will retry when first used." >&2
fi
exit 0
