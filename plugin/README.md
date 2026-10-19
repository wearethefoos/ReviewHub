# ReviewHub plugin for Claude Code

Makes Claude a good citizen of a ReviewHub workflow: it publishes its own account of every pull request it writes, can run an independent security review, and helps you review agent-authored pull requests by what matters.

## Install

```
/plugin marketplace add wearethefoos/ReviewHub
/plugin install reviewhub@reviewhub
```

The skills use the `reviewhub` CLI, which comes with the plugin: `bin/reviewhub` downloads the release binary matching the plugin's version from [GitHub releases](https://github.com/wearethefoos/ReviewHub/releases) on first use (a session start hook does it in the background), checks it against the release's `SHA256SUMS`, and keeps it in the plugin's data directory (or `~/.cache/reviewhub`). When the plugin updates, the next run fetches the matching binary and removes the old one.

### Updating

Third-party marketplaces don't auto-update in Claude Code by default, so a session start hook checks GitHub once a day and tells you when a newer release is out. To update: `/plugin marketplace update reviewhub`, then `/plugin` → Installed → reviewhub → **Update now**, and `/reload-plugins` (in a shell: `claude plugin marketplace update reviewhub && claude plugin update reviewhub@reviewhub`). To get updates automatically, open `/plugin` → Marketplaces → reviewhub → **Enable auto-update**. Set `REVIEWHUB_NO_UPDATE_CHECK=1` to turn the check off; it is also off when `DISABLE_UPDATES` or `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` is set. Supported: Linux (x86-64, ARM64), macOS (Intel, Apple Silicon), Windows (x86-64, through Git Bash). It needs `curl` or `wget`, and `sha256sum` or `shasum`.

The CLI reads GitHub with `GITHUB_TOKEN`, or your `gh auth login`.

To use a CLI you built yourself, set `REVIEWHUB_BIN=/path/to/reviewhub`.

Claude Code cloud sessions (including ones started from the Claude app) can't install plugins. For those, run `reviewhub init` in the repository: it puts these same skills, the subagent and the hooks into the repository's `.claude/` directory. See the [main README](../README.md#repository-setup-reviewhub-init).

## What's included

| Component | Kind | What it does |
| --- | --- | --- |
| `/reviewhub:review <pr>` | skill | Briefs you on a pull request: blockers, deviations, decisions to make, what wasn't validated, and which files to read. Then walks you through them. Never posts to GitHub without asking. |
| `reviewhub:review-manifest` | skill | After Claude opens or updates a pull request, writes the review manifest (the request, deviations, decisions, validation evidence, gaps, rollout) and publishes it as a PR comment. |
| `/reviewhub:security-review <pr>` | skill | Hands the pull request to the `security-reviewer` subagent, then publishes its findings, each with a patch or a fix prompt. |
| `security-reviewer` | subagent | Reviews the change in a fresh context, without the author's conversation, and doesn't trust the manifest's claims. Runs on Opus; splits large pull requests into batches for subagents on models that fit each batch (see [Security reviews](../README.md#security-reviews)). |
| PR-created hook | hook | After Claude runs `gh pr create` (or the GitHub MCP equivalent) in a repository that uses ReviewHub, reminds it to publish the manifest, and to have the `security-reviewer` subagent review the pull request when `reviewhub security-status` says no CI workflow does. |
| Session start hook | hook | Fetches the CLI in the background so the first skill doesn't wait for the download. |

## Opting a repository in

The plugin only publishes manifests on its own in repositories that ask for it. Add this to the repository's `CLAUDE.md` (or `AGENTS.md`):

```markdown
## ReviewHub

Pull requests in this repository are reviewed with ReviewHub. Publish a review manifest with every pull request you open or update (reviewhub:review-manifest skill).
```

Elsewhere, Claude offers to publish a manifest and waits for your answer.

## Without Claude Code

The skill and subagent files are plain Markdown instructions. Other agents can use [`skills/review-manifest/SKILL.md`](skills/review-manifest/SKILL.md) and [`agents/security-reviewer.md`](agents/security-reviewer.md) as prompts.
