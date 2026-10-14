# ReviewHub plugin for Claude Code

Makes Claude a good citizen of a ReviewHub workflow: it publishes its own account of every pull request it writes, can run an independent security review, and helps you review agent-authored pull requests by what matters.

## Install

```
/plugin marketplace add wearethefoos/ReviewHub
/plugin install reviewhub@reviewhub
```

The skills use the `reviewhub` CLI, which needs Rust to install:

```sh
cargo install --locked --git https://github.com/wearethefoos/ReviewHub reviewhub
```

It reads GitHub with `GITHUB_TOKEN`, or your `gh auth login`.

## What's included

| Component | Kind | What it does |
| --- | --- | --- |
| `/reviewhub:review <pr>` | skill | Briefs you on a pull request: blockers, deviations, decisions to make, what wasn't validated, and which files to read. Then walks you through them. Never posts to GitHub without asking. |
| `reviewhub:review-manifest` | skill | After Claude opens or updates a pull request, writes the review manifest (original prompt, deviations, decisions, validation evidence, gaps, rollout) and publishes it as a PR comment. |
| `/reviewhub:security-review <pr>` | skill | Hands the pull request to the `security-reviewer` subagent, then publishes its findings, each with a patch or a fix prompt. |
| `security-reviewer` | subagent | Reviews the change in a fresh context, without the author's conversation, and doesn't trust the manifest's claims. |
| PR-created hook | hook | After Claude runs `gh pr create` (or the GitHub MCP equivalent) in a repository that uses ReviewHub, reminds it to publish the manifest. |

## Opting a repository in

The plugin only publishes manifests on its own in repositories that ask for it. Add this to the repository's `CLAUDE.md` (or `AGENTS.md`):

```markdown
## ReviewHub

Pull requests in this repository are reviewed with ReviewHub. Publish a review manifest with every pull request you open or update (reviewhub:review-manifest skill).
```

Elsewhere, Claude offers to publish a manifest and waits for your answer.

## Without Claude Code

The skill and subagent files are plain Markdown instructions. Other agents can use [`skills/review-manifest/SKILL.md`](skills/review-manifest/SKILL.md) and [`agents/security-reviewer.md`](agents/security-reviewer.md) as prompts.
