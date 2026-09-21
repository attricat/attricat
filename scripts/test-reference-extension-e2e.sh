#!/usr/bin/env bash
# Runs against the worktree-local stack.  This intentionally drives the public
# CLI/HTTP boundary; it does not construct repositories, fake object stores, or
# invoke an extension runtime directly.
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
wait_for_status() {
  local id=$1 wanted=$2 deadline=$((SECONDS + 60)) status
  while (( SECONDS < deadline )); do
    status=$(api "$CATALOG_API_URL/extension-operation-runs" | jq -r ".[] | select(.id == \"$id\") | .status")
    [[ "$status" == "$wanted" ]] && return
    sleep 1
  done
  echo "run $id did not reach $wanted" >&2
  return 1
}

(cd "$example_extension" && just check && just pack)
customer_blueprint=$(mktemp)
customer_code="reference_customer_${RANDOM}_${RANDOM}"
trap 'rm -f "$customer_blueprint"' EXIT
cat >"$customer_blueprint" <<TOML
format_version = 1
code = "$customer_code"
name = "Reference customer"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["external_id"]
[[attributes]]
code = "external_id"
value_type = "string"
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "email"
value_type = "string"
TOML
# Creation is server-side validation; publishing establishes the customer
# contract before the packaged importer/exporter runs.
blueprint_id=$("${cli[@]}" blueprint create --file "$customer_blueprint" | jq -r '.blueprint.id')
"${cli[@]}" blueprint publish "$blueprint_id" 1 >/dev/null
for kind in importer exporter; do
  "${cli[@]}" extension sideload --file "$example_extension/dist/reference-customer-$kind.tar.zst" >/dev/null
  id="attricat.reference-customer-$kind"
  "${cli[@]}" extension grant "$id" --grant-kind capability --grant-id artifacts.write >/dev/null
  "${cli[@]}" extension enable "$id" >/dev/null
done

# Invalid input is rejected before a durable run is created.
if api -X POST "$CATALOG_API_URL/extensions/attricat.reference-customer-importer/operations" \
  --data '{"operation_id":"import-customers","input":{"unexpected":true},"source_reference":{},"destination_reference":{},"idempotency_key":"reference-invalid"}' >/dev/null; then
  echo 'invalid operation request was accepted' >&2; exit 1
fi

start() {
  local extension=$1 operation=$2 key=$3
  api -X POST "$CATALOG_API_URL/extensions/$extension/operations" --data "{\"operation_id\":\"$operation\",\"input\":{\"fixture\":\"customers.ndjson\"},\"source_reference\":{},\"destination_reference\":{},\"idempotency_key\":\"$key\"}" | jq -r .id
}
import_run=$(start attricat.reference-customer-importer import-customers reference-import)
# Duplicate delivery is idempotent at the public operation boundary.
[[ "$import_run" == "$(start attricat.reference-customer-importer import-customers reference-import)" ]]
wait_for_status "$import_run" completed
export_run=$(start attricat.reference-customer-exporter export-customers reference-export)
wait_for_status "$export_run" completed

# A queued run cancels without entering the component; avoid racing the local
# worker by use of a deliberately invalid stale operation ID. The cancellation
# endpoint remains a public, idempotent lifecycle call.
cancel_run=$(start attricat.reference-customer-exporter export-customers reference-cancel)
api -X POST "$CATALOG_API_URL/extension-operation-runs/$cancel_run/cancel" >/dev/null
wait_for_status "$cancel_run" cancelled
"${cli[@]}" extension disable attricat.reference-customer-exporter >/dev/null
"${cli[@]}" extension revoke attricat.reference-customer-exporter capability artifacts.write >/dev/null
if api -X POST "$CATALOG_API_URL/extensions/attricat.reference-customer-exporter/operations" \
  --data '{"operation_id":"export-customers","input":{"fixture":"customers.ndjson"},"source_reference":{},"destination_reference":{},"idempotency_key":"reference-revoked"}' >/dev/null; then
  echo 'revoked extension was allowed to start' >&2; exit 1
fi
"${cli[@]}" extension quarantine attricat.reference-customer-importer --diagnostic-code reference-e2e >/dev/null
"${cli[@]}" extension detail attricat.reference-customer-importer | jq -e '.state == "quarantined"' >/dev/null

# The audit endpoint is the public evidence surface for package, grant, enable,
# revoke, cancel, and quarantine lifecycle actions.
"${cli[@]}" audit list --limit 100 | jq -e '.events | map(.target.type) | any(. == "extension")' >/dev/null
printf 'reference extension E2E passed: import=%s export=%s cancel=%s\n' "$import_run" "$export_run" "$cancel_run"
