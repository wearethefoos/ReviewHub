---
name: security-review
description: Run an independent ReviewHub security review of a pull request and publish the findings as a PR comment. Use when the user asks for a security review of a pull request for ReviewHub.
argument-hint: "<owner/repo#number>"
disable-model-invocation: true
allowed-tools: Bash(reviewhub *)
---

# Security review for ReviewHub

The review must be independent of whoever wrote the change, including you if you wrote it in this session. Delegate it.

1. Resolve the pull request from `$ARGUMENTS` (`owner/repo#123` or a URL). If it's missing, ask for it.
2. Run `reviewhub security-review <pr>` in the background (it takes several minutes; don't run it with a short timeout). Add `--force` to review again when the head commit already has a report or a CI workflow reviews it. It starts a separate, headless Claude Code reviewer that only gets the pull request reference (and reviews a large pull request in batches, on models that fit each), then validates and publishes the report itself, updating an earlier security comment rather than adding another. The user invoking this skill is the request to publish. Don't review the change or touch the report yourself.
3. If it fails, show the user its error. If Claude Code (`claude`) isn't installed or logged in, say so.
4. Report the comment URL and the critical and high findings in a few lines (read them with `reviewhub review <pr> --json`). Offer to fix them, noting that a fix should get its own manifest update and a fresh security review.
