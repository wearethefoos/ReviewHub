---
name: reviewhub-review
description: Review a pull request with ReviewHub, focusing the human reviewer on what matters (security findings, deviations, decisions, untested code) instead of the whole diff. Use when the user asks to review a pull request, especially one written by an agent.
argument-hint: "<owner/repo#number>"
allowed-tools: Bash(reviewhub review *), Bash(gh pr view *)
---

# Review a pull request with ReviewHub

Your job is to spend the reviewer's attention well: lead with what needs a human decision, and skip what doesn't.

1. Resolve the pull request from `$ARGUMENTS` (`owner/repo#123` or a URL), or the current branch's pull request (`gh pr view --json url`).
2. Run `reviewhub review <pr> --all` and read the briefing. It ranks every file: Focus files need line-by-line reading, Skim files a glance, Skip files none. It also shows which reports exist (the author's manifest, the security report and metrics); any of them may be missing or invalid. When you need exact fields, such as an alternative's prompt or a finding's patch, use `reviewhub review <pr> --json` (diffs left out).
3. Give the user a short briefing, in this order, leaving out empty sections:
   - **Blockers:** critical and high security findings, failed validation, blocking rollout steps.
   - **Deviations** from the original prompt, with the agent's reason. Ask whether each is acceptable.
   - **High-impact decisions** with the alternatives. Ask which way the user wants to go.
   - **Not validated:** gaps and who should close them.
   - **Where to read:** the Focus files and, for each, the one or two reasons it ranks high.
   - The size of what they can skip (files and lines).
   Say which reports are missing. Without a manifest or security report the ranking is weaker; offer the `reviewhub-manifest` or `reviewhub-security-review` skill.
4. Then walk through the Focus files one at a time when the user wants to. Use `reviewhub review <pr> --diff` for annotated diffs, read the code yourself, and check the author's claims against it: does the validation evidence actually cover the risky lines, and does the code do what the manifest says?
5. To act on a decision, gap or finding, use the ready-made `prompt` from the report when there is one.

Don't post comments, approve or request changes on GitHub unless the user asks. If they do, draft the text and show it first.
