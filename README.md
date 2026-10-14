# ReviewHub

A review view on top of GitHub for pull requests written by coding agents.

Agents produce more code than people can read line by line, and much of it doesn't need reading. ReviewHub combines three reports with the GitHub diff to show a reviewer only what needs their judgement:

- **The ask:** the user's original prompt and follow-ups, next to what was delivered.
- **Deviations:** where the agent did something other than what was asked, and why.
- **Decisions:** questions the prompt left open, what the agent chose, and alternatives with ready-to-run prompts.
- **Validation:** what the agent ran to prove the change works, with evidence.
- **Gaps:** what the agent couldn't validate, and how to close each gap.
- **Rollout:** what humans or other agents still need to do to ship.
- **Security:** findings from a separate agent, each with a patch to apply or a prompt to hand to an agent.
- **Focus ranking:** coverage of the changed lines, complexity, risk signals and the reports above rank every file as *Focus*, *Skim* or *Skip*, so the diff opens on the files that matter and collapses the rest.

![The briefing for the demo pull request](docs/screenshot-briefing.png)

## Quick start

```sh
npm install
npm run dev
```

Open <http://localhost:3000> and click **Open demo pull request**. The demo is an agent-authored change that adds partial refunds to a payments API, with a manifest, a security report and coverage data.

To load real pull requests, copy `.env.example` to `.env.local` and set `GITHUB_TOKEN` to a token that can read pull requests and contents. Then open `owner/repo#123` or paste a pull request URL into the search box. The token is only used on the server.

## How it works

```
authoring agent ──► manifest  ─┐
security agent  ──► security  ─┼─► PR comments (or .reviewhub/*.json) ──► ReviewHub ◄── GitHub diff
CI              ──► metrics   ─┘
```

Each report has its own author, so no agent grades its own work. See [`docs/reports.md`](docs/reports.md) for the formats, how to publish them and how the focus score is computed.

To set up a repository:

1. Have the authoring agent publish a manifest with every pull request. With Claude Code, install the [plugin](#claude-code-plugin) and opt the repository in. Other agents can follow [`plugin/skills/review-manifest/SKILL.md`](plugin/skills/review-manifest/SKILL.md).
2. Run an independent security review on each pull request: `/reviewhub:security-review owner/repo#123`, or another agent prompted with [`plugin/agents/security-reviewer.md`](plugin/agents/security-reviewer.md).
3. Publish coverage from CI, for example with [`docs/examples/reviewhub-metrics.yml`](docs/examples/reviewhub-metrics.yml).

### Claude Code plugin

```
/plugin marketplace add wearethefoos/ReviewHub
/plugin install reviewhub@reviewhub
```

- `/reviewhub:review owner/repo#123`: a briefing on the pull request, then a walk through what needs your judgement.
- `/reviewhub:security-review owner/repo#123`: an independent security review by a subagent, published as a comment.
- `reviewhub:review-manifest`: Claude writes and publishes the manifest for pull requests it opens, in repositories whose `CLAUDE.md` mentions ReviewHub (a hook reminds it after `gh pr create`).

See [`plugin/README.md`](plugin/README.md).

### CLI

`reviewhub` is a single Rust binary for reviewing from the terminal and for the agents and CI jobs that publish reports.

```sh
cargo install --locked --git https://github.com/wearethefoos/ReviewHub reviewhub
```

Review a pull request in the terminal. It uses `GITHUB_TOKEN`, or your `gh auth login` if that's not set:

```sh
reviewhub review owner/repo#123            # the briefing: ask, focus files, security, deviations, decisions, gaps
reviewhub review owner/repo#123 --diff     # plus the Focus files' diffs, annotated inline
reviewhub review owner/repo#123 --all      # plus skipped files, passed checks, prompts, every rollout step
reviewhub review owner/repo#123 --json     # bundle + focus analysis as JSON, for scripts and agents
reviewhub review --bundle fixtures/demo/bundle.json   # the demo, offline
```

Produce and publish reports:

```sh
reviewhub schema manifest                                   # JSON Schema to give an agent
reviewhub validate manifest manifest.json                   # exit code 1 with readable errors if invalid
reviewhub metrics --lcov coverage/lcov.info > metrics.json  # coverage report from lcov
reviewhub publish security security.json --repo owner/repo --pr 123   # create or update the PR comment
reviewhub comment manifest manifest.json                    # just print the comment body
```

The CLI reads reports with the same trust rules as the web app, validates them against the same `schemas/`, and ranks files with the same scoring (its tests check it against the web app's results for the demo pull request).

## Development

| Command             | What it does                                     |
| ------------------- | ------------------------------------------------ |
| `npm run dev`       | Start the dev server                             |
| `npm test`          | Unit tests (Vitest)                              |
| `npm run typecheck` | TypeScript                                       |
| `npm run lint`      | ESLint                                           |
| `npm run generate`  | Regenerate `schemas/` and `fixtures/demo/` from the Zod schemas and the demo |
| `cargo test`        | CLI tests, including scoring parity with the web app |
| `npm run build`     | Production build                                 |

Stack: Next.js (App Router, server components), React, Tailwind CSS with GitHub's Primer colours, Zod, Octokit. The CLI is Rust (clap, serde, jsonschema, ureq).

```
src/app/                 routes: home, /[owner]/[repo]/pull/[number], /open
src/components/views/    one component per tab: Briefing, Changes, Decisions, Verification, Security, Rollout
src/lib/schema/          report schemas (source of truth for schemas/*.json)
src/lib/focus/           file classification and focus scoring
src/lib/sources/         GitHub loader, comment parsing, demo fixture
src/lib/diff/            unified diff parsing
cli/                     reviewhub CLI (Rust)
schemas/                 JSON Schemas generated from src/lib/schema
fixtures/demo/           demo pull request + the web app's focus analysis of it (CLI parity tests)
plugin/                  Claude Code plugin: skills, security-reviewer subagent, hook
.claude-plugin/          plugin marketplace (this repository)
```

## Not built yet

- Signing in with GitHub (OAuth) instead of a server-side token.
- Posting review comments and approvals back to GitHub from ReviewHub.
- Sending a decision alternative or security fix straight to an agent, instead of copying the prompt.
- Caching GitHub responses; every page view fetches the pull request again.
- Prebuilt CLI binaries; for now install with `cargo install`.
