# ReviewHub reports

ReviewHub builds its view of a pull request from the GitHub diff plus three JSON reports. Each report has its own author, so no single agent grades its own work:

| Report     | Written by                                    | Answers                                                                     |
| ---------- | --------------------------------------------- | --------------------------------------------------------------------------- |
| `manifest` | The agent that authored the change            | What was asked, where it deviated, what it decided, validated and left open |
| `security` | A separate agent that did not write the code  | What could be exploited, with a patch or prompt to fix each finding         |
| `metrics`  | CI                                            | Which changed lines tests exercise, and how complexity changed             |

All reports are optional. Without them ReviewHub still ranks files using path heuristics, but the briefing is mostly empty.

Reports are public on the pull request. Write them without private data: the manifest describes the user's request rather than quoting it, and validation rejects any report containing text that looks like a credential (GitHub, AWS, Anthropic, OpenAI, Google, Slack, Stripe and npm tokens, JSON Web Tokens and private keys), wherever ReviewHub reads or publishes it.

JSON Schemas live in [`schemas/`](../schemas), and `reviewhub schema <kind>` prints them. They are generated from the Rust types in [`crates/core/src/model.rs`](../crates/core/src/model.rs), which the CLI and the browser extension (through WebAssembly) both use, so both accept exactly the same reports.

## Publishing a report

### As a pull request comment (preferred)

The comment must contain a marker followed by a fenced JSON block:

````markdown
<!-- reviewhub:manifest -->
**ReviewHub review manifest**

<details><summary>Report data</summary>

```json
{ "version": "1", ... }
```

</details>
````

The [`reviewhub` CLI](../README.md#cli) does this for you and updates its own earlier comment rather than adding a new one:

```sh
reviewhub publish manifest manifest.json --repo owner/repo --pr 123   # GITHUB_TOKEN or `gh auth login`
```

Or print the body and post it with any tool: `reviewhub comment manifest manifest.json`.

If several comments of the same kind exist, the most recently updated one wins.

**Trust.** Anyone can comment on a public pull request, and a fake manifest could steer reviewers away from code. ReviewHub (the CLI and the extension alike) only reads report comments from:

- repository owners, members and collaborators,
- the pull request's author,
- `github-actions[bot]`,
- logins you add: `REVIEWHUB_TRUSTED_AUTHORS` (comma separated) for the CLI, the extension's settings page for the browser.

### As a file on the branch

ReviewHub falls back to `.reviewhub/manifest.json`, `.reviewhub/security.json` and `.reviewhub/metrics.json` at the pull request's head commit. Files under `.reviewhub/` are left out of the review. Comments are preferred because they don't end up in the repository's history.

### The briefing comment

`reviewhub briefing owner/repo#123 --publish` (run by the workflow `reviewhub init` adds) keeps one comment starting with `<!-- reviewhub:briefing -->` on the pull request: the briefing in Markdown, with links into the diff, to the web app and to the extension. It is output, not a report, and is never read back as one. The workflow refreshes it when the pull request changes and when a report comment is posted with a person's or app's token. Comments posted with a workflow's own `GITHUB_TOKEN` don't trigger other workflows, so a workflow that publishes a report should refresh the briefing itself (see [`examples/reviewhub-metrics.yml`](examples/reviewhub-metrics.yml)).

## `manifest`

| Field        | Purpose                                                                                                                          |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------- |
| `agent`      | Name, model and a link to the agent session.                                                                                     |
| `prompt`     | The user's request **rephrased** for reviewers (goal, scope, constraints), plus later requests that changed the scope. Never verbatim: no credentials, personal or other private data. |
| `summary`    | What was delivered, in a few sentences.                                                                                          |
| `deviations` | Each place the delivery differs from the request: `requested`, `delivered`, `reason`, `severity` (`minor`/`major`).              |
| `decisions`  | Questions the prompt left open: `context`, `chosen`, `rationale`, `impact` (`low`/`medium`/`high`) and `alternatives`. Give an alternative a `prompt` when switching to it is realistic. |
| `validation` | What was run: `kind`, `command`, `result` (`passed`/`failed`/`partial`/`skipped`) and a short `evidence` excerpt.                 |
| `gaps`       | What could not be validated: `why`, `howToValidate`, `owner` (`human`/`agent`/`ci`/`ops`), `effort`.                              |
| `rollout`    | What has to happen to ship: `when` (`before-merge`/`after-merge`/`after-deploy`), `owner`, `blocking`, optional `prompt`.        |
| `files`      | Per file: `role` (`core`, `supporting`, `test`, `config`, `docs`, `generated`, `mechanical`), a `note`, and a `reviewHint`.       |

Deviations, decisions, validations and gaps take `files: [{ "path": "...", "lines": [start, end] }]`. Line numbers refer to the new version of the file. These references are what puts annotations next to the right lines in the diff, so include them whenever an item is about specific code.

## `security`

```json
{
  "version": "1",
  "scanner": { "name": "security-reviewer" },
  "commit": "<head sha>",
  "summary": "…",
  "findings": [
    {
      "id": "sqli",
      "title": "SQL injection through orderId",
      "severity": "critical",
      "category": "CWE-89",
      "confidence": "high",
      "description": "…",
      "file": "src/payments/refunds.ts",
      "lines": [28, 30],
      "suggestion": {
        "explanation": "Use a parameterized query.",
        "diff": "--- a/src/payments/refunds.ts\n+++ b/src/payments/refunds.ts\n@@ …",
        "prompt": "Replace the interpolated SQL in createRefund with a parameterized query…"
      }
    }
  ]
}
```

`suggestion.diff` must be a unified diff that applies with `git apply` on the head commit. ReviewHub flags the report when `commit` doesn't match the pull request's head.

## `metrics`

Build it from an lcov report in CI:

```sh
reviewhub metrics --lcov coverage/lcov.info --commit "$GITHUB_SHA" > metrics.json
```

`uncoveredLines` per file is what lets ReviewHub mark changed lines no test runs. `quality.files[].complexityDelta` and `lintIssues` are optional and raise a file's focus score.

## How the focus score works

Every changed file gets a score from 0 to 100, and a tier:

- **Focus** (40 and up): read line by line.
- **Skim** (12 to 39): check the shape.
- **Skip** (below 12): generated, mechanical or low risk. Collapsed by default.

| Signal                                       | Weight                                       |
| -------------------------------------------- | -------------------------------------------- |
| Security finding                             | critical 60, high 45, medium 25, low 10      |
| Deviation in the file                        | major 30, minor 12                           |
| Decision in the file                         | high 30, medium 15, low 5                    |
| Failed or partial validation of the file     | 25                                           |
| Validation gap in the file                   | 18                                           |
| Changed lines not covered by tests           | 2 per line, up to 25                         |
| Code file absent from the coverage report    | 15                                           |
| Complexity increase                          | 2 per point, up to 20                        |
| Sensitive path (auth, payments, crypto, …)   | 10                                           |
| Size                                         | √(changed lines), up to 15                   |
| Role                                         | core 10, test 10, supporting/config 6, …     |

Generated files are capped at 5 unless the security report has a finding in them. An agent's `reviewHint: "must-review"` raises a file to Focus. `reviewHint: "skip"` is only honoured when no independent signal (security, coverage, complexity, sensitive path, failed validation) contradicts it; otherwise ReviewHub shows that the hint was overridden.

The scoring lives in [`crates/core/src/focus.rs`](../crates/core/src/focus.rs). The CLI and the extension run the same code, so a pull request ranks the same in the terminal and in the browser.
