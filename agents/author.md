# Instructions for the authoring agent

Copy this section into the `CLAUDE.md` (or equivalent agent instructions) of any repository whose pull requests are reviewed with ReviewHub.

---

## Review manifest

Every pull request you open must come with a ReviewHub review manifest. Reviewers read it before the diff, so it decides where they spend their attention. Write it for a reviewer who has not seen the conversation.

Write the manifest to a file outside the repository (for example `$TMPDIR/manifest.json`), validate it, then publish it as a comment on the pull request with the `reviewhub` CLI (install it with `cargo install --locked --git https://github.com/wearethefoos/ReviewHub reviewhub`; it uses `GITHUB_TOKEN` or your `gh` login):

```sh
reviewhub validate manifest "$TMPDIR/manifest.json"
reviewhub publish manifest "$TMPDIR/manifest.json" --repo <owner/repo> --pr <number>
```

Update and republish it whenever you push changes that affect it. Print the schema with `reviewhub schema manifest`; field-by-field documentation is in `docs/reports.md`.

Rules:

1. **`prompt.original` is the user's request verbatim.** Don't paraphrase or tidy it up. Add later messages that changed the scope to `prompt.followUps`, also verbatim.
2. **Report every deviation.** Anything you delivered differently from what was asked, including changes to shared code the user didn't ask you to touch, is a deviation. Use `major` when it changes behaviour other code or users depend on.
3. **Report decisions the prompt didn't settle.** Include the alternatives you rejected. For `high`-impact decisions, give at least one alternative with a `prompt` that another agent could run to switch to it. Leave out trivial choices (naming, file layout) unless they matter to the reviewer.
4. **Validation is evidence, not intent.** Only list checks you actually ran, with the exact `command` and a short `evidence` excerpt of the output. Use `partial` or `skipped` honestly; a reviewer trusts the whole manifest less after one inflated result.
5. **Gaps are as important as validation.** List everything you could not verify (no database, no credentials, no production-sized data, untested error paths, concurrency) with concrete steps to close each gap and who should do it.
6. **Rollout lists what has to happen outside the diff.** Migrations, feature flags, config, docs, communication, monitoring. Mark steps that must happen before merging as `blocking`.
7. **Point at code.** Give deviations, decisions and gaps `files` with line ranges in the new version of the file, so ReviewHub can annotate the diff.
8. **Classify files.** Mark lockfiles, snapshots and generated code as `generated`, pure renames and import updates as `mechanical`. Use `reviewHint: "must-review"` for the code you are least sure about. Use `"skip"` only for files that need no human attention; ReviewHub ignores the hint when other signals disagree.
