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
2. Run `reviewhub security-plan <pr>` and read only its `coordinatorModel` (the strong model, `opus` unless `REVIEWHUB_MODEL_STRONG` says otherwise). Launch the `security-reviewer` subagent on that model with only the pull request reference and this instruction: "Review <pr> and write the ReviewHub security report." Don't pass it your own view of the change. On a large pull request it splits the review into batches for subagents of its own, so it can take a while.
3. When it returns, run `reviewhub validate security <file>` yourself.
4. Publish with `reviewhub publish security <file> --repo <owner/repo> --pr <number>`. The user invoking this skill is the request to publish; it updates an earlier security comment rather than adding another.
5. Report the comment URL and the critical and high findings in a few lines. Offer to fix them, noting that a fix should get its own manifest update and a fresh security review.
