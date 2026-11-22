# Releases

Versions follow [SemVer](https://semver.org). While the version is 0.x, a minor version can change
tool arguments.

## How a release is made

`main` is protected, so a release goes through a pull request like any other change:

1. `scripts/release.sh` picks the next version from the Conventional Commits since the last tag
   (`feat:` bumps the minor version, `fix:` and `perf:` the patch version, `!` or
   `BREAKING CHANGE` the major version), or takes one given as an argument:

   ```sh
   scripts/release.sh          # next version from the commits
   scripts/release.sh 0.3.0    # an explicit version
   ```

   It sets the version in `Cargo.toml` and `Cargo.lock`, regenerates `CHANGELOG.md` with
   [git-cliff](https://git-cliff.org), and opens a `chore(release): vX.Y.Z` pull request. It needs
   `git-cliff`, `gh` and `cargo`.

2. When the pull request is merged, `.github/workflows/release.yml` builds the archives for
   Linux (x86_64, aarch64), macOS (Apple silicon, Intel) and Windows (x86_64), writes
   `SHA256SUMS`, and publishes the GitHub release `vX.Y.Z` with the notes from the changelog.

A failed release can be retried with **Actions > Release > Run workflow**: it releases the version
in `Cargo.toml` if that tag does not exist yet.

## What a release contains

Each archive holds the server binary, the Lua bridge (`lua/`), the install scripts (`scripts/`),
`README.md`, `CHANGELOG.md` and `LICENSE`. The [one-line installers](installation.md) download
the archive for the platform and check it against `SHA256SUMS`.
