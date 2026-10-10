#!/usr/bin/env bash
# Exercises the maintained sibling example against the worktree-local public API.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
example_extension=$(cd "$root/../../attricat-extension-example" && pwd)
: "${CATALOG_TOKEN:?Set a workspace-owner personal API token before running this test.}"
if [[ -z "${CATALOG_API_URL:-}" && -n "${API_PORT:-}" ]]; then
  CATALOG_API_URL="http://127.0.0.1:$API_PORT/api"
fi
: "${CATALOG_API_URL:?Run 'just setup' and source .worktree first.}"
export CATALOG_API_URL
command -v jq >/dev/null || { echo 'jq is required' >&2; exit 1; }

cli=(cargo run --quiet -p acli -- --token "$CATALOG_TOKEN")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

(cd "$example_extension" && just check && just pack)
manifest="$example_extension/manifest.json"
extension_id=$(jq -er '.catalog.id' "$manifest")
version=$(jq -er '.version' "$manifest")
archive="$example_extension/dist/$extension_id-$version.tar.zst"
test -s "$archive" || { echo "missing packaged example archive: $archive" >&2; exit 1; }

# An upgrade clears grants. Reinstall and explicitly authorize the exact release.
"${cli[@]}" extension remove "$extension_id" >/dev/null 2>&1 || true
"${cli[@]}" extension sideload --file "$archive" >/dev/null
"${cli[@]}" extension configure "$extension_id" --configuration '{}' >/dev/null
while IFS= read -r permission; do
  "${cli[@]}" extension grant "$extension_id" --grant-kind capability --grant-id "$permission" >/dev/null
done < <(jq -r '.permissions[]' "$manifest")
while IFS= read -r contract; do
  "${cli[@]}" extension grant "$extension_id" --grant-kind event_publish --grant-id "$contract" >/dev/null
done < <(jq -r '.event_contracts.exports[]?.id' "$manifest")
while IFS= read -r contract; do
  "${cli[@]}" extension grant "$extension_id" --grant-kind event_subscribe --grant-id "$contract" >/dev/null
done < <(jq -r '.event_contracts.consumes[]? | "\(.provider):\(.contract)"' "$manifest")
"${cli[@]}" extension enable "$extension_id" >/dev/null
"${cli[@]}" extension detail "$extension_id" | jq -e '.installation.state == "enabled"' >/dev/null
"${cli[@]}" extension artifact "$extension_id" formula-workbench --output "$work/workbench.js" >/dev/null
test -s "$work/workbench.js"

# Trigger the real v1.1 event handler with a fresh blueprint and record, not an
# existing development fixture. The extension must write the computed value in
# the same default context after an ordinary Catalog mutation.
code="reference_formula_$(date +%s)_$$"
cat > "$work/blueprint.toml" <<TOML
format_version = 1
code = "$code"
name = "Reference formula E2E"
kind = "record"

[[attributes]]
code = "price_net"
value_type = "number"

[[attributes]]
code = "price_gross"
value_type = "number"

[views.dropdown_option]
type = "dropdown_option"
fields = ["price_net"]

[extensions.$extension_id.formulas]
price_gross = "price_net * 2"
TOML
blueprint_id=$("${cli[@]}" blueprint create --file "$work/blueprint.toml" | jq -er '.blueprint.id')
"${cli[@]}" blueprint publish "$blueprint_id" 1 >/dev/null
cat > "$work/values.toml" <<'TOML'
[[values]]
kind = "scalar"
attribute_code = "price_net"
value = 10.0
TOML
context_id=00000000-0000-4000-8000-000000000001
record_id=$("${cli[@]}" record create --blueprint "$code" --values "$work/values.toml" --context-id "$context_id" | jq -er '.id')
cat > "$work/updated.toml" <<'TOML'
[[values]]
kind = "scalar"
attribute_code = "price_net"
value = 17.0
TOML
"${cli[@]}" record update "$record_id" --values "$work/updated.toml" --context-id "$context_id" >/dev/null
for _ in $(seq 1 60); do
  preview=$("${cli[@]}" record resolved-preview "$record_id" --context-id "$context_id")
  if echo "$preview" | jq -e --arg context "$context_id" '.values.price_gross.value == 34 and .values.price_gross.source_context.id == $context' >/dev/null; then
    "${cli[@]}" audit list --limit 100 | jq -e '.events | map(.target.type) | any(. == "extension")' >/dev/null
    printf 'reference extension E2E passed: extension=%s blueprint=%s record=%s\n' "$extension_id" "$blueprint_id" "$record_id"
    exit 0
  fi
  sleep 1
done
echo 'reference extension did not compute price_gross = 34 in the default context' >&2
exit 1
