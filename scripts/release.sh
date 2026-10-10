#!/usr/bin/env bash
# Bump the workspace version, commit it and push a `vX.Y.Z` tag. The tag starts
# the release: CI publishes the versioned container image and release.yml
# (cargo-dist) builds the standalone `acli` binaries and the GitHub Release.
#
# Usage: scripts/release.sh major|minor|patch|X.Y.Z[-pre]
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

bump="${1:-}"
current="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
[[ "$current" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+) ]] || {
  echo "cannot read the workspace version from Cargo.toml" >&2
  exit 1
}
major="${BASH_REMATCH[1]}" minor="${BASH_REMATCH[2]}" patch="${BASH_REMATCH[3]}"

case "$bump" in
  major) next="$((major + 1)).0.0" ;;
  minor) next="$major.$((minor + 1)).0" ;;
  patch) next="$major.$minor.$((patch + 1))" ;;
  *)
    if [[ "$bump" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
      next="$bump"
    else
      echo "usage: just release major|minor|patch|X.Y.Z[-pre]" >&2
      exit 2
    fi
    ;;
esac
tag="v$next"

[[ "$(git rev-parse --abbrev-ref HEAD)" == main ]] || {
  echo "releases are cut from main" >&2
  exit 1
}
[[ -z "$(git status --porcelain --untracked-files=no)" ]] || {
  echo "the working tree has uncommitted changes; commit or stash them first" >&2
  exit 1
}
git fetch --quiet --tags origin main
[[ "$(git rev-parse HEAD)" == "$(git rev-parse origin/main)" ]] || {
  echo "main is not at origin/main; pull or push first" >&2
  exit 1
}
if git rev-parse --quiet --verify "refs/tags/$tag" >/dev/null; then
  echo "$tag already exists" >&2
  exit 1
fi

previous="$(git describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null || true)"
echo "Releasing $tag (workspace $current${previous:+, last tag $previous})"
if [[ -n "$previous" ]]; then
  git log --oneline "$previous..HEAD" | cat
fi
read -r -p "Commit, tag and push $tag? [y/N] " answer
[[ "$answer" == y || "$answer" == Y ]] || exit 1

# The first top-level `version` is the one under [workspace.package].
perl -0pi -e "s/^version = \".*\"\$/version = \"$next\"/m" Cargo.toml
cargo update --workspace --offline --quiet
git commit --quiet -m "Release $tag" -- Cargo.toml Cargo.lock
git tag -a "$tag" -m "Release $tag"
git push --atomic origin main "$tag"
echo "Pushed $tag. CI publishes the image; release.yml publishes acli and the GitHub Release."
