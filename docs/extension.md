# The Chrome extension

The extension opens any GitHub pull request in ReviewHub's review page: the ask, deviations, decisions, validation gaps, security findings and rollout steps, with the diff ranked so you read the files that matter. It talks only to `api.github.com`; nothing else is involved.

It works in Chrome and other Chromium browsers (Edge, Brave, Arc). Not on a computer? The same review page runs as a web app at <https://wearethefoos.github.io/ReviewHub/review.html>.

## Install

Install [ReviewHub from the Chrome Web Store](https://chromewebstore.google.com/detail/reviewhub/eoejgblmpnccaaohifkeajpblaidgnjo) and click **Add to Chrome**. Optionally pin it from the puzzle-piece menu so its toolbar button is always visible.

The extension asks for access to `github.com` (to add the **Open in ReviewHub** button to pull requests) and `api.github.com` (to read pull requests and reports). It updates itself when a new version is released.

### Without the Chrome Web Store

To run a specific version, or a build that isn't in the store yet:

1. Download `reviewhub-extension-vX.Y.Z.zip` from the [releases](https://github.com/wearethefoos/ReviewHub/releases).
2. Unzip it into a folder you'll keep: Chrome loads the extension from that folder, so don't delete it afterwards.
3. Open `chrome://extensions` (in Edge: `edge://extensions`) and turn on **Developer mode** (top right).
4. Click **Load unpacked** and choose the unzipped folder (the one containing `manifest.json`).

Extensions loaded this way don't update themselves: to update, unzip the new release over the same folder and click the reload icon on the ReviewHub card. Chrome identifies an unpacked extension by its folder, so loading a different folder installs a separate copy without your settings. Remove an unpacked copy before installing from the store, or you'll have two **Open in ReviewHub** buttons.

## Set up a token (optional)

Public repositories work without a token, within GitHub's limit of 60 anonymous requests an hour. For private repositories, or the 5,000-an-hour limit, add a token:

1. Create a [fine-grained token](https://github.com/settings/personal-access-tokens/new) with access to the repositories you review and these repository permissions, both **Read-only**: **Pull requests** and **Contents**.
2. Open the extension's settings: the gear icon on the review page, or right-click the toolbar button → **Options**.
3. Paste it into **GitHub token** and click **Save and test**. You should see "Signed in as *you*" and how many API requests you have left.

To also comment, approve and request changes from ReviewHub, give the token **Pull requests: Read and write** instead (Contents stays read-only).

The token is kept in the extension's local storage in this browser and is only ever sent to `api.github.com`.

In the same settings, **Also trust reports from** lists extra GitHub logins (for example your CI bot) whose report comments ReviewHub should read; see [Where the reports come from](#where-the-reports-come-from).

## Open a pull request

- **On github.com:** on any pull request page (conversation, files, commits or checks), click the green **Open in ReviewHub** button at the bottom right.
- **Toolbar button:** on a pull request, it opens that pull request in a new tab. Anywhere else it opens the start page.
- **Start page:** type `owner/repo#123` or paste a pull request URL. The box at the top of every review page does the same.
- **Demo:** the start page has **Open demo pull request**, an agent-authored change with every report filled in. No token needed.

## Review it

The header shows the pull request, its state and **View on GitHub**. Below it are six tabs; the numbers on them count what needs attention (red: failed validation, or critical or high security findings).

| Tab | What it shows |
| --- | --- |
| **Briefing** | Start here. What was asked (the agent's summary of the request and later changes to it) next to what the agent delivered; where to spend your review (how many files and lines to read closely, and the files to start with); what needs your judgement (security findings, deviations, decisions, what wasn't validated); validation results and coverage; blocking rollout steps. |
| **Changes** | The diff, grouped by review tier: **Focus** (read these line by line), **Skim** (check the shape, not every line) and **Skip** (generated, mechanical or low risk, collapsed). Security findings, deviations, decisions, gaps and uncovered lines appear inline, with a link to their details. |
| **Decisions** | Where the agent did something other than what was asked, and the questions the prompt left open with the alternatives it rejected. **Copy prompt to switch** gives you a prompt to hand an agent if you prefer an alternative. |
| **Verification** | The checks the agent ran, with commands and evidence (failures first); what it could not validate, with steps to close each gap; coverage and complexity of the changed files. |
| **Security** | Findings from the independent security review: severity, location, suggested fix. **Copy patch (git apply)**, **Copy agent prompt**, or **Copy prompt to fix all**. |
| **Rollout** | What has to happen before merging, after merging and after deploying. Tick steps off as they're done; the ticks are saved in this browser only. |

### Comment and submit a review

In the extension (not the web app, and not on the demo), you can write a GitHub review without leaving ReviewHub. It needs a token with **Pull requests: Read and write**.

- **Comment on a line:** hover a line in **Changes** and click the **+** in its gutter. For several lines, drag from the **+** over them, or click one line's **+** and shift-click another's (within one hunk).
- **Comment on a finding:** security findings, deviations, decisions and gaps shown on the diff have a **Comment** link that starts a comment with their text. A security finding whose fix fits the diff becomes a GitHub suggestion the author can apply in one click. On the **Security** tab, **Add to review** does the same without opening the editor.
- **Submit:** **Review changes** in the header shows your pending comments; add an overall comment and choose **Comment**, **Approve** or **Request changes**. Everything goes to GitHub as one review.

Pending comments are kept in this browser per pull request until you submit them. Comments written before the pull request got new commits are marked **Outdated** and aren't submitted.

A suggested flow: read the Briefing, settle the security findings and deviations, check the decisions you care about, read the **Focus** files in Changes, skim the rest, then work through Rollout.

The page doesn't refresh on its own. After the agent or CI publishes new reports, click **Reload from GitHub** in the sidebar (or reload the tab).

## Where the reports come from

The sidebar's **Reports** box shows which reports were found and who published them: the review manifest (from the authoring agent), the security report (from an independent reviewer) and coverage (from CI). They're read from comments on the pull request, or from `.reviewhub/*.json` files in its head commit.

Only comments from people who can be trusted count: repository owners, members and collaborators, the pull request's author, `github-actions[bot]`, and the logins under **Also trust reports from**. Report comments from anyone else are ignored.

A pull request without reports still opens: files are then ranked by coverage, security findings and path heuristics only. To get reports on your pull requests, set the repository up with [`reviewhub init`](../README.md#repository-setup-reviewhub-init).

## Troubleshooting

| Message | What to do |
| --- | --- |
| Pull request not found. For private repositories, add a token | GitHub answers "not found" for private repositories you can't see. [Add a token](#set-up-a-token-optional). |
| Pull request not found, or the token can't see it | Give the token access to this repository (or its organisation must allow fine-grained tokens). |
| GitHub rejected the token | The token expired or was revoked. Create a new one. |
| GitHub refused the request. The token may lack access | Check the token has read access to **Pull requests** and **Contents** for this repository. |
| Rate limit used up | Wait for the hourly reset, or add a token. |
| GitHub refused the review. The token needs Pull requests: Read and write | Edit the token's permissions on GitHub (or create a new one) and save it again. |
| GitHub didn't accept the review: … | GitHub's own reason, for example "Can not approve your own pull request". |
| No **Open in ReviewHub** button | Reload the GitHub tab after installing or updating the extension. The button only shows on pull request pages. |
| A report shows a red ✗ | It was published but isn't valid; expand **Invalid report** in the sidebar for the error. |
| "The reports mention files that are not in this diff" | The reports were written for an earlier version of the pull request; ask the agent to republish them. |
