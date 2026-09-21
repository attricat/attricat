#!/usr/bin/env bash
# Runs against the worktree-local stack through its public CLI/HTTP boundary.
# The sibling example is the maintained host-integration extension.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
example_extension=$(cd "$root/../../attricat-extension-example" && pwd)
: "${CATALOG_TOKEN:?Set a workspace-owner personal API token before running this test.}"
if [[ -z "${CATALOG_API_URL:-}" && -n "${API_PORT:-}" ]]; then
  CATALOG_API_URL="http://127.0.0.1:$API_PORT"
fi
: "${CATALOG_API_URL:?Run 'just setup' and source .catalog-worktree first.}"
command -v jq >/dev/null || { echo 'jq is required' >&2; exit 1; }

cli=(cargo run --quiet -p cli -- --token "$CATALOG_TOKEN")
api() { curl --fail-with-body --silent --show-error -H "Authorization: Bearer $CATALOG_TOKEN" -H 'Content-Type: application/json' "$@"; }

(cd "$example_extension" && just check && just pack)
manifest="$example_extension/manifest.json"
extension_id=$(jq -r '.catalog.id' "$manifest")
version=$(jq -r '.version' "$manifest")
archive="$example_extension/dist/$extension_id-$version.tar.zst"
test -s "$archive" || { echo "missing packaged example archive: $archive" >&2; exit 1; }

# Package, side-load, grant every declared capability, and enable the same
# extension maintained by the sibling checkout. This proves host installer,
# permissions, component validation, and client-artifact delivery together.
# A prior interrupted verification may have left a disabled installation.
"${cli[@]}" extension remove "$extension_id" >/dev/null 2>&1 || true
"${cli[@]}" extension sideload --file "$archive" >/dev/null
while IFS= read -r permission; do
  "${cli[@]}" extension grant "$extension_id" --grant-kind capability --grant-id "$permission" >/dev/null
done < <(jq -r '.permissions[]' "$manifest")
"${cli[@]}" extension enable "$extension_id" >/dev/null

detail=$("${cli[@]}" extension detail "$extension_id")
echo "$detail" | jq -e '.installation.state == "enabled"' >/dev/null
release_id=$(echo "$detail" | jq -r '.installation.installed_release_id')
[[ "$release_id" != "null" && -n "$release_id" ]] || { echo 'enabled extension did not expose a release ID' >&2; exit 1; }

# Fetch a real declared client contribution through the mediated artifact API.
contribution=$(jq -r '.ui[] | select(.artifact != null) | .id' "$manifest" | head -1)
artifact=$(mktemp)
trap 'rm -f "$artifact"' EXIT
"${cli[@]}" extension artifact "$extension_id" "$contribution" --output "$artifact" >/dev/null
test -s "$artifact"

# Quarantine is a public lifecycle action; the audit API is the public evidence
# surface for package, grants, enablement, artifact delivery, and quarantine.
"${cli[@]}" extension quarantine "$extension_id" --diagnostic-code reference-e2e >/dev/null
"${cli[@]}" extension detail "$extension_id" | jq -e '.installation.state == "quarantined"' >/dev/null
"${cli[@]}" audit list --limit 100 | jq -e '.events | map(.target.type) | any(. == "extension")' >/dev/null
printf 'reference extension E2E passed: extension=%s release=%s contribution=%s\n' "$extension_id" "$release_id" "$contribution"
