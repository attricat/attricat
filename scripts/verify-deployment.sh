#!/usr/bin/env bash
# Disposable release-path exercise used by CI and available to operators.
# Database/object backup and restore are deployment-platform responsibilities;
# see the Operate section of the docs site for restore rehearsal.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
compose=(docker compose -f deploy/compose.ci.yml)
image="${ATTRICAT_IMAGE:-attricat:ci}"
evidence="${DEPLOYMENT_EVIDENCE_DIR:-deployment-evidence}"
session_cookies="$(mktemp)"
sso_cookies="$(mktemp)"
extension_archive="$(mktemp)"
tls="deploy/ci-tls"
# The newest published first-party extension must keep installing and running.
extension_release=v0.15.0
extension_sha256=98e1d47b1c191e15b6b7101b22aa28784f2cf1981a77f85cd94b0d34953a2f93
mkdir -p "$evidence"
cleanup() {
  rm -f "$session_cookies" "$sso_cookies" "$extension_archive"
  "${compose[@]}" down --volumes --remove-orphans >/dev/null 2>&1 || true
}
trap cleanup EXIT

if [[ "${SKIP_IMAGE_BUILD:-false}" != "true" ]]; then
  docker build --build-arg VCS_REF="$(git rev-parse HEAD)" --build-arg VCS_BRANCH="${GITHUB_HEAD_REF:-${GITHUB_REF_NAME:-$(git rev-parse --abbrev-ref HEAD)}}" --build-arg VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)" -t "$image" .
fi
export ATTRICAT_IMAGE="$image"

run_app() {
  docker run --rm --read-only --tmpfs /tmp:size=256m,mode=1777 \
    --security-opt no-new-privileges "$@"
}
# Reads all its input. `grep -q` stops at the first match, so with pipefail the
# writer's broken pipe would fail a check that passed, depending on timing.
contains() {
  grep "$@" >/dev/null
}
wait_url() {
  local url=$1
  for _ in $(seq 1 90); do curl --fail --silent "$url" >/dev/null 2>&1 && return 0; sleep 2; done
  "${compose[@]}" logs >&2
  return 1
}
assert_status() {
  local expected=$1 url=$2
  local actual
  actual="$(curl --max-time 40 --silent --output /dev/null --write-out '%{http_code}' "$url")"
  [[ "$actual" == "$expected" ]] || {
    echo "expected HTTP $expected from $url, received $actual" >&2
    return 1
  }
}
assert_container_hardening() {
  local service=$1 container
  container="$("${compose[@]}" ps -q "$service")"
  [[ -n "$container" ]]
  [[ "$(docker inspect --format '{{.Config.User}}' "$container")" == "10001:10001" ]]
  [[ "$(docker inspect --format '{{.HostConfig.ReadonlyRootfs}}' "$container")" == "true" ]]
  docker inspect --format '{{json .HostConfig.SecurityOpt}}' "$container" | contains 'no-new-privileges'
}

[[ "$(docker image inspect --format '{{.Config.User}}' "$image")" == "10001:10001" ]]
[[ "$(run_app --entrypoint /usr/bin/id "$image" -u)" == "10001" ]]
# The application artifact must not contain database or object-storage utility
# binaries. Operators use their platform's separately maintained tooling.
run_app --entrypoint /bin/sh "$image" -c \
  '! command -v postgres && ! command -v pg_dump && ! command -v pg_restore && ! command -v aws'

# A private certificate authority for the identity provider, as with a corporate one.
mkdir -p "$tls"
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj "/CN=release test CA" \
  -keyout "$tls/ca.key" -out "$tls/ca.pem" 2>/dev/null
openssl req -newkey rsa:2048 -nodes -subj "/CN=idp.test" \
  -keyout "$tls/idp.key" -out "$tls/idp.csr" 2>/dev/null
openssl x509 -req -days 1 -in "$tls/idp.csr" -CA "$tls/ca.pem" -CAkey "$tls/ca.key" -CAcreateserial \
  -extfile <(printf 'subjectAltName=DNS:idp.test\n') -out "$tls/idp.pem" 2>/dev/null
