# ReviewHub

Next.js app that renders agent-authored GitHub pull requests as a focused review. See README.md for the product and docs/reports.md for the report formats.

## Commands

- Web: `npm test`, `npm run typecheck`, `npm run lint`, `npm run build`.
- CLI (`cli/`, Rust): `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- After changing `src/lib/schema/`, `src/lib/focus/` or the demo fixture, run `npm run generate` and commit `schemas/` and `fixtures/` (CI checks they match).

## Conventions

- Next.js 16: `params` and `searchParams` are Promises. Read `node_modules/next/dist/docs/` when unsure about an API.
- Server components by default. Client components (`"use client"`) only for interactivity: copy buttons, the rollout checklist.
- Colours come from the Primer tokens in `src/app/globals.css` (`bg-canvas`, `text-fg-muted`, `border-border`, `text-danger`, …). Don't use raw Tailwind palette colours; they break dark mode.
- Report text is untrusted. Render it with `Markdown`/`InlineMarkdown` (no raw HTML), never `dangerouslySetInnerHTML`.
- The demo fixture in `src/lib/sources/demo.ts` is parsed through the schemas and checked in `score.test.ts`. Keep it realistic; it doubles as the product demo.
- Focus scoring exists twice: `src/lib/focus/` (web) and `cli/src/focus.rs` + `cli/src/classify.rs` (CLI). Change both, run `npm run generate`, and `cargo test` checks they agree (`matches_the_web_app`). Update the table in docs/reports.md too.
- The same goes for the comment format (`src/lib/sources/extract.ts` / `cli/src/reports.rs`) and the GitHub trust rules (`src/lib/sources/github.ts` / `cli/src/github.rs`).
- Regexes shared with Rust must avoid lookaround (the `regex` crate does not support it).
