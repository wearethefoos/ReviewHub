# ReviewHub

Reviews agent-authored GitHub pull requests by what matters. Nothing is hosted: a Chrome extension (`extension/`), a Claude Code plugin (`plugin/`) and a Rust CLI (`crates/cli`), all built on one Rust core (`crates/core`). See README.md, and docs/reports.md for the report formats.

## Commands

- Rust: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Extension (in `extension/`): `npm run wasm` (rebuild after any change to `crates/core` or `crates/wasm`), `npm run typecheck`, `npm run lint`, `npm test`, `npm run build`.
- Plugin: `claude plugin validate . --strict && claude plugin validate ./plugin --strict`, `scripts/test-plugin-hook.sh`, `scripts/test-plugin-shim.sh target/debug/reviewhub`.
- Commits and PR titles follow Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `ci:`, `chore:`, …; `feat!:` for breaking changes; optional scopes `core`, `cli`, `extension`, `plugin`, `web`). release-please derives versions and CHANGELOG.md from them, so pick the type for what users notice: a user-visible change is `feat` or `fix`, not `chore`.
- Versions: never edit them by hand. release-please bumps them in its release PR (RELEASING.md); `scripts/bump-version.sh X.Y.Z` exists for manual cases. Don't edit CHANGELOG.md either.

## Conventions

- All logic (report types, validation, comment format, GitHub trust rules, diff parsing, scoring) lives in `crates/core`. The CLI and the extension only do I/O and rendering. Don't reimplement core logic in TypeScript; expose it through `crates/wasm` instead.
- Report types in `crates/core/src/model.rs` are the source of truth. Doc comments become JSON Schema descriptions that agents read. After changing them: `UPDATE_SCHEMAS=1 cargo test -p reviewhub-core schemas` and `cargo test -p reviewhub-core --features ts export_bindings`; commit `schemas/` and `extension/src/generated/` (CI checks both).
- Optional fields: use `#[serde(default, skip_serializing_if = "Option::is_none")]` so JSON never contains `null`, and structs carry `ts(export, optional_fields)` so TypeScript types match.
- Scoring weights live in `crates/core/src/focus.rs`; keep the table in docs/reports.md in sync.
- Extension: colours come from the Primer tokens in `extension/src/globals.css` (`bg-canvas`, `text-fg-muted`, `border-border`, …); raw Tailwind palette colours break dark mode. Report text is untrusted: render it with `Markdown`/`InlineMarkdown`, never `dangerouslySetInnerHTML`. The GitHub token stays in `chrome.storage.local` and is only sent to api.github.com.
- Plugin: `plugin/bin/reviewhub` downloads the CLI release matching the plugin version (or, when installed by `reviewhub init`, the version in the `VERSION` file next to it). Right after a release its binaries take ~10 minutes to build; the launcher falls back to the newest cached CLI meanwhile (RELEASING.md).
- `reviewhub init` compiles in the plugin's skills, subagent, launcher and hook script (`crates/cli/src/init.rs`), renaming `reviewhub:x` to `reviewhub-x`. When you add a cross-reference between plugin files, check the rename list; a test fails on leftovers.
- The briefing comment (`crates/cli/src/briefing.rs`) is posted by a trusted account, so every piece of report text goes through `esc` (no HTML, no fake report markers, no @mentions).
- The web app is the extension's review page built with `npm run build:web`; code that touches `chrome.*` must check `isExtension()` from `src/lib/settings.ts`.
- The demo pull request (`fixtures/demo/bundle.json`) is used by the extension, the CLI tests and the docs. Keep it realistic.