chmod 644 "$tls/idp.key" "$tls/idp.pem" "$tls/ca.pem"
"${compose[@]}" up -d postgres rustfs rustfs-init mailpit idp
"${compose[@]}" run --rm migrate
"${compose[@]}" up -d api file-worker
wait_url http://127.0.0.1:3000/health/ready
wait_url http://127.0.0.1:3001/health/ready
assert_container_hardening api
assert_container_hardening file-worker
curl --fail --silent http://127.0.0.1:3000/ | contains -i '<div id="root"'
# The web app's pages carry the security headers, not only the API's responses.
curl --fail --silent --head http://127.0.0.1:3000/ | contains -i '^x-content-type-options: nosniff'
curl --fail --silent --head http://127.0.0.1:3000/ | contains -i "^content-security-policy: frame-ancestors 'none'"
curl --fail --silent http://127.0.0.1:3000/api/health/ready | contains ready
curl --fail --silent --cookie-jar "$session_cookies" \
  --header 'content-type: application/json' \
  --data '{"login_identifier":"default.local","email":"owner@example.test","password":"test-release-password"}' \
  http://127.0.0.1:3000/api/auth/login >/dev/null
for path in auth/session blueprints contexts workspace/navigation/sidebar extensions/runtime; do
  curl --fail --silent --cookie "$session_cookies" "http://127.0.0.1:3000/api/$path" >/dev/null
done

# The release path must also write: a record and a file through the running
# API and object store, and a real email through the mail relay.
csrf="$(awk '$6 == "attricat_csrf" { print $7 }' "$session_cookies")"
token="$(curl --fail --silent --cookie "$session_cookies" --header "x-attricat-csrf: $csrf" \
  --header 'content-type: application/json' \
  --data '{"label":"release-test","permissions":["blueprints.read","blueprints.write","blueprints.publish","records.read","records.write","records.publish"]}' \
  http://127.0.0.1:3000/api/personal-access-tokens | jq -r .secret)"
[[ -n "$token" && "$token" != null ]]
acli() {
  "${compose[@]}" exec -T -e ATTRICAT_API_URL=http://127.0.0.1:3000/api -e "ATTRICAT_TOKEN=$token" api acli "$@"
}
write_in_api() {
  "${compose[@]}" exec -T api sh -c "cat > /tmp/$1"
}
write_in_api smoke-blueprint.yaml <<'YAML'
format_version: 1
code: release_smoke
name: Release smoke
kind: record
attributes:
  - code: name
    value_type: string
  - code: attachment
    value_type: file
views:
  dropdown_option:
    type: dropdown_option
    fields: [name]
  detail:
    type: stack
    children:
      - {type: heading, text: Release smoke}
      - {type: field, field: name}
  table:
    type: table
    columns:
      - {field: name}
YAML
write_in_api smoke-values.yaml <<'YAML'
values:
  - kind: scalar
    attribute_code: name
    value: Release smoke
YAML
blueprint_id="$(acli blueprint create --file /tmp/smoke-blueprint.yaml | jq -r .blueprint.id)"
acli blueprint publish "$blueprint_id" 1 >/dev/null
record_id="$(acli record create --blueprint release_smoke --values /tmp/smoke-values.yaml \
  --context-id 00000000-0000-4000-8000-000000000001 | jq -r .id)"
curl --fail --silent --header "Authorization: Bearer $token" \
  "http://127.0.0.1:3000/api/v1/records/$record_id" | contains 'Release smoke'
