#!/usr/bin/env bash
# Prepares a release pull request. Picks the next version from Conventional Commits (or
# takes the one given), sets it in Cargo.toml and Cargo.lock, regenerates CHANGELOG.md on a
# release/vX.Y.Z branch and opens the pull request. Merging it publishes the release
# (.github/workflows/release.yml).
#
#   scripts/release.sh [X.Y.Z]
#
# Needs git-cliff, gh and cargo. REMOTE names the GitHub remote (default: origin).
set -euo pipefail

remote="${REMOTE:-origin}"
for tool in git-cliff gh cargo; do
  command -v "$tool" >/dev/null || { echo "$tool is required" >&2; exit 1; }
done
if [[ -n "$(git status --porcelain)" ]]; then
  echo "The working tree has changes; commit or stash them first." >&2
  exit 1
fi

git fetch --quiet "$remote" main --tags
current=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
if [[ $# -gt 0 ]]; then
  version="${1#v}"
else
  version=$(git-cliff --bumped-version 2>/dev/null)
  version="${version#v}"
fi
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Not a SemVer version: $version" >&2
  exit 1
fi
if git rev-parse -q --verify "refs/tags/v$version" >/dev/null; then
  echo "v$version is already released; there are no feat or fix commits since, or pass a version." >&2
  exit 1
fi

branch="release/v$version"
git switch --quiet -c "$branch" "$remote/main"
perl -0pi -e "s/^version = \"\Q$current\E\"/version = \"$version\"/m" Cargo.toml
cargo update --workspace --offline --quiet
git-cliff --tag "v$version" --output CHANGELOG.md
notes=$(git-cliff --unreleased --tag "v$version" --strip header)

git add Cargo.toml Cargo.lock CHANGELOG.md
git commit --quiet -m "chore(release): v$version"
git push --quiet -u "$remote" "$branch"
gh pr create --base main --head "$branch" --title "chore(release): v$version" \
  --body "Merging this publishes v$version.

$notes"
