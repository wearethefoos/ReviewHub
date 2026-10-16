# Privacy policy

ReviewHub (the Chrome extension, the website and web app at wearethefoos.github.io/ReviewHub, the `reviewhub` CLI and the Claude Code plugin) is published by Big Fat Coder B.V. ReviewHub has no servers: Big Fat Coder B.V. does not collect, receive, store or share any data about you or your use of ReviewHub.

## What the extension handles, and where it goes

- **Pull request data.** To show a review, the extension reads the pull request, its changed files, its comments and report files from the GitHub REST API (`api.github.com`). This data is processed in your browser to render the review page and is not sent anywhere else. The review is kept in memory for the open tab only.
- **Your GitHub token (optional).** If you add a GitHub token in the settings, it is stored in the extension's local storage (`chrome.storage.local`) in your browser and is only ever sent to `api.github.com`, to authenticate the requests above.
- **Update check (unpacked installs only).** A copy loaded unpacked from a release zip asks `api.github.com` for the latest ReviewHub release once a day, to tell you when to update. Chrome Web Store installs don't; Chrome updates them.
- **Settings and checklist state.** The logins you choose to trust, and the rollout steps you tick off, are stored in your browser's local storage.
- **The GitHub page you are on.** On github.com, the extension checks the page address to decide whether to show the **Open in ReviewHub** button on pull requests. It does not read or record the page's content.

The extension has no analytics, tracking, advertising or remote code, and does not sell or transfer data to anyone.

The web app works the same way. Its token is kept in the browser tab's session storage, or in the browser's local storage if you choose to remember it.

Requests to GitHub are subject to [GitHub's privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).

## Removing your data

Remove the token in the extension's settings, or uninstall the extension, which deletes everything it stored. For the web app, clear the site's data in your browser.

## Changes and contact

Changes to this policy are published in this file, in the [ReviewHub repository](https://github.com/wearethefoos/ReviewHub). Questions: open an issue in that repository.
