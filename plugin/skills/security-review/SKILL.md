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
2. Run `reviewhub security-plan <pr>`. If its `mode` is `batched`, the pull request is large and its files are reviewed in batches first: launch one general-purpose subagent per batch, all in one message so they run in parallel, each with exactly its batch's `prompt` (complete; add nothing) and on its batch's `model`, and wait for them. They write their reports and notes to files; you don't need to read them.
3. Launch the `security-reviewer` subagent on the plan's `coordinatorModel` with only the pull request reference and this instruction: "Review <pr> and write the ReviewHub security report." When batched, add: "The batch reviews are done: <the plan's `planFile`>." Don't pass it your own view of the change.
4. When it returns, run `reviewhub validate security <file>` yourself.
5. Publish with `reviewhub publish security <file> --repo <owner/repo> --pr <number>`. The user invoking this skill is the request to publish; it updates an earlier security comment rather than adding another.
6. Report the comment URL and the critical and high findings in a few lines. Offer to fix them, noting that a fix should get its own manifest update and a fresh security review.
