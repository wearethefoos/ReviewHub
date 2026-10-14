# ReviewHub

Review agent-authored pull requests by what matters, not by line count.

Agents produce more code than people can read line by line, and much of it doesn't need reading. ReviewHub puts three reports next to the GitHub diff and shows a reviewer only what needs their judgement:

- **The ask:** the user's original prompt and follow-ups, next to what was delivered.
- **Deviations:** where the agent did something other than what was asked, and why.
- **Decisions:** questions the prompt left open, what the agent chose, and alternatives with ready-to-run prompts.
- **Validation:** what the agent ran to prove the change works, with evidence.
- **Gaps:** what the agent couldn't validate, and how to close each gap.
- **Rollout:** what humans or other agents still need to do to ship.
- **Security:** findings from a separate agent, each with a patch to apply or a prompt to hand to an agent.
- **Focus ranking:** coverage of the changed lines, complexity, risk signals and the reports above rank every file as *Focus*, *Skim* or *Skip*, so the diff opens on the files that matter and collapses the rest.

Nothing is hosted. ReviewHub is a browser extension and a Claude Code plugin; both talk to GitHub directly.

![The briefing for the demo pull request](docs/screenshot-briefing.png)

## Install

### Chrome extension

Download `reviewhub-extension-<version>.zip` from the [latest release](https://github.com/wearethefoos/ReviewHub/releases/latest), unzip it, open `chrome://extensions`, turn on **Developer mode** and choose **Load unpacked**. The extension works in Chrome and other Chromium browsers (Edge, Brave, Arc).

Then:

- Click **Open in ReviewHub** on any pull request on github.com, or the toolbar button while on one.
- For private repositories, and a higher rate limit than GitHub's 60 anonymous requests an hour, add a fine-grained token with read-only *Pull requests* and *Contents* access in the extension's settings. It stays in your browser and is only sent to `api.github.com`.
- The extension's start page has a demo pull request that needs no token.

### Claude Code plugin

```
/plugin marketplace add wearethefoos/ReviewHub
/plugin install reviewhub@reviewhub
```

- `/reviewhub:review owner/repo#123`: a briefing on the pull request in your session, then a walk through what needs your judgement.
- `/reviewhub:security-review owner/repo#123`: an independent security review by a subagent, published as a PR comment.
- `reviewhub:review-manifest`: Claude writes and publishes the manifest for pull requests it opens, in repositories that opt in by mentioning ReviewHub in their `CLAUDE.md` (a hook reminds it after `gh pr create`).

The plugin brings the `reviewhub` CLI with it: on first use it downloads the release binary matching the plugin's version and checks it against the release's `SHA256SUMS`. Updating the plugin updates the CLI. See [`plugin/README.md`](plugin/README.md).

### CLI on its own

For CI, or terminals without Claude Code:

```sh
curl -fsSL https://raw.githubusercontent.com/wearethefoos/ReviewHub/main/install.sh | sh
```

This installs the latest release to `~/.local/bin` after verifying its checksum. Binaries are built for Linux (x86-64, ARM64), macOS (Intel, Apple Silicon) and Windows (x86-64).

```sh
reviewhub review owner/repo#123            # the briefing: ask, focus files, security, deviations, decisions, gaps
reviewhub review owner/repo#123 --diff     # plus the Focus files' diffs, annotated inline
reviewhub review owner/repo#123 --json     # bundle + focus analysis as JSON, for scripts and agents
reviewhub review --bundle fixtures/demo/bundle.json   # the demo, offline

reviewhub schema manifest                                   # JSON Schema to give an agent
reviewhub validate manifest manifest.json                   # exit code 1 with readable errors if invalid
reviewhub metrics --lcov coverage/lcov.info > metrics.json  # coverage report from lcov
reviewhub publish security security.json --repo owner/repo --pr 123   # create or update the PR comment
```

It uses `GITHUB_TOKEN`, or your `gh auth login`.

## How it works

```
authoring agent ──► manifest  ─┐
security agent  ──► security  ─┼─► PR comments (or .reviewhub/*.json) ──► extension / CLI ◄── GitHub diff
CI              ──► metrics   ─┘
```

Each report has its own author, so no agent grades its own work. Reports are only read from people with write access to the repository, the pull request's author, `github-actions[bot]` and logins you choose to trust. See [`docs/reports.md`](docs/reports.md) for the formats and how the focus score is computed.

To set up a repository:

1. Have the authoring agent publish a manifest with every pull request: install the plugin and opt the repository in, or have other agents follow [`plugin/skills/review-manifest/SKILL.md`](plugin/skills/review-manifest/SKILL.md).
2. Run an independent security review on each pull request: `/reviewhub:security-review`, or another agent prompted with [`plugin/agents/security-reviewer.md`](plugin/agents/security-reviewer.md).
3. Publish coverage from CI, for example with [`docs/examples/reviewhub-metrics.yml`](docs/examples/reviewhub-metrics.yml).

## Development

```
crates/core/       Rust: report types and validation, GitHub trust rules, diff parsing, focus scoring
crates/cli/        Rust: the reviewhub binary (terminal briefing, publishing, GitHub client)
crates/wasm/       Rust: reviewhub-core compiled to WebAssembly for the extension
extension/         Chrome extension (Vite, React, Tailwind with GitHub's Primer colours)
plugin/            Claude Code plugin: skills, security-reviewer subagent, hooks, CLI launcher
.claude-plugin/    plugin marketplace (this repository)
schemas/           JSON Schemas, generated from crates/core
fixtures/demo/     the demo pull request
```

All logic lives in `crates/core`. The extension only fetches from GitHub and renders; it calls the same Rust code as the CLI through WebAssembly, with TypeScript types generated from the Rust types.

```sh
# Rust
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
UPDATE_SCHEMAS=1 cargo test -p reviewhub-core schemas                 # after changing report types
cargo test -p reviewhub-core --features ts export_bindings            # regenerate extension/src/generated

# Extension (needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli at the Cargo.lock version)
cd extension && npm ci
npm run wasm && npm run build     # then load extension/dist unpacked
npm test && npm run typecheck && npm run lint

# Plugin
claude --plugin-dir ./plugin      # try local changes; set REVIEWHUB_BIN=target/debug/reviewhub to use a local CLI build
scripts/test-plugin-hook.sh && scripts/test-plugin-shim.sh target/debug/reviewhub
```

Releases are cut from a tag; see [`RELEASING.md`](RELEASING.md).
