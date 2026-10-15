# Chrome Web Store listing

What to fill in on the [Chrome Web Store Developer Dashboard](https://chrome.google.com/webstore/devconsole) for ReviewHub. To set the listing up and automate publishing, see [RELEASING.md](../../RELEASING.md#chrome-web-store).

## Package

Store item `eoejgblmpnccaaohifkeajpblaidgnjo`: <https://chromewebstore.google.com/detail/reviewhub/eoejgblmpnccaaohifkeajpblaidgnjo>. The first version was uploaded by hand from the [releases](https://github.com/wearethefoos/ReviewHub/releases); later versions are uploaded by the release workflow.

## Store listing

**Name:** ReviewHub (from the manifest)

**Summary** (from the manifest): Review agent-authored GitHub pull requests by what matters: the ask, deviations, decisions, validation gaps and security findings.

**Description:**

```
Coding agents write more code than people can read line by line, and much of it doesn't need reading. ReviewHub opens a GitHub pull request in a review page that shows what needs your judgement first.

• The ask: the original prompt and follow-ups, next to what the agent delivered.
• Deviations: where the agent did something other than what was asked, and why.
• Decisions: questions the prompt left open, what the agent chose, and the alternatives, each with a prompt you can hand to an agent.
• Verification: what the agent ran to prove the change works, with evidence, and what it couldn't validate.
• Security: findings from a separate reviewing agent, with a patch to apply or a prompt to fix them.
• Rollout: what still has to happen before and after merging.
• Focus ranking: test coverage of the changed lines, complexity, risk signals and the reports above rank every file as Focus, Skim or Skip, so you read the files that matter and skim the rest.

Click "Open in ReviewHub" on any pull request on github.com, or the toolbar button. Try the demo pull request on the start page; no setup needed.

Reports are published as pull request comments by the authoring agent, a security reviewer and CI. Set a repository up with the ReviewHub CLI and Claude Code plugin: https://github.com/wearethefoos/ReviewHub

Public repositories work without a token. For private repositories, add a read-only fine-grained GitHub token in the settings; it stays in your browser and is only sent to api.github.com.

ReviewHub has no servers and collects no data. Open source under the Apache License 2.0.
```

**Category:** Developer Tools

**Language:** English

**Graphics** (in this folder):

| Field | File |
| --- | --- |
| Store icon (128×128) | `extension/public/icons/128.png` |
| Screenshots (1280×800) | `screenshot-1-briefing.png` … `screenshot-5-verification.png` |
| Small promo tile (440×280) | `promo-small-440x280.png` |

**Official URL / Homepage:** https://github.com/wearethefoos/ReviewHub

**Support URL:** https://github.com/wearethefoos/ReviewHub/issues

## Privacy practices

**Single purpose:**

```
Open a GitHub pull request in a review page that shows the agent's account of the change, security findings and a focus ranking of the changed files.
```

**Permission justifications:**

| Permission | Justification |
| --- | --- |
| `storage` | Stores the user's optional GitHub token and the logins whose reports they trust, in the browser. |
| `activeTab` | When the toolbar button is clicked, reads the current tab's address to open the pull request shown in it. |
| Host permission `https://api.github.com/*` | Reads the pull request, its changed files, comments and report files from the GitHub API to build the review. |
| Host permission `https://github.com/*` (content script) | Adds an "Open in ReviewHub" button to pull request pages on github.com. The script only reads the page address. |

**Remote code:** No, I am not using remote code. (The WebAssembly module is part of the package.)

**Data usage:** tick **Authentication information** (the optional GitHub token, stored locally and sent only to api.github.com) and **Website content** (pull request data read from the GitHub API and processed locally). Then certify all three statements: data is not sold to third parties, not used or transferred for purposes unrelated to the single purpose, and not used to determine creditworthiness or for lending.

**Privacy policy URL:** https://github.com/wearethefoos/ReviewHub/blob/main/PRIVACY.md

## Distribution

**Visibility:** Public. **Regions:** All regions.

## Regenerating the graphics

The screenshots are the demo pull request in the built extension, taken with Playwright at 1280×800 (`?demo`, `?demo&tab=changes`, `decisions`, `security`, `verification`). Retake them when the review page changes noticeably.
