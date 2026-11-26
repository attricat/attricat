#!/usr/bin/env bash
# Build a production image from a committed ref (local `main` by default) and
# tag it for this host only. The build context is `git archive` of that ref, so
# uncommitted changes, the current branch and untracked files never leak in.
set -euo pipefail

ref="${1:-main}"
image="${2:-attricat:snapshot}"
jobs="${ATTRICAT_SNAPSHOT_JOBS:-1}"

commit="$(git rev-parse --verify --quiet "$ref^{commit}")" || {
  echo "Unknown git ref: $ref" >&2
  exit 1
}
short="$(git rev-parse --short "$commit")"
version="$(git describe --tags --always "$commit")-snapshot"

upstream="$(git rev-parse --abbrev-ref --symbolic-full-name "$ref@{upstream}" 2>/dev/null || true)"
if [[ -n "$upstream" ]]; then
  read -r ahead behind < <(git rev-list --left-right --count "$ref...$upstream")
  if ((ahead || behind)); then
    echo "Note: $ref is $ahead ahead and $behind behind $upstream (last fetch); building local $ref." >&2
  fi
fi

echo "Building $image from $ref at $short ($version), cargo jobs: $jobs"
git archive --format=tar "$commit" \
  | DOCKER_BUILDKIT=1 docker build \
      --build-arg VCS_REF="$commit" \
      --build-arg VCS_BRANCH="$ref" \
      --build-arg VERSION="$version" \
      --build-arg CARGO_BUILD_JOBS="$jobs" \
      --label "org.attricat.snapshot=true" \
      -t "$image" \
      -

echo "Built $image from $ref at $short."
echo "Use it with ATTRICAT_IMAGE=$image docker compose up -d"