# A 1x1 PNG, compared byte for byte after going through the object store.
smoke_png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=='
printf '%s' "$smoke_png" | base64 -d | write_in_api smoke.png
file_id="$(acli file upload "$record_id" attachment --file /tmp/smoke.png | jq -r '(.files // .)[0].id // .id')"
# The API serves a file once the file worker has prepared it.
for _ in $(seq 1 60); do
  [[ "$(curl --fail --silent --header "Authorization: Bearer $token" \
    "http://127.0.0.1:3000/api/files/$file_id/download" | base64 -w0 || true)" == "$smoke_png" ]] \
    && file_served=true && break
  sleep 1
done
[[ "${file_served:-false}" == true ]]
curl --fail --silent --output /dev/null --header 'content-type: application/json' \
  --data '{"email":"owner@example.test"}' http://127.0.0.1:3000/api/auth/password-reset
for _ in $(seq 1 30); do
  "${compose[@]}" exec -T mailpit wget -qO- http://127.0.0.1:8025/api/v1/messages \
    | contains 'owner@example.test' && mail_delivered=true && break
  sleep 1
done
[[ "${mail_delivered:-false}" == true ]]

assert_status 401 http://127.0.0.1:3000/api/metrics
curl --fail --silent -H 'Authorization: Bearer release-test-api-metrics-token' http://127.0.0.1:3000/api/metrics \
  | contains attricat_http_requests_total
assert_status 401 http://127.0.0.1:3001/metrics
curl --fail --silent -H 'Authorization: Bearer release-test-token' http://127.0.0.1:3001/metrics \
  | contains attricat_file_worker_queue_depth

# Dependency outages withdraw readiness for both roles without changing liveness.
"${compose[@]}" stop rustfs
assert_status 200 http://127.0.0.1:3000/health/live
assert_status 503 http://127.0.0.1:3000/health/ready
assert_status 200 http://127.0.0.1:3001/health/live
assert_status 503 http://127.0.0.1:3001/health/ready
"${compose[@]}" start rustfs
wait_url http://127.0.0.1:3000/health/ready
wait_url http://127.0.0.1:3001/health/ready
"${compose[@]}" stop postgres
assert_status 200 http://127.0.0.1:3000/health/live
assert_status 503 http://127.0.0.1:3000/health/ready
assert_status 200 http://127.0.0.1:3001/health/live
assert_status 503 http://127.0.0.1:3001/health/ready
"${compose[@]}" start postgres
wait_url http://127.0.0.1:3000/health/ready
wait_url http://127.0.0.1:3001/health/ready

api_post() {
  curl --fail-with-body --silent --cookie "$session_cookies" --header "x-attricat-csrf: $csrf" \
    --header 'content-type: application/json' "$@"
}
# The extension runtime: a published release, granted and enabled, computes a value
# in its WASM component when a real event reaches it.
curl --fail --silent --location --output "$extension_archive" \
  "https://github.com/attricat/attricat-extension-example/releases/download/$extension_release/attricat-extension-example-${extension_release#v}.tar.zst"
sha256sum --check <<<"$extension_sha256  $extension_archive" >/dev/null
extension=attricat-extension-example
curl --fail-with-body --silent --cookie "$session_cookies" --header "x-attricat-csrf: $csrf" \
  --header 'content-type: application/zstd' --data-binary "@$extension_archive" \
  http://127.0.0.1:3000/api/extensions/sideload >/dev/null
manifest="$(curl --fail --silent --cookie "$session_cookies" "http://127.0.0.1:3000/api/extensions/$extension" | jq .installation.manifest)"
for permission in $(jq -r '.permissions[]?, .optional_permissions[]?' <<<"$manifest"); do
  api_post --data "{\"grant_kind\":\"capability\",\"grant_id\":\"$permission\"}" \
    "http://127.0.0.1:3000/api/extensions/$extension/grants" >/dev/null
done
for rule in $(jq -r '.host_permissions[]?.id, .optional_host_permissions[]?.id' <<<"$manifest"); do
  api_post --data "{\"grant_kind\":\"host_permission\",\"grant_id\":\"$rule\"}" \
    "http://127.0.0.1:3000/api/extensions/$extension/grants" >/dev/null
done
for contract in $(jq -r '.event_contracts.exports[]?.id' <<<"$manifest"); do
  api_post --data "{\"grant_kind\":\"event_publish\",\"grant_id\":\"$contract\"}" \
    "http://127.0.0.1:3000/api/extensions/$extension/grants" >/dev/null
done
api_post --data '{}' "http://127.0.0.1:3000/api/extensions/$extension/enable" >/dev/null
formula_blueprint="$(jq -n '{definition: "format_version: 1\ncode: release_formula\nname: Release formula\nkind: record\nattributes:\n  - code: price_net\n    value_type: number\n  - code: price_gross\n    value_type: number\nviews:\n  dropdown_option:\n    type: dropdown_option\n    fields: [price_net]\nextensions:\n  attricat-extension-example:\n    formulas: {price_gross: price_net * 2}\n"}' \
  | api_post --data @- http://127.0.0.1:3000/api/blueprints | jq -r .blueprint.id)"
api_post --request POST "http://127.0.0.1:3000/api/blueprints/$formula_blueprint/versions/1/publish" >/dev/null
default_context=00000000-0000-4000-8000-000000000001
formula_value() {
  jq -n --arg context "$default_context" --argjson price "$1" \
    '[{kind: "scalar", attribute_code: "price_net", value: $price, context_id: $context}]'
}
formula_record="$(jq -n --argjson values "$(formula_value 10)" '{blueprint: {code: "release_formula", version: 1}, values: $values}' \
  | api_post --data @- http://127.0.0.1:3000/api/v1/records | jq -r .id)"
jq -n --argjson values "$(formula_value 21)" '{values: $values}' \
  | api_post --request PUT --data @- "http://127.0.0.1:3000/api/v1/records/$formula_record" >/dev/null
for _ in $(seq 1 120); do
  curl --fail --silent --cookie "$session_cookies" \
    "http://127.0.0.1:3000/api/records/$formula_record/resolved-preview?context_id=$default_context" \
    | jq -e '[.values[].value] | index(42)' >/dev/null && formula_computed=true && break
  sleep 1
done
[[ "${formula_computed:-false}" == true ]]

# Single sign-on: register a provider behind the private CA, link the owner through
# its test sign-in, then sign in with it from a browser that has no session.
idp_curl() {
  curl --silent --show-error --cacert "$tls/ca.pem" --resolve idp.test:8443:127.0.0.1 "$@"
}
idp_sign_in() {
  local jar=$1 url=$2 action callback completed
  action="$(idp_curl --cookie "$jar" --cookie-jar "$jar" "$url" \
    | grep -o 'action="[^"]*"' | head -n 1 | cut -d'"' -f2 | sed 's/&amp;/\&/g')"
  [[ -n "$action" ]]
  callback="$(idp_curl --cookie "$jar" --cookie-jar "$jar" --output /dev/null --write-out '%{redirect_url}' \
    --data-urlencode username=ada@acme.example --data-urlencode password=dev-password "$action")"
  [[ "$callback" == http://127.0.0.1:3000/api/auth/oidc/callback/* ]]
  completed="$(curl --fail --silent --cookie "$jar" --cookie-jar "$jar" --output /dev/null \
    --write-out '%{redirect_url}' "$callback")"
  # This page also reports failures, naming only an error code.
  [[ "$completed" == */oidc/complete* && "$completed" != *error* ]] || {
    echo "single sign-on did not complete: $completed" >&2
    return 1
  }
}
for _ in $(seq 1 90); do
  idp_curl --fail --output /dev/null https://idp.test:8443/realms/acme/.well-known/openid-configuration 2>/dev/null \
    && break
  sleep 2
done
api_post --request PUT --data '{"issuer":"https://idp.test:8443/realms/acme","client_id":"attricat","client_secret":"attricat-dev-secret","display_name":"Release SSO"}' \
  http://127.0.0.1:3000/api/workspace/identity-provider >/dev/null
link_url="$(api_post --cookie-jar "$session_cookies" --data '{}' http://127.0.0.1:3000/api/auth/oidc/link \
  | jq -r .authorization_url)"
idp_sign_in "$session_cookies" "$link_url"
api_post --request PATCH --data '{"enabled":true}' http://127.0.0.1:3000/api/workspace/identity-provider >/dev/null
sso_url="$(curl --fail --silent --cookie-jar "$sso_cookies" --header 'content-type: application/json' \
  --data '{"login_identifier":"default.local"}' http://127.0.0.1:3000/api/auth/oidc/start | jq -r .authorization_url)"
idp_sign_in "$sso_cookies" "$sso_url"
curl --fail --silent --cookie "$sso_cookies" http://127.0.0.1:3000/api/auth/session | contains 'owner@example.test'

# Exercise rollback orchestration with a distinct retained-release fixture image.
# It intentionally shares application binaries with the candidate; this verifies
# identity replacement and rollback gating, not cross-version compatibility.
rollback_image=attricat:rollback-candidate
rollback_version="rollback-fixture-$(git rev-parse --short=12 HEAD)"
printf 'ARG BASE_IMAGE\nFROM ${BASE_IMAGE}\nLABEL org.opencontainers.image.version="%s"\n' "$rollback_version" \
  | docker build --build-arg BASE_IMAGE="$image" -t "$rollback_image" -
image_id="$(docker image inspect --format '{{.Id}}' "$image")"
rollback_image_id="$(docker image inspect --format '{{.Id}}' "$rollback_image")"
[[ "$image_id" != "$rollback_image_id" ]]
[[ "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.version"}}' "$rollback_image")" == "$rollback_version" ]]
ATTRICAT_IMAGE="$rollback_image" docker compose -f deploy/compose.ci.yml up -d --force-recreate api file-worker
wait_url http://127.0.0.1:3000/health/ready
wait_url http://127.0.0.1:3001/health/ready
api_container="$(ATTRICAT_IMAGE="$rollback_image" "${compose[@]}" ps -q api)"
[[ "$(docker inspect --format '{{.Image}}' "$api_container")" == "$rollback_image_id" ]]

# A real rollback: the previous release on this migrated database, then forward
# again. A previous version isn't guaranteed to run on a migrated database, so a
# failed rollback warns and goes in the evidence. Coming forward again must work.
previous_tag="$(git ls-remote --tags --refs origin 'v*' | sed 's|.*refs/tags/||' \
  | grep -vxF "${GITHUB_REF_NAME:-}" | sort -V | tail -n 1)"
previous_image="${ROLLBACK_IMAGE:-ghcr.io/attricat/attricat:${previous_tag#v}}"
real_rollback=failed
rollback_check() {
  docker pull --quiet "$previous_image" >/dev/null
  ATTRICAT_IMAGE="$previous_image" "${compose[@]}" up -d --no-deps --force-recreate api file-worker
  wait_url http://127.0.0.1:3000/health/ready
  wait_url http://127.0.0.1:3001/health/ready
  curl --fail --silent --output /dev/null --cookie-jar "$session_cookies" \
    --header 'content-type: application/json' \
    --data '{"login_identifier":"default.local","email":"owner@example.test","password":"test-release-password"}' \
    http://127.0.0.1:3000/api/auth/login
  curl --fail --silent --cookie "$session_cookies" \
    "http://127.0.0.1:3000/api/v1/records/$record_id" | contains 'Release smoke'
}
if (rollback_check); then
  real_rollback=passed
else
  echo "::warning::rolling back to $previous_image did not work on this migrated database"
fi
"${compose[@]}" up -d --no-deps --force-recreate api file-worker
wait_url http://127.0.0.1:3000/health/ready
wait_url http://127.0.0.1:3001/health/ready
curl --fail --silent --output /dev/null --cookie-jar "$session_cookies" \
  --header 'content-type: application/json' \
  --data '{"login_identifier":"default.local","email":"owner@example.test","password":"test-release-password"}' \
  http://127.0.0.1:3000/api/auth/login
curl --fail --silent --cookie "$session_cookies" \
  "http://127.0.0.1:3000/api/v1/records/$record_id" | contains 'Release smoke'
{
  echo "image=$image"
  echo "image_id=$image_id"
  echo "rollback_fixture_version=$rollback_version"
  echo "rollback_image_id=$rollback_image_id"
  echo 'smoke=passed'
  echo 'writes=passed'
  echo 'api_alias_authorization=passed'
  echo 'dependency_outages=passed'
  echo 'runtime_utilities_absent=passed'
  echo 'rollback_seam=passed'
  echo 'extension_runtime=passed'
  echo 'single_sign_on=passed'
  echo "real_rollback=$real_rollback previous=$previous_image"
} | tee "$evidence/result.txt"
