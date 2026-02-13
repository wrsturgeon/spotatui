# To create a release

Releases are automated via GitHub Actions using [`.github/workflows/cd.yml`](https://github.com/LargeModGames/spotatui/blob/master/.github/workflows/cd.yml).

The workflow runs when you push a tag.

## Stable release

1. Bump the version in `Cargo.toml` and run the app to update `Cargo.lock`.
2. Update the "Unreleased" header for the new version in `CHANGELOG.md` (`### Added/Fixed/Changed` as appropriate).
3. Commit and push your changes.
4. Create an annotated tag with a message (the tag message is shown on the GitHub release page):
   - `git tag -a v0.7.0 -m "Release v0.7.0"`
5. Push that specific tag:
   - `git push origin v0.7.0`
6. Wait for the build on the [Actions page](https://github.com/LargeModGames/spotatui/actions).
7. Stable tags (no suffix) also trigger publish jobs (crates.io, AUR, winget, Homebrew).

## Pre-release (recommended for canary testing)

Use a SemVer pre-release tag like `v0.7.0-rc1` / `v0.7.0-beta.1`.

1. Create an annotated pre-release tag with tester notes:
   - `git tag -a v0.7.0-rc1 -m "RC1: canary build for Spotify API migration"`
2. Push that specific tag:
   - `git push origin v0.7.0-rc1`
3. The workflow will automatically mark the GitHub release as `prerelease: true`.
4. Pre-release tags skip ecosystem publishing jobs (crates.io, AUR, winget, Homebrew).

### Homebrew Packaging

Homebrew publishing is automated via the CD workflow. When you push a new tag:

1. The `publish-homebrew` job downloads the release artifacts
2. Calculates SHA256 checksums for each platform binary
3. Updates the formula in [homebrew-spotatui](https://github.com/LargeModGames/homebrew-spotatui)



TODO: Scoop packaging is not yet set up for spotatui. If you'd like to contribute packaging, PRs are welcome!
