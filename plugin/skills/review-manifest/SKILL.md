---
name: review-manifest
description: Write and publish a ReviewHub review manifest for a pull request you authored. Use right after you open or update a pull request in a repository that uses ReviewHub (its CLAUDE.md mentions ReviewHub), or when the user asks for a review manifest.
argument-hint: "[owner/repo#number]"
allowed-tools: Bash(reviewhub *), Bash(gh pr view *), Bash(git log *), Bash(git diff *)
---

# Review manifest

ReviewHub shows reviewers your account of a pull request before they see the diff: what was asked, where you deviated, what you decided, what you validated and what is still open. It uses this to decide which files a human reads closely. Write the manifest for a reviewer who has not seen this conversation.

Only publish when the repository opts in (its `CLAUDE.md` or `AGENTS.md` mentions ReviewHub) or the user asked for a manifest. Otherwise offer to publish one and wait for an answer: publishing posts a comment on the pull request.

## Steps

1. **Find the pull request.** Use `$ARGUMENTS` if given (`owner/repo#123` or a URL). Otherwise use the pull request you just opened, or `gh pr view --json number,url,headRefOid`.
2. **Get the schema** with `reviewhub schema manifest` if you don't know it.
3. **Write the manifest** to a file outside the repository, such as `$TMPDIR/reviewhub-manifest.json`, following the rules below. Use `git diff <base>...HEAD` to get line numbers in the new version of each file.
4. **Validate** with `reviewhub validate manifest <file>` and fix every error.
5. **Publish** with `reviewhub publish manifest <file> --repo <owner/repo> --pr <number>`. It updates your earlier manifest comment if there is one, so republish whenever you push changes that affect it.
6. **Tell the user** the comment URL and, in one or two lines, the deviations, high-impact decisions and blocking gaps you reported.

## Rules

1. **`prompt.original` is the user's request, rephrased for reviewers, never a verbatim copy.** The manifest is public on the pull request, and prompts can hold private data. Keep the goal, scope and constraints, in the user's terms where possible, so a reviewer can compare them with what you delivered. Leave out credentials and tokens, personal data, customer details, internal hostnames and URLs, and anything else the user wouldn't post on the pull request; describe them instead ("the production database URL the user gave"). Add later requests that changed the scope to `prompt.followUps`, summarized the same way; leave out messages that only steered the session ("open the PR", "looks good"). Never put secrets anywhere in the manifest: `reviewhub validate` rejects text that looks like a credential.
2. **Report every deviation.** Anything you delivered differently from what was asked, including changes to shared code the user didn't ask you to touch, is a deviation. Use `major` when it changes behaviour other code or users depend on.
3. **Report decisions the prompt didn't settle.** Include the alternatives you rejected. For `high`-impact decisions, give at least one alternative with a `prompt` that another agent could run to switch to it. Leave out trivial choices (naming, file layout) unless they matter to the reviewer.
4. **Validation is evidence, not intent.** Only list checks you actually ran in this session, with the exact `command` and a short `evidence` excerpt of the output. Use `partial` or `skipped` honestly; a reviewer trusts the whole manifest less after one inflated result.
5. **Gaps are as important as validation.** List everything you could not verify (no database, no credentials, no production-sized data, untested error paths, concurrency) with concrete steps to close each gap and who should do it (`human`, `agent`, `ci` or `ops`).
6. **Rollout lists what has to happen outside the diff.** Migrations, feature flags, config, docs, communication, monitoring. Mark steps that must happen before merging as `blocking`. Give agent-owned steps a `prompt`.
7. **Point at code.** Give deviations, decisions and gaps `files` with `lines: [start, end]` in the new version of the file, so ReviewHub can annotate the diff.
8. **Classify files.** Mark lockfiles, snapshots and generated code as `generated`, pure renames and import updates as `mechanical`. Use `reviewHint: "must-review"` for the code you are least sure about. Use `"skip"` only for files that need no human attention; ReviewHub ignores the hint when other signals disagree.

Don't publish security findings or coverage yourself: those come from an independent reviewer (the `security-review` skill) and from CI, so that no agent grades its own work.
