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
: "${CATALOG_API_URL:?Run 'just setup' and source .worktree first.}"
command -v jq >/dev/null || { echo 'jq is required' >&2; exit 1; }

cli=(cargo run --quiet -p cli -- --token "$CATALOG_TOKEN")
api() { curl --fail-with-body --silent --show-error -H "Authorization: Bearer $CATALOG_TOKEN" -H 'Content-Type: application/json' "$@"; }

(cd "$example_extension" && just check && just pack)
manifest="$example_extension/importer/manifest.json"
archive="$example_extension/dist/reference-customer-importer.tar.zst"
extension_id=$(jq -r '.catalog.id' "$manifest")
test -s "$archive" || { echo "missing packaged example archive: $archive" >&2; exit 1; }

# Package, side-load, grant every declared capability, and enable the maintained
# reference importer. A prior interrupted verification may have left an
# installation in any terminal or disabled state.
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

# Exercise the real v1.3 component and host-managed artifact path. The importer
# completes one durable operation by writing the packaged customer fixture to a
# host artifact; completion therefore proves task dispatch, Wasmtime execution,
# permission mediation, artifact storage, and checkpointing together.
operation_id=$(jq -r '.server.operations[0].id' "$manifest")
idempotency_key="reference-e2e-$(date +%s)-$$"
run=$(api -X POST "$CATALOG_API_URL/extensions/$extension_id/operations" -d "$(jq -nc --arg operation_id "$operation_id" --arg idempotency_key "$idempotency_key" '{operation_id:$operation_id,input:{fixture:"customers.ndjson"},idempotency_key:$idempotency_key}')")
run_id=$(echo "$run" | jq -er '.id')
for _ in $(seq 1 120); do
  runs=$(api "$CATALOG_API_URL/extension-operation-runs")
  status=$(echo "$runs" | jq -r --arg run_id "$run_id" '.[] | select(.id == $run_id) | .status')
  if [[ "$status" == "completed" ]]; then
    echo "$runs" | jq -e --arg run_id "$run_id" '.[] | select(.id == $run_id and .progress.customers == 2 and .progress.bytes > 0)' >/dev/null
    break
  fi
  if [[ "$status" == "failed" || "$status" == "dead_letter" || "$status" == "cancelled" ]]; then
    echo "reference operation ended in $status" >&2
    exit 1
  fi
  sleep 1
done
[[ "$status" == "completed" ]] || { echo 'reference operation did not complete in time' >&2; exit 1; }

# Quarantine is a public lifecycle action; the audit API is the public evidence
# surface for package, grants, enablement, operation execution, and quarantine.
"${cli[@]}" extension quarantine "$extension_id" --diagnostic-code reference-e2e >/dev/null
"${cli[@]}" extension detail "$extension_id" | jq -e '.installation.state == "quarantined"' >/dev/null
"${cli[@]}" audit list --limit 100 | jq -e '.events | map(.target.type) | any(. == "extension")' >/dev/null
printf 'reference extension E2E passed: extension=%s release=%s operation=%s run=%s\n' "$extension_id" "$release_id" "$operation_id" "$run_id"
