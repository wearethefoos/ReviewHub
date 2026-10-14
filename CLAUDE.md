# ReviewHub

Next.js app that renders agent-authored GitHub pull requests as a focused review. See README.md for the product and docs/reports.md for the report formats.

## Commands

- `npm test`, `npm run typecheck`, `npm run lint`, `npm run build`: run all four before pushing.
- After changing anything in `src/lib/schema/`, run `npm run schema` and commit `schemas/` (CI checks they match).

## Conventions

- Next.js 16: `params` and `searchParams` are Promises. Read `node_modules/next/dist/docs/` when unsure about an API.
- Server components by default. Client components (`"use client"`) only for interactivity: copy buttons, the rollout checklist.
- Colours come from the Primer tokens in `src/app/globals.css` (`bg-canvas`, `text-fg-muted`, `border-border`, `text-danger`, …). Don't use raw Tailwind palette colours; they break dark mode.
- Report text is untrusted. Render it with `Markdown`/`InlineMarkdown` (no raw HTML), never `dangerouslySetInnerHTML`.
- The demo fixture in `src/lib/sources/demo.ts` is parsed through the schemas and checked in `score.test.ts`. Keep it realistic; it doubles as the product demo.
- Scoring weights live in `src/lib/focus/score.ts`. If you change them, update the table in docs/reports.md.
