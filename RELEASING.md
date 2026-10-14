# Releasing

The CLI, the Claude plugin and the extension share one version. The plugin downloads the CLI binary of its own version from the GitHub release, so **a plugin version must never reach `main` before its release exists**: plugin users install from `main`, and would get a launcher pointing at binaries that aren't there yet.

So release from a branch, and merge after the release is published:

1. On a branch from `main`, set the version everywhere:

   ```sh
   scripts/bump-version.sh 0.2.0
   ```

   This updates `Cargo.toml`, `plugin/.claude-plugin/plugin.json`, `extension/package.json` and `extension/public/manifest.json`, and the lockfiles. Chrome requires plain `X.Y.Z` versions.

2. Commit, push the branch and open a pull request. Wait for CI to pass.

3. Tag the branch's head and push the tag:

   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```

   Or, instead of pushing a tag, run the **Release** workflow from the Actions tab on the branch with the version as input; it creates the tag on that commit when it publishes.

   The [release workflow](.github/workflows/release.yml) checks the tag matches the versions, builds the CLI for Linux (x86-64, ARM64, static musl), macOS (Intel, Apple Silicon) and Windows, builds the extension, and publishes them with `SHA256SUMS` as release `v0.2.0`.

4. When the release is published, merge the pull request. Plugin users get the new version on their next marketplace update, and the CLI follows on first use.

5. Merging to `main` also redeploys the web app to GitHub Pages ([pages workflow](.github/workflows/pages.yml); one-time setup: Settings → Pages → Source: GitHub Actions).

6. If you publish the extension on the Chrome Web Store, upload `reviewhub-extension-v0.2.0.zip` from the release.

If the release workflow fails, fix it on the branch, delete the tag (`git push --delete origin v0.2.0 && git tag -d v0.2.0`) and tag again. Nothing reaches users until the pull request is merged.
