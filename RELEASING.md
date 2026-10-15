# Releasing

Releases are automated with [release-please](https://github.com/googleapis/release-please-action) from [Conventional Commits](https://www.conventionalcommits.org/). The CLI, the Claude plugin and the extension share one version; the plugin (and every repository set up with `reviewhub init`) downloads the CLI binary of exactly that version.

## Commit messages

Pull requests are squash-merged, and their titles become the commits release-please reads, so PR titles must follow Conventional Commits (a check enforces it):

| Prefix | Example | Effect |
| --- | --- | --- |
| `feat:` | `feat: briefing comment links to the web app` | minor bump, listed under Features |
| `fix:` | `fix(cli): handle PRs without a head repository` | patch bump, listed under Bug fixes |
| `perf:`, `revert:`, `docs:` | | patch bump, listed in the changelog |
| `refactor:`, `test:`, `build:`, `ci:`, `chore:` | | no release on their own, not listed |
| `feat!:` or a `BREAKING CHANGE:` footer | `feat!: rename the manifest's files field` | minor bump before 1.0, major after |

Scopes are optional; useful ones are `core`, `cli`, `extension`, `plugin`, `web`.

## How a release happens

1. **Merge pull requests to `main`.** On every push, the [Release Please workflow](.github/workflows/release-please.yml) opens or updates a release pull request ("chore(main): release X.Y.Z") with:
   - the new version in `version.txt`, `Cargo.toml`, `plugin/.claude-plugin/plugin.json`, `extension/package.json` and `extension/public/manifest.json`;
   - the updated `Cargo.lock` and `extension/package-lock.json` (a follow-up job commits these with `scripts/bump-version.sh`);
   - the new `CHANGELOG.md` section.
2. **Merge the release pull request when you want to ship.** release-please tags `vX.Y.Z` and creates the GitHub release with the changelog.
3. **The builds are attached.** The same workflow calls the [release workflow](.github/workflows/release.yml), which builds the CLI for Linux (x86-64, ARM64, static musl), macOS (Intel, Apple Silicon) and Windows, and the extension, and uploads them with `SHA256SUMS` to the release. This takes about 10 minutes.
4. **Users update.** Plugin users get the new version on their next marketplace update; repositories pick it up when someone runs `reviewhub init` with the new CLI. Merging to `main` also redeploys the web app to GitHub Pages ([pages workflow](.github/workflows/pages.yml)).

While the builds in step 3 are running, the plugin on `main` already has the new version but its binaries aren't downloadable yet. The launcher (`plugin/bin/reviewhub`) handles that: it keeps using the newest CLI it already has and tells the user, and fetches the new one once it's there.

5. **The extension goes to the Chrome Web Store.** Once the [Chrome Web Store secrets](#chrome-web-store) are set, the release workflow uploads the extension zip and submits it for review; Chrome publishes it when the review passes (usually within a few days) and installed copies update themselves. Without the secrets, that step is skipped with a notice.

## Setup

- **`RELEASE_PLEASE_TOKEN` secret (recommended).** Pull requests and commits made with the workflow's own `GITHUB_TOKEN` don't trigger other workflows, so without this secret CI won't run on the release pull request. Add a fine-grained personal access token (or a GitHub App token) with *Contents* and *Pull requests* read/write access to this repository.
- **Settings → Actions → General → Workflow permissions:** allow GitHub Actions to create pull requests.
- **Settings → Pages → Source: GitHub Actions**, for the web app.

## Chrome Web Store

The first submission is done by hand; after that, every release uploads itself.

1. **Register** as a Chrome Web Store developer at the [developer dashboard](https://chrome.google.com/webstore/devconsole) (one-time registration fee). Register with, or add, the Big Fat Coder B.V. account details, and verify the contact email.
2. **Create the item:** **Add new item**, upload `reviewhub-extension-vX.Y.Z.zip` from the latest release, and fill in the store listing, privacy practices and distribution from [`docs/chrome-web-store/`](docs/chrome-web-store/README.md) (text, screenshots and the promo tile are there). Submit it for review.
3. **Note two IDs:** the item's extension ID (on the item's page, also in its store URL) and your publisher ID (in the dashboard's URL, or under **Account**).
4. **Get API credentials:** in a Google Cloud project, enable the Chrome Web Store API and create an OAuth client and a refresh token, following the [chrome-webstore-upload-keys guide](https://github.com/fregante/chrome-webstore-upload-keys).
5. **Add repository secrets** (Settings → Secrets and variables → Actions): `CHROME_EXTENSION_ID`, `CHROME_PUBLISHER_ID`, `CHROME_CLIENT_ID`, `CHROME_CLIENT_SECRET`, `CHROME_REFRESH_TOKEN`.

From then on, the `chrome-web-store` job of the [release workflow](.github/workflows/release.yml) runs [`chrome-webstore-upload-cli`](https://github.com/fregante/chrome-webstore-upload-cli) with the release's zip. The store doesn't accept a new version while the previous one is still in review; the job then fails with a message saying so, and once the review is done, **Re-run failed jobs** on that run uploads the same build. The store also rejects a version it already has, so when rebuilding an existing release by hand, untick **Upload the extension to the Chrome Web Store**. A listing change that needs a new review (new permissions, a changed privacy answer) is done in the dashboard; until it has been published there once, API submissions may be refused.

## By hand

If release-please can't be used (for example to rebuild a release's assets), run **Actions → Release → Run workflow** with a version that matches `version.txt`. For a new version it creates the tag and the release from the branch you run it on; to rebuild an existing release's assets, pick that release's tag under "Use workflow from" so the builds match the tagged code. Pushing a `vX.Y.Z` tag yourself doesn't start a build: the release workflow only runs when Release Please calls it or when you run it from the Actions tab. To set versions by hand, use `scripts/bump-version.sh X.Y.Z`, never edit them individually; `scripts/check-version.sh` verifies they agree.
